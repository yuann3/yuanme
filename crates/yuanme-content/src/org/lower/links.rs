//! Links, and the images a link to an image file shows.

use yuanme_org::types::{LinkType, Object};

use super::{Lower, squash};
use crate::document::{Kind, NodeId};

/// What every kind of Org link has, whether written `[[…]]`, `<…>` or
/// plain.
pub(super) struct OrgLink<'a, 's> {
    pub(super) link_type: &'a LinkType<'a>,
    pub(super) path: &'a str,
    /// The target as written, such as `https://…` or `file:a.org`.
    pub(super) raw: &'a str,
    /// `::search` after a file's path.
    pub(super) search: Option<&'a str>,
    /// What it shows, if it says: only `[[link][description]]` can.
    pub(super) description: &'a [Object<'s>],
    pub(super) source: &'s str,
}

impl<'s> Lower<'s> {
    pub(super) fn link(&mut self, link: OrgLink<'_, 's>, parent: NodeId) {
        let OrgLink {
            link_type,
            path,
            raw,
            search,
            description,
            source,
        } = link;
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
                self.doc.names.undescribed.insert(id);
            }
        } else {
            for o in description {
                self.object(o, id);
            }
        }
    }
}

/// What a link resolves to once the whole file is read.
pub(super) enum Search {
    /// `[[#id]]`.
    CustomId(String),
    /// `[[*Title]]`.
    Title(String),
    /// `[[name]]`: a target or `#+NAME` first, then a headline's title.
    Fuzzy(String),
}

/// Image links ox-html shows as images (`org-html-inline-image-rules`).
const IMAGES: &[&str] = &["avif", "gif", "jpeg", "jpg", "png", "svg", "webp"];

pub(super) fn is_image(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| IMAGES.iter().any(|i| ext.eq_ignore_ascii_case(i)))
}
