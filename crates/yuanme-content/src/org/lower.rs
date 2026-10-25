//! organic's tree lowered into the Document. Where a construct is in
//! Markdown too it becomes the node Markdown's does, so it is written as
//! Markdown's HTML; constructs only Org has keep ox-html's element and
//! class names (#16). The output is flat: headlines are headings among
//! their sections' blocks, with no outline wrappers.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use yuanme_diag::{Diagnostic, FileId, Span, codes};
use yuanme_org::types::{
    AffiliatedKeywordValue, AffiliatedKeywords, AstNode, CheckboxType, Document as OrgDocument,
    DocumentElement, Element, FootnoteReference, Heading, LinkType, NodeProperty, Object,
    Paragraph, PlainList, PlainListType, SrcBlock, StandardProperties, Table, Timestamp,
    TimestampRangeType, TimestampType,
};

use super::{Keyword, Read};
use crate::document::{Align, Checkbox, Document, Kind, NodeId, footnote_identifier};
use crate::smartypants::Dashes;
use crate::{markdown, math};

/// How `x_y` and `x^{y}` read: `^:{}` by default, so `snake_case` stays
/// as it is (Org divergence 1).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scripts {
    Braces,
    All,
    None,
}

/// What a link resolves to once the whole file is read.
enum Search {
    /// `[[#id]]`.
    CustomId(String),
    /// `[[*Title]]`.
    Title(String),
    /// `[[name]]`: a target or `#+NAME` first, then a headline's title.
    Fuzzy(String),
}

/// Image links ox-html shows as images (`org-html-inline-image-rules`).
const IMAGES: &[&str] = &["avif", "gif", "jpeg", "jpg", "png", "svg", "webp"];

pub(super) fn lower(
    text: &str,
    file: FileId,
    parsed: &OrgDocument<'_>,
    macros: &[(String, String)],
) -> Read {
    let mut l = Lower {
        text,
        file,
        doc: Document::new(0..text.len()),
        diagnostics: Vec::new(),
        scripts: Scripts::Braces,
        exclude: vec!["noexport".into()],
        select: vec!["export".into()],
        verbatim: BTreeSet::new(),
        verse: None,
        titles: Vec::new(),
        ids: Vec::new(),
        searches: Vec::new(),
        definitions: BTreeSet::new(),
        references: Vec::new(),
        anonymous: 0,
        headers: Vec::new(),
    };
    let keywords = l.keywords(parsed);
    for p in parsed.get_additional_properties() {
        l.property(p, None);
    }
    let selecting = parsed.children.iter().any(|h| l.has_select(h));
    // With a select tag, only the trees it marks are exported, and the text
    // before the first headline isn't (`org-export--prune-tree`).
    if let Some(zeroth) = &parsed.zeroth_section
        && !selecting
    {
        l.elements(&zeroth.children, Document::ROOT);
    }
    for h in &parsed.children {
        l.heading(h, selecting, false);
    }
    l.finish(macros);
    let preamble_end = preamble_end(text, parsed);
    Read {
        document: Some(l.doc),
        keywords,
        preamble: 0..preamble_end,
        diagnostics: l.diagnostics,
    }
}

/// Where the preamble ends: at the first headline or body text, after the
/// keywords, comments, blank lines and file-level property drawer.
fn preamble_end(text: &str, parsed: &OrgDocument<'_>) -> usize {
    let body = parsed
        .zeroth_section
        .iter()
        .flat_map(|z| &z.children)
        .find(|e| {
            !matches!(
                e,
                Element::Keyword(_) | Element::Comment(_) | Element::PropertyDrawer(_)
            )
        });
    let first = body
        .map(|e| e.get_source())
        .or_else(|| parsed.children.first().map(|h| h.source));
    first.map_or(text.len(), |s| offset(text, s))
}

/// Where `s`, a slice of `text`, starts in it.
fn offset(text: &str, s: &str) -> usize {
    (s.as_ptr() as usize)
        .checked_sub(text.as_ptr() as usize)
        .filter(|&o| o <= text.len())
        .expect("a slice of the file")
}

/// The spaces and tabs after an object, which org-element counts as its
/// post-blank: plain text and line breaks have none.
fn post_blank<'s>(o: &Object<'s>) -> &'s str {
    match o {
        Object::PlainText(_) | Object::LineBreak(_) => "",
        _ => {
            let source = o.get_source();
            &source[source.trim_end_matches([' ', '\t']).len()..]
        }
    }
}

fn is_image(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| IMAGES.iter().any(|i| ext.eq_ignore_ascii_case(i)))
}

/// Runs of whitespace as one space, as Org compares link paths and titles.
fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Lower<'s> {
    text: &'s str,
    file: FileId,
    doc: Document,
    diagnostics: Vec<Diagnostic>,
    scripts: Scripts,
    exclude: Vec<String>,
    select: Vec<String>,
    /// Text nodes smartypants must leave alone: entities.
    verbatim: BTreeSet<NodeId>,
    /// In a verse block, whether the next text starts a line.
    verse: Option<bool>,
    /// Each headline's title as written, and its heading.
    titles: Vec<(String, NodeId)>,
    /// `:ID:` properties, with the heading each is on.
    ids: Vec<(String, Option<NodeId>, Range<usize>)>,
    /// In-page links, resolved once every heading has its id.
    searches: Vec<(NodeId, Search, Range<usize>)>,
    definitions: BTreeSet<String>,
    references: Vec<(String, Range<usize>)>,
    anonymous: usize,
    /// Math macros from `#+LATEX_HEADER`.
    headers: Vec<math::Header>,
}

impl<'s> Lower<'s> {
    fn at(&self, s: &str) -> Range<usize> {
        let start = offset(self.text, s);
        start..start + s.trim_end().len()
    }

    fn span(&self, range: Range<usize>) -> Span {
        Span {
            file: self.file,
            range,
        }
    }

    /// Org outside the Org subset, which fails the build (Org divergence 4).
    fn unsupported(&mut self, what: &str, source: &str, help: &str) {
        let d = Diagnostic::error(
            &codes::ORG_UNSUPPORTED_CONSTRUCT,
            format!("{what} aren't in the Org subset"),
            self.span(self.at(source)),
        )
        .help(help);
        self.diagnostics.push(d);
    }

    fn no_effect(&mut self, message: String, at: Range<usize>, note: &str) {
        let d = Diagnostic::warning(&codes::ORG_NO_EFFECT, message, self.span(at)).note(note);
        self.diagnostics.push(d);
    }

