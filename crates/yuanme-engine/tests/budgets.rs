//! The Site budgets (#15, ADR 0004), measured through the Engine's one
//! seam. Each expected number is worked out here from the built files with
//! the `brotli` crate, and each Budget is tried at its number and one byte,
//! or one request, under it, and far under it, where it is estimated (#91).

use std::collections::BTreeSet;

use regex::Regex;
use yuanme_content::config::Budget;
use yuanme_engine::{Body, Build, Cache, Inputs, Measured, Mode, build};

/// Bytes of Brotli at quality 11, as #15 counts them.
fn br(bytes: &[u8]) -> u64 {
    brotli_at(11, bytes)
}

/// Bytes of Brotli at quality 5, which a number under half its limit is
/// estimated with (#91).
fn estimate(bytes: &[u8]) -> u64 {
    brotli_at(5, bytes)
}

fn brotli_at(quality: i32, bytes: &[u8]) -> u64 {
    let params = brotli::enc::BrotliEncoderParams {
        quality,
        lgwin: 22,
        ..Default::default()
    };
    let mut out = Vec::new();
    brotli::BrotliCompress(&mut &bytes[..], &mut out, &params).unwrap();
    out.len() as u64
}

/// Text that doesn't compress, from a fixed seed.
fn noise(seed: u64, len: usize) -> String {
    let mut x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (0..len)
        .map(|_| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            b"abcdefghijklmnopqrstuvwxyz0123456789"[(x >> 33) as usize % 36] as char
        })
        .collect()
}

const PAGE: &str = r#"<!doctype html>
<html><head>
<link rel="stylesheet" href="/styles/site.css">
<link rel="stylesheet" href="https://cdn.example.com/x.css">
<link rel="preload" href="/fonts/text.woff2" as="font" crossorigin>
<link rel="icon" href="/favicon.svg">
<style>body{margin:0}</style>
<script>window.page = 1</script>
<script type="application/ld+json">{"@type": "BlogPosting"}</script>
<script type="module" src="/scripts/a.ts"></script>
<script type="module" src="/scripts/b.ts"></script>
</head><body>{{ entry.content }}</body></html>
"#;

/// One Entry's Page that has something of every kind a Budget counts.
fn project(budgets: &str) -> Inputs {
    let mut inputs = Inputs::default();
    // First, where `budgets = false` must go.
    let config = format!(
        "{budgets}\n[site]\nurl = \"https://e.com\"\n\n[collections.posts]\nroute = \"/blog/{{slug}}/\"\n"
    );
    // A body of more than half the HTML budget, which isn't the HTML's.
    let body = format!(
        "---\ntitle: Sea\n---\nThe sea is $x^2 + y_1$ deep.\n\n{}\n",
        noise(1, 12000)
    );
    let css = format!(
        "@font-face {{ font-family: Text; src: url(\"/fonts/text.woff2\") format(\"woff2\"); }}\nbody {{ font-family: Text, serif; }}\n.n::after {{ content: \"{}\"; }}\n",
        noise(2, 600)
    );
    let shared = format!(
        "export function shared(name: string): void {{\n  document.body.dataset[name] = \"{}\";\n}}\n",
        noise(3, 900)
    );
    let font: Vec<u8> = noise(4, 2000).into_bytes();
    let files: [(&str, &[u8]); 9] = [
        ("yuanme.toml", config.as_bytes()),
        ("templates/posts/entry.html", PAGE.as_bytes()),
        ("content/posts/sea.md", body.as_bytes()),
        ("styles/site.css", css.as_bytes()),
        ("fonts/text.woff2", &font),
        (
            "static/favicon.svg",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        ),
        (
            "scripts/a.ts",
            b"import { shared } from \"./shared\";\nshared(\"a\");\n",
        ),
        (
            "scripts/b.ts",
            b"import { shared } from \"./shared\";\nshared(\"b\");\n",
        ),
        ("scripts/shared.ts", shared.as_bytes()),
    ];
    for (path, bytes) in files {
        inputs.insert(path, bytes.to_vec());
    }
    inputs
}

fn run(inputs: &Inputs) -> Build {
    let build = build(inputs, &mut Cache::memory("1.0.0"), Mode::Build);
    let errors: Vec<_> = build
        .diagnostics
        .iter()
        .filter(|d| !d.code.name.starts_with("budget/"))
        .map(|d| d.render(&build.sources, false))
        .collect();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    build
}

fn text<'b>(build: &'b Build, path: &str) -> &'b str {
    match &build.files[path.trim_start_matches('/')].body {
        Body::Bytes(bytes) => std::str::from_utf8(bytes).unwrap(),
        _ => panic!("{path} is held in memory"),
    }
}

