//! The URL and normalized-DOM checks on small Sites: what
//! `docs/parity-exceptions.md` says decodes to the same DOM compares equal,
//! everything else is a difference, and each tolerated difference names its
//! Parity exception.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use parity::{Difference, Report, compare, exception_ids};

const EXCEPTIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/parity-exceptions.md");

/// A Site from `=== path` sections.
fn site(spec: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for section in spec.split("=== ").skip(1) {
        let (path, body) = section.split_once('\n').unwrap();
        let path = dir.path().join(path.trim());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    dir
}

fn run(baseline: &str, candidate: &str) -> Report {
    let (b, c) = (site(baseline), site(candidate));
    compare(
        b.path(),
        c.path(),
        &exception_ids(Path::new(EXCEPTIONS)).unwrap(),
    )
    .unwrap()
}

fn page(head: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><title>T</title>{head}</head><body>{body}</body></html>\n"
    )
}

/// The only differences are DOM differences on these Pages.
fn dom_pages(report: &Report) -> Vec<&str> {
    report
        .differences
        .iter()
        .map(|d| match d {
            Difference::Dom { page, .. } => page.as_str(),
            other => panic!("expected only DOM differences, got {other:?}"),
        })
        .collect()
}

#[test]
fn markup_that_decodes_to_the_same_dom_is_equal() {
    let baseline = format!(
        "=== index.html\n{}=== _astro/about.DhiEpsgV.css\np{{color:red}}\n",
        page(
            r#"<meta charset="utf-8"><link rel="stylesheet" href="/_astro/about.DhiEpsgV.css">"#,
            r#"<nav class="rail b a" data-astro-cid-lp6sgjcj="true"><a href="/" data-section="writing">Notes &amp; posts</a><button hidden="true">x</button></nav><p>it&#39;s  two
   spaces</p><!-- kept -->"#,
        )
    );
    let candidate = r#"=== index.html
<!doctype html>
<html lang=en>
  <head>
    <link href="/assets/about-0123456789abcdef.css" rel=stylesheet>
    <meta charset=utf-8>
    <title>T</title>
  </head>
  <body>
    <nav data-y-1a2b3c class="a rail b">
      <a data-section=writing href="/">Notes &#38; posts</a>
      <button hidden>x</button>
    </nav>
    <p>
      it's two spaces
    </p>
  </body>
</html>
=== assets/about-0123456789abcdef.css
p{color:red}
"#;
    let report = run(&baseline, candidate);
    assert_eq!(report.differences, []);
    assert_eq!(report.tolerated.get("assets-dir"), Some(&1));
}

#[test]
fn changed_text_attributes_or_order_are_differences() {
    let body = r#"<nav><a href="/" data-section="writing">Writing</a><span aria-hidden="true">·</span><a href="/">Home</a></nav><p>one two</p><pre>a  b</pre>"#;
    let baseline = format!("=== index.html\n{}", page("", body));
    for changed in [
        body.replace("one two", "one three"),
        body.replace("data-section=\"writing\"", "data-section=\"projects\""),
        // Only HTML's boolean attributes treat "true" as bare.
        body.replace("aria-hidden=\"true\"", "aria-hidden"),
        body.replace(
            r#"<a href="/" data-section="writing">Writing</a><span aria-hidden="true">·</span>"#,
            r#"<span aria-hidden="true">·</span><a href="/" data-section="writing">Writing</a>"#,
        ),
        // Whitespace inside <pre> is kept.
        body.replace("a  b", "a b"),
        body.replace("</nav>", "<a href=\"/\">More</a></nav>"),
    ] {
        assert_ne!(changed, body);
        let report = run(
            &baseline,
            &format!("=== index.html\n{}", page("", &changed)),
        );
        assert_eq!(dom_pages(&report), ["/index.html"], "{changed}");
    }
}

#[test]
fn the_head_compares_stylesheets_in_order_and_the_rest_as_a_set() {
    let css = "=== a.css\na{}\n=== b.css\nb{}\n";
    let head = r#"<meta name="a" content="1"><link rel="stylesheet" href="/a.css"><meta name="b" content="2"><link rel="stylesheet" href="/b.css">"#;
    let baseline = format!("=== index.html\n{}{css}", page(head, ""));
    let metas_swapped = r#"<meta name="b" content="2"><link rel="stylesheet" href="/a.css"><link rel="stylesheet" href="/b.css"><meta name="a" content="1">"#;
    let report = run(
        &baseline,
        &format!("=== index.html\n{}{css}", page(metas_swapped, "")),
    );
    assert_eq!(report.differences, []);

    for changed in [
        r#"<meta name="a" content="1"><link rel="stylesheet" href="/b.css"><meta name="b" content="2"><link rel="stylesheet" href="/a.css">"#,
        r#"<meta name="a" content="1"><link rel="stylesheet" href="/a.css"><link rel="stylesheet" href="/b.css">"#,
    ] {
        let report = run(
            &baseline,
            &format!("=== index.html\n{}{css}", page(changed, "")),
        );
        assert_eq!(dom_pages(&report), ["/index.html"], "{changed}");
    }
}

