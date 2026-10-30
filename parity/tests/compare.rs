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
