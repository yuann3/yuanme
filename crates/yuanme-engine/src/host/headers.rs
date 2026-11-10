//! `_headers` and `_redirects`, which Cloudflare and Netlify both read.

use std::fmt::Write;

use yuanme_content::Config;

use super::Rule;

/// The `_headers` that Cloudflare and Netlify both read. A header an
/// earlier rule set is detached first on Cloudflare, which would otherwise
/// join the two values with a comma; Netlify lets the later rule win, and
/// has no `!` lines.
pub(super) fn headers_file(rules: &[Rule<'_>], detach: bool) -> Vec<u8> {
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

pub(super) fn redirects_file(config: &Config) -> Option<(String, Vec<u8>)> {
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
