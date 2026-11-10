//! Objects: text, markup, math, footnote references and the rest of
//! what a paragraph holds.

use std::ops::Range;

use yuanme_org::types::{
    FootnoteReference, Object, StandardProperties, Timestamp, TimestampRangeType, TimestampType,
};

use super::{Lower, Scripts, squash};
use crate::document::{Document, Kind, NodeId, footnote_identifier};

impl<'s> Lower<'s> {
    /// Takes off the whitespace a block's text starts and ends with, and
    /// the indentation of its lines, which Markdown's text has none of.
    pub(super) fn trim(&mut self, id: NodeId) {
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
    pub(super) fn text(&mut self, s: &str, parent: NodeId) {
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

    pub(super) fn object(&mut self, o: &Object<'s>, parent: NodeId) {
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
                        self.text(post_blank(o), parent);
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

/// `inner` between `open` and `close`.
fn strip<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    s.strip_prefix(open)?.strip_suffix(close)
}
