//! Where the assets stage's Diagnostics point: into the Templates a
//! Page's reference was written in, and into the stylesheets and Scripts
//! it bundles, which it adds to the Files only once one has a Diagnostic.

use std::collections::BTreeMap;
use std::sync::Arc;

use yuanme_assets::css;
use yuanme_diag::{Diagnostic, Edit, FileId, Files, Level, Span, codes};
use yuanme_render::template::Page;

use super::project::{Problem, Project};

/// One Diagnostic for each mistake, though many Pages make it.
#[derive(Default)]
pub(super) struct Diagnostics(pub(super) Vec<Diagnostic>);

impl Diagnostics {
    pub(super) fn push(&mut self, d: Diagnostic) {
        let key = |d: &Diagnostic| (d.code.name, d.message.clone(), d.primary().clone());
        if !self.0.iter().any(|seen| key(seen) == key(&d)) {
            self.0.push(d);
        }
    }

    pub(super) fn failed(&self) -> bool {
        self.0.iter().any(|d| d.level == Level::Error)
    }
}

/// Where a URL in a rendered Page was written: the first of the Page's
/// Templates whose text holds it, or else the start of the Page's own, and
/// whether it was found.
pub(super) fn reference_span(
    page: &Page,
    raw: &str,
    templates: &BTreeMap<String, FileId>,
    sources: &Files,
) -> (Span, bool) {
    let files = page.templates.iter().filter_map(|t| templates.get(t));
    for &file in files.clone() {
        if let Some(start) = sources.text(file).find(raw) {
            let span = Span {
                file,
                range: start..start + raw.len(),
            };
            return (span, true);
        }
    }
    let file = *files
        .clone()
        .next()
        .expect("an HTML Page has a Template file");
    (Span { file, range: 0..0 }, false)
}

/// The edit that adds `type="module"` to the `<script>` whose `src` is at
/// `span`.
pub(super) fn module_type(span: &Span, sources: &Files) -> Option<Edit> {
    let text = sources.text(span.file);
    let tag = text[..span.range.start].rfind("<script")? + "<script".len();
    Some(Edit {
        span: Span {
            file: span.file,
            range: tag..tag,
        },
        replacement: " type=\"module\"".into(),
    })
}

/// Stylesheets and Scripts added to the Files, for Diagnostics in them.
#[derive(Default)]
pub(super) struct SourceFiles(BTreeMap<String, FileId>);

impl SourceFiles {
    /// A bundle's error, in the stylesheet it is in.
    pub(super) fn error(
        &mut self,
        e: css::Error<Problem>,
        entry: &str,
        project: &Project<'_>,
        sources: &mut Files,
    ) -> Diagnostic {
        let (needle, file) = match &e.kind {
            css::ErrorKind::Project(problem) => (problem.url.as_str(), problem.file.clone()),
            css::ErrorKind::Css(_) => ("", None),
        };
        let span = match (file, &e.at) {
            (Some((path, at)), _) => Span {
                file: self.file(project, sources, &path),
                range: at..at,
            },
            (None, Some(at)) => self.span(project, sources, &at.file, at.line, at.column, needle),
            (None, None) => self.span(project, sources, entry, 1, 1, needle),
        };
        match e.kind {
            css::ErrorKind::Project(problem) => problem.diagnostic(span),
            css::ErrorKind::Css(message) => {
                Diagnostic::error(&codes::ASSET_INVALID_CSS, message, span)
            }
        }
    }

    /// A lossy copy of a file that isn't UTF-8 matches its bytes up to the
    /// first bad one.
    pub(super) fn file(
        &mut self,
        project: &Project<'_>,
        sources: &mut Files,
        path: &str,
    ) -> FileId {
        *self.0.entry(path.to_string()).or_insert_with(|| {
            let bytes = project.bytes(path).unwrap_or_else(|_| Arc::from([]));
            sources.add(path, String::from_utf8_lossy(&bytes))
        })
    }

    /// `line` from 1 and `column` from 1 in UTF-16 units, as lightningcss
    /// reports them (rolldown's columns are from 0); `needle`, when found on
    /// the line from there, is what the span covers.
    pub(super) fn span(
        &mut self,
        project: &Project<'_>,
        sources: &mut Files,
        path: &str,
        line: u32,
        column: u32,
        needle: &str,
    ) -> Span {
        let file = self.file(project, sources, path);
        let text = sources.text(file);
        let start: usize = text
            .split_inclusive('\n')
            .take(line.saturating_sub(1) as usize)
            .map(str::len)
            .sum();
        let line_text = text[start..].split('\n').next().unwrap_or("");
        let mut units = 1;
        let mut at = line_text.len();
        for (i, c) in line_text.char_indices() {
            if units >= column as usize {
                at = i;
                break;
            }
            units += c.len_utf16();
        }
        let from = start + at;
        let range = match line_text[at..].find(needle).filter(|_| !needle.is_empty()) {
            Some(i) => from + i..from + i + needle.len(),
            None => from..from,
        };
        Span { file, range }
    }
}
