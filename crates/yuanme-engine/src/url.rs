//! URLs as the Site's files name each other: paths from the root,
//! percent-decoded and normalized as a browser resolves them.

/// The `?query#fragment` of a URL, kept after its path is replaced.
pub(crate) fn suffix(url: &str) -> &str {
    url.find(['?', '#']).map_or("", |i| &url[i..])
}

pub(crate) fn has_scheme(url: &str) -> bool {
    let Some(colon) = url.find(':') else {
        return false;
    };
    let scheme = &url[..colon];
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

/// `/a/./b/../c` → `/a/c`. A `..` above the root stays at the root, as
/// browsers resolve it.
pub(crate) fn normalize(path: &str) -> String {
    let segments: Vec<&str> = path[1..].split('/').collect();
    let mut out: Vec<&str> = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        match *segment {
            "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
        if last && matches!(*segment, "." | "..") {
            out.push("");
        }
    }
    format!("/{}", out.join("/"))
}

pub(crate) fn percent_decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = path.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The URL a visitor loads a file at.
pub(crate) fn url_of(path: &str) -> String {
    match path.strip_suffix("index.html") {
        Some(dir) if dir.is_empty() || dir.ends_with('/') => format!("/{dir}"),
        _ => format!("/{path}"),
    }
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