#[test]
fn highlighted_code_is_compared_by_its_text() {
    let shiki = r#"<pre class="astro-code github-dark" style="background-color:#24292e" tabindex="0" data-language="rust"><code><span class="line"><span style="color:#F97583">fn</span><span style="color:#E1E4E8"> main</span></span>
<span class="line"><span>  x</span></span></code></pre>"#;
    let baseline = format!("=== index.html\n{}", page("", shiki));
    let ferriki = "<pre class=\"y-code\" tabindex=\"0\" data-language=\"rust\"><code><span class=\"k\">fn</span> main\n  x</code></pre>";
    let report = run(&baseline, &format!("=== index.html\n{}", page("", ferriki)));
    assert_eq!(report.differences, []);

    let report = run(
        &baseline,
        &format!(
            "=== index.html\n{}",
            page("", &ferriki.replace("  x", " x"))
        ),
    );
    assert_eq!(dom_pages(&report), ["/index.html"]);
}

#[test]
fn a_missing_or_added_page_is_a_difference() {
    let report = run(
        &format!(
            "=== index.html\n{}=== a/index.html\n{}",
            page("", ""),
            page("", "")
        ),
        &format!(
            "=== index.html\n{}=== b/index.html\n{}",
            page("", ""),
            page("", "")
        ),
    );
    assert_eq!(
        report.differences,
        [
            Difference::Missing("/a/index.html".into()),
            Difference::Added("/b/index.html".into()),
        ]
    );
}

