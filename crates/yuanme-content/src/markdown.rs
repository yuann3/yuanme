//! Markdown into the Document (#47): comrak parses CommonMark and GFM, and
//! its tree is lowered here and dropped.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use comrak::nodes::{AstNode, ListType, NodeValue, Sourcepos, TableAlignment};
use comrak::{Arena, Options};
use jiff::tz::TimeZone;
use yuanme_diag::{Diagnostic, FileId, Files, Level, codes};

use crate::config::{Math, TheoremKind};
use crate::document::{Align, Checkbox, Document, Kind, NodeId, footnote_identifier};
use crate::fields::Cx;
use crate::slug::Slugger;
use crate::smartypants::{self, Dashes, Piece};
use crate::{math, theorem};

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
    let doc = parse(files.text(file), body, &Math::default(), &mut cx);
    (doc, diagnostics)
}

/// Parses an Entry's body, `text[body]`, with the Site's `[math]`. Spans
/// are bytes in `text`.
pub(crate) fn parse(
    text: &str,
    body: Range<usize>,
    math: &Math,
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
        theorems: &math.theorems,
        cx,
    };
    for child in root.children() {
        lower.node(child, Document::ROOT);
    }
    let Lower { mut doc, ok, .. } = lower;
    tex_delimiters(&doc, text, cx);
    number_footnotes(&mut doc);
    smarten(&mut doc, Dashes::Default, &BTreeSet::new());
    theorem::number(&mut doc, &math.theorems);
    let before = cx.diagnostics.len();
    let labels = math::render(&mut doc, text, cx.file, &math.macros, &[], cx.diagnostics);
    let ok = ok
        && !cx.diagnostics[before..]
            .iter()
            .any(|d| d.level == Level::Error);
    heading_ids(&mut doc, &labels);
    ok.then_some(doc)
}

