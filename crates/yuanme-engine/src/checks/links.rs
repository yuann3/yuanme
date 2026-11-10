//! #15's static output checks on links: every URL on the Site's origin that
//! a Page links to or loads is a file of the build, at the URL it is served
//! from, so no visitor waits on a redirect or finds nothing there.

use std::collections::BTreeMap;

use rayon::prelude::*;

use yuanme_assets::html;
use yuanme_content::config::{self, Config};
use yuanme_content::site::{RouteKind, Site};
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Files, Span, codes};

use crate::image::{Settings, Variant};
use crate::images::{self, Heavy};
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
/// and each Page that loads a raster of `heavy`. A Page with nothing to
/// report is remembered in `cache`, by everything its check read, and not
/// checked again until some of it changes.
#[expect(
    clippy::too_many_arguments,
    reason = "each is one output of the build, which a struct would only rename"
)]
pub(crate) fn links(
    config: &Config,
    site: &Site,
    pages: &BTreeMap<String, Vec<String>>,
    files: &BTreeMap<String, Output>,
    images: &BTreeMap<String, (Variant, Settings)>,
    heavy: &BTreeMap<String, Heavy>,
    templates: &BTreeMap<String, FileId>,
    sources: &Files,
    inputs: &Inputs,
    cache: &mut Cache,
) -> Vec<Diagnostic> {
    // What a check that finds nothing reads besides its Page: Site config,
    // for the origin and the redirects, and the paths a link can find.
    // `heavy` is only its paths, since a Page that loads one is reported.
    let mut site_key = cache.key("links");
    site_key.update(inputs.files["yuanme.toml"].hash.as_bytes());
    for path in files.keys().chain(images.keys()).chain(heavy.keys()) {
        site_key.update(&(path.len() as u64).to_le_bytes());
        site_key.update(path.as_bytes());
        site_key.update(&[0xff]);
    }
    let page_key = |output: &str, hash: &blake3::Hash| {
        let mut key = site_key.clone();
        key.update(output.as_bytes());
        key.update(hash.as_bytes());
        key.finalize()
    };
    let clean = std::mem::take(&mut cache.clean);
    let origin = config.site.url.trim_end_matches('/');
    let entries: BTreeMap<&str, FileId> = site
        .routes
        .iter()
        .filter_map(|r| match r.kind {
            RouteKind::Entry(i) => Some((r.output.as_str(), site.entries[i].file)),
            _ => None,
        })
        .collect();
    let urls: Vec<String> = files.keys().map(|p| url_of(p)).collect();
    let mut out = Vec::new();
    let mut push = |d: Diagnostic| {
        let key = |d: &Diagnostic| (d.code.name, d.message.clone(), d.primary().clone());
        if !out.iter().any(|seen| key(seen) == key(&d)) {
            out.push(d);
        }
    };

    // Each Page on a thread of its own, since this pass reads every byte
    // of the Site's HTML on every build.
    // With the key of each Page that found nothing.
    let found: Vec<(Option<blake3::Hash>, Vec<Diagnostic>)> = pages
        .par_iter()
        .map(|(output, used)| {
            let mut found = Vec::new();
            let Some(Output {
                body: Body::Bytes(bytes),
                hash,
            }) = files.get(output)
            else {
                return (None, found);
            };
            let key = page_key(output, hash);
            if clean.get(output) == Some(&key) {
                return (Some(key), found);
            }
            let Ok(text) = std::str::from_utf8(bytes) else {
                return (None, found);
            };
            let base = url_of(output);
            for link in html::urls(text) {
                let url = link.url.trim();
                let Some((path, absolute)) = internal(url, &base, origin) else {
                    continue;
                };
                let raw = &text[link.at.clone()];
                // Searching the Templates and the Entry for each link costs
                // more than the rest of the pass, so only a Diagnostic does.
                let place = || span(raw, used, entries.get(output.as_str()), templates, sources);
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
                    && let Some(h) = heavy.get(&path[1..])
                {
                    let d = images::unoptimized(url, h, place().0);
                    found.push(d.note(format!("in the Page written to `{output}`")));
                }
                let d = match verdict(&path, files, images, config) {
                    Verdict::Ok if on_origin(url, absolute, origin) => continue,
                    Verdict::Ok => redirected(path.clone()),
                    Verdict::Redirect(to) => redirected(to),
                    Verdict::Broken => {
                        let d = Diagnostic::warning(
                            &codes::LINK_BROKEN,
                            format!("`{url}` is no file of the Site"),
                            place().0,
                        );
                        match config::similar(&path, urls.iter().map(String::as_str)) {
                            Some(near) => d.help(format!("did you mean `{}`?", fixed(near))),
                            None => d,
                        }
                    }
                };
                found.push(d.note(format!("in the Page written to `{output}`")));
            }
            (found.is_empty().then_some(key), found)
        })
        .collect();
    for (output, (clean, found)) in pages.keys().zip(found) {
        if let Some(key) = clean {
            cache.clean.insert(output.clone(), key);
        }
        for d in found {
            push(d);
        }
    }

    for r in &config.host.redirects {
        if !r.to.starts_with('/') {
            continue;
        }
        let path = percent_decode(&r.to[..r.to.len() - suffix(&r.to).len()])
            .map(|p| normalize(&p))
            .unwrap_or_default();
        let at = Span {
            file: config.file,
            range: r.to_at.clone(),
        };
        match verdict(&path, files, images, config) {
            Verdict::Ok => {}
            Verdict::Redirect(to) => push(
                Diagnostic::warning(
                    &codes::LINK_REDIRECTED,
                    format!("`{}` redirects again, to `{to}`", r.to),
                    at,
                )
                .help(format!("redirect straight to `{to}`")),
            ),
            Verdict::Broken => push(Diagnostic::warning(
                &codes::LINK_BROKEN,
                format!("`{}` is no file of the Site", r.to),
                at,
            )),
        }
    }
    out
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
        .expect("an HTML Page has a Template file");
    (Span { file, range: 0..0 }, false)
}