    /// Every keyword, for the Entry's fields, and the in-buffer settings
    /// that change how the body reads. Org applies a setting wherever it
    /// is written.
    fn keywords(&mut self, parsed: &OrgDocument<'s>) -> Vec<Keyword> {
        let end = preamble_end(self.text, parsed);
        let mut out = Vec::new();
        let mut exclude = Vec::new();
        let mut select = Vec::new();
        for node in AstNode::from(parsed).iter_all_ast_nodes() {
            let AstNode::Keyword(k) = node else { continue };
            let key = k.key.to_ascii_uppercase();
            let start = offset(self.text, k.source);
            match key.as_str() {
                "SETUPFILE" => {
                    let d = Diagnostic::error(
                        &codes::ORG_SETUPFILE,
                        "`#+SETUPFILE` isn't read: an Entry is one file".to_string(),
                        self.span(self.at(k.source)),
                    )
                    .help("put shared settings where yuanme reads them: `#+OPTIONS` in each file, LaTeX macros in `[math]` in yuanme.toml, and styles, scripts and analytics in Templates");
                    self.diagnostics.push(d);
                    continue;
                }
                "INCLUDE" => {
                    self.unsupported(
                        "`#+INCLUDE` keywords",
                        k.source,
                        "copy the text in, or link to the other file",
                    );
                    continue;
                }
                "MACRO" => {
                    self.unsupported(
                        "macros",
                        k.source,
                        "write the text the macro would expand to",
                    );
                    continue;
                }
                // Raw HTML is written where it stands; the other back
                // ends' are dropped, as ox-html drops them (ADR 0009).
                "HTML" | "LATEX" | "BEAMER" | "ASCII" | "MAN" | "MD" | "ODT" | "TEXINFO" => {
                    continue;
                }
                "OPTIONS" => {
                    for token in k.value.split_whitespace() {
                        match token {
                            "^:t" => self.scripts = Scripts::All,
                            "^:nil" => self.scripts = Scripts::None,
                            "^:{}" => self.scripts = Scripts::Braces,
                            _ => {}
                        }
                    }
                }
                "EXCLUDE_TAGS" => exclude.extend(k.value.split_whitespace().map(String::from)),
                "LATEX_HEADER" => self.latex_header(k.value),
                "SELECT_TAGS" => select.extend(k.value.split_whitespace().map(String::from)),
                _ => {}
            }
            let key_start = offset(self.text, k.key);
            // An empty value may be no slice of the file.
            let value_start = if k.value.is_empty() {
                key_start + k.key.len()
            } else {
                offset(self.text, k.value)
            };
            out.push(Keyword {
                key: k.key.to_string(),
                key_at: key_start..key_start + k.key.len(),
                value: k.value.to_string(),
                value_at: value_start..value_start + k.value.len(),
                preamble: start < end,
            });
        }
        if !exclude.is_empty() {
            self.exclude = exclude;
        }
        if !select.is_empty() {
            self.select = select;
        }
        out
    }

    fn has_select(&self, h: &Heading<'_>) -> bool {
        h.tags.iter().any(|t| self.select.iter().any(|s| s == t))
            || h.children.iter().any(|c| match c {
                DocumentElement::Heading(c) => self.has_select(c),
                DocumentElement::Section(_) => false,
            })
    }

    /// A property of a headline, or of the file when `heading` is `None`.
    /// Returns a `CUSTOM_ID`.
    fn property(&mut self, p: &NodeProperty<'s>, heading: Option<NodeId>) -> Option<String> {
        let name = p.property_name.to_ascii_uppercase();
        let value = p.value.unwrap_or("").trim();
        match name.as_str() {
            "CUSTOM_ID" if !value.is_empty() => return Some(value.to_string()),
            "ID" if !value.is_empty() => {
                let at = self.at(p.source);
                self.ids.push((value.to_string(), heading, at));
            }
            _ if name.starts_with("EXPORT_") => {
                let replacement = match name.as_str() {
                    "EXPORT_FILE_NAME" => {
                        "one Org file is one Entry, so give this subtree a file of its own"
                    }
                    "EXPORT_TITLE" => "use `#+TITLE` in the Entry's own file",
                    "EXPORT_DATE" => "use `#+DATE` in the Entry's own file",
                    "EXPORT_OPTIONS" => "use `#+OPTIONS` in the Entry's own file",
                    _ => "one Org file is one Entry, and its keywords set its fields",
                };
                let d = Diagnostic::error(
                    &codes::ORG_UNSUPPORTED_CONSTRUCT,
                    format!(
                        "`:{}:` properties aren't in the Org subset",
                        p.property_name
                    ),
                    self.span(self.at(p.source)),
                )
                .help(replacement);
                self.diagnostics.push(d);
            }
            _ => {}
        }
        None
    }

    /// `selected` says a select tag is on it or above it, and `selecting`
    /// that the file has one at all.
    fn heading(&mut self, h: &Heading<'s>, selecting: bool, selected: bool) {
        let tagged = |tags: &[String]| h.tags.iter().any(|t| tags.iter().any(|x| x == t));
        if h.is_comment || tagged(&self.exclude) {
            return;
        }
        let selected = selected || tagged(&self.select);
        if selecting && !selected && !self.has_select(h) {
            return;
        }
        let line = h.source.split_inclusive('\n').next().unwrap_or(h.source);
        let at = self.at(line);
        let level = usize::from(h.level);
        let mut node = None;
        if h.tags.contains(&"ignore") || h.is_footnote_section {
            // The heading goes; what is under it stays.
        } else if level > 5 {
            let what = if level >= 15 {
                "inline tasks"
            } else {
                "headlines deeper than level 5"
            };
            self.unsupported(
                what,
                line,
                "`*` is `<h2>` and `*****` is `<h6>`, the deepest heading HTML has; make it a list item or a bold paragraph",
            );
        } else {
            let kind = Kind::Heading {
                level: (level + 1) as u8,
                id: String::new(),
                attrs: Vec::new(),
            };
            let id = self.doc.push(Document::ROOT, kind, at);
            for o in &h.title {
                self.object(o, id);
            }
            self.trim(id);
            self.titles.push((squash(&h.get_raw_value()), id));
            node = Some(id);
        }
        let properties: Vec<_> = h.get_additional_properties().collect();
        for p in properties {
            if let Some(custom) = self.property(p, node)
                && let Some(id) = node
                && let Kind::Heading { id: anchor, .. } = self.doc.kind_mut(id)
            {
                *anchor = custom;
            }
        }
        // `org-export-with-archived-trees` is `headline`.
        if h.is_archived {
            return;
        }
        for child in &h.children {
            match child {
                DocumentElement::Section(s) => self.elements(&s.children, Document::ROOT),
                DocumentElement::Heading(c) => self.heading(c, selecting, selected),
            }
        }
    }

