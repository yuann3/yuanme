//! Headlines, their properties, and which of them are exported.

use yuanme_diag::{Diagnostic, codes};
use yuanme_org::types::{DocumentElement, Heading, NodeProperty};

use super::{Lower, squash};
use crate::document::{Document, Kind, NodeId};

impl<'s> Lower<'s> {
    pub(super) fn has_select(&self, h: &Heading<'_>) -> bool {
        h.tags.iter().any(|t| self.select.iter().any(|s| s == t))
            || h.children.iter().any(|c| match c {
                DocumentElement::Heading(c) => self.has_select(c),
                DocumentElement::Section(_) => false,
            })
    }

    /// A property of a headline, or of the file when `heading` is `None`.
    /// Returns a `CUSTOM_ID`.
    pub(super) fn property(
        &mut self,
        p: &NodeProperty<'s>,
        heading: Option<NodeId>,
    ) -> Option<String> {
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
    pub(super) fn heading(&mut self, h: &Heading<'s>, selecting: bool, selected: bool) {
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
}
