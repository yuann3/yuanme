//! The Host adapters (#25): one host-neutral model of the Site's routing,
//! its header rules, redirects and 404 Page, checked against the strictest
//! host's limits and rendered into the files one static host reads.

mod cloudflare;
mod compress;
mod github_pages;
mod headers;
mod vercel;

use std::collections::BTreeMap;
use std::ops::Range;

use yuanme_content::config::{Adapter, Config, HeaderRule, Host};
use yuanme_diag::{Diagnostic, Span, codes};

pub use cloudflare::wrangler;
pub(crate) use compress::brotli;
use compress::precompressed;
use github_pages::{custom_domain, refresh, stub};
use headers::{headers_file, redirects_file};
use vercel::vercel_config;

use crate::url::url_of;
use crate::{Body, Cache, Output};

/// For hashed files, whose name changes with their bytes (#15).
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// For everything else. It is sent rather than left to the host's default,
/// so that a CDN's browser TTL can't keep a Page stale after a deploy.
const REVALIDATE: &str = "public, max-age=0, must-revalidate";

/// Cloudflare's, the strictest supported host's: every Site is held to
/// them, so it can move hosts.
const HEADER_RULES: usize = 100;
const REDIRECTS: usize = 2_000;
const LINE: usize = 2_000;

/// One header rule of the model: yuanme's, or the Author's. Rules for one
/// path are one rule, since `_headers` parsers key them by path.
struct Rule<'a> {
    path: &'a str,
    /// `None` for yuanme's own.
    headers: Vec<(&'a str, &'a str, Option<&'a HeaderRule>)>,
}

fn rules(config: &Config) -> Vec<Rule<'_>> {
    let mut rules = vec![
        Rule {
            path: "/*",
            headers: vec![
                ("Cache-Control", REVALIDATE, None),
                ("X-Content-Type-Options", "nosniff", None),
            ],
        },
        Rule {
            path: "/assets/*",
            headers: vec![("Cache-Control", IMMUTABLE, None)],
        },
    ];
    for author in &config.host.headers {
        let i = match rules.iter().position(|r| r.path == author.path) {
            Some(i) => i,
            None => {
                rules.push(Rule {
                    path: &author.path,
                    headers: Vec::new(),
                });
                rules.len() - 1
            }
        };
        for (name, value) in &author.headers {
            let headers = &mut rules[i].headers;
            let header = (name.as_str(), value.as_str(), Some(author));
            match headers.iter_mut().find(|h| h.0.eq_ignore_ascii_case(name)) {
                Some(h) => *h = header,
                None => headers.push(header),
            }
        }
    }
    rules
}

/// A path with at most one `*`, which matches any characters, `/` too.
fn matches(pattern: &str, url: &str) -> bool {
    match pattern.split_once('*') {
        Some((before, after)) => {
            url.len() >= before.len() + after.len()
                && url.starts_with(before)
                && url.ends_with(after)
        }
        None => pattern == url,
    }
}

/// The value of `name` for `url`, and the Author's rule that set it: the
/// last rule that matches and sets it wins, as on every host.
fn header<'a>(
    rules: &'a [Rule<'a>],
    url: &str,
    name: &str,
) -> Option<(&'a str, Option<&'a HeaderRule>)> {
    rules
        .iter()
        .rev()
        .filter(|r| matches(r.path, url))
        .find_map(|r| r.headers.iter().find(|h| h.0.eq_ignore_ascii_case(name)))
        .map(|&(_, value, from)| (value, from))
}

