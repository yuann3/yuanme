//! Elements: blocks, drawers, source and example blocks, and LaTeX
//! environments.

use std::ops::Range;

use yuanme_diag::{Diagnostic, codes};
use yuanme_org::parser::{ParseError, parse_with_settings};
use yuanme_org::settings::GlobalSettings;
use yuanme_org::types::{
    AffiliatedKeywordValue, AffiliatedKeywords, Element, SrcBlock, StandardProperties,
};

use super::affiliated::affiliated;
use super::{Lower, offset, squash};
use crate::document::{Document, Kind, NodeId, footnote_identifier};
use crate::org::NoFiles;

impl<'s> Lower<'s> {
    /// Elements in order. A source block decides whether the `#+RESULTS:`
    /// after it are shown.
    pub(super) fn elements(&mut self, elements: &[Element<'s>], parent: NodeId) {
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
            // Org reads a block's type in any case, and ox-latex the text
            // after it as the title (#17).
            Element::SpecialBlock(b) => {
                let name = b.block_type.to_lowercase();
                let title = b.parameters.map(str::trim).filter(|t| !t.is_empty());
                let theorem = self.theorems.contains_key(&name);
                let kind = if theorem {
                    Kind::Theorem {
                        kind: name,
                        name: String::new(),
                    }
                } else {
                    if let Some(title) = title {
                        self.no_effect(
                            format!("a title has no effect on `#+begin_{}`", b.block_type),
                            self.at(title),
                            "only Theorem blocks show a title",
                        );
                    }
                    Kind::Directive {
                        name: b.block_type.to_string(),
                    }
                };
                let id = self.doc.push(parent, kind, at);
                self.decorate(&b.affiliated_keywords, id, true);
                if let Some(title) = title.filter(|_| theorem) {
                    self.title(title, id);
                }
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

    /// A Theorem block's title, read as the objects a paragraph holds, into
    /// a Caption. org-element leaves it as text, but the title is shown,
    /// and math in it is math (#17).
    fn title(&mut self, title: &'s str, parent: NodeId) {
        let caption = self.doc.push(parent, Kind::Caption, self.at(title));
        let settings = GlobalSettings {
            file_access: &NoFiles,
            ..GlobalSettings::default()
        };
        let parsed = match parse_with_settings(title, &settings) {
            Ok(parsed) => parsed,
            Err(e) => {
                let code = match e {
                    ParseError::OutOfFuel { .. } => &codes::ORG_OUT_OF_FUEL,
                    ParseError::TooDeep { .. } => &codes::ORG_TOO_DEEP,
                    ParseError::Failed(message) => panic!("organic failed to parse: {message}"),
                };
                let d = Diagnostic::error(
                    code,
                    "this title takes too long to parse",
                    self.span(self.at(title)),
                )
                .help("markup opened many times and never closed is almost always a mistake");
                self.diagnostics.push(d);
                return;
            }
        };
        // A title that reads as something other than a paragraph, such as
        // `- a`, is shown as it is written.
        let paragraph = match (&parsed.zeroth_section, parsed.children.is_empty()) {
            (Some(z), true) => match z.children.as_slice() {
                [Element::Paragraph(p)] => Some(p),
                _ => None,
            },
            _ => None,
        };
        match paragraph {
            Some(p) => {
                for o in &p.children {
                    self.object(o, caption);
                }
            }
            None => self.text(title, caption),
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

/// The environments math-core has that go inside math rather than stand
/// alone as an equation, as `equation`, `align`, `gather` and `multline`
/// do (#17).
const INNER: &[&str] = &[
    "aligned", "gathered", "cases", "dcases", "rcases", "drcases", "matrix", "pmatrix", "bmatrix",
    "Bmatrix", "vmatrix", "Vmatrix", "array", "darray", "subarray",
];

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
