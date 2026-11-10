//! Affiliated keywords: `#+NAME`, `#+ATTR_HTML` and `#+CAPTION`.

use std::ops::Range;

use yuanme_diag::{Diagnostic, codes};
use yuanme_org::types::{AffiliatedKeywordValue, AffiliatedKeywords, Element, StandardProperties};

use super::{Lower, offset, squash};
use crate::document::{Kind, NodeId};

impl<'s> Lower<'s> {
    /// `#+NAME` as the element's id, `#+ATTR_HTML` as its attributes where
    /// `attributes` says it takes them, and a warning for a caption it
    /// can't show.
    pub(super) fn decorate(&mut self, a: &AffiliatedKeywords<'s>, id: NodeId, attributes: bool) {
        self.name(a, id);
        self.affiliated(a, id, attributes);
    }

    /// `decorate` without the `#+NAME`.
    pub(super) fn affiliated(&mut self, a: &AffiliatedKeywords<'s>, id: NodeId, attributes: bool) {
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

    pub(super) fn name(&mut self, a: &AffiliatedKeywords<'s>, id: NodeId) {
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
    pub(super) fn affiliated_at(
        &self,
        a: &AffiliatedKeywords<'s>,
        key: &str,
    ) -> Option<Range<usize>> {
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
    pub(super) fn attr_html(&mut self, a: &AffiliatedKeywords<'s>) -> Vec<(String, String)> {
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
    pub(super) fn caption(&mut self, a: &AffiliatedKeywords<'s>, parent: NodeId) -> bool {
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
}

pub(super) fn affiliated<'a, 's>(e: &'a Element<'s>) -> Option<&'a AffiliatedKeywords<'s>> {
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
