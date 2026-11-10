//! `[host]`: the adapter, headers and redirects.

use std::ops::Range;

use super::check::Checker;
use super::{Table, as_str, as_table, boolean, entries, int, items, required, string, table};

/// The static host the Site is deployed to (#25).
#[derive(Debug)]
pub struct Host {
    pub adapter: Adapter,
    /// Where `adapter` is set, or else nowhere: `0..0`.
    pub adapter_at: Range<usize>,
    /// Only with the `static` adapter.
    pub precompress: bool,
    /// In the order written: where two set one header, the later one wins.
    pub headers: Vec<HeaderRule>,
    pub redirects: Vec<Redirect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adapter {
    /// Workers static assets.
    Cloudflare,
    Netlify,
    /// The Build Output API.
    Vercel,
    GithubPages,
    CloudflarePages,
    /// Nothing host-specific, for Caddy, nginx or any other server.
    Static,
}

impl Adapter {
    pub const ALL: [Adapter; 6] = [
        Adapter::Cloudflare,
        Adapter::Netlify,
        Adapter::Vercel,
        Adapter::GithubPages,
        Adapter::CloudflarePages,
        Adapter::Static,
    ];

    /// The adapter `[host] adapter` names.
    pub fn named(name: &str) -> Option<Adapter> {
        Adapter::ALL.into_iter().find(|a| a.name() == name)
    }

    /// As `[host] adapter` names it.
    pub fn name(self) -> &'static str {
        match self {
            Adapter::Cloudflare => "cloudflare",
            Adapter::Netlify => "netlify",
            Adapter::Vercel => "vercel",
            Adapter::GithubPages => "github-pages",
            Adapter::CloudflarePages => "cloudflare-pages",
            Adapter::Static => "static",
        }
    }

    /// Where the build is written unless the Author says otherwise:
    /// `vercel deploy --prebuilt` reads `.vercel/output/` in the project.
    pub fn out_dir(self) -> &'static str {
        match self {
            Adapter::Vercel => ".vercel/output",
            _ => "dist",
        }
    }
}

/// `[[host.headers]]`.
#[derive(Debug)]
pub struct HeaderRule {
    /// A URL path, with at most one `*`.
    pub path: String,
    pub path_at: Range<usize>,
    /// By name, as written.
    pub headers: Vec<(String, String)>,
}

/// `[[host.redirects]]`.
#[derive(Debug)]
pub struct Redirect {
    pub from: String,
    pub from_at: Range<usize>,
    /// A URL path or an absolute URL.
    pub to: String,
    pub to_at: Range<usize>,
    pub status: u16,
}

pub(super) fn read(host: Option<&Table<'_>>, checker: &mut Checker) -> Host {
    let Some(host) = host else {
        return Host {
            adapter: Adapter::Cloudflare,
            adapter_at: 0..0,
            precompress: false,
            headers: Vec::new(),
            redirects: Vec::new(),
        };
    };
    let adapter = match string(host, "adapter") {
        None => Adapter::Cloudflare,
        Some(name) => Adapter::named(&name).expect("checked as a choice"),
    };
    let precompress = boolean(host, "precompress");
    if precompress && adapter != Adapter::Static {
        checker.invalid(
            format!(
                "`host.precompress` needs the `static` adapter: {} doesn't serve pre-compressed files",
                adapter.name()
            ),
            host["precompress"].span(),
        );
    }
    let tables = |key: &str| host.get(key).map_or(&[][..], items);
    let headers = tables("headers")
        .iter()
        .map(|rule| header_rule(as_table(rule), checker))
        .collect();
    let mut redirects: Vec<Redirect> = Vec::new();
    for redirect in tables("redirects") {
        let redirect = redirect_rule(as_table(redirect), &redirects, checker);
        redirects.push(redirect);
    }
    Host {
        adapter,
        adapter_at: host.get("adapter").map_or(0..0, |a| a.span()),
        precompress,
        headers,
        redirects,
    }
}

/// `[[host.headers]]`.
fn header_rule(rule: &Table<'_>, checker: &mut Checker) -> HeaderRule {
    let path = &rule["path"];
    let path_at = path.span();
    let path = as_str(path).to_string();
    if let Some(why) = path_problem(&path, true) {
        checker.invalid(format!("`{path}` {why}"), path_at.clone());
    }
    let headers: Vec<(String, String)> = entries(table(rule, "headers"))
        .iter()
        .map(|(name, value)| {
            let name_text = name.get_ref().to_string();
            let value_text = as_str(value).to_string();
            if name_text.is_empty()
                || !name_text
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            {
                let message = format!(
                    "`{name_text}` is not a header name: those are letters, digits and `-`"
                );
                checker.invalid(message, name.span());
            } else if value_text.chars().any(char::is_control) {
                let message = format!(
                    "the value of `{name_text}` holds a control character, such as a line break"
                );
                checker.invalid(message, value.span());
            }
            (name_text, value_text)
        })
        .collect();
    if headers.is_empty() {
        checker.invalid(
            "a header rule needs at least one header".into(),
            rule["headers"].span(),
        );
    }
    HeaderRule {
        path,
        path_at,
        headers,
    }
}

/// `[[host.redirects]]`; `before` are the ones above it.
fn redirect_rule(redirect: &Table<'_>, before: &[Redirect], checker: &mut Checker) -> Redirect {
    let from = required(redirect, "from");
    let from_at = redirect["from"].span();
    let to = required(redirect, "to");
    let to_at = redirect["to"].span();
    if let Some(why) = path_problem(&from, false) {
        checker.invalid(format!("`{from}` {why}"), from_at.clone());
    } else if before.iter().any(|r| r.from == from) {
        checker.invalid(format!("`{from}` is redirected twice"), from_at.clone());
    }
    let absolute = to.starts_with("https://") || to.starts_with("http://");
    if !absolute && !to.starts_with('/') {
        let message = format!(
            "`{to}` must be a URL path from the Site root, such as \"/blog/\", or an absolute URL"
        );
        checker.invalid(message, to_at.clone());
    } else if !absolute && to.contains(|c: char| c.is_whitespace() || ":*".contains(c)) {
        let message =
            format!("`{to}` can't hold whitespace, `:` or `*`: hosts read them differently");
        checker.invalid(message, to_at.clone());
    } else if to == from {
        checker.invalid(format!("`{from}` redirects to itself"), to_at.clone());
    }
    let status = match redirect.get("status") {
        None => 308,
        Some(status) => {
            let n = int(status);
            if ![301, 302, 307, 308].contains(&n) {
                checker.invalid(
                    format!("a redirect's status is 301, 302, 307 or 308, not {n}"),
                    status.span(),
                );
            }
            n as u16
        }
    };
    Redirect {
        from,
        from_at,
        to,
        to_at,
        status,
    }
}

/// What makes `path` mean different things to different hosts, if anything.
/// Header rules may hold one `*`, as every host reads it; redirects match
/// one URL, so a GitHub Pages stub can stand in for them.
fn path_problem(path: &str, splat: bool) -> Option<&'static str> {
    if path.contains(|c: char| c.is_whitespace() || "?#:".contains(c)) {
        return Some("can't hold whitespace, `?`, `#` or `:`: hosts read them differently");
    }
    match path.matches('*').count() {
        0 => None,
        1 if splat => None,
        _ if splat => Some("can hold only one `*`"),
        _ => Some("can't hold `*`: a redirect is from one URL"),
    }
}