#[test]
fn a_link_to_another_page_is_a_difference() {
    let pages = format!(
        "=== blog/index.html\n{}=== archive/index.html\n{}",
        page("", ""),
        page("", "")
    );
    let report = run(
        &format!(
            "=== index.html\n{}{pages}",
            page("", r#"<a href="/blog/">x</a>"#)
        ),
        &format!(
            "=== index.html\n{}{pages}",
            page("", r#"<a href="/archive/">x</a>"#)
        ),
    );
    assert_eq!(
        report.differences[..2],
        [
            Difference::MissingRef("/blog/".into()),
            Difference::AddedRef("/archive/".into()),
        ]
    );
    assert!(
        matches!(&report.differences[2], Difference::Dom { page, .. } if page == "/index.html")
    );
    assert_eq!(report.differences.len(), 3);
}

#[test]
fn a_reference_the_baseline_resolves_must_resolve() {
    let html = page(
        "",
        r#"<img src="/_astro/p.ChlaUixY_1DHuWQ.jpg" srcset="/_astro/p.ChlaUixY_1DHuWQ.jpg 1x, /_astro/p.ChlaUixY_Z2cmLsz.jpg 2x">"#,
    );
    let one = "=== _astro/p.ChlaUixY_1DHuWQ.jpg\n1\n";
    let two = "=== _astro/p.ChlaUixY_Z2cmLsz.jpg\n2\n";
    let report = run(
        &format!("=== index.html\n{html}{one}{two}"),
        &format!("=== index.html\n{html}{one}"),
    );
    assert_eq!(
        report.differences,
        [Difference::Broken("asset:p.jpg".into())]
    );
}

#[test]
fn tolerated_differences_name_their_exception() {
    let about = format!("=== about/index.html\n{}", page("", ""));
    let report = run(
        &format!(
            "=== index.html\n{}{about}=== fonts/atkinson-regular.woff\nw\n=== ica.pdf\np\n",
            page(
                r#"<link rel="canonical" href="https://www.eyuan.me/"><meta name="generator" content="Astro v5.16.5"><meta name="astro-view-transitions-enabled" content="true">"#,
                r#"<a href="/about">About</a>"#,
            )
        ),
        &format!(
            "=== index.html\n{}{about}=== robots.txt\nUser-agent: *\n",
            page(
                r#"<meta name="generator" content="yuanme 0.0.0"><link rel="canonical" href="https://eyuan.me/">"#,
                r#"<a href="/about/">About</a>"#,
            )
        ),
    );
    // ica.pdf is kept although nothing links to it.
    assert_eq!(report.differences, [Difference::Missing("/ica.pdf".into())]);
    for (id, n) in [
        ("apex-origin", 1),
        ("trailing-slash-hrefs", 1),
        ("head-metas", 3),
        ("robots-txt", 1),
        ("unreferenced-files", 1),
    ] {
        assert_eq!(report.tolerated.get(id), Some(&n), "{id}");
    }
}

#[test]
fn a_rule_whose_exception_is_missing_fails_the_test() {
    let ids: BTreeSet<String> = exception_ids(Path::new(EXCEPTIONS)).unwrap();
    assert!(ids.contains("assets-dir") && ids.contains("film-motion-change"));
    assert!(!ids.contains("Id"));

    let mut without = ids.clone();
    without.remove("apex-origin");
    let s = site(&format!("=== index.html\n{}", page("", "")));
    let err = compare(s.path(), s.path(), &without).unwrap_err();
    assert!(err.contains("apex-origin"), "{err}");
}

#[test]
fn a_url_inside_a_data_url_is_not_a_link() {
    let css = r#"=== a.css
.grain{background-image:url("data:image/svg+xml,%3Csvg%3E%3Crect filter='url(%23n)'/%3E%3C/svg%3E")}
"#;
    let other = r#"=== a.css
.grain{background-image:url(data:image/svg+xml,%3Csvg%3E%3Crect%20filter=%22url(%2523n)%22/%3E%3C/svg%3E)}
"#;
    let html = format!(
        "=== index.html\n{}",
        page(r#"<link rel="stylesheet" href="/a.css">"#, "")
    );
    assert_eq!(
        run(&format!("{html}{css}"), &format!("{html}{other}")).differences,
        []
    );
}

/// The Reference site's port (#75): what the Engine writes in place of
/// Astro's bundles, images and fonts.
#[test]
fn the_engines_assets_stand_in_for_astros() {
    let baseline = format!(
        "=== index.html\n{}=== _astro/about.DhiEpsgV.css\na{{}}\n=== _astro/Shell.astro_astro_type_script_index_0_lang.C3Ek_E9W.js\nx\n=== _astro/park.ChlaUixY_1DHuWQ.jpg\nj\n=== _astro/park.ChlaUixY_Z1CycSd.webp\nw\n",
        page(
            r#"<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link href="https://fonts.googleapis.com/css2?family=Geist&display=swap" rel="stylesheet"><style>.row[data-astro-cid-jqm7f7gd]{color:red}</style><link rel="stylesheet" href="/_astro/about.DhiEpsgV.css"><script type="module" src="/_astro/Shell.astro_astro_type_script_index_0_lang.C3Ek_E9W.js"></script>"#,
            r#"<picture><source srcset="/_astro/park.ChlaUixY_Z1CycSd.webp 480w" type="image/webp"><img src="/_astro/park.ChlaUixY_1DHuWQ.jpg" srcset="/_astro/park.ChlaUixY_1DHuWQ.jpg 480w"></picture><script type="module">document.body.dataset.copy = ''</script>"#,
        )
    );
    let candidate = format!(
        "=== index.html\n{}=== _headers\n/assets/*\n=== assets/global-0123456789abcdef.css\na{{}}\n=== assets/sidecars-0123456789abcdef.css\nb{{}}\n=== assets/page-0123456789abcdef.js\nx\n=== assets/park-480-0123456789abcdef.jpg\nj\n=== assets/geist-latin-0123456789abcdef.woff2\nf\n",
        page(
            r#"<link rel="preload" as="font" type="font/woff2" href="/assets/geist-latin-0123456789abcdef.woff2" crossorigin><link rel="stylesheet" href="/assets/global-0123456789abcdef.css"><link rel="stylesheet" href="/assets/sidecars-0123456789abcdef.css"><script type="module" src="/assets/page-0123456789abcdef.js"></script>"#,
            r#"<picture><img src="/assets/park-480-0123456789abcdef.jpg" srcset="/assets/park-480-0123456789abcdef.jpg 480w"></picture>"#,
        )
    );
    let report = run(&baseline, &candidate);
    assert_eq!(report.differences, []);
    for id in [
        "assets-dir",
        "scoped-styles",
        "single-initial-script",
        "image-formats",
        "self-hosted-fonts",
        "immutable-assets",
    ] {
        assert!(
            report.tolerated.contains_key(id),
            "{id}: {:?}",
            report.tolerated
        );
    }

    // A width is kept apart from the name it ends: a photo isn't its 480w copy's
    // neighbour of another width, and park-2 is not park.
    let other = candidate.replace("park-480-", "park-2-480-");
    assert_eq!(
        run(&baseline, &other).differences[..2],
        [
            Difference::Missing("asset:park.jpg".into()),
            Difference::Added("asset:park-2.jpg".into()),
        ]
    );
    // An image, a classic script and an unhashed stylesheet still count.
    for changed in [
        candidate.replace("<picture>", "<picture><source srcset=\"/assets/park-480-0123456789abcdef.jpg\" type=\"image/avif\">"),
        candidate.replace("</picture>", "</picture><script>boot()</script>"),
        candidate.replace("<link rel=\"preload\"", "<link rel=\"stylesheet\" href=\"https://example.com/x.css\"><link rel=\"preload\""),
    ] {
        assert_eq!(dom_pages(&run(&baseline, &changed)), ["/index.html"], "{changed}");
    }
}

#[test]
fn the_ports_markup_and_content_differences_name_their_exception() {
    let tags = |items: &str| {
        format!(
            "=== tags/index.html\n{}",
            page("", &format!("<ul>{items}</ul>"))
        )
    };
    let (life, c) = (
        r#"<li><a href="/tags/life/">#life</a></li>"#,
        r#"<li><a href="/tags/c/">#c</a></li>"#,
    );
    let pages =
        "=== tags/life/index.html\n<!DOCTYPE html>\n=== tags/c/index.html\n<!DOCTYPE html>\n";
    let baseline = format!(
        "=== index.html\n{}=== 404.html\n{}=== projects/hrust/index.html\n{}{}{pages}",
        page(
            r#"<meta property="og:image" content="https://www.eyuan.me/blog-placeholder-1.jpg"><meta property="twitter:card" content="summary_large_image"><meta property="twitter:image" content="https://www.eyuan.me/blog-placeholder-1.jpg">"#,
            "<p><span>8 min read</span></p>",
        ),
        page(
            r#"<link rel="canonical" href="https://www.eyuan.me/404/"><meta property="og:url" content="https://www.eyuan.me/404"><meta property="twitter:url" content="https://www.eyuan.me/404">"#,
            "",
        ),
        page("", r#"<a href="LICENSE">LICENSE</a>"#),
        tags(&format!("{life}{c}")),
    );
    let candidate = format!(
        "=== index.html\n{}=== 404.html\n{}=== projects/hrust/index.html\n{}{}{pages}",
        page(
            r#"<meta property="twitter:card" content="summary"><link rel="alternate" type="application/rss+xml" title="Eyuan" href="/rss.xml">"#,
            "<p><span>9 min read</span></p>",
        ),
        page("", ""),
        page(
            "",
            r#"<a href="https://github.com/yuann3/http-rust/blob/master/LICENSE">LICENSE</a>"#,
        ),
        tags(&format!("{c}{life}")),
    );
    let report = run(&baseline, &candidate);
    assert_eq!(report.differences, []);
    for id in [
        "og-image",
        "head-metas",
        "reading-time",
        "not-found-canonical",
        "dead-links",
        "collection-order",
    ] {
        assert!(
            report.tolerated.contains_key(id),
            "{id}: {:?}",
            report.tolerated
        );
    }

    for changed in [
        // Only the reading time's number, only the tags index's order, and
        // only the 404 Page's own URL.
        candidate.replace("9 min read", "9 min to read"),
        candidate.replace(&tags(&format!("{c}{life}")), &tags(c)),
        candidate.replace(
            "<title>T</title><meta property=\"twitter:card\"",
            "<title>T</title><link rel=\"canonical\" href=\"https://eyuan.me/\"><meta property=\"twitter:card\"",
        ),
        candidate.replace("http-rust/blob/master/LICENSE", "http-rust"),
    ] {
        assert_ne!(changed, candidate);
        assert!(!run(&baseline, &changed).differences.is_empty(), "{changed}");
    }
    let reordered = baseline.replace(
        "=== index.html\n",
        &format!(
            "=== blog/index.html\n{}=== index.html\n",
            page("", &format!("<ul>{life}{c}</ul>"))
        ),
    );
    let swapped = candidate.replace(
        "=== index.html\n",
        &format!(
            "=== blog/index.html\n{}=== index.html\n",
            page("", &format!("<ul>{c}{life}</ul>"))
        ),
    );
    assert_eq!(dom_pages(&run(&reordered, &swapped)), ["/blog/index.html"]);
}