    /// Elements in order. A source block decides whether the `#+RESULTS:`
    /// after it are shown.
    fn elements(&mut self, elements: &[Element<'s>], parent: NodeId) {
        let mut i = 0;
        while i < elements.len() {
            if let Element::SrcBlock(s) = &elements[i] {
                let (code, results) = exports(s);
                if code {
                    self.src(s, parent);
                }
                if let Some(next) = elements.get(i + 1)
                    && affiliated(next).is_some_and(|a| a.keywords.contains_key("results"))
                {
                    if results {
                        self.element(next, parent);
                    }
                    i += 1;
                }
            } else {
                self.element(&elements[i], parent);
            }
            i += 1;
        }
    }

    fn element(&mut self, e: &Element<'s>, parent: NodeId) {
        let at = self.at(e.get_source());
        match e {
            Element::Paragraph(p) => self.paragraph(p, parent),
            Element::PlainList(l) => self.list(l, parent),
            Element::CenterBlock(b) => {
                let kind = Kind::Directive {
                    name: "org-center".into(),
                };
                let id = self.doc.push(parent, kind, at);
                self.decorate(&b.affiliated_keywords, id, false);
                self.elements(&b.children, id);
            }
            Element::QuoteBlock(b) => {
                let id = self.doc.push(parent, Kind::BlockQuote, at);
                self.decorate(&b.affiliated_keywords, id, false);
                self.elements(&b.children, id);
            }
            Element::SpecialBlock(b) => {
                let kind = Kind::Directive {
                    name: b.block_type.to_string(),
                };
                let id = self.doc.push(parent, kind, at);
                self.decorate(&b.affiliated_keywords, id, true);
                self.elements(&b.children, id);
            }
            Element::DynamicBlock(b) => self.unsupported(
                "dynamic blocks",
                b.source,
                "dynamic blocks are filled in by Emacs; paste in what it wrote and remove the `#+BEGIN:` and `#+END:` lines",
            ),
            Element::FootnoteDefinition(f) => {
                let key = footnote_identifier(f.label);
                if self.definitions.insert(key) {
                    let kind = Kind::FootnoteDefinition {
                        label: f.label.to_string(),
                    };
                    let id = self.doc.push(Document::ROOT, kind, at);
                    self.elements(&f.children, id);
                }
            }
            Element::Drawer(d) => {
                // ox-html writes a drawer's contents, without the drawer;
                // `org-export-with-drawers` is `(not "LOGBOOK")`.
                if !d.drawer_name.eq_ignore_ascii_case("LOGBOOK") {
                    self.elements(&d.children, parent);
                }
            }
            Element::Table(t) => self.table(t, parent),
            Element::VerseBlock(v) => {
                let id = self.doc.push(parent, Kind::Paragraph, at);
                self.doc
                    .attrs_mut(id)
                    .push(("class".into(), "verse".into()));
                self.decorate(&v.affiliated_keywords, id, false);
                self.verse = Some(true);
                for o in &v.children {
                    self.object(o, id);
                }
                self.verse = None;
            }
            Element::ExampleBlock(b) => {
                self.switches(b.switches);
                let text = line(&dedent(&b.get_value(), b.preserve_indent.is_some()));
                let id = self.doc.push(parent, Kind::CodeBlock { lang: None, text }, at);
                self.decorate(&b.affiliated_keywords, id, false);
            }
            Element::ExportBlock(b) => {
                if b.export_type.is_some_and(|t| t.eq_ignore_ascii_case("html")) {
                    let raw = line(&b.get_value());
                    self.doc.push(parent, Kind::HtmlBlock(raw), at);
                }
            }
            Element::SrcBlock(s) => self.src(s, parent),
            Element::FixedWidthArea(f) => {
                let text = f.value.join("\n");
                let id = self.doc.push(parent, Kind::CodeBlock { lang: None, text }, at);
                self.decorate(&f.affiliated_keywords, id, false);
            }
            Element::HorizontalRule(r) => {
                let id = self.doc.push(parent, Kind::ThematicBreak, at);
                self.decorate(&r.affiliated_keywords, id, false);
            }
            Element::Keyword(k) => {
                if k.key.eq_ignore_ascii_case("html") {
                    self.doc
                        .push(parent, Kind::HtmlBlock(k.value.to_string()), at);
                }
            }
            Element::BabelCall(c) => self.unsupported(
                "`#+CALL` lines",
                c.source,
                "yuanme never runs code; paste in the results the call wrote",
            ),
            Element::LatexEnvironment(env) => self.environment(env.value, &env.affiliated_keywords, parent, at),
            // Read, not rendered (#16).
            Element::Comment(_)
            | Element::CommentBlock(_)
            | Element::PropertyDrawer(_)
            | Element::Clock(_)
            | Element::DiarySexp(_)
            | Element::Planning(_) => {}
        }
    }

    /// A LaTeX environment standing alone, which is display math when it
    /// is one an equation can be.
    fn environment(
        &mut self,
        value: &'s str,
        a: &AffiliatedKeywords<'s>,
        parent: NodeId,
        at: Range<usize>,
    ) {
        let indent = value.len() - value.trim_start().len();
        let name = value[indent..]
            .strip_prefix("\\begin{")
            .and_then(|v| v.split_once('}'))
            .map_or("", |(name, _)| name);
        // math-core reports an environment it doesn't have; one it has
        // only goes inside math, as LaTeX has it.
        if INNER.contains(&name) {
            let start = offset(self.text, value) + indent + "\\begin{".len();
            let d = Diagnostic::error(
                &codes::MATH_UNSUPPORTED,
                format!("`{name}` goes inside math, not on its own"),
                self.span(start..start + name.len()),
            )
            .help("put it inside `\\[…\\]`, or inside `equation` to number it");
            self.diagnostics.push(d);
        }
        if let Some(name_at) = self.affiliated_at(a, "name") {
            let label = match a.keywords.get("name") {
                Some(AffiliatedKeywordValue::SingleString(n)) => n.trim(),
                _ => "",
            };
            let d = Diagnostic::error(
                &codes::MATH_NAMED_ENVIRONMENT,
                "an equation is labelled with `\\label`, not `#+NAME`",
                self.span(name_at),
            )
            .help(format!(
                "remove the `#+NAME` and write `\\label{{{label}}}` inside the environment"
            ));
            self.diagnostics.push(d);
            // So a link to it isn't reported too.
            self.doc
                .names
                .targets
                .entry(squash(label))
                .or_insert_with(|| label.into());
        }
        let kind = Kind::Math {
            display: true,
            tex: line(value),
            mathml: String::new(),
        };
        let id = self.doc.push(parent, kind, at);
        self.affiliated(a, id, false);
    }

    /// A `#+LATEX_HEADER` line (#17): one that defines macros defines them
    /// for the Entry's math, a `\usepackage` of what yuanme's math has
    /// anyway is accepted, and any other has no effect.
    fn latex_header(&mut self, value: &'s str) {
        let tex = value.trim();
        if tex.is_empty() {
            return;
        }
        let start = offset(self.text, value) + value.find(tex).unwrap_or(0);
        let at = start..start + tex.len();
        if let Some(rest) = tex.strip_prefix("\\usepackage") {
            let packages = rest
                .split_once('{')
                .and_then(|(_, p)| p.split_once('}'))
                .map_or("", |(p, _)| p);
            if !packages.is_empty() && packages.split(',').all(|p| PACKAGES.contains(&p.trim())) {
                return;
            }
        } else if let Some(defined) = declare_math_operator(tex) {
            self.headers.push(math::Header {
                tex: defined,
                at,
                verbatim: false,
            });
            return;
        } else if [
            "\\newcommand",
            "\\renewcommand",
            "\\providecommand",
            "\\def",
        ]
        .iter()
        .any(|c| tex.starts_with(c))
        {
            self.headers.push(math::Header {
                tex: tex.to_string(),
                at,
                verbatim: true,
            });
            return;
        }
        self.no_effect(
            "this `#+LATEX_HEADER` has no effect in yuanme".into(),
            at,
            "yuanme reads `\\newcommand`, `\\renewcommand`, `\\def` and `\\DeclareMathOperator` lines, which define macros for the Entry's math, and accepts `\\usepackage` of amsmath, amssymb, amsfonts, amsthm, mathtools and bm",
        );
    }

    /// `#+NAME` as the element's id, `#+ATTR_HTML` as its attributes where
    /// `attributes` says it takes them, and a warning for a caption it
    /// can't show.
    fn decorate(&mut self, a: &AffiliatedKeywords<'s>, id: NodeId, attributes: bool) {
        self.name(a, id);
        self.affiliated(a, id, attributes);
    }

    /// `decorate` without the `#+NAME`.
    fn affiliated(&mut self, a: &AffiliatedKeywords<'s>, id: NodeId, attributes: bool) {
        if attributes {
            let attrs = self.attr_html(a);
            self.doc.attrs_mut(id).extend(attrs);
        } else if let Some(at) = self.affiliated_at(a, "attr_html") {
            self.no_effect(
                "`#+ATTR_HTML` has no effect here".into(),
                at,
                "it puts attributes on images, links, tables, special blocks and captioned source blocks",
            );
        }
        if let Some(at) = self.affiliated_at(a, "caption") {
            self.no_effect(
                "`#+CAPTION` has no effect here".into(),
                at,
                "only an image alone in its paragraph, a table and a source block show a caption",
            );
        }
    }

    fn name(&mut self, a: &AffiliatedKeywords<'s>, id: NodeId) {
        if let Some(AffiliatedKeywordValue::SingleString(name)) = a.keywords.get("name") {
            let name = name.trim();
            if !name.is_empty() {
                self.doc.attrs_mut(id).insert(0, ("id".into(), name.into()));
                self.doc
                    .names
                    .targets
                    .entry(squash(name))
                    .or_insert_with(|| name.into());
            }
        }
    }

    /// Where an affiliated keyword's value is written.
    fn affiliated_at(&self, a: &AffiliatedKeywords<'s>, key: &str) -> Option<Range<usize>> {
        let value = a.keywords.get(key)?;
        let first = match value {
            AffiliatedKeywordValue::SingleString(s) => s,
            AffiliatedKeywordValue::ListOfStrings(l) => l.first()?,
            AffiliatedKeywordValue::OptionalPair { val, .. } => val,
            AffiliatedKeywordValue::ObjectTree(t) => {
                let (_, objects) = t.first()?;
                objects.first()?.get_source()
            }
        };
        Some(self.at(first))
    }

    /// `#+ATTR_HTML: :name value …` as attributes, read as
    /// `org-export-read-attribute` reads them: a value runs to the next
    /// `:name`, and `nil` or nothing leaves the attribute out.
    fn attr_html(&mut self, a: &AffiliatedKeywords<'s>) -> Vec<(String, String)> {
        let Some(AffiliatedKeywordValue::ListOfStrings(lines)) = a.keywords.get("attr_html") else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = Vec::new();
        for line in lines {
            let start = offset(self.text, line);
            let mut name: Option<String> = None;
            let mut value: Vec<&str> = Vec::new();
            let mut flush = |name: &mut Option<String>, value: &mut Vec<&str>| {
                if let Some(n) = name.take() {
                    let v = value.join(" ");
                    if !v.is_empty() && v != "nil" {
                        out.retain(|(o, _)| *o != n);
                        out.push((n, v));
                    }
                }
                value.clear();
            };
            for (i, word) in line.split_whitespace().enumerate() {
                let is_name = word.len() > 1
                    && word.starts_with(':')
                    && word[1..]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                if is_name {
                    flush(&mut name, &mut value);
                    name = Some(word[1..].to_string());
                } else if name.is_none() || word.starts_with(':') {
                    let at = offset(line, word) + start;
                    let d = Diagnostic::error(
                        &codes::ORG_INVALID_ATTRIBUTE,
                        format!("`{word}` isn't an attribute name"),
                        self.span(at..at + word.len()),
                    )
                    .help("write each attribute as `:name value`, such as `#+ATTR_HTML: :class wide :width 300`");
                    self.diagnostics.push(d);
                    if i == 0 {
                        break;
                    }
                } else {
                    value.push(word);
                }
            }
            flush(&mut name, &mut value);
        }
        out
    }

    /// The caption of an element: its `#+CAPTION` lines joined with a
    /// space, as Org joins them. A short caption, `#+CAPTION[short]:`, is
    /// left out.
    fn caption(&mut self, a: &AffiliatedKeywords<'s>, parent: NodeId) -> bool {
        let Some(AffiliatedKeywordValue::ObjectTree(lines)) = a.keywords.get("caption") else {
            return false;
        };
        let at = self.affiliated_at(a, "caption").unwrap_or(0..0);
        let id = self.doc.push(parent, Kind::Caption, at);
        for (i, (_, objects)) in lines.iter().enumerate() {
            if i > 0 {
                self.text(" ", id);
            }
            for o in objects {
                self.object(o, id);
            }
        }
        self.trim(id);
        true
    }

    fn switches(&mut self, switches: Option<&'s str>) {
        let Some(switches) = switches else { return };
        if switches
            .split_whitespace()
            .any(|s| ["-n", "+n", "-r", "-l"].contains(&s))
        {
            let at = self.at(switches);
            self.no_effect(
                format!("`{}` has no effect in yuanme", switches.trim()),
                at,
                "a code block's lines aren't numbered and its labels aren't rewritten",
            );
        }
    }

    fn src(&mut self, s: &SrcBlock<'s>, parent: NodeId) {
        self.switches(s.switches);
        let at = self.at(s.source);
        let text = line(&dedent(&s.get_value(), s.preserve_indent.is_some()));
        let kind = Kind::CodeBlock {
            lang: s.language.map(str::to_string),
            text,
        };
        let a = &s.affiliated_keywords;
        if a.keywords.contains_key("caption") {
            let figure = self.doc.push(parent, Kind::Figure, at.clone());
            self.name(a, figure);
            let attrs = self.attr_html(a);
            self.doc.attrs_mut(figure).extend(attrs);
            self.caption(a, figure);
            self.doc.push(figure, kind, at);
        } else {
            let id = self.doc.push(parent, kind, at);
            self.name(a, id);
            if let Some(at) = self.affiliated_at(a, "attr_html") {
                self.no_effect(
                    "`#+ATTR_HTML` has no effect on a source block without a caption".into(),
                    at,
                    "its attributes go on the `<figure>` a `#+CAPTION` makes",
                );
            }
        }
    }

    fn paragraph(&mut self, p: &Paragraph<'s>, parent: NodeId) {
        let at = self.at(p.source);
        let a = &p.affiliated_keywords;
        let image = standalone_image(&p.children);
        if a.keywords.contains_key("caption")
            && let Some(image) = image
        {
            let figure = self.doc.push(parent, Kind::Figure, at);
            self.name(a, figure);
            self.object(image, figure);
            self.trim(figure);
            self.caption(a, figure);
            self.links_attrs(a, figure);
            return;
        }
        let id = self.doc.push(parent, Kind::Paragraph, at);
        for o in &p.children {
            self.object(o, id);
        }
        self.trim(id);
        self.name(a, id);
        self.links_attrs(a, id);
        if let Some(at) = self.affiliated_at(a, "caption") {
            self.no_effect(
                "`#+CAPTION` has no effect on this paragraph".into(),
                at,
                "only an image alone in its paragraph shows a caption",
            );
        }
    }

    /// A paragraph's `#+ATTR_HTML` goes on its first link or image, as
    /// ox-html puts it.
    fn links_attrs(&mut self, a: &AffiliatedKeywords<'s>, id: NodeId) {
        let attrs = self.attr_html(a);
        if attrs.is_empty() {
            return;
        }
        let first = self.doc.descendants(id).into_iter().find(|&n| {
            matches!(
                self.doc.node(n).kind,
                Kind::Link { .. } | Kind::Image { .. }
            )
        });
        match first {
            Some(link) => {
                for (name, value) in attrs {
                    match (&self.doc.node(link).kind, name.as_str()) {
                        // The alt text is the image's children.
                        (Kind::Image { .. }, "alt") => {
                            self.doc.children_mut(link).clear();
                            let span = self.doc.node(link).span.clone();
                            self.doc.push(link, Kind::Text(value), span);
                        }
                        _ => self.doc.attrs_mut(link).push((name, value)),
                    }
                }
            }
            None => {
                let at = self.affiliated_at(a, "attr_html").unwrap_or(0..0);
                self.no_effect(
                    "`#+ATTR_HTML` has no effect on a paragraph without a link or image".into(),
                    at,
                    "it puts attributes on the paragraph's first link or image",
                );
            }
        }
    }

    fn table(&mut self, t: &Table<'s>, parent: NodeId) {
        let at = self.at(t.source);
        // A row of alignment cookies such as `<r>` sets the columns'
        // alignment and isn't shown, as `:with-special-rows` is nil.
        let cookie = |cell: &str| {
            let c = cell.trim();
            c.len() >= 2
                && c.starts_with('<')
                && c.ends_with('>')
                && c[1..c.len() - 1]
                    .trim_start_matches(['l', 'r', 'c'])
                    .chars()
                    .all(|d| d.is_ascii_digit())
        };
        let special = |row: &yuanme_org::types::TableRow<'_>| {
            !row.children.is_empty()
                && row.children.iter().any(|c| cookie(c.contents))
                && row
                    .children
                    .iter()
                    .all(|c| c.contents.trim().is_empty() || cookie(c.contents))
        };
        let columns = t
            .children
            .iter()
            .map(|r| r.children.len())
            .max()
            .unwrap_or(0);
        let mut align = vec![Align::None; columns];
        for row in t.children.iter().filter(|r| special(r)) {
            for (i, c) in row.children.iter().enumerate() {
                let c = c.contents.trim();
                if cookie(c) {
                    align[i] = match c.as_bytes()[1] {
                        b'l' => Align::Left,
                        b'r' => Align::Right,
                        b'c' => Align::Center,
                        _ => align[i],
                    };
                }
            }
        }
        let rows: Vec<_> = t.children.iter().filter(|r| !special(r)).collect();
        // Rows before the first rule are the head, if any row follows it.
        let rule = rows.iter().position(|r| r.children.is_empty());
        let heads = match rule {
            Some(r) if rows[r..].iter().any(|r| !r.children.is_empty()) => r,
            _ => 0,
        };
        let id = self.doc.push(parent, Kind::Table { align }, at);
        let a = &t.affiliated_keywords;
        self.name(a, id);
        let attrs = self.attr_html(a);
        self.doc.attrs_mut(id).extend(attrs);
        self.caption(a, id);
        for (n, row) in rows.iter().enumerate() {
            if row.children.is_empty() {
                continue;
            }
            let at = self.at(row.source);
            let r = self.doc.push(id, Kind::TableRow { head: n < heads }, at);
            for cell in &row.children {
                let at = self.at(cell.contents);
                let c = self.doc.push(r, Kind::TableCell, at);
                for o in &cell.children {
                    self.object(o, c);
                }
                self.trim(c);
            }
        }
    }

