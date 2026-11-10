//! #15's static output checks on links: every URL on the Site's origin that
//! a Page links to or loads is a file of the build, at the URL it is served
//! from, so no visitor waits on a redirect or finds nothing there.

use std::collections::{BTreeMap, HashMap};

use rayon::prelude::*;

use yuanme_assets::html_urls;
use yuanme_content::{Config, RouteKind, Site, similar};
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Files, Span, codes};

use super::heavy::{self, Heavy};
use crate::image::{Settings, Variant};
use crate::url::{internal, normalize, percent_decode, suffix, url_of};
use crate::{Body, Cache, Inputs, Output};

/// What a link on the Site's origin finds.
enum Verdict {
    Ok,
    /// A host redirects it to this path.
    Redirect(String),
    Broken,
}

/// The links of each HTML Page in `pages`, by output path with the
/// Templates it rendered, and the `to` of each redirect, against `files`
/// and the image variants a build that encodes none leaves out of them;
/// and each Page that loads a raster of `heavy`. Each field is one output
/// of the build.
pub(crate) struct Links<'a> {
    pub(crate) config: &'a Config,
    pub(crate) site: &'a Site,
    pub(crate) pages: &'a BTreeMap<String, Vec<String>>,
    pub(crate) files: &'a BTreeMap<String, Output>,
    pub(crate) images: &'a BTreeMap<String, (Variant, Settings)>,
    pub(crate) heavy: &'a BTreeMap<String, Heavy>,
    pub(crate) templates: &'a BTreeMap<String, FileId>,
    pub(crate) sources: &'a Files,
    pub(crate) inputs: &'a Inputs,
}

/// What checking each Page reads besides the Page, worked out once.
struct Pass<'a> {
    /// What a check that finds nothing reads besides its Page.
    site_key: blake3::Hasher,
    /// The key each Page that found nothing had in the last build.
    clean: HashMap<String, blake3::Hash>,
    /// Each Entry's file, by its Page's output path.
    entries: BTreeMap<&'a str, FileId>,
    /// Every file's URL, for suggestions.
    urls: Vec<String>,
}

impl Links<'_> {
    /// Every link Diagnostic. A Page with nothing to report is remembered
    /// in `cache`, by everything its check read, and not checked again
    /// until some of it changes.
    pub(crate) fn check(&self, cache: &mut Cache) -> Vec<Diagnostic> {
        let pass = Pass {
            site_key: self.site_key(cache),
            clean: std::mem::take(&mut cache.clean),
            entries: self
                .site
                .routes
                .iter()
                .filter_map(|r| match r.kind {
                    RouteKind::Entry(i) => Some((r.output.as_str(), self.site.entries[i].file)),
                    _ => None,
                })
                .collect(),
            urls: self.files.keys().map(|p| url_of(p)).collect(),
        };
        // Each Page on a thread of its own, since this pass reads every byte
        // of the Site's HTML on every build.
        let found: Vec<(Option<blake3::Hash>, Vec<Diagnostic>)> = self
            .pages
            .par_iter()
            .map(|(output, used)| self.page(output, used, &pass))
            .collect();
        let mut out = Vec::new();
        for (output, (clean, found)) in self.pages.keys().zip(found) {
            if let Some(key) = clean {
                cache.clean.insert(output.clone(), key);
            }
            for d in found {
                push_new(&mut out, d);
            }
        }
        for d in self.redirects() {
            push_new(&mut out, d);
        }
        out
    }

    fn origin(&self) -> &str {
        self.config.site.url.trim_end_matches('/')
    }

    /// Site config, for the origin and the redirects, and the paths a link
    /// can find. `heavy` is only its paths, since a Page that loads one is
    /// reported.
    fn site_key(&self, cache: &Cache) -> blake3::Hasher {
        let mut key = cache.key("links");
        key.update(self.inputs.files["yuanme.toml"].hash.as_bytes());
        let paths = self.files.keys().chain(self.images.keys());
        for path in paths.chain(self.heavy.keys()) {
            key.update(&(path.len() as u64).to_le_bytes());
            key.update(path.as_bytes());
            key.update(&[0xff]);
        }
        key
    }

    /// The Diagnostics of the Page written to `output`, which rendered the
    /// Templates `used`, and its key if it found none.
    fn page(
        &self,
        output: &str,
        used: &[String],
        pass: &Pass<'_>,
    ) -> (Option<blake3::Hash>, Vec<Diagnostic>) {
        let mut found = Vec::new();
        let Some(Output {
            body: Body::Bytes(bytes),
            hash,
        }) = self.files.get(output)
        else {
            return (None, found);
        };
        let mut key = pass.site_key.clone();
        key.update(output.as_bytes());
        key.update(hash.as_bytes());
        let key = key.finalize();
        if pass.clean.get(output) == Some(&key) {
            return (Some(key), found);
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return (None, found);
        };
        let origin = self.origin();
        let base = url_of(output);
        for link in html_urls(text) {
            let url = link.url.trim();
            let Some((path, absolute)) = internal(url, &base, origin) else {
                continue;
            };
            let raw = &text[link.at.clone()];
            // Searching the Templates and the Entry for each link costs
            // more than the rest of the pass, so only a Diagnostic does.
            let entry = pass.entries.get(output);
            let place = || span(raw, used, entry, self.templates, self.sources);
            let fixed = |to: &str| {
                let prefix = if absolute { origin } else { "" };
                format!("{prefix}{to}{}", suffix(url))
            };
            let redirected = |to: String| {
                let (span, found) = place();
                let fix = fixed(&to);
                let d = Diagnostic::warning(
                    &codes::LINK_REDIRECTED,
                    format!("`{url}` redirects to `{fix}`"),
                    span.clone(),
                );
                if found && raw == url {
                    let edit = Edit {
                        span: span.clone(),
                        replacement: fix.clone(),
                    };
                    let message = format!("link to `{fix}`");
                    d.suggest(message, Applicability::MaybeIncorrect, vec![edit])
                } else {
                    d.help(format!("link to `{fix}`"))
                }
            };
            if link.loaded
                && let Some(h) = self.heavy.get(&path[1..])
            {
                let d = heavy::unoptimized(url, h, place().0);
                found.push(d.note(format!("in the Page written to `{output}`")));
            }
            let d = match verdict(&path, self.files, self.images, self.config) {
                Verdict::Ok if on_origin(url, absolute, origin) => continue,
                Verdict::Ok => redirected(path.clone()),
                Verdict::Redirect(to) => redirected(to),
                Verdict::Broken => {
                    let d = Diagnostic::warning(
                        &codes::LINK_BROKEN,
                        format!("`{url}` is no file of the Site"),
                        place().0,
                    );
                    match similar(&path, pass.urls.iter().map(String::as_str)) {
                        Some(near) => d.help(format!("did you mean `{}`?", fixed(near))),
                        None => d,
                    }
                }
            };
            found.push(d.note(format!("in the Page written to `{output}`")));
        }
        (found.is_empty().then_some(key), found)
    }

    /// The `to` of each redirect on the Site's own origin.
    fn redirects(&self) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for r in &self.config.host.redirects {
            if !r.to.starts_with('/') {
                continue;
            }
            let path = percent_decode(&r.to[..r.to.len() - suffix(&r.to).len()])
                .map(|p| normalize(&p))
                .unwrap_or_default();
            let at = Span {
                file: self.config.file,
                range: r.to_at.clone(),
            };
            match verdict(&path, self.files, self.images, self.config) {
                Verdict::Ok => {}
                Verdict::Redirect(to) => out.push(
                    Diagnostic::warning(
                        &codes::LINK_REDIRECTED,
                        format!("`{}` redirects again, to `{to}`", r.to),
                        at,
                    )
                    .help(format!("redirect straight to `{to}`")),
                ),
                Verdict::Broken => out.push(Diagnostic::warning(
                    &codes::LINK_BROKEN,
                    format!("`{}` is no file of the Site", r.to),
                    at,
                )),
            }
        }
        out
    }
}

