//! Markdown into the Document (#47): comrak parses CommonMark and GFM, and
//! its tree is lowered here and dropped.

use std::ops::Range;

use comrak::nodes::{AstNode, ListType, NodeValue, Sourcepos, TableAlignment};
use comrak::{Arena, Options};
use jiff::tz::TimeZone;
use yuanme_diag::{Diagnostic, FileId, Files, codes};

use crate::document::{Align, Document, Kind, NodeId};
use crate::fields::Cx;
use crate::slug::Slugger;
use crate::smartypants::{self, Dashes, Piece};

/// The comrak extensions the Engine reads (#2, #17). Raw HTML is kept and
/// written out as it is (ADR 0009); comrak's own HTML is never used.
fn options() -> Options<'static> {
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
    let doc = parse(files.text(file), body, &mut cx);
    (doc, diagnostics)
}

/// Parses an Entry's body, `text[body]`. Spans are bytes in `text`.
pub(crate) fn parse(text: &str, body: Range<usize>, cx: &mut Cx<'_>) -> Option<Document> {
    let source = &text[body.clone()];
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, source, &options());
    let mut lower = Lower {
        doc: Document::new(body.clone()),
        lines: line_starts(source),
        base: body.start,
        ok: true,
        cx,
    };
    for child in root.children() {
        lower.node(child, Document::ROOT);
    }
    let Lower { mut doc, ok, .. } = lower;
    smarten(&mut doc);
    heading_ids(&mut doc);
    ok.then_some(doc)
}

/// remark-smartypants over the Document, which needs its text in mdast's
/// order: comrak moves footnote definitions to the end, mdast keeps them
/// where they are written.
fn smarten(doc: &mut Document) {
    let mut blocks = doc.node(Document::ROOT).children.clone();
    blocks.sort_by_key(|&b| doc.node(b).span.start);
    let mut order = Vec::new();
    for b in blocks {
        prose(doc, b, &mut order);
    }
    let pieces: Vec<Piece<'_>> = order
        .iter()
        .map(|&id| match &doc.node(id).kind {
            Kind::Text(s) => Piece::Text(s),
            Kind::Code(s) => Piece::Code(s),
            _ => Piece::ParagraphStart,
        })
        .collect();
    let smart = smartypants::smarten(&pieces, Dashes::Default);
    let texts: Vec<NodeId> = order
        .into_iter()
        .filter(|&id| matches!(doc.node(id).kind, Kind::Text(_)))
        .collect();
    for (id, new) in texts.into_iter().zip(smart) {
        *doc.kind_mut(id) = Kind::Text(new);
    }
}

/// The nodes remark-smartypants visits under `id`, in order: text, inline
/// code and paragraphs. Code blocks, raw HTML, math and image alt text are
/// not prose.
fn prose(doc: &Document, id: NodeId, out: &mut Vec<NodeId>) {
    match doc.node(id).kind {
        Kind::Text(_) | Kind::Code(_) | Kind::Paragraph => out.push(id),
        Kind::Image { .. } => return,
        _ => {}
    }
    for &c in &doc.node(id).children {
        prose(doc, c, out);
    }
}

/// Each heading without an `{#id}` gets one from its smartened text, as
/// Astro gives it.
fn heading_ids(doc: &mut Document) {
    let mut slugger = Slugger::default();
    for id in doc.descendants(Document::ROOT) {
        let Kind::Heading { id: anchor, .. } = &doc.node(id).kind else {
            continue;
        };
        if !anchor.is_empty() {
            continue;
        }
        let new = slugger.heading_id(&doc.text(id));
        if let Kind::Heading { id: anchor, .. } = doc.kind_mut(id) {
            *anchor = new;
        }
    }
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

struct Lower<'c, 'd> {
    doc: Document,
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
                let attrs = ast.attrs.as_deref();
                let classes = attrs.map(|a| a.classes.join(" ")).filter(|c| !c.is_empty());
                Kind::Heading {
                    level: h.level,
                    // Filled in once smartypants has run, as Astro's are (#18).
                    id: attrs.and_then(|a| a.id.clone()).unwrap_or_default(),
                    attrs: classes
                        .map(|c| ("class".to_string(), c))
                        .into_iter()
                        .chain(attrs.into_iter().flat_map(|a| a.pairs.iter().cloned()))
                        .collect(),
                }
            }
            NodeValue::ThematicBreak => Kind::ThematicBreak,
            NodeValue::BlockQuote => Kind::BlockQuote,
            NodeValue::List(l) => Kind::List {
                ordered: l.list_type == ListType::Ordered,
                start: l.start as u64,
                tight: l.tight,
            },
            NodeValue::Item(_) => Kind::Item { checked: None },
            NodeValue::TaskItem(t) => Kind::Item {
                checked: Some(t.symbol.is_some()),
            },
            NodeValue::CodeBlock(c) if c.info.split_whitespace().next() == Some("math") => {
                Kind::Math {
                    display: true,
                    tex: c.literal.clone(),
                }
            }
            // mdast's values, which the writer and word count follow, end
            // without the final newline.
            NodeValue::CodeBlock(c) => Kind::CodeBlock {
                lang: c.info.split_whitespace().next().map(str::to_string),
                text: c
                    .literal
                    .strip_suffix('\n')
                    .unwrap_or(&c.literal)
                    .to_string(),
            },
            NodeValue::HtmlBlock(h) => Kind::HtmlBlock(
                h.literal
                    .strip_suffix('\n')
                    .unwrap_or(&h.literal)
                    .to_string(),
            ),
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
            NodeValue::FootnoteDefinition(f) => Kind::FootnoteDefinition {
                label: f.name.clone(),
            },
            NodeValue::BlockDirective(d) => match self.directive(&d.info, span.clone()) {
                Some(kind) => kind,
                None => return,
            },
            // mdast keeps a soft break as a newline in its text, which
            // smartypants then sees.
            NodeValue::Text(t) => return self.text(parent, t, span),
            NodeValue::SoftBreak => return self.text(parent, "\n", span),
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
                label: f.name.clone(),
            },
            NodeValue::Math(m) => Kind::Math {
                display: m.display_math,
                tex: m.literal.clone(),
            },
            other => unreachable!("an extension yuanme leaves off: {other:?}"),
        };
        let id = self.doc.push(parent, kind, span);
        drop(ast);
        for child in node.children() {
            self.node(child, id);
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
    fn directive(&mut self, info: &str, span: Range<usize>) -> Option<Kind> {
        let name: String = info
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let opener = span.start..span.start + 3;
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
