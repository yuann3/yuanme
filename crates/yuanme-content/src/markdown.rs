//! Markdown into the Document (#47): comrak parses CommonMark and GFM, and
//! its tree is lowered here and dropped.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use comrak::nodes::{AstNode, ListType, NodeBlockDirective, NodeValue, Sourcepos, TableAlignment};
use comrak::{Arena, Options};
use jiff::tz::TimeZone;
use yuanme_diag::{Diagnostic, FileId, Files, Level, codes};

use crate::document::{Align, Checkbox, Document, Kind, NodeId, footnote_identifier};
use crate::fields::Cx;
use crate::math;
use crate::slug::Slugger;
use crate::smartypants::{self, Dashes, Piece};

/// The comrak extensions the Engine reads (#2, #17). Raw HTML is kept and
/// written out as it is (ADR 0009); comrak's own HTML is never used.
pub fn options() -> Options<'static> {
    let mut o = Options::default();
    let e = &mut o.extension;
    e.strikethrough = true;
    e.table = true;
    e.autolink = true;
    e.tasklist = true;
    e.footnotes = true;
    e.math_dollars = true;
    e.math_code = true;
    e.block_directive = true;
    e.header_attributes = true;
    o
}

/// Parses `file`'s body, the bytes `body` of it, outside any Site: for
/// tests and tools.
pub fn document(
    files: &Files,
    file: FileId,
    body: Range<usize>,
) -> (Option<Document>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let mut cx = Cx {
        file,
        diagnostics: &mut diagnostics,
        timezone: &TimeZone::UTC,
        dir: "",
        is_file: &|_| false,
    };
    let doc = parse(files.text(file), body, &[], &mut cx);
    (doc, diagnostics)
}

/// Parses an Entry's body, `text[body]`, with the Site's math `macros`.
/// Spans are bytes in `text`.
pub(crate) fn parse(
    text: &str,
    body: Range<usize>,
    macros: &[(String, String)],
    cx: &mut Cx<'_>,
) -> Option<Document> {
    let source = &text[body.clone()];
    let arena = Arena::new();
    let root = match comrak::parse_document_bounded(&arena, source, &options()) {
        Ok(root) => root,
        Err(comrak::OutOfFuel { offset }) => {
            let line_end = source[offset..]
                .find(['\r', '\n'])
                .map_or(source.len(), |i| offset + i);
            let at = body.start + offset..body.start + line_end;
            let d = cx
                .error(
                    &codes::MARKDOWN_OUT_OF_FUEL,
                    "this block takes too long to parse".into(),
                    at,
                )
                .help("blocks nested this deep are almost always a mistake, such as `:::` fences that never close");
            cx.push(d);
            return None;
        }
    };
    let mut lower = Lower {
        doc: Document::new(body.clone()),
        source,
        lines: line_starts(source),
        base: body.start,
        ok: true,
        cx,
    };
    for child in root.children() {
        lower.node(child, Document::ROOT);
    }
    let Lower { mut doc, ok, .. } = lower;
    number_footnotes(&mut doc);
    smarten(&mut doc, Dashes::Default, &BTreeSet::new());
    let before = cx.diagnostics.len();
    let labels = math::render(&mut doc, text, cx.file, macros, cx.diagnostics);
    let ok = ok
        && !cx.diagnostics[before..]
            .iter()
            .any(|d| d.level == Level::Error);
    heading_ids(&mut doc, &labels);
    ok.then_some(doc)
}

/// Numbers each footnote reference in the order the writer meets them:
/// the body first, then each referenced definition in turn, whose own
/// references can number more.
pub(crate) fn number_footnotes(doc: &mut Document) {
    let root = doc.node(Document::ROOT).children.clone();
    let mut definitions = BTreeMap::new();
    let mut blocks = Vec::new();
    for &id in &root {
        match &doc.node(id).kind {
            Kind::FootnoteDefinition { label } => {
                definitions.insert(footnote_identifier(label), id);
            }
            _ => blocks.push(id),
        }
    }
    let mut order: Vec<String> = Vec::new();
    let mut numbers = BTreeMap::new();
    let mut written = 0;
    loop {
        let ids: Vec<NodeId> = blocks.iter().flat_map(|&b| doc.descendants(b)).collect();
        for id in ids {
            if let Kind::FootnoteReference { label, number } = doc.kind_mut(id) {
                let key = footnote_identifier(label);
                *number = *numbers.entry(key.clone()).or_insert_with(|| {
                    order.push(key);
                    order.len()
                });
            }
        }
        let Some(key) = order.get(written) else {
            return;
        };
        written += 1;
        blocks = definitions.get(key).copied().into_iter().collect();
    }
}

