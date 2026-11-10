//! The github-pages adapter, which can't send headers or redirect: a
//! refresh page stands in for each redirect it can serve as HTML.

/// Where a refresh page that stands in for a redirect from `from` goes, if
/// GitHub Pages would serve it there as HTML: `/a/` is `a/index.html`, and
/// `/a` is `a.html`.
pub(super) fn stub(from: &str) -> Option<String> {
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

pub(super) fn refresh(site: &str, to: &str) -> String {
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
pub(super) fn custom_domain(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let domain = rest.split(['/', ':']).next().unwrap_or(rest);
    (!domain.is_empty() && !domain.ends_with(".github.io")).then_some(domain)
}
