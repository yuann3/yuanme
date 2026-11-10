//! Paragraphs, and the figure a captioned image alone in one makes.

use yuanme_org::types::{AffiliatedKeywords, LinkType, Object, Paragraph};

use super::Lower;
use super::links::is_image;
use crate::document::{Kind, NodeId};

impl<'s> Lower<'s> {
    pub(super) fn paragraph(&mut self, p: &Paragraph<'s>, parent: NodeId) {
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