/// remark-smartypants over the Document, which needs its text in mdast's
/// order: comrak moves footnote definitions to the end, mdast keeps them
/// where they are written. `verbatim` text, such as an Org entity's, is
/// read for context, as inline code is, and never changed.
pub(crate) fn smarten(doc: &mut Document, dashes: Dashes, verbatim: &BTreeSet<NodeId>) {
    let mut blocks = doc.node(Document::ROOT).children.clone();
    blocks.sort_by_key(|&b| doc.node(b).span.start);
    let mut order = Vec::new();
    for b in blocks {
        prose(doc, b, &mut order);
    }
    let pieces: Vec<Piece<'_>> = order
        .iter()
        .map(|&id| match &doc.node(id).kind {
            Kind::Text(s) if !verbatim.contains(&id) => Piece::Text(s),
            Kind::Text(s) | Kind::Code(s) | Kind::Timestamp { text: s, .. } => Piece::Code(s),
            _ => Piece::ParagraphStart,
        })
        .collect();
    let smart = smartypants::smarten(&pieces, dashes);
    let texts: Vec<NodeId> = order
        .into_iter()
        .filter(|&id| matches!(doc.node(id).kind, Kind::Text(_)) && !verbatim.contains(&id))
        .collect();
    for (id, new) in texts.into_iter().zip(smart) {
        *doc.kind_mut(id) = Kind::Text(new);
    }
}

/// The nodes remark-smartypants visits under `id`, in order: text, inline
/// code and paragraphs, and Org's timestamps, which are read as code is.
/// Code blocks, raw HTML, math and image alt text are not prose.
fn prose(doc: &Document, id: NodeId, out: &mut Vec<NodeId>) {
    match doc.node(id).kind {
        Kind::Text(_) | Kind::Code(_) | Kind::Timestamp { .. } | Kind::Paragraph => out.push(id),
        Kind::Image { .. } => return,
        _ => {}
    }
    for &c in &doc.node(id).children {
        prose(doc, c, out);
    }
}

/// Each heading without an `{#id}` gets one from its smartened text, as
/// Astro gives it, but never one of the Entry's `labels`, which hold every
/// `{#id}`.
pub(crate) fn heading_ids(doc: &mut Document, labels: &BTreeSet<String>) {
    let headings: Vec<NodeId> = doc
        .descendants(Document::ROOT)
        .into_iter()
        .filter(|&id| matches!(doc.node(id).kind, Kind::Heading { .. }))
        .collect();
    let mut slugger = Slugger::default();
    for id in headings {
        if matches!(&doc.node(id).kind, Kind::Heading { id: anchor, .. } if !anchor.is_empty()) {
            continue;
        }
        let text = doc.text(id);
        let mut new = slugger.heading_id(&text);
        while labels.contains(&new) {
            new = slugger.heading_id(&text);
        }
        if let Kind::Heading { id: anchor, .. } = doc.kind_mut(id) {
            *anchor = new;
        }
    }
}

/// Where each line starts. A line ends at `\n`, `\r\n` or a lone `\r`, as
/// comrak counts lines.
fn line_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let ends = bytes.iter().enumerate().filter_map(|(i, &b)| match b {
        b'\n' => Some(i + 1),
        b'\r' if bytes.get(i + 1) != Some(&b'\n') => Some(i + 1),
        _ => None,
    });
    std::iter::once(0).chain(ends).collect()
}

struct Lower<'c, 'd> {
    doc: Document,
    /// The body, which `lines` index.
    source: &'c str,
    lines: Vec<usize>,
    base: usize,
    ok: bool,
    cx: &'c mut Cx<'d>,
}

impl Lower<'_, '_> {
    /// comrak's 1-based line and byte column, inclusive at the end, as a
    /// byte range in the file.
    fn span(&self, pos: Sourcepos) -> Range<usize> {
        let at = |line: usize, column: usize| match self.lines.get(line.wrapping_sub(1)) {
            Some(start) => self.base + start + column.saturating_sub(1),
            None => self.base,
        };
        let start = at(pos.start.line, pos.start.column);
        (start)..at(pos.end.line, pos.end.column + 1).max(start)
    }

