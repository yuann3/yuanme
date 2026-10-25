//! #15's static output checks on links: every URL on the Site's origin that
//! a Page links to or loads is a file of the build, at the URL it is served
//! from, so no visitor waits on a redirect or finds nothing there.

use std::collections::BTreeMap;

use yuanme_assets::html;
use yuanme_content::config::{self, Config};
use yuanme_content::site::{RouteKind, Site};
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Files, Span, codes};

use crate::assets::{has_scheme, normalize, percent_decode, suffix};
use crate::image::{Settings, Variant};
use crate::images::{self, Heavy};
use crate::{Body, Output};

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
/// and each Page that loads a raster of `heavy`.
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
) -> Vec<Diagnostic> {
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

    for (output, used) in pages {
        let Some(Output {
            body: Body::Bytes(bytes),
            ..
        }) = files.get(output)
        else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        let base = url_of(output);
        for link in html::urls(text) {
            let url = link.url.trim();
            let Some((path, absolute)) = internal(url, &base, origin) else {
                continue;
            };
            let raw = &text[link.at.clone()];
            let (span, found) = span(raw, used, entries.get(output.as_str()), templates, sources);
            let fixed = |to: &str| {
                let prefix = if absolute { origin } else { "" };
                format!("{prefix}{to}{}", suffix(url))
            };
            let redirected = |to: String| {
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
                let d = images::unoptimized(url, h, span.clone());
                push(d.note(format!("in the Page written to `{output}`")));
            }
            let d = match verdict(&path, files, images, config) {
                Verdict::Ok if on_origin(url, absolute, origin) => continue,
                Verdict::Ok => redirected(path.clone()),
                Verdict::Redirect(to) => redirected(to),
                Verdict::Broken => {
                    let d = Diagnostic::warning(
                        &codes::LINK_BROKEN,
                        format!("`{url}` is no file of the Site"),
                        span.clone(),
                    );
                    match config::similar(&path, urls.iter().map(String::as_str)) {
                        Some(near) => d.help(format!("did you mean `{}`?", fixed(near))),
                        None => d,
                    }
                }
            };
            push(d.note(format!("in the Page written to `{output}`")));
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

/// The path from the root that `url`, written in the Page at `base`, names
/// on the Site's origin, decoded and without its query and fragment, and
/// whether `url` was absolute. `None` for another origin, a scheme such as
/// `mailto:`, or a fragment alone.
pub(crate) fn internal(url: &str, base: &str, origin: &str) -> Option<(String, bool)> {
    if url.is_empty() || url.starts_with('#') {
        return None;
    }
    let (written, absolute) = if url.starts_with("//") || has_scheme(url) {
        let (scheme, rest) = match url.strip_prefix("//") {
            Some(rest) => (origin.split_once("://")?.0, rest),
            None => url.split_once("://")?,
        };
        if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
            return None;
        }
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (_, site_host) = origin.split_once("://")?;
        let host = &rest[..end];
        let same = host.eq_ignore_ascii_case(site_host);
        // `www.` and the apex are one Site, which the host redirects
        // between (#10).
        let www = host.strip_prefix("www.") == Some(site_host)
            || site_host.strip_prefix("www.") == Some(host);
        if !same && !www {
            return None;
        }
        let path = match &rest[end..] {
            p if p.starts_with('/') => p.to_string(),
            p => format!("/{p}"),
        };
        (path, true)
    } else if url.starts_with('/') {
        (url.to_string(), false)
    } else {
        let dir = &base[..base.rfind('/').expect("from the root") + 1];
        (format!("{dir}{url}"), false)
    };
    let path = &written[..written.len() - suffix(&written).len()];
    Some((normalize(&percent_decode(path)?), absolute))
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

/// The URL a visitor loads a file at.
fn url_of(path: &str) -> String {
    match path.strip_suffix("index.html") {
        Some(dir) if dir.is_empty() || dir.ends_with('/') => format!("/{dir}"),
        _ => format!("/{path}"),
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