fn file(build: &Build, path: &str, size: fn(&[u8]) -> u64) -> u64 {
    match &build.files[path.trim_start_matches('/')].body {
        Body::Bytes(bytes) => size(bytes),
        _ => panic!("{path} is held in memory"),
    }
}

fn all(re: &str, text: &str) -> Vec<String> {
    Regex::new(re)
        .unwrap()
        .captures_iter(text)
        .map(|c| c[1].to_string())
        .collect()
}

fn page<'b>(build: &'b Build, url: &str) -> &'b Measured {
    build
        .budgets
        .iter()
        .find(|m| m.url == url)
        .unwrap_or_else(|| panic!("{url} wasn't measured"))
}

/// Each Budget's number for `/blog/sea/`, from its files, each measured
/// in bytes by `size`.
fn expected(build: &Build, size: fn(&[u8]) -> u64) -> ([(Budget, u64); 6], u64) {
    let html = text(build, "blog/sea/index.html");
    let (head, rest) = html.split_once("<body>").unwrap();
    let (body, tail) = rest.split_once("</body>").unwrap();
    let first_flight = format!("{head}<body></body>{tail}");

    let stylesheets = all(r#"<link rel="stylesheet" href="(/assets/[^"]+)""#, html);
    assert_eq!(stylesheets.len(), 2, "site.css and math.css: {html}");
    let modules = all(r#"<script type="module" src="(/assets/[^"]+)""#, html);
    let preloaded = all(r#"<link rel="modulepreload" href="(/assets/[^"]+)""#, html);
    assert_eq!((modules.len(), preloaded.len()), (2, 1), "{html}");
    let font_urls = |css: &str| all(r#"url\("?(/assets/[^")]+\.woff2)"?\)"#, text(build, css));
    let [text_font] = font_urls(&stylesheets[0]).try_into().unwrap();
    let [math_font] = font_urls(&stylesheets[1]).try_into().unwrap();
    assert!(math_font.contains("math"), "{math_font}");

    let css: u64 = stylesheets
        .iter()
        .map(|s| file(build, s, size))
        .sum::<u64>()
        + size(b"body{margin:0}");
    let js: u64 = modules
        .iter()
        .chain(&preloaded)
        .map(|s| file(build, s, size))
        .sum::<u64>()
        + size(b"window.page = 1");
    // The Page, 2 stylesheets and the one from the CDN, 2 Scripts and the
    // chunk they share, and 2 fonts: the preload is the font's own URL.
    let requests = 1 + 3 + 3 + 2;
    let numbers = [
        (Budget::Html, size(first_flight.as_bytes())),
        (Budget::Css, css),
        (Budget::Js, js),
        (Budget::Fonts, file(build, &text_font, size)),
        (Budget::MathFont, file(build, &math_font, size)),
        (Budget::Requests, requests),
    ];
    (numbers, size(body.as_bytes()))
}

/// `[budgets]` with each limit given in kB, as an Author writes it.
fn limits(numbers: &[(Budget, u64)], less: u64) -> String {
    let mut out = String::from("\n[budgets]\n");
    for (budget, n) in numbers {
        let n = n - less;
        let value = match budget {
            Budget::Requests => n.to_string(),
            _ => (n as f64 / 1000.0).to_string(),
        };
        out.push_str(&format!("{} = {value}\n", budget.key()));
    }
    out
}

fn budget_codes(build: &Build) -> Vec<&'static str> {
    build
        .diagnostics
        .iter()
        .map(|d| d.code.name)
        .filter(|c| c.starts_with("budget/"))
        .collect()
}

#[test]
fn every_budget_is_measured_as_brotli_and_warns_one_byte_over() {
    let build = run(&project("budgets = false\n"));
    assert!(
        build.budgets.is_empty(),
        "`budgets = false` measures nothing"
    );
    let (numbers, body) = expected(&build, br);

    // Under half the Starters' limits, each number is an estimate (#91).
    let starters = run(&project(""));
    let (estimates, estimated_body) = expected(&starters, estimate);
    assert_ne!(
        (estimates, estimated_body),
        (numbers, body),
        "an estimate differs from Brotli at quality 11"
    );
    let measured = page(&starters, "/blog/sea/");
    let got: Vec<(Budget, u64)> = measured.values.iter().map(|(b, n)| (*b, *n)).collect();
    assert_eq!(got, estimates);
    assert_eq!(measured.body, Some(estimated_body));
    let bytes: BTreeSet<Budget> = Budget::ALL
        .into_iter()
        .filter(|b| *b != Budget::Requests)
        .collect();
    assert_eq!(measured.estimated, bytes);

    let at = run(&project(&limits(&numbers, 0)));
    let measured = page(&at, "/blog/sea/");
    let got: Vec<(Budget, u64)> = measured.values.iter().map(|(b, n)| (*b, *n)).collect();
    assert_eq!(got, numbers);
    assert_eq!(measured.body, Some(body), "the body is reported");
    assert!(
        measured.estimated.is_empty(),
        "every number is at its limit"
    );
    assert_eq!(budget_codes(&at), [] as [&str; 0], "a Page at its limits");
    assert_eq!(at.budgets.len(), 1, "only HTML Pages are measured");

    let over = run(&project(&limits(&numbers, 1)));
    assert_eq!(
        budget_codes(&over),
        [
            "budget/html",
            "budget/css",
            "budget/js",
            "budget/fonts",
            "budget/math-font",
            "budget/requests"
        ]
    );
    for d in &over.diagnostics {
        assert_eq!(
            d.level,
            yuanme_diag::Level::Warning,
            "only strict mode fails a build"
        );
        assert!(d.message.contains("`/blog/sea/`"), "{}", d.message);
    }
    insta::assert_snapshot!(
        over.diagnostics
            .iter()
            .map(|d| d.render(&over.sources, false))
            .collect::<Vec<_>>()
            .join("\n\n")
    );

    // An Acknowledged overage of one byte, or one request, silences each;
    // one for a Page the Site doesn't have is a mistake.
    let mut config = limits(&numbers, 1);
    config.push_str("\n[budgets.overages.\"/blog/sea/\"]\n");
    for (budget, _) in &numbers {
        let one = if *budget == Budget::Requests {
            "1"
        } else {
            "0.001"
        };
        config.push_str(&format!("{} = {one}\n", budget.key()));
    }
    config.push_str("\n[budgets.overages.\"/blog/seas/\"]\njs = 1\n");
    let acknowledged = run(&project(&config));
    assert_eq!(budget_codes(&acknowledged), ["budget/unknown-page"]);
    let d = &acknowledged.diagnostics[0];
    assert_eq!(
        &acknowledged.sources.text(d.primary().file)[d.primary().range.clone()],
        "\"/blog/seas/\""
    );
    assert!(
        d.children
            .iter()
            .any(|c| c.message.contains("`/blog/sea/`")),
        "{d:#?}"
    );
}

/// An inline script whose Brotli is exactly `size` bytes. Noise grows its
/// Brotli by under a byte a character, so some length of it hits `size`.
fn script_of(size: u64) -> String {
    let code = |len| format!("// {}", noise(5, len));
    let (mut low, mut high) = (0, size as usize * 2);
    while low < high {
        let mid = (low + high) / 2;
        match br(code(mid).as_bytes()) < size {
            true => low = mid + 1,
            false => high = mid,
        }
    }
    let code = (low..low + 8)
        .map(code)
        .find(|c| br(c.as_bytes()) == size)
        .expect("noise of some length");
    format!("<script>{code}</script>")
}

#[test]
fn with_no_budget_config_a_site_gets_the_starters_numbers() {
    let mut inputs = Inputs::default();
    let config = "[site]\nurl = \"https://e.com\"\n\n[routes.over]\npath = \"/over/\"\ntemplate = \"over.html\"\n\n[routes.at]\npath = \"/at/\"\ntemplate = \"at.html\"\n";
    inputs.insert("yuanme.toml", config.as_bytes().to_vec());
    // The Starters' JS budget is 3 kB.
    inputs.insert("templates/over.html", script_of(3001).into_bytes());
    inputs.insert("templates/at.html", script_of(3000).into_bytes());
    let build = run(&inputs);
    assert_eq!(page(&build, "/over/").values[&Budget::Js], 3001);
    let at = page(&build, "/at/");
    assert_eq!(at.values[&Budget::Js], 3000);
    // Its HTML, under half its limit, is estimated, but not its JS.
    let html = text(&build, "at/index.html").as_bytes();
    assert_ne!(estimate(html), br(html));
    assert_eq!(at.values[&Budget::Html], estimate(html));
    assert_eq!(at.estimated, BTreeSet::from([Budget::Html]));
    let warned: Vec<&str> = build
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        warned,
        ["`/over/` loads 3.001 kB of JS, over its budget of 3 kB"]
    );
}

#[test]
fn every_budget_code_is_tried_here() {
    let tried: BTreeSet<&str> = Budget::ALL
        .iter()
        .map(|b| match b {
            Budget::MathFont => "budget/math-font",
            b => ["budget/", b.key()].concat().leak(),
        })
        .chain(["budget/unknown-page"])
        .collect();
    let all: BTreeSet<&str> = yuanme_diag::codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| n.starts_with("budget/"))
        .collect();
    assert_eq!(tried, all);
}
