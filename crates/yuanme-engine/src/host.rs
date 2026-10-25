//! The Host adapters (#25): one host-neutral model of the Site's routing,
//! its header rules, redirects and 404 Page, checked against the strictest
//! host's limits and rendered into the files one static host reads.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::io::Write as _;
use std::ops::Range;
use std::sync::Arc;

use rayon::prelude::*;
use yuanme_content::config::{Adapter, Config, HeaderRule};
use yuanme_diag::{Diagnostic, Span, codes};

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

/// The URL a visitor loads a file at.
fn url_of(path: &str) -> String {
    match path.strip_suffix("index.html") {
        Some(dir) if dir.is_empty() || dir.ends_with('/') => format!("/{dir}"),
        _ => format!("/{path}"),
    }
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

/// The `_headers` that Cloudflare and Netlify both read. A header an
/// earlier rule set is detached first on Cloudflare, which would otherwise
/// join the two values with a comma; Netlify lets the later rule win, and
/// has no `!` lines.
fn headers_file(rules: &[Rule<'_>], detach: bool) -> Vec<u8> {
    let mut out = String::new();
    let mut set: Vec<&str> = Vec::new();
    for rule in rules {
        if !out.is_empty() {
            out.push('\n');
        }
        writeln!(out, "{}", rule.path).unwrap();
        for (name, value, _) in &rule.headers {
            if detach && set.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                writeln!(out, "  ! {name}").unwrap();
            }
            writeln!(out, "  {name}: {value}").unwrap();
        }
        set.extend(rule.headers.iter().map(|h| h.0));
    }
    out.into_bytes()
}

fn redirects_file(config: &Config) -> Option<(String, Vec<u8>)> {
    let redirects = &config.host.redirects;
    if redirects.is_empty() {
        return None;
    }
    let mut out = String::new();
    for r in redirects {
        writeln!(out, "{} {} {}", r.from, r.to, r.status).unwrap();
    }
    Some(("_redirects".into(), out.into_bytes()))
}

/// `.vercel/output/config.json`: yuanme's rules and the Author's, then
/// redirects, then `trailingSlash: true` as Vercel compiles it, then the
/// 404 Page for a path no file serves.
fn vercel_config(config: &Config, rules: &[Rule<'_>], not_found: bool) -> String {
    let json = |s: &str| serde_json::to_string(s).expect("a string serializes");
    let mut routes: Vec<String> = Vec::new();
    for rule in rules {
        let headers: Vec<String> = rule
            .headers
            .iter()
            .map(|(n, v, _)| format!("{}: {}", json(n), json(v)))
            .collect();
        routes.push(format!(
            "{{ \"src\": {}, \"headers\": {{ {} }}, \"continue\": true }}",
            json(&regex(rule.path)),
            headers.join(", ")
        ));
    }
    let redirect = |src: &str, to: &str, status: u16| {
        format!(
            "{{ \"src\": {}, \"headers\": {{ \"Location\": {} }}, \"status\": {status} }}",
            json(src),
            json(to)
        )
    };
    for r in &config.host.redirects {
        routes.push(redirect(&regex(&r.from), &r.to, r.status));
    }
    // A file with no extension is only ever in `.well-known`.
    routes.push(format!(
        "{{ \"src\": {} }}",
        json(r"^/\.well-known(?:/.*)?$")
    ));
    routes.push(redirect(r"^/((?:[^/]+/)*[^/\.]+)$", "/$1/", 308));
    routes.push(redirect(r"^/((?:[^/]+/)*[^/]+\.\w+)/$", "/$1", 308));
    routes.push("{ \"handle\": \"filesystem\" }".into());
    if not_found {
        routes.push("{ \"src\": \"/.*\", \"dest\": \"/404.html\", \"status\": 404 }".into());
    }
    format!(
        "{{\n  \"version\": 3,\n  \"routes\": [\n    {}\n  ]\n}}\n",
        routes.join(",\n    ")
    )
}

/// A path with at most one `*`, as an anchored PCRE.
fn regex(path: &str) -> String {
    let mut out = String::from("^");
    for c in path.chars() {
        match c {
            '*' => out.push_str(".*"),
            c if r"\.+?()|[]{}^$".contains(c) => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push('$');
    out
}

/// Where a refresh page that stands in for a redirect from `from` goes, if
/// GitHub Pages would serve it there as HTML: `/a/` is `a/index.html`, and
/// `/a` is `a.html`.
fn stub(from: &str) -> Option<String> {
    let path = &from[1..];
    let name = path.rsplit('/').next().unwrap_or(path);
    if path.is_empty() || path.ends_with('/') {
        Some(format!("{path}index.html"))
    } else if path.ends_with(".html") {
        Some(path.to_string())
    } else if name.contains('.') {
        None
    } else {
        Some(format!("{path}.html"))
    }
}

fn refresh(site: &str, to: &str) -> String {
    let attr = |s: &str| {
        s.replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
    };
    let canonical = match to.starts_with('/') {
        true => format!("{}{to}", site.trim_end_matches('/')),
        false => to.to_string(),
    };
    let (to, canonical) = (attr(to), attr(&canonical));
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>Redirecting</title>\n<link rel=\"canonical\" href=\"{canonical}\">\n<meta name=\"robots\" content=\"noindex\">\n<meta http-equiv=\"refresh\" content=\"0; url={to}\">\n<p><a href=\"{to}\">{to}</a></p>\n"
    )
}

/// The Site's domain, unless GitHub gave it: a `CNAME` names a custom one.
fn custom_domain(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let domain = rest.split(['/', ':']).next().unwrap_or(rest);
    (!domain.is_empty() && !domain.ends_with(".github.io")).then_some(domain)
}

/// Text that Caddy's `precompressed` and nginx's `brotli_static` and
/// `gzip_static` serve from a sibling file.
const TEXT: &[&str] = &[
    ".html",
    ".css",
    ".js",
    ".mjs",
    ".json",
    ".xml",
    ".txt",
    ".svg",
    ".webmanifest",
];

/// A `.br` at quality 11 and a `.gz` beside each text file. Brotli at 11
/// takes about a second per MB, so its output is cached by the input's
/// bytes (ADR 0011).
fn precompressed(files: &BTreeMap<String, Output>, cache: &mut Cache) -> Vec<(String, Vec<u8>)> {
    let text: Vec<(&String, &Arc<[u8]>)> = files
        .iter()
        .filter(|(path, _)| TEXT.iter().any(|e| path.ends_with(e)))
        .filter_map(|(path, output)| match &output.body {
            Body::Bytes(bytes) => Some((path, bytes)),
            _ => None,
        })
        .collect();
    let keys: Vec<blake3::Hash> = text
        .iter()
        .map(|(_, bytes)| {
            let mut key = cache.key("brotli");
            key.update(bytes);
            key.finalize()
        })
        .collect();
    let cached: Vec<Option<Arc<[u8]>>> = keys.iter().map(|k| cache.get("brotli", k)).collect();
    let made: Vec<(Arc<[u8]>, bool, Vec<u8>)> = text
        .par_iter()
        .zip(cached)
        .map(|((_, bytes), cached)| {
            let fresh = cached.is_none();
            let br = cached.unwrap_or_else(|| brotli(bytes).into());
            (br, fresh, gzip(bytes))
        })
        .collect();
    let mut out = Vec::new();
    for (((path, _), key), (br, fresh, gz)) in text.iter().zip(keys).zip(made) {
        if fresh {
            cache.insert("brotli", key, br.clone());
        }
        out.push((format!("{path}.br"), br.to_vec()));
        out.push((format!("{path}.gz"), gz));
    }
    out
}

fn brotli(bytes: &[u8]) -> Vec<u8> {
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        lgwin: 22,
        ..Default::default()
    };
    let mut out = Vec::new();
    brotli::BrotliCompress(&mut &bytes[..], &mut out, &params).expect("writing to memory");
    out
}

/// No file name and a zero time in its header, so the bytes are the same
/// every build.
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(bytes).expect("writing to memory");
    encoder.finish().expect("writing to memory")
}