    fn list(&mut self, l: &PlainList<'s>, parent: NodeId) {
        let at = self.at(l.source);
        // ox-html drops a paragraph's `<p>` when it is first in its item
        // and alone or followed by a list; the Document's lists are tight
        // or loose as a whole, so a list is tight when every item is.
        let tight = l.children.iter().all(|item| {
            item.children.iter().enumerate().all(|(i, c)| {
                !matches!(c, Element::Paragraph(_))
                    || (i == 0
                        && (item.children.len() == 1
                            || (item.children.len() == 2
                                && matches!(item.children[1], Element::PlainList(_)))))
            })
        });
        let checkbox = |c: &Option<(CheckboxType, &str)>| {
            c.as_ref().map(|(c, _)| match c {
                CheckboxType::On => Checkbox::Checked,
                CheckboxType::Off => Checkbox::Unchecked,
                CheckboxType::Trans => Checkbox::Partial,
            })
        };
        let id = match l.list_type {
            PlainListType::Descriptive => {
                let id = self.doc.push(parent, Kind::DescriptionList { tight }, at);
                for item in &l.children {
                    let at = self.at(item.source);
                    let term = self.doc.push(id, Kind::DescriptionTerm, at.clone());
                    for o in &item.tag {
                        self.object(o, term);
                    }
                    self.trim(term);
                    let details = self.doc.push(id, Kind::DescriptionDetails, at);
                    self.elements(&item.children, details);
                }
                id
            }
            list_type => {
                // `[@n]` on the first item starts the list there, as a
                // Markdown list starting at n does.
                let start = l.children.first().and_then(|i| i.counter).unwrap_or(1);
                let kind = Kind::List {
                    ordered: matches!(list_type, PlainListType::Ordered),
                    start,
                    tight,
                };
                let id = self.doc.push(parent, kind, at);
                for (n, item) in l.children.iter().enumerate() {
                    let at = self.at(item.source);
                    let kind = Kind::Item {
                        checked: checkbox(&item.checkbox),
                        value: item.counter.filter(|_| n > 0),
                    };
                    let i = self.doc.push(id, kind, at);
                    self.elements(&item.children, i);
                }
                id
            }
        };
        self.decorate(&l.affiliated_keywords, id, false);
    }