/// Pushes `d` unless one with its code, message and primary span is in
/// `out`: a link in a Template is in every Page it renders.
fn push_new(out: &mut Vec<Diagnostic>, d: Diagnostic) {
    let key = |d: &Diagnostic| (d.code.name, d.message.clone(), d.primary().clone());
    if !out.iter().any(|seen| key(seen) == key(&d)) {
        out.push(d);
    }
}

/// Whether `url` is on the Site's exact origin, as it must be to resolve:
/// another scheme, or `www.` against the apex, is a redirect.
fn on_origin(url: &str, absolute: bool, origin: &str) -> bool {
    let host = |u: &str| {
        u.split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    match url.strip_prefix("//") {
        _ if !absolute => true,
        Some(rest) => origin
            .split_once("://")
            .is_some_and(|(_, o)| host(o) == host(rest)),
        None => url
            .get(..origin.len())
            .is_some_and(|o| o.eq_ignore_ascii_case(origin)),
    }
}

/// A file of the Site wins over a redirect from its URL, which is an error
/// of its own (`host/redirect-conflict`).
fn verdict(
    path: &str,
    files: &BTreeMap<String, Output>,
    images: &BTreeMap<String, (Variant, Settings)>,
    config: &Config,
) -> Verdict {
    let file = &path[1..];
    let has = |p: &str| files.contains_key(p) || images.contains_key(p);
    let redirect = config.host.redirects.iter().find(|r| r.from == path);
    if file.is_empty() || file.ends_with('/') {
        if has(&format!("{file}index.html")) {
            Verdict::Ok
        } else if let Some(r) = redirect {
            Verdict::Redirect(r.to.clone())
        } else if !file.is_empty() && has(file.trim_end_matches('/')) {
            Verdict::Redirect(path.trim_end_matches('/').to_string())
        } else {
            Verdict::Broken
        }
    } else if has(file) {
        match file.strip_suffix(".html") {
            // Workers and Cloudflare Pages drop `index.html` and `.html`.
            Some(stem) => match stem.strip_suffix("index") {
                Some(dir) if dir.is_empty() || dir.ends_with('/') => {
                    Verdict::Redirect(format!("/{dir}"))
                }
                _ => Verdict::Redirect(format!("/{stem}")),
            },
            None => Verdict::Ok,
        }
    } else if let Some(r) = redirect {
        Verdict::Redirect(r.to.clone())
    } else if has(&format!("{file}/index.html")) {
        Verdict::Redirect(format!("{path}/"))
    } else {
        Verdict::Broken
    }
}

/// Where `raw` was written: the first of the Page's Templates, or else its
/// Entry, whose text holds it; or else the start of the Page's Template,
/// and whether it was found.
fn span(
    raw: &str,
    used: &[String],
    entry: Option<&FileId>,
    templates: &BTreeMap<String, FileId>,
    sources: &Files,
) -> (Span, bool) {
    let files = used.iter().filter_map(|t| templates.get(t)).chain(entry);
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
        .expect("render writes each HTML Page from at least one Template");
    (Span { file, range: 0..0 }, false)
}