    fn node<'a>(&mut self, node: &'a AstNode<'a>, parent: NodeId) {
        let ast = node.data.borrow();
        let span = self.span(ast.sourcepos);
        let kind = match &ast.value {
            NodeValue::Paragraph => Kind::Paragraph,
            NodeValue::Heading(h) => {
                // `{#a .b id=c class=d}`: `#a` wins over `id=`, and the
                // classes join, so each attribute is written once.
                let mut id = None;
                let mut classes = Vec::new();
                let mut attrs = Vec::new();
                if let Some(a) = ast.attrs.as_deref() {
                    id = a.id.clone();
                    classes.extend(a.classes.iter().cloned());
                    for (name, value) in &a.pairs {
                        match name.as_str() {
                            "id" => id = id.or_else(|| Some(value.clone())),
                            "class" => classes.push(value.clone()),
                            _ => attrs.push((name.clone(), value.clone())),
                        }
                    }
                }
                if !classes.is_empty() {
                    attrs.insert(0, ("class".to_string(), classes.join(" ")));
                }
                Kind::Heading {
                    level: h.level,
                    // Filled in once smartypants has run, as Astro's are (#18).
                    id: id.unwrap_or_default(),
                    attrs,
                }
            }
            NodeValue::ThematicBreak => Kind::ThematicBreak,
            NodeValue::BlockQuote => Kind::BlockQuote,
            NodeValue::List(l) => Kind::List {
                ordered: l.list_type == ListType::Ordered,
                start: l.start as u64,
                tight: l.tight,
            },
            NodeValue::Item(_) => Kind::Item {
                checked: None,
                value: None,
            },
            // mdast reads a task item with nothing after its box as an
            // item whose text is the box.
            NodeValue::TaskItem(t) if node.first_child().is_none() => {
                let item = Kind::Item {
                    checked: None,
                    value: None,
                };
                let item = self.doc.push(parent, item, span.clone());
                let p = self.doc.push(item, Kind::Paragraph, span.clone());
                let text = format!("[{}]", t.symbol.unwrap_or(' '));
                self.doc.push(p, Kind::Text(text), span);
                return;
            }
            NodeValue::TaskItem(t) => Kind::Item {
                checked: Some(match t.symbol {
                    Some(_) => Checkbox::Checked,
                    None => Checkbox::Unchecked,
                }),
                value: None,
            },
            NodeValue::CodeBlock(c) if c.info.split_whitespace().next() == Some("math") => {
                Kind::Math {
                    display: true,
                    tex: c.literal.clone(),
                    mathml: String::new(),
                }
            }
            // mdast's values, which the writer and word count follow, end
            // without the final newline.
            NodeValue::CodeBlock(c) => Kind::CodeBlock {
                lang: c.info.split_whitespace().next().map(str::to_string),
                text: without_line_end(&c.literal),
            },
            NodeValue::HtmlBlock(h) => Kind::HtmlBlock(without_line_end(&h.literal)),
            NodeValue::Table(t) => Kind::Table {
                align: t
                    .alignments
                    .iter()
                    .map(|a| match a {
                        TableAlignment::None => Align::None,
                        TableAlignment::Left => Align::Left,
                        TableAlignment::Center => Align::Center,
                        TableAlignment::Right => Align::Right,
                    })
                    .collect(),
            },
            NodeValue::TableRow(head) => Kind::TableRow { head: *head },
            NodeValue::TableCell => Kind::TableCell,
            // comrak leaves a definition inside another where it is, and
            // mdast never writes it.
            NodeValue::FootnoteDefinition(_) if parent != Document::ROOT => return,
            NodeValue::FootnoteDefinition(f) => Kind::FootnoteDefinition {
                label: self.label(&span).unwrap_or(&f.name).to_string(),
            },
            NodeValue::BlockDirective(d) => match self.directive(d, span.clone()) {
                Some(kind) => kind,
                None => return,
            },
            // mdast keeps a soft break as a newline in its text, which
            // smartypants then sees.
            NodeValue::Text(t) => return self.text(parent, t, span),
            NodeValue::SoftBreak => {
                let ending = self.line_ending(ast.sourcepos.start.line);
                return self.text(parent, ending, span);
            }
            NodeValue::Code(c) => Kind::Code(c.literal.clone()),
            NodeValue::Emph => Kind::Emphasis,
            NodeValue::Strong => Kind::Strong,
            NodeValue::Strikethrough => Kind::Strikethrough,
            NodeValue::Link(l) => Kind::Link {
                url: l.url.clone(),
                title: (!l.title.is_empty()).then(|| l.title.clone()),
            },
            NodeValue::Image(l) => Kind::Image {
                url: l.url.clone(),
                title: (!l.title.is_empty()).then(|| l.title.clone()),
            },
            NodeValue::LineBreak => Kind::LineBreak,
            NodeValue::HtmlInline(h) => Kind::HtmlInline(h.clone()),
            NodeValue::FootnoteReference(f) => Kind::FootnoteReference {
                label: self.label(&span).unwrap_or(&f.name).to_string(),
                number: 0,
            },
            NodeValue::Math(m) => Kind::Math {
                display: m.display_math,
                tex: m.literal.clone(),
                mathml: String::new(),
            },
            other => unreachable!("an extension yuanme leaves off: {other:?}"),
        };
        let id = self.doc.push(parent, kind, span);
        drop(ast);
        for child in node.children() {
            self.node(child, id);
        }
    }

    /// The label of the footnote written at `span` as it is written, as
    /// comrak changes its whitespace.
    fn label(&self, span: &Range<usize>) -> Option<&str> {
        let rest = self
            .source
            .get(span.start - self.base..)?
            .strip_prefix("[^")?;
        rest.get(..rest.find(']')?)
    }

    /// How the 1-based `line` ends in the source: `\n`, `\r\n` or `\r`.
    fn line_ending(&self, line: usize) -> &'static str {
        let end = self.lines.get(line).map_or(self.source.len(), |&e| e);
        let before = &self.source[..end];
        if before.ends_with("\r\n") {
            "\r\n"
        } else if before.ends_with('\r') {
            "\r"
        } else {
            "\n"
        }
    }

    /// Text joins the text before it, as one mdast text node would.
    fn text(&mut self, parent: NodeId, text: &str, span: Range<usize>) {
        if let Some(&last) = self.doc.node(parent).children.last()
            && let Kind::Text(before) = self.doc.kind_mut(last)
        {
            before.push_str(text);
            let end = span.end.max(self.doc.node(last).span.end);
            self.doc.extend_span(last, end);
            return;
        }
        self.doc.push(parent, Kind::Text(text.to_string()), span);
    }

    /// `:::name[Title]{…}`. Theorem kinds arrive with #51; until then every
    /// directive is a `<div class="NAME">` (#17).
    fn directive(&mut self, d: &NodeBlockDirective, span: Range<usize>) -> Option<Kind> {
        let info = d.info.as_str();
        let name: String = info
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let opener = span.start..span.start + d.fence_length;
        if name.is_empty() {
            let d = match info.trim() {
                // A bare `:::` opens a directive that swallows the rest of
                // the file (#2).
                "" => self
                    .cx
                    .error(
                        &codes::MARKDOWN_EMPTY_DIRECTIVE,
                        "this `:::` opens a block with no name".into(),
                        opener,
                    )
                    .help("a `:::` that closes a block must be no shorter than its opening fence, and an outer block needs more colons than the blocks inside it"),
                info => self
                    .cx
                    .error(
                        &codes::MARKDOWN_EMPTY_DIRECTIVE,
                        format!("a directive needs a name before `{info}`"),
                        opener,
                    )
                    .help("yuanme reads `:::name[Title]{#id}`, such as `:::theorem{#thm:main}` or `:::proof`; Pandoc's `::: {.name}` divs aren't read"),
            };
            self.cx.push(d);
            self.ok = false;
            return None;
        }
        if info[name.len()..].trim_start().starts_with('[') {
            let message = format!("a title has no effect on `:::{name}`");
            let d = Diagnostic::warning(&codes::MARKDOWN_NO_EFFECT, message, self.cx.span(opener))
                .note("only Theorem blocks show a title");
            self.cx.push(d);
        }
        Some(Kind::Directive { name })
    }
}

/// A code or HTML block's literal as mdast's value: without its final line
/// ending.
fn without_line_end(literal: &str) -> String {
    let text = literal.strip_suffix('\n').unwrap_or(literal);
    text.strip_suffix('\r').unwrap_or(text).to_string()
}
