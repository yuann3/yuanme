//! The files a rendered Page names (#20): `<link href>` and `<script src>`,
//! found by a tokenizer that knows only as much HTML as that takes.
//! Comments, and the text of `<script>`, `<style>`, `<textarea>` and
//! `<title>`, hold no tags.

use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub struct Reference {
    pub kind: Kind,
    /// The URL, with character references decoded.
    pub url: String,
    /// Where the attribute's value is in the HTML, quotes excluded.
    pub at: Range<usize>,
    /// A `<link>` with `preload` or `modulepreload` among its `rel`s.
    pub preload: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `<link>` with `stylesheet` among its `rel`s.
    Stylesheet,
    /// Any other `<link>`, such as a preload or an icon.
    Link,
    /// `<script type="module" src>`.
    Module,
    /// `<script src>` that runs as a classic script.
    Classic,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Scan {
    pub references: Vec<Reference>,
    /// Where a stylesheet that must come after the Template's own goes:
    /// just after the last `<link rel=stylesheet>`, or else just before
    /// `</head>`. `None` when the Page has neither.
    pub stylesheet_at: Option<usize>,
    /// Every `class` attribute's value, decoded, in order.
    pub classes: Vec<String>,
    /// Each `<style>`, and each `<script>` without `src` that runs.
    pub inline: Vec<Inline>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Inline {
    /// A script, else a stylesheet.
    pub script: bool,
    /// Where its text is.
    pub at: Range<usize>,
}

pub fn scan(html: &str) -> Scan {
    let bytes = html.as_bytes();
    let mut references = Vec::new();
    let mut classes = Vec::new();
    let mut inline = Vec::new();
    let mut last_stylesheet = None;
    let mut head_end = None;
    let mut i = 0;
    while let Some(lt) = memchr(b'<', bytes, i) {
        i = lt + 1;
        if html[i..].starts_with("!--") {
            i = html[i + 3..]
                .find("-->")
                .map_or(bytes.len(), |e| i + 3 + e + 3);
            continue;
        }
        if bytes.get(i) == Some(&b'/') {
            if head_end.is_none() && starts_with_tag(&html[i + 1..], "head") {
                head_end = Some(lt);
            }
            continue;
        }
        if !bytes.get(i).is_some_and(u8::is_ascii_alphabetic) {
            continue;
        }
        let name_end = html[i..]
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .map_or(bytes.len(), |e| i + e);
        let name = html[i..name_end].to_ascii_lowercase();
        let (attributes, end) = attributes(html, name_end);
        i = end;
        let get = |name: &str| attributes.iter().find(|a| a.name == name);
        if let Some(class) = get("class") {
            classes.push(decode(class.value(html)));
        }
        // Whether the element's text is a script, else a stylesheet.
        let mut text_is = None;
        match name.as_str() {
            "link" => {
                let rels = get("rel").map_or(String::new(), |a| a.value(html).to_ascii_lowercase());
                let has = |rel: &str| rels.split_ascii_whitespace().any(|r| r == rel);
                let stylesheet = has("stylesheet");
                let preload = has("preload") || has("modulepreload");
                if stylesheet {
                    last_stylesheet = Some(end);
                }
                if let Some(href) = get("href") {
                    let kind = if stylesheet {
                        Kind::Stylesheet
                    } else {
                        Kind::Link
                    };
                    references.push(Reference {
                        preload,
                        ..href.reference(html, kind)
                    });
                }
            }
            "script" => {
                let ty = get("type")
                    .map_or(String::new(), |a| a.value(html).trim().to_ascii_lowercase());
                let kind = match ty.as_str() {
                    "module" => Some(Kind::Module),
                    "" => Some(Kind::Classic),
                    _ if ty.contains("javascript") || ty.contains("ecmascript") => {
                        Some(Kind::Classic)
                    }
                    // A data block, such as JSON-LD, which the browser
                    // never fetches.
                    _ => None,
                };
                match (kind, get("src")) {
                    (Some(kind), Some(src)) => references.push(src.reference(html, kind)),
                    (Some(_), None) => text_is = Some(true),
                    (None, _) => {}
                }
            }
            "style" => text_is = Some(false),
            _ => {}
        }
        if ["script", "style", "textarea", "title"].contains(&name.as_str()) {
            let start = i;
            i = closing(html, i, &name);
            if let Some(script) = text_is {
                // Just before `</`, unless the element is never closed.
                let end = if i < bytes.len() { i - 1 } else { i };
                inline.push(Inline {
                    script,
                    at: start..end,
                });
            }
        }
    }
    Scan {
        references,
        stylesheet_at: last_stylesheet.or(head_end),
        classes,
        inline,
    }
}

struct Attribute {
    name: String,
    value: Range<usize>,
}

impl Attribute {
    fn value<'h>(&self, html: &'h str) -> &'h str {
        &html[self.value.clone()]
    }

    fn reference(&self, html: &str, kind: Kind) -> Reference {
        Reference {
            kind,
            url: decode(self.value(html)),
            at: self.value.clone(),
            preload: false,
        }
    }
}