    /// Takes off the whitespace a block's text starts and ends with, and
    /// the indentation of its lines, which Markdown's text has none of.
    fn trim(&mut self, id: NodeId) {
        for n in self.doc.descendants(id) {
            if self.verbatim.contains(&n) {
                continue;
            }
            if let Kind::Text(s) = self.doc.kind_mut(n)
                && s.contains('\n')
            {
                let lines: Vec<&str> = s.split('\n').collect();
                let last = lines.len() - 1;
                let joined: Vec<&str> = lines
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        let l = if i > 0 {
                            l.trim_start_matches([' ', '\t'])
                        } else {
                            l
                        };
                        if i < last {
                            l.trim_end_matches([' ', '\t'])
                        } else {
                            l
                        }
                    })
                    .collect();
                *s = joined.join("\n");
            }
        }
        let children = self.doc.node(id).children.clone();
        let space = |c: char| c.is_ascii_whitespace();
        if let Some(&first) = children.first()
            && !self.verbatim.contains(&first)
            && let Kind::Text(s) = self.doc.kind_mut(first)
        {
            *s = s.trim_start_matches(space).to_string();
        }
        if let Some(&last) = children.last()
            && !self.verbatim.contains(&last)
            && let Kind::Text(s) = self.doc.kind_mut(last)
        {
            *s = s.trim_end_matches(space).to_string();
        }
        let doc = &self.doc;
        let kept: Vec<NodeId> = children
            .into_iter()
            .filter(|&c| !matches!(&doc.node(c).kind, Kind::Text(s) if s.is_empty()))
            .collect();
        *self.doc.children_mut(id) = kept;
    }

    /// Text joins the text before it, as one mdast text node would. `\-`
    /// is a soft hyphen, as ox-html's special strings make it; smartypants
    /// does `--`, `---` and `...`.
    fn text(&mut self, s: &str, parent: NodeId) {
        if s.is_empty() {
            return;
        }
        if let Some(start) = self.verse {
            return self.verse_text(s, parent, start);
        }
        let s = s.replace("\\-", "\u{AD}");
        if let Some(&last) = self.doc.node(parent).children.last()
            && !self.verbatim.contains(&last)
            && let Kind::Text(before) = self.doc.kind_mut(last)
        {
            before.push_str(&s);
            return;
        }
        let at = self.doc.node(parent).span.clone();
        self.doc.push(parent, Kind::Text(s), at);
    }

    /// ox-html's verse: each line ends in `<br>`, and its leading spaces
    /// are kept as non-breaking ones.
    fn verse_text(&mut self, s: &str, parent: NodeId, mut start: bool) {
        self.verse = None;
        for (i, piece) in s.split('\n').enumerate() {
            if i > 0 {
                let at = self.doc.node(parent).span.clone();
                self.doc.push(parent, Kind::LineBreak, at);
                start = true;
            }
            let piece = if start {
                let rest = piece.trim_start_matches(' ');
                let nbsp = "\u{A0}".repeat(piece.len() - rest.len());
                if !piece.is_empty() {
                    start = false;
                }
                format!("{nbsp}{rest}")
            } else {
                piece.to_string()
            };
            self.text(&piece, parent);
        }
        self.verse = Some(start);
    }

    fn object(&mut self, o: &Object<'s>, parent: NodeId) {
        let at = self.at(o.get_source());
        match o {
            Object::PlainText(t) => self.text(t.source, parent),
            Object::Bold(b) => self.markup(Kind::Strong, &b.children, parent, at),
            Object::Italic(b) => self.markup(Kind::Emphasis, &b.children, parent, at),
            Object::Underline(b) => self.markup(Kind::Underline, &b.children, parent, at),
            Object::StrikeThrough(b) => self.markup(Kind::Strikethrough, &b.children, parent, at),
            Object::Code(c) => {
                self.doc.push(parent, Kind::Code(c.contents.into()), at);
            }
            Object::Verbatim(c) => {
                self.doc.push(parent, Kind::Code(c.contents.into()), at);
            }
            Object::RegularLink(l) => {
                let search = l.get_search_option();
                self.link(
                    &l.link_type,
                    &l.get_path(),
                    &l.get_raw_link(),
                    search.as_deref(),
                    &l.children,
                    l.source,
                    parent,
                );
            }
            Object::PlainLink(l) => self.link(
                &l.link_type,
                l.path,
                l.raw_link,
                l.search_option,
                &[],
                l.source,
                parent,
            ),
            Object::AngleLink(l) => self.link(
                &l.link_type,
                &l.get_path(),
                l.raw_link,
                l.search_option,
                &[],
                l.source,
                parent,
            ),
            Object::RadioLink(_) | Object::RadioTarget(_) => self.unsupported(
                "radio targets and links",
                o.get_source(),
                "use a `<<target>>` and link to it with `[[target][text]]`",
            ),
            Object::OrgMacro(_) => self.unsupported(
                "macros",
                o.get_source(),
                "write the text the macro would expand to",
            ),
            Object::Citation(_) | Object::CitationReference(_) => self.unsupported(
                "citations",
                o.get_source(),
                "cite in the text, or with a footnote, until citations arrive",
            ),
            Object::InlineBabelCall(_) | Object::InlineSourceBlock(_) => self.unsupported(
                "inline calls and source blocks",
                o.get_source(),
                "yuanme never runs code; use `~code~` to show code, or paste in the result",
            ),
            Object::Entity(e) => {
                let id = self.doc.push(parent, Kind::Text(e.utf8.into()), at);
                self.verbatim.insert(id);
            }
            Object::LatexFragment(f) => {
                let v = f.value;
                // `\ref{label}` and `\eqref{label}` in prose are the Engine's
                // to number, as they name equations across the Entry.
                for (command, parens) in [("\\ref{", false), ("\\eqref{", true)] {
                    if let Some(label) = v.strip_prefix(command).and_then(|l| l.strip_suffix('}'))
                        && !label.contains(['{', '}'])
                    {
                        let kind = Kind::Ref {
                            label: label.trim().into(),
                            parens,
                            number: String::new(),
                        };
                        self.doc.push(parent, kind, at);
                        return;
                    }
                }
                let (display, tex) = if let Some(t) = strip(v, "\\(", "\\)") {
                    (false, t)
                } else if let Some(t) = strip(v, "\\[", "\\]") {
                    (true, t)
                } else if let Some(t) = strip(v, "$$", "$$") {
                    (true, t)
                } else if let Some(t) = strip(v, "$", "$") {
                    (false, t)
                } else {
                    (false, v)
                };
                let kind = Kind::Math {
                    display,
                    tex: tex.into(),
                    mathml: String::new(),
                };
                self.doc.push(parent, kind, at);
            }
            Object::ExportSnippet(s) => {
                if s.backend.eq_ignore_ascii_case("html") {
                    let raw = s.contents.unwrap_or("").to_string();
                    self.doc.push(parent, Kind::HtmlInline(raw), at);
                }
            }
            Object::FootnoteReference(f) => self.reference(f, parent, at),
            Object::LineBreak(_) => {
                self.doc.push(parent, Kind::LineBreak, at);
            }
            Object::Target(t) => {
                let id = t.value.split_whitespace().collect::<Vec<_>>().join("-");
                self.doc
                    .names
                    .targets
                    .entry(squash(t.value))
                    .or_insert_with(|| id.clone());
                self.doc.push(parent, Kind::Target { id }, at);
            }
            Object::StatisticsCookie(c) => self.text(c.value, parent),
            Object::Subscript(s) => {
                self.script(Kind::Subscript, s.use_brackets, &s.children, o, parent, at)
            }
            Object::Superscript(s) => self.script(
                Kind::Superscript,
                s.use_brackets,
                &s.children,
                o,
                parent,
                at,
            ),
            Object::Timestamp(t) => self.timestamp(t, parent, at),
        }
        self.text(post_blank(o), parent);
    }

    fn markup(&mut self, kind: Kind, children: &[Object<'s>], parent: NodeId, at: Range<usize>) {
        let id = self.doc.push(parent, kind, at);
        for o in children {
            self.object(o, id);
        }
    }

    fn script(
        &mut self,
        kind: Kind,
        braces: bool,
        children: &[Object<'s>],
        o: &Object<'s>,
        parent: NodeId,
        at: Range<usize>,
    ) {
        let shown = match self.scripts {
            Scripts::All => true,
            Scripts::Braces => braces,
            Scripts::None => false,
        };
        if shown {
            self.markup(kind, children, parent, at);
        } else {
            let source = o.get_source();
            self.text(&source[..source.len() - post_blank(o).len()], parent);
        }
    }

    fn timestamp(&mut self, t: &Timestamp<'s>, parent: NodeId, at: Range<usize>) {
        let raw = t.get_raw_value();
        let datetime = match t.timestamp_type {
            TimestampType::Diary => None,
            _ => t.start.as_ref().map(|d| {
                let date = format!(
                    "{:04}-{:02}-{:02}",
                    d.get_year().get_value(),
                    d.get_month().get_value(),
                    d.get_day_of_month().get_value()
                );
                match &t.start_time {
                    Some(time) => format!(
                        "{date}T{:02}:{:02}:00",
                        time.get_hour().get_value(),
                        time.get_minute().get_value()
                    ),
                    None => date,
                }
            }),
        };
        // ox-html's special strings make a range's `--` an en dash.
        let text = match t.range_type {
            TimestampRangeType::DateRange => raw.replacen("--", "\u{2013}", 1),
            _ => raw.to_string(),
        };
        self.doc
            .push(parent, Kind::Timestamp { datetime, text }, at);
    }

    /// A footnote reference. An inline or anonymous one also defines its
    /// footnote, which joins the others at the end.
    fn reference(&mut self, f: &FootnoteReference<'s>, parent: NodeId, at: Range<usize>) {
        let label = match f.label {
            Some(label) => label.to_string(),
            None => {
                // A `.` is in no label Org reads, so this one is unique.
                self.anonymous += 1;
                format!("anonymous.{}", self.anonymous)
            }
        };
        let key = footnote_identifier(&label);
        if !f.definition.is_empty() && self.definitions.insert(key.clone()) {
            let kind = Kind::FootnoteDefinition {
                label: label.clone(),
            };
            let def = self.doc.push(Document::ROOT, kind, at.clone());
            let p = self.doc.push(def, Kind::Paragraph, at.clone());
            for o in &f.definition {
                self.object(o, p);
            }
            self.trim(p);
        }
        self.references.push((key, at.clone()));
        let kind = Kind::FootnoteReference { label, number: 0 };
        self.doc.push(parent, kind, at);
    }

    #[allow(clippy::too_many_arguments)] // the fields every kind of Org link has
    fn link(
        &mut self,
        link_type: &LinkType<'_>,
        path: &str,
        raw: &str,
        search: Option<&str>,
        description: &[Object<'s>],
        source: &'s str,
        parent: NodeId,
    ) {
        let at = self.at(source);
        let url = match link_type {
            LinkType::Protocol(p) => match p.as_ref() {
                "http" | "https" | "mailto" | "ftp" => raw.to_string(),
                "doi" => format!("https://doi.org/{path}"),
                "file+sys" | "file+emacs" => path.to_string(),
                other => {
                    return self.unsupported(
                        &format!("`{other}:` links"),
                        source,
                        "link to a URL, another Entry's file, or a heading in this one",
                    );
                }
            },
            LinkType::File => match search {
                Some(s) if s.starts_with('#') => format!("{path}{s}"),
                Some(s) => format!("{path}::{s}"),
                None => path.to_string(),
            },
            LinkType::Id => format!("id:{path}"),
            LinkType::CustomId | LinkType::Fuzzy => String::new(),
            LinkType::CodeRef => {
                return self.unsupported(
                    "links to code references",
                    source,
                    "link to the source block's `#+NAME` instead",
                );
            }
        };
        let remote = matches!(link_type, LinkType::Protocol(p) if p == "http" || p == "https");
        if description.is_empty()
            && (matches!(link_type, LinkType::File) || remote)
            && search.is_none()
            && is_image(path)
        {
            let src = if remote { raw } else { path };
            let image = Kind::Image {
                url: src.to_string(),
                title: None,
            };
            let id = self.doc.push(parent, image, at.clone());
            // ox-html's alt text is the file's name.
            let name = src.rsplit('/').next().unwrap_or(src).to_string();
            self.doc.push(id, Kind::Text(name), at);
            return;
        }
        let kind = Kind::Link { url, title: None };
        let id = self.doc.push(parent, kind, at.clone());
        match link_type {
            LinkType::CustomId => {
                let search = Search::CustomId(path.to_string());
                self.searches.push((id, search, at.clone()));
            }
            LinkType::Fuzzy => {
                let search = match path.strip_prefix('*') {
                    Some(title) => Search::Title(squash(title)),
                    None => Search::Fuzzy(squash(path)),
                };
                self.searches.push((id, search, at.clone()));
            }
            _ => {}
        }
        if description.is_empty() {
            // A link to a heading in this file shows its title, filled in
            // once it is known.
            if !matches!(link_type, LinkType::CustomId | LinkType::Fuzzy) {
                let shown = raw.strip_prefix("file:").unwrap_or(raw);
                self.doc.push(id, Kind::Text(shown.to_string()), at);
            }
        } else {
            for o in description {
                self.object(o, id);
            }
        }
    }

    /// What only the whole file says: footnote numbers, smart punctuation,
    /// heading ids, and where in-page links go.
    fn finish(&mut self, macros: &[(String, String)]) {
        for (key, at) in std::mem::take(&mut self.references) {
            if !self.definitions.contains(&key) {
                let message = format!("the footnote `{}` is never defined", key.to_lowercase());
                let d = Diagnostic::error(&codes::ORG_UNDEFINED_FOOTNOTE, message, self.span(at))
                    .help("define it on a line of its own, such as `[fn:1] The note.`, or inline, as `[fn:1:The note.]`");
                self.diagnostics.push(d);
            }
        }
        markdown::number_footnotes(&mut self.doc);
        markdown::smarten(&mut self.doc, Dashes::Oldschool, &self.verbatim);
        let labels = math::render(
            &mut self.doc,
            self.text,
            self.file,
            macros,
            &self.headers,
            &mut self.diagnostics,
        );
        markdown::heading_ids(&mut self.doc, &labels);
        let anchor = |doc: &Document, id: NodeId| match &doc.node(id).kind {
            Kind::Heading { id, .. } => id.clone(),
            _ => unreachable!("a heading"),
        };
        let mut names = std::mem::take(&mut self.doc.names);
        for (title, id) in &self.titles {
            names
                .titles
                .entry(title.clone())
                .or_insert_with(|| anchor(&self.doc, *id));
        }
        for (value, heading, at) in std::mem::take(&mut self.ids) {
            let id = heading.map_or_else(String::new, |h| anchor(&self.doc, h));
            names.ids.push((value, id, at));
        }
        let headings: BTreeMap<String, NodeId> = self
            .doc
            .descendants(Document::ROOT)
            .into_iter()
            .filter_map(|n| match &self.doc.node(n).kind {
                Kind::Heading { id, .. } => Some((id.clone(), n)),
                _ => None,
            })
            .collect();
        for (link, search, at) in std::mem::take(&mut self.searches) {
            let (query, found) = match &search {
                Search::CustomId(id) => (
                    format!("#{id}"),
                    headings.contains_key(id).then(|| id.clone()),
                ),
                Search::Title(t) => (format!("*{t}"), names.titles.get(t).cloned()),
                Search::Fuzzy(t) => (
                    t.clone(),
                    names
                        .targets
                        .get(t)
                        .or_else(|| names.titles.get(t))
                        .cloned(),
                ),
            };
            let Some(fragment) = found else {
                let message = match &search {
                    Search::CustomId(id) => format!("no heading here has the `CUSTOM_ID` `{id}`"),
                    Search::Title(t) => format!("no headline here is titled `{t}`"),
                    Search::Fuzzy(t) => format!(
                        "nothing here is called `{t}`: no `<<target>>`, `#+NAME` or headline title"
                    ),
                };
                let mut d = Diagnostic::error(&codes::LINK_UNRESOLVED, message, self.span(at));
                if let Search::Fuzzy(t) = &search
                    && (t.ends_with(".org") || t.ends_with(".md"))
                {
                    d = d.help(format!("a link to another file is written `[[file:{t}]]`"));
                }
                self.diagnostics.push(d);
                continue;
            };
            if self.doc.node(link).children.is_empty() {
                let shown = match headings.get(&fragment) {
                    Some(&h) => self.doc.text(h),
                    None => query.trim_start_matches(['*', '#']).to_string(),
                };
                let span = self.doc.node(link).span.clone();
                self.doc.push(link, Kind::Text(shown), span);
            }
            if let Kind::Link { url, .. } = self.doc.kind_mut(link) {
                *url = format!("#{fragment}");
            }
        }
        self.doc.names = names;
    }
}

/// `:exports` in a source block's header: whether its code and its results
/// are shown. Org's default is `code`.
fn exports(s: &SrcBlock<'_>) -> (bool, bool) {
    let headers = s.affiliated_keywords.keywords.get("header");
    let header_lines: Vec<&str> = match headers {
        Some(AffiliatedKeywordValue::ListOfStrings(l)) => l.clone(),
        _ => Vec::new(),
    };
    let mut value = "code";
    for args in header_lines.into_iter().chain(s.parameters) {
        let mut words = args.split_whitespace();
        while let Some(w) = words.next() {
            if w == ":exports"
                && let Some(v) = words.next()
            {
                value = v;
            }
        }
    }
    match value {
        "results" => (false, true),
        "both" => (true, true),
        "none" => (false, false),
        _ => (true, false),
    }
}

fn affiliated<'a, 's>(e: &'a Element<'s>) -> Option<&'a AffiliatedKeywords<'s>> {
    Some(match e {
        Element::Paragraph(x) => &x.affiliated_keywords,
        Element::PlainList(x) => &x.affiliated_keywords,
        Element::CenterBlock(x) => &x.affiliated_keywords,
        Element::QuoteBlock(x) => &x.affiliated_keywords,
        Element::SpecialBlock(x) => &x.affiliated_keywords,
        Element::DynamicBlock(x) => &x.affiliated_keywords,
        Element::FootnoteDefinition(x) => &x.affiliated_keywords,
        Element::Drawer(x) => &x.affiliated_keywords,
        Element::Table(x) => &x.affiliated_keywords,
        Element::VerseBlock(x) => &x.affiliated_keywords,
        Element::CommentBlock(x) => &x.affiliated_keywords,
        Element::ExampleBlock(x) => &x.affiliated_keywords,
        Element::ExportBlock(x) => &x.affiliated_keywords,
        Element::SrcBlock(x) => &x.affiliated_keywords,
        Element::DiarySexp(x) => &x.affiliated_keywords,
        Element::FixedWidthArea(x) => &x.affiliated_keywords,
        Element::HorizontalRule(x) => &x.affiliated_keywords,
        Element::Keyword(x) => &x.affiliated_keywords,
        Element::BabelCall(x) => &x.affiliated_keywords,
        Element::LatexEnvironment(x) => &x.affiliated_keywords,
        Element::Comment(_)
        | Element::PropertyDrawer(_)
        | Element::Clock(_)
        | Element::Planning(_) => return None,
    })
}

/// The one image link in a paragraph of nothing else, which a caption
/// makes a figure.
fn standalone_image<'a, 's>(objects: &'a [Object<'s>]) -> Option<&'a Object<'s>> {
    let mut image = None;
    for o in objects {
        match o {
            Object::PlainText(t) if t.source.trim().is_empty() => {}
            Object::RegularLink(l)
                if image.is_none()
                    && l.children.is_empty()
                    && l.search_option.is_none()
                    && matches!(&l.link_type, LinkType::File | LinkType::Protocol(_))
                    && is_image(&l.path) =>
            {
                image = Some(o);
            }
            _ => return None,
        }
    }
    image
}

/// The environments math-core has that go inside math rather than stand
/// alone as an equation, as `equation`, `align`, `gather` and `multline`
/// do (#17).
const INNER: &[&str] = &[
    "aligned", "gathered", "cases", "dcases", "rcases", "drcases", "matrix", "pmatrix", "bmatrix",
    "Bmatrix", "vmatrix", "Vmatrix", "array", "darray", "subarray",
];

/// Packages whose `\usepackage` a `#+LATEX_HEADER` may have: yuanme's math
/// has what they give, or they give nothing math needs (#17).
const PACKAGES: &[&str] = &[
    "amsmath",
    "amssymb",
    "amsfonts",
    "amsthm",
    "mathtools",
    "bm",
];

/// `\DeclareMathOperator{\X}{Y}` as the macro it defines,
/// `\newcommand{\X}{\operatorname{Y}}`, or `\operatorname*` for the
/// starred form.
fn declare_math_operator(tex: &str) -> Option<String> {
    let rest = tex.strip_prefix("\\DeclareMathOperator")?;
    let (star, rest) = match rest.strip_prefix('*') {
        Some(rest) => ("*", rest),
        None => ("", rest),
    };
    let rest = rest.trim_start();
    let (name, rest) = match rest.strip_prefix('{') {
        Some(r) => r.split_once('}')?,
        None => {
            let end = 1 + rest
                .get(1..)?
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(rest.len() - 1);
            rest.split_at(end)
        }
    };
    let name = name.trim();
    if !name.starts_with('\\') {
        return None;
    }
    let body = rest.trim().strip_prefix('{')?.strip_suffix('}')?;
    Some(format!(
        "\\newcommand{{{name}}}{{\\operatorname{star}{{{body}}}}}"
    ))
}

/// `inner` between `open` and `close`.
fn strip<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    s.strip_prefix(open)?.strip_suffix(close)
}

/// A block's lines without the indentation they all share, as
/// org-element reads a block unless its `-i` keeps it.
fn dedent(text: &str, keep: bool) -> String {
    let indent = |l: &str| l.len() - l.trim_start_matches([' ', '\t']).len();
    let common = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(indent)
        .min()
        .unwrap_or(0);
    if keep || common == 0 {
        return text.to_string();
    }
    text.split_inclusive('\n')
        .map(|l| &l[indent(l).min(common)..])
        .collect()
}

/// A block's text as mdast's: without its final line ending.
fn line(text: &str) -> String {
    let text = text.strip_suffix('\n').unwrap_or(text);
    text.strip_suffix('\r').unwrap_or(text).to_string()
}