/// Warns on each paragraph with math written as LaTeX writes it, which
/// Markdown reads as text (#17): `\(…\)`, `\[…\]` or `\begin{…}`.
fn tex_delimiters(doc: &Document, text: &str, cx: &mut Cx<'_>) {
    for id in doc.descendants(Document::ROOT) {
        let node = doc.node(id);
        if !matches!(node.kind, Kind::Paragraph) {
            continue;
        }
        // The paragraph as written, with what isn't prose blanked out.
        let mut prose = text[node.span.clone()].as_bytes().to_vec();
        for c in doc.descendants(id) {
            let inner = &doc.node(c);
            if matches!(
                inner.kind,
                Kind::Code(_) | Kind::HtmlInline(_) | Kind::Math { .. }
            ) {
                let from = inner.span.start.saturating_sub(node.span.start);
                let to = (inner.span.end - node.span.start).min(prose.len());
                prose[from.min(to)..to].fill(b' ');
            }
        }
        let prose = String::from_utf8_lossy(&prose);
        let paired = |open: &str, close: &str| {
            let i = prose.find(open)?;
            prose[i + 2..].contains(close).then_some(i..i + 2)
        };
        let found = paired("\\(", "\\)")
            .map(|at| {
                (
                    at,
                    "`\\(…\\)` isn't math in Markdown",
                    "write inline math as `$…$`",
                )
            })
            .or_else(|| {
                paired("\\[", "\\]").map(|at| {
                    (
                        at,
                        "`\\[…\\]` isn't math in Markdown",
                        "write display math as `$$…$$`",
                    )
                })
            })
            .or_else(|| {
                let i = prose.find("\\begin{")?;
                let end = prose[i..].find('}').map_or(i + 7, |e| i + e + 1);
                Some((
                    i..end,
                    "an environment isn't math in Markdown on its own",
                    "put it inside `$$…$$`",
                ))
            });
        if let Some((at, message, help)) = found {
            let at = node.span.start + at.start..node.span.start + at.end;
            let d = Diagnostic::warning(&codes::MATH_TEX_DELIMITER, message, cx.span(at))
                .help(help)
                .note(
                    "in Markdown `\\(` and `\\[` are escaped brackets, as on GitHub and in Pandoc",
                );
            cx.push(d);
        }
    }
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
    theorems: &'c BTreeMap<String, TheoremKind>,
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
                let (id, attrs) = match ast.attrs.as_deref() {
                    Some(a) => attributes(a.id.clone(), &a.classes, &a.pairs),
                    None => (None, Vec::new()),
                };
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
            NodeValue::BlockDirective(d) => {
                let fence = d.fence_length;
                drop(ast);
                return self.directive(node, fence, parent, span);
            }
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

    /// `:::name[Title]{#label .class key=value}`, read from the opening
    /// line as written: comrak's info string has its escapes resolved, so
    /// a title read from it couldn't be lowered with spans in the file. A
    /// directive whose name is a Theorem kind is a Theorem block; any other
    /// is a `<div class="NAME">` (#17).
    fn directive<'a>(
        &mut self,
        node: &'a AstNode<'a>,
        fence: usize,
        parent: NodeId,
        span: Range<usize>,
    ) {
        let opener = span.start..span.start + fence;
        let start = opener.end - self.base;
        let end = self.source[start..]
            .find(['\r', '\n'])
            .map_or(self.source.len(), |i| start + i);
        let line = &self.source[start..end];
        // Where `line[range]` is in the file.
        let at = |range: Range<usize>| opener.end + range.start..opener.end + range.end;
        let mut i = line.len() - line.trim_start().len();
        let name: String = line[i..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if name.is_empty() {
            let d = match line.trim() {
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
            return;
        }
        i += name.len();
        let skip = |i: usize| i + line[i..].len() - line[i..].trim_start().len();
        let mut title = None;
        if line[skip(i)..].starts_with('[')
            && let Some(len) = closing(&line[skip(i)..], '[', ']')
        {
            title = Some(skip(i) + 1..skip(i) + len - 1);
            i = skip(i) + len;
        }
        let mut attrs = Vec::new();
        if line[skip(i)..].starts_with('{')
            && let Some(len) = closing(&line[skip(i)..], '{', '}')
            && let Some(read) = directive_attributes(&line[skip(i) + 1..skip(i) + len - 1])
        {
            attrs = read;
            i = skip(i) + len;
        }
        let rest = line[i..].trim();
        if !rest.is_empty() {
            let from = skip(i);
            let message = format!("`{rest}` has no effect on `:::{name}`");
            let d = Diagnostic::warning(
                &codes::MARKDOWN_NO_EFFECT,
                message,
                self.cx.span(at(from..from + rest.len())),
            )
            .note("yuanme reads `:::name[Title]{#label .class key=value}`, with `\\]` for a `]` in the title and `\"…\"` around a value with spaces");
            self.cx.push(d);
        }

        let theorem = self.theorems.contains_key(&name);
        if title.is_some() && !theorem {
            let message = format!("a title has no effect on `:::{name}`");
            let d = Diagnostic::warning(&codes::MARKDOWN_NO_EFFECT, message, self.cx.span(opener))
                .note("only Theorem blocks show a title");
            self.cx.push(d);
        }
        let kind = if theorem {
            Kind::Theorem {
                kind: name,
                name: String::new(),
            }
        } else {
            Kind::Directive { name }
        };
        let id = self.doc.push(parent, kind, span);
        if let Some((_, label)) = attrs.iter().find(|(a, _)| a == "id") {
            self.doc
                .names
                .targets
                .entry(label.clone())
                .or_insert_with(|| label.clone());
        }
        *self.doc.attrs_mut(id) = attrs;
        if let Some(title) = title.filter(|t| theorem && !t.is_empty()) {
            self.title(start + title.start..start + title.end, id);
        }
        for child in node.children() {
            self.node(child, id);
        }
    }

    /// A Theorem block's title, `self.source[range]`, as inline Markdown in
    /// a Caption.
    fn title(&mut self, range: Range<usize>, parent: NodeId) {
        let source = self.source;
        let text = &source[range.clone()];
        let at = self.base + range.start..self.base + range.end;
        let caption = self.doc.push(parent, Kind::Caption, at.clone());
        let arena = Arena::new();
        let root = match comrak::parse_document_bounded(&arena, text, &options()) {
            Ok(root) => root,
            Err(comrak::OutOfFuel { .. }) => {
                let d = self
                    .cx
                    .error(
                        &codes::MARKDOWN_OUT_OF_FUEL,
                        "this title takes too long to parse".into(),
                        at,
                    )
                    .help("markup opened many times and never closed is almost always a mistake");
                self.cx.push(d);
                self.ok = false;
                return;
            }
        };
        // A title that reads as a block of its own, such as `1. Main`, is
        // shown as it is written.
        let paragraph = root
            .first_child()
            .filter(|p| p.next_sibling().is_none())
            .filter(|p| matches!(p.data.borrow().value, NodeValue::Paragraph));
        let Some(paragraph) = paragraph else {
            return self.text(caption, text, at);
        };
        let lines = std::mem::replace(&mut self.lines, line_starts(text));
        let base = std::mem::replace(&mut self.base, at.start);
        self.source = text;
        for child in paragraph.children() {
            self.node(child, caption);
        }
        (self.source, self.lines, self.base) = (source, lines, base);
    }
}

/// `{#a .b id=c class=d}` as an element's id and its other attributes:
/// `#a` wins over `id=`, and the classes join, so each attribute is
/// written once.
fn attributes(
    mut id: Option<String>,
    classes: &[String],
    pairs: &[(String, String)],
) -> (Option<String>, Vec<(String, String)>) {
    let mut classes = classes.to_vec();
    let mut attrs = Vec::new();
    for (name, value) in pairs {
        match name.as_str() {
            "id" => id = id.or_else(|| Some(value.clone())),
            "class" => classes.push(value.clone()),
            _ => attrs.push((name.clone(), value.clone())),
        }
    }
    if !classes.is_empty() {
        attrs.insert(0, ("class".to_string(), classes.join(" ")));
    }
    (id, attrs)
}

/// The attributes between a directive's `{` and `}`, the id first, or
/// none if a word there is not `#label`, `.class` or `key=value`. Unlike a
/// heading's, a Label is read whatever it holds, so a wrong one is
/// reported as a Label rather than as text with no effect. A value can be
/// quoted, with `\` escaping in the quotes.
fn directive_attributes(inner: &str) -> Option<Vec<(String, String)>> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quoted = false;
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                word.get_or_insert_default();
            }
            '\\' if quoted => word.get_or_insert_default().push(chars.next()?),
            c if c.is_whitespace() && !quoted => words.extend(word.take()),
            c => word.get_or_insert_default().push(c),
        }
    }
    if quoted {
        return None;
    }
    words.extend(word);
    let mut id = None;
    let mut classes = Vec::new();
    let mut pairs = Vec::new();
    for w in words {
        if let Some(label) = w.strip_prefix('#').filter(|l| !l.is_empty()) {
            id = Some(label.to_string());
        } else if let Some(class) = w.strip_prefix('.').filter(|c| !c.is_empty()) {
            classes.push(class.to_string());
        } else {
            let (key, value) = w.split_once('=')?;
            if key.is_empty()
                || !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_:.".contains(c))
            {
                return None;
            }
            pairs.push((key.to_string(), value.to_string()));
        }
    }
    let (id, mut attrs) = attributes(id, &classes, &pairs);
    if let Some(id) = id {
        attrs.insert(0, ("id".to_string(), id));
    }
    Some(attrs)
}

/// The length of the group `s` opens with `open`, through the `close` that
/// ends it, with groups nested inside it and `\` escaping a character. A
/// `"…"` in `{…}` can hold a `}`.
fn closing(s: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut quoted = false;
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '"' if open == '{' => quoted = !quoted,
            _ if quoted => {}
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// A code or HTML block's literal as mdast's value: without its final line
/// ending.
fn without_line_end(literal: &str) -> String {
    let text = literal.strip_suffix('\n').unwrap_or(literal);
    text.strip_suffix('\r').unwrap_or(text).to_string()
}