/// The attributes of the tag whose name ends at `i`, and where the tag
/// ends. The first of two with the same name wins, as in browsers.
fn attributes(html: &str, mut i: usize) -> (Vec<Attribute>, usize) {
    let bytes = html.as_bytes();
    let mut out: Vec<Attribute> = Vec::new();
    loop {
        while bytes
            .get(i)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == b'/')
        {
            i += 1;
        }
        match bytes.get(i) {
            None => return (out, i),
            Some(b'>') => return (out, i + 1),
            _ => {}
        }
        let start = i;
        i += 1;
        while bytes
            .get(i)
            .is_some_and(|b| !(b.is_ascii_whitespace() || b"/>=".contains(b)))
        {
            i += 1;
        }
        let name = html[start..i].to_ascii_lowercase();
        let mut j = i;
        while bytes.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        let value = if bytes.get(j) == Some(&b'=') {
            j += 1;
            while bytes.get(j).is_some_and(u8::is_ascii_whitespace) {
                j += 1;
            }
            i = j;
            match bytes.get(j) {
                Some(&q @ (b'"' | b'\'')) => {
                    let end = memchr(q, bytes, j + 1).unwrap_or(bytes.len());
                    i = (end + 1).min(bytes.len());
                    j + 1..end
                }
                _ => {
                    while bytes
                        .get(i)
                        .is_some_and(|b| !(b.is_ascii_whitespace() || *b == b'>'))
                    {
                        i += 1;
                    }
                    j..i
                }
            }
        } else {
            i..i
        };
        if !out.iter().any(|a| a.name == name) {
            out.push(Attribute { name, value });
        }
    }
}

/// Just past the end tag of the raw text element `name` whose text starts
/// at `i`.
fn closing(html: &str, mut i: usize, name: &str) -> usize {
    while let Some(lt) = memchr(b'<', html.as_bytes(), i) {
        i = lt + 1;
        if html[i..].starts_with('/') && starts_with_tag(&html[i + 1..], name) {
            return i;
        }
    }
    html.len()
}

/// Whether `text` starts with the tag name `name`, in any case.
fn starts_with_tag(text: &str, name: &str) -> bool {
    text.get(..name.len())
        .is_some_and(|t| t.eq_ignore_ascii_case(name))
        && text[name.len()..]
            .bytes()
            .next()
            .is_none_or(|b| b.is_ascii_whitespace() || b == b'/' || b == b'>')
}

fn memchr(needle: u8, bytes: &[u8], from: usize) -> Option<usize> {
    bytes
        .get(from..)?
        .iter()
        .position(|&b| b == needle)
        .map(|p| from + p)
}

/// Character references in an attribute value: the ones an escaping
/// Template writes, and numeric ones. Others stay as written.
fn decode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.as_bytes()[..rest.len().min(12)]
            .iter()
            .position(|&b| b == b';')
        else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..semi];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| name.strip_prefix('#').map(str::parse))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A URL a Page links to or loads, from any element's `href`, `src` or
/// `poster`, or one candidate of a `srcset` or `imagesrcset`.
#[derive(Debug, PartialEq, Eq)]
pub struct Url {
    /// With character references decoded.
    pub url: String,
    /// Where it is in the HTML, as written.
    pub at: Range<usize>,
    /// Not an `href` the visitor follows, as `<a>`'s is: any `src`,
    /// `srcset` or `poster`, and `<link href>`, which an icon is.
    pub loaded: bool,
}