/// Checks the model against `files`, the Site's, and unless `render` is
/// false, as in dev, adds the adapter's files to them. For Vercel every
/// file moves under `static/`, as the Build Output API reads them.
pub(crate) fn host(
    config: &Config,
    mut files: BTreeMap<String, Output>,
    render: bool,
    cache: &mut Cache,
) -> (BTreeMap<String, Output>, Vec<Diagnostic>) {
    let at = |range: Range<usize>| Span {
        file: config.file,
        range,
    };
    let host = &config.host;
    let rules = rules(config);
    let mut diagnostics = Vec::new();

    // Limits.
    if let Some(rule) = rules.get(HEADER_RULES) {
        let author = rule.headers[0].2.expect("yuanme's rules come first");
        let message = format!(
            "this is header rule {}, counting yuanme's 2 cache rules, but Cloudflare accepts {HEADER_RULES}",
            HEADER_RULES + 1
        );
        diagnostics.push(
            Diagnostic::error(&codes::HOST_OVER_LIMIT, message, at(author.path_at.clone()))
                .help("merge rules: a `*` in a path, such as `/fonts/*`, covers many files"),
        );
    }
    if let Some(redirect) = host.redirects.get(REDIRECTS) {
        let message = format!(
            "this is redirect {}, but Cloudflare accepts {REDIRECTS}",
            REDIRECTS + 1
        );
        diagnostics.push(Diagnostic::error(
            &codes::HOST_OVER_LIMIT,
            message,
            at(redirect.from_at.clone()),
        ));
    }
    let long = |line: &str| line.chars().count() > LINE;
    for rule in &host.headers {
        let lines = std::iter::once(rule.path.clone())
            .chain(rule.headers.iter().map(|(n, v)| format!("  {n}: {v}")));
        if let Some(line) = lines.into_iter().find(|l| long(l)) {
            let message = format!(
                "this rule makes a line of {} characters in `_headers`, but Cloudflare accepts {LINE}",
                line.chars().count()
            );
            diagnostics.push(Diagnostic::error(
                &codes::HOST_OVER_LIMIT,
                message,
                at(rule.path_at.clone()),
            ));
        }
    }
    for r in &host.redirects {
        let line = format!("{} {} {}", r.from, r.to, r.status);
        if long(&line) {
            let message = format!(
                "this redirect makes a line of {} characters in `_redirects`, but Cloudflare accepts {LINE}",
                line.chars().count()
            );
            diagnostics.push(Diagnostic::error(
                &codes::HOST_OVER_LIMIT,
                message,
                at(r.from_at.clone()),
            ));
        }
    }

    // A redirect from a URL the Site serves: hosts disagree which wins.
    let mut served: BTreeMap<String, &str> = BTreeMap::new();
    for path in files.keys() {
        let url = url_of(path);
        if url.len() > 1 && url.ends_with('/') {
            served.insert(url.trim_end_matches('/').to_string(), path);
        }
        served.insert(url, path);
        served.insert(format!("/{path}"), path);
    }
    for r in &host.redirects {
        if let Some(path) = served.get(&r.from) {
            let message = format!("`{}` is where the Site serves `{path}`", r.from);
            diagnostics.push(
                Diagnostic::error(
                    &codes::HOST_REDIRECT_CONFLICT,
                    message,
                    at(r.from_at.clone()),
                )
                .note("Cloudflare would follow the redirect, and Netlify serve the file")
                .help("remove the redirect, or the file it hides"),
            );
        }
    }

    // #15's static output checks: hashed assets stay immutable, and Pages
    // revalidate, whatever the Author's rules add.
    let mut warned: Vec<&HeaderRule> = Vec::new();
    for path in files.keys() {
        let (ok, what): (fn(&str) -> bool, _) = if path.starts_with("assets/") {
            (
                |v| v.contains("immutable"),
                format!("`{path}`, a hashed asset, which must stay immutable"),
            )
        } else if path.ends_with(".html") {
            (
                |v| {
                    v.contains("no-cache")
                        || v.contains("no-store")
                        || (v.contains("max-age=0") && v.contains("must-revalidate"))
                },
                format!(
                    "the Page `{path}`, which must revalidate (`max-age=0, must-revalidate`) so a deploy shows at once"
                ),
            )
        } else {
            continue;
        };
        let Some((value, Some(rule))) = header(&rules, &url_of(path), "cache-control") else {
            continue;
        };
        if !ok(value) && !warned.iter().any(|w| std::ptr::eq(*w, rule)) {
            warned.push(rule);
            let message = format!("this rule sets `Cache-Control: {value}` on {what}");
            diagnostics.push(
                Diagnostic::warning(&codes::HOST_CACHE_RULE, message, at(rule.path_at.clone()))
                    .help("narrow the rule's `path` to the files it is for"),
            );
        }
    }

    // What the adapter can't send.
    let cant = |message: String, range: Range<usize>| {
        Diagnostic::warning(&codes::HOST_UNSUPPORTED, message, at(range))
    };
    match host.adapter {
        Adapter::GithubPages => {
            diagnostics.push(
                cant(
                    "GitHub Pages can't send headers, so hashed assets aren't cached as immutable, and every file is cached for 10 minutes".into(),
                    host.adapter_at.clone(),
                )
                .note("nor does it serve brotli or HTTP/3")
                .help("a host that can is the default: `adapter = \"cloudflare\"`"),
            );
            for rule in &host.headers {
                diagnostics.push(cant(
                    "GitHub Pages can't send this rule's headers".into(),
                    rule.path_at.clone(),
                ));
            }
            for r in &host.redirects {
                if stub(&r.from).is_none() {
                    let message = format!(
                        "GitHub Pages redirects with an HTML page, which can't be served at `{}`",
                        r.from
                    );
                    diagnostics.push(cant(message, r.from_at.clone()));
                }
            }
        }
        Adapter::Static => {
            for rule in &host.headers {
                diagnostics.push(cant(
                    "the static adapter writes no host configuration: the server must send this rule's headers".into(),
                    rule.path_at.clone(),
                ));
            }
            for r in &host.redirects {
                let message = format!(
                    "the static adapter writes no host configuration: the server must redirect `{}`",
                    r.from
                );
                diagnostics.push(cant(message, r.from_at.clone()));
            }
        }
        _ => {}
    }

    let failed = diagnostics
        .iter()
        .any(|d| d.level == yuanme_diag::Level::Error);
    if !render || failed {
        return (files, diagnostics);
    }

    let mut written: Vec<(String, Vec<u8>)> = Vec::new();
    match host.adapter {
        Adapter::Cloudflare | Adapter::CloudflarePages => {
            written.push(("_headers".into(), headers_file(&rules, true)));
            written.extend(redirects_file(config));
        }
        Adapter::Netlify => {
            written.push(("_headers".into(), headers_file(&rules, false)));
            written.extend(redirects_file(config));
        }
        Adapter::Vercel => {}
        Adapter::GithubPages => {
            // Jekyll would drop `_`-prefixed folders without it.
            written.push((".nojekyll".into(), Vec::new()));
            if let Some(domain) = custom_domain(&config.site.url) {
                written.push(("CNAME".into(), format!("{domain}\n").into_bytes()));
            }
            for r in &host.redirects {
                if let Some(path) = stub(&r.from) {
                    written.push((path, refresh(&config.site.url, &r.to).into_bytes()));
                }
            }
        }
        Adapter::Static if host.precompress => written.extend(precompressed(&files, cache)),
        Adapter::Static => {}
    }
    for (path, bytes) in written {
        if files.contains_key(&path) {
            let message = format!(
                "`static/{path}` and the {} adapter would both write `{path}`",
                host.adapter.name()
            );
            diagnostics.push(
                Diagnostic::error(&codes::HOST_OUTPUT_COLLISION, message, at(host.adapter_at.clone()))
                    .help("delete the Static file, and write its rules as `[[host.headers]]` and `[[host.redirects]]`"),
            );
            continue;
        }
        let output = Output {
            hash: blake3::hash(&bytes),
            body: Body::Bytes(bytes.into()),
        };
        files.insert(path, output);
    }
    if host.adapter == Adapter::Vercel {
        let config_json = vercel_config(config, &rules, files.contains_key("404.html"));
        files = files
            .into_iter()
            .map(|(path, output)| (format!("static/{path}"), output))
            .collect();
        let bytes = config_json.into_bytes();
        let output = Output {
            hash: blake3::hash(&bytes),
            body: Body::Bytes(bytes.into()),
        };
        files.insert("config.json".into(), output);
    }
    (files, diagnostics)
}

/// The output file the host serves the path `output` from, such as
/// Vercel's `static/blog/pew/index.html` for a Route's
/// `blog/pew/index.html`. The inverse of [`served`].
pub fn file_of(host: &Host, output: &str) -> String {
    match host.adapter {
        Adapter::Vercel => format!("static/{output}"),
        _ => output.to_string(),
    }
}

/// The path from the Site's root that the host serves an output file at,
/// such as `blog/pew/index.html` for Vercel's `static/blog/pew/index.html`,
/// or none for a file only the host reads, such as `_headers`.
pub fn served<'p>(host: &Host, path: &'p str) -> Option<&'p str> {
    let own: &[&str] = match host.adapter {
        Adapter::Vercel => return path.strip_prefix("static/"),
        Adapter::Cloudflare | Adapter::CloudflarePages | Adapter::Netlify => {
            &["_headers", "_redirects"]
        }
        Adapter::GithubPages => &[".nojekyll", "CNAME"],
        Adapter::Static if host.precompress && (path.ends_with(".br") || path.ends_with(".gz")) => {
            return None;
        }
        Adapter::Static => &[],
    };
    (!own.contains(&path)).then_some(path)
}
