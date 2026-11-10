//! organic's tree lowered into the Document. Where a construct is in
//! Markdown too it becomes the node Markdown's does, so it is written as
//! Markdown's HTML; constructs only Org has keep ox-html's element and
//! class names (#16). The output is flat: headlines are headings among
//! their sections' blocks, with no outline wrappers.

mod affiliated;
mod blocks;
mod headlines;
mod links;
mod lists;
mod objects;
mod paragraphs;
mod settings;
mod tables;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use yuanme_diag::{Diagnostic, FileId, Span, codes};
use yuanme_org::types::{Document as OrgDocument, Element, StandardProperties};

use super::Read;
use crate::config::{Math, TheoremKind};
use crate::document::{Document, Kind, NodeId};
use crate::smartypants::Dashes;
use crate::{markdown, math, theorem};
use links::Search;

/// How `x_y` and `x^{y}` read: `^:{}` by default, so `snake_case` stays
/// as it is (Org divergence 1).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scripts {
    Braces,
    All,
    None,
}

pub(super) fn lower(text: &str, file: FileId, parsed: &OrgDocument<'_>, math: &Math) -> Read {
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
        theorems: &math.theorems,
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
    l.finish(math);
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
    theorems: &'s BTreeMap<String, TheoremKind>,
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

    /// What only the whole file says: footnote numbers, smart punctuation,
    /// heading ids, and where in-page links go.
    fn finish(&mut self, math: &Math) {
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
        theorem::number(&mut self.doc, &math.theorems);
        let labels = math::render(
            &mut self.doc,
            self.text,
            self.file,
            &math.macros,
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
                // A Label too, as `[[#thm:a]]` (ADR 0010).
                Search::CustomId(id) => (
                    format!("#{id}"),
                    headings
                        .contains_key(id)
                        .then(|| id.clone())
                        .or_else(|| names.targets.get(id).cloned()),
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
                    Search::CustomId(id) => {
                        format!("nothing here has the `CUSTOM_ID` or Label `{id}`")
                    }
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
                let shown = match (names.numbers.get(&fragment), headings.get(&fragment)) {
                    (Some(number), _) => number.clone(),
                    (None, Some(&h)) => self.doc.text(h),
                    (None, None) => query.trim_start_matches(['*', '#']).to_string(),
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