/// Every URL in `html` that a browser follows or fetches, for the static
/// output checks (#15). `<base>` is not read.
pub fn urls(html: &str) -> Vec<Url> {
    let bytes = html.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(lt) = memchr(b'<', bytes, i) {
        i = lt + 1;
        if html[i..].starts_with("!--") {
            i = html[i + 3..]
                .find("-->")
                .map_or(bytes.len(), |e| i + 3 + e + 3);
            continue;
        }
        if !bytes.get(i).is_some_and(u8::is_ascii_alphabetic) {
            continue;
        }
        let name_end = html[i..]
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .map_or(bytes.len(), |e| i + e);
        let name = html[i..name_end].to_ascii_lowercase();
        let (attributes, end) = attributes(html, name_end);
        i = end;
        // A data block's `src` is never fetched.
        let data_block = name == "script"
            && attributes.iter().any(|a| {
                a.name == "type" && {
                    let ty = a.value(html).trim().to_ascii_lowercase();
                    !["", "module"].contains(&ty.as_str())
                        && !ty.contains("javascript")
                        && !ty.contains("ecmascript")
                }
            });
        for a in &attributes {
            match a.name.as_str() {
                "href" | "src" | "poster" if !data_block => out.push(Url {
                    url: decode(a.value(html)),
                    at: a.value.clone(),
                    loaded: a.name != "href" || name == "link",
                }),
                "srcset" | "imagesrcset" => {
                    let mut at = a.value.start;
                    for candidate in a.value(html).split(',') {
                        let lead = candidate.len() - candidate.trim_start().len();
                        let url = candidate.trim_start().split_ascii_whitespace().next();
                        if let Some(url) = url {
                            let start = at + lead;
                            out.push(Url {
                                url: decode(url),
                                at: start..start + url.len(),
                                loaded: true,
                            });
                        }
                        at += candidate.len() + 1;
                    }
                }
                _ => {}
            }
        }
        if ["script", "style", "textarea", "title"].contains(&name.as_str()) {
            i = closing(html, i, &name);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(html: &str) -> Vec<(Kind, String, &str)> {
        scan(html)
            .references
            .into_iter()
            .map(|r| (r.kind, r.url, &html[r.at]))
            .collect()
    }

    #[test]
    fn links_and_scripts_are_found_with_their_kind_and_place() {
        let html = r#"<!doctype html><HTML><head>
<LINK REL="Preload Stylesheet" href='/a.css'><link rel=icon href=/i.svg>
<link rel="alternate stylesheet" href="/b.css?x=1&amp;y=&#x27;2&#39;">
<script type=module src="/m.ts"></script><script src="/c.js" type="text/javascript"></script>
<script type="application/ld+json" src="/d.json"></script><script>let s = '<link rel=stylesheet href=/no.css>'</script>
<!-- <link rel=stylesheet href=/no.css> --><style>a{}</style><title><link href=/no></title>
<link href="/e.css" href="/no.css" rel=stylesheet><link rel=stylesheet>
</head><body><textarea></textarea><link rel=preload href = "/f.woff2"></body></html>"#;
        assert_eq!(
            refs(html),
            [
                (Kind::Stylesheet, "/a.css".into(), "/a.css"),
                (Kind::Link, "/i.svg".into(), "/i.svg"),
                (
                    Kind::Stylesheet,
                    "/b.css?x=1&y='2'".into(),
                    "/b.css?x=1&amp;y=&#x27;2&#39;"
                ),
                (Kind::Module, "/m.ts".into(), "/m.ts"),
                (Kind::Classic, "/c.js".into(), "/c.js"),
                (Kind::Stylesheet, "/e.css".into(), "/e.css"),
                (Kind::Link, "/f.woff2".into(), "/f.woff2"),
            ]
        );
    }

    #[test]
    fn preloads_and_the_text_of_inline_styles_and_scripts_are_found() {
        let html = r#"<head><link rel="Preload" href=/f.woff2 as=font><link rel=modulepreload href=/c.js><link rel=icon href=/i.svg>
<style media=print>a{}</style><script>let a = "</scrip";</script><SCRIPT TYPE=module>import "/x.js"</SCRIPT>
<script type="application/ld+json">{}</script><script type=importmap>{}</script><script src=/s.js>ignored</script></head>"#;
        let scan = scan(html);
        let preloads: Vec<&str> = scan
            .references
            .iter()
            .filter(|r| r.preload)
            .map(|r| r.url.as_str())
            .collect();
        assert_eq!(preloads, ["/f.woff2", "/c.js"]);
        let inline: Vec<(bool, &str)> = scan
            .inline
            .iter()
            .map(|i| (i.script, &html[i.at.clone()]))
            .collect();
        assert_eq!(
            inline,
            [
                (false, "a{}"),
                (true, "let a = \"</scrip\";"),
                (true, "import \"/x.js\"")
            ]
        );
    }

    #[test]
    fn every_url_a_browser_follows_or_fetches_is_found_where_it_is_written() {
        let html = r#"<head><link rel=canonical href="https://e.com/a/"><script type=module src=/m.ts></script>
<script type="application/json" src="/data.json"></script><!-- <a href="/no/"> --><style>a{background:url(/no.png)}</style>
</head><body><A HREF='/b?x=1&amp;y=2#top'>b</A><img src="/c.png" srcset="/c-1x.png 1x,  /c-2x.png 2x" alt="">
<video poster=/v.jpg><source srcset="/s.avif"></video><textarea><a href="/no/"></textarea><a>none</a></body>"#;
        let found = urls(html);
        let followed: Vec<&str> = found
            .iter()
            .filter(|u| !u.loaded)
            .map(|u| u.url.as_str())
            .collect();
        assert_eq!(followed, ["/b?x=1&y=2#top"]);
        let found: Vec<(String, &str)> = found.into_iter().map(|u| (u.url, &html[u.at])).collect();
        let want = [
            ("https://e.com/a/", "https://e.com/a/"),
            ("/m.ts", "/m.ts"),
            ("/b?x=1&y=2#top", "/b?x=1&amp;y=2#top"),
            ("/c.png", "/c.png"),
            ("/c-1x.png", "/c-1x.png"),
            ("/c-2x.png", "/c-2x.png"),
            ("/v.jpg", "/v.jpg"),
            ("/s.avif", "/s.avif"),
        ];
        let want: Vec<(String, &str)> = want.iter().map(|(u, at)| (u.to_string(), *at)).collect();
        assert_eq!(found, want);
    }

    #[test]
    fn a_later_stylesheet_goes_after_the_last_link_or_else_before_the_head_ends() {
        let html = "<head><link rel=stylesheet href=/a.css><link rel=stylesheet href=/b.css>\n<meta x></head>";
        let at = scan(html).stylesheet_at.unwrap();
        assert_eq!(
            &html[..at],
            "<head><link rel=stylesheet href=/a.css><link rel=stylesheet href=/b.css>"
        );
        let html = "<head><title>T</title><link rel=icon href=/i.svg></HEAD><body></body>";
        assert_eq!(
            &html[scan(html).stylesheet_at.unwrap()..],
            "</HEAD><body></body>"
        );
        // `</header>` is not `</head>`.
        assert_eq!(scan("<body><header></header>").stylesheet_at, None);
    }

    /// The class tokens Tailwind is fed (#59).
    #[test]
    fn every_class_attribute_is_found_decoded_outside_comments_and_raw_text() {
        let html = r#"<head><title><i class=no></title><style>.no{}</style></head>
<body CLASS="page  dark:bg-black"><div class='px-4 [&amp;>p]:mt-2' class=no><!-- <b class=no> -->
<svg class=icon><use href=#i /></svg><script>let s = '<b class=no>'</script><p class=>x</p></body>"#;
        assert_eq!(
            scan(html).classes,
            ["page  dark:bg-black", "px-4 [&>p]:mt-2", "icon", ""]
        );
    }

    #[test]
    fn a_tag_cut_off_by_the_end_of_the_page_ends_there() {
        for html in [
            "<link href=\"/a.css",
            "<link href=",
            "<script src=/a.js",
            "<!-- <link",
            "<",
            "<a",
        ] {
            scan(html);
        }
        assert_eq!(
            refs("<link href=\"/a.css"),
            [(Kind::Link, "/a.css".into(), "/a.css")]
        );
    }
}
