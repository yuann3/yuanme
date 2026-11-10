//! Plain lists: ordered, unordered and descriptive.

use yuanme_org::types::{CheckboxType, Element, PlainList, PlainListType};

use super::Lower;
use crate::document::{Checkbox, Kind, NodeId};

impl<'s> Lower<'s> {
    pub(super) fn list(&mut self, l: &PlainList<'s>, parent: NodeId) {
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
}
