//! The Host adapters and #15's static output checks (#64), built from the
//! `hosts` fixture through the Engine's one seam, and read back the way
//! each host reads its files.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::io::Read;
use std::path::Path;

use yuanme_engine::{Body, Build, Cache, Inputs, Mode, build};

const ADAPTERS: &[&str] = &[
    "cloudflare",
    "cloudflare-pages",
    "netlify",
    "vercel",
    "github-pages",
    "static",
];

fn fixture() -> Inputs {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sites/hosts");
    let mut inputs = Inputs::at(root.clone());
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path.strip_prefix(&root).unwrap().to_str().unwrap();
            inputs.insert(relative, std::fs::read(&path).unwrap());
        }
    }
    inputs
}

/// The fixture with `[host]` set to `host`, such as `adapter = "netlify"`.
fn with_host(host: &str) -> Inputs {
    let mut inputs = fixture();
    let config = text(&inputs, "yuanme.toml").replace("adapter = \"cloudflare\"", host);
    inputs.insert("yuanme.toml", config.into_bytes());
    inputs
}

fn text(inputs: &Inputs, path: &str) -> String {
    let (_, input) = inputs.iter().find(|(p, _)| *p == path).unwrap();
    String::from_utf8(input.bytes.as_deref().unwrap().to_vec()).unwrap()
}

fn run(inputs: &Inputs) -> Build {
    build(inputs, &mut Cache::memory("1.0.0"), Mode::Build)
}

fn diagnostics(build: &Build) -> String {
    build
        .diagnostics
        .iter()
        .map(|d| d.render(&build.sources, false))
        .collect::<Vec<_>>()
        .join("\n")
}

fn bytes<'a>(build: &'a Build, path: &str) -> &'a [u8] {
    match &build.files[path].body {
        Body::Bytes(bytes) => bytes,
        other => panic!("{path} is {other:?}"),
    }
}

fn file(build: &Build, path: &str) -> String {
    String::from_utf8(bytes(build, path).to_vec()).unwrap()
}

/// The hashed stylesheet's URL.
fn stylesheet(build: &Build) -> String {
    let path = build
        .files
        .keys()
        .find(|p| p.contains("assets/site-"))
        .expect("the stylesheet");
    format!("/{}", path.trim_start_matches("static/"))
}

/// Every output path, and the bytes of each file the adapter added.
fn show(build: &Build, plain: &Build) -> String {
    let mut out = String::new();
    for (path, output) in &build.files {
        let site = path
            .strip_prefix("static/")
            .filter(|_| build.files.contains_key("config.json"));
        if plain.files.contains_key(site.unwrap_or(path)) {
            writeln!(out, "{path}").unwrap();
            continue;
        }
        match &output.body {
            Body::Bytes(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) => writeln!(out, "=== {path}\n{text}===").unwrap(),
                Err(_) => writeln!(out, "=== {path}\n<{} bytes>", bytes.len()).unwrap(),
            },
            other => panic!("{path} is {other:?}"),
        }
    }
    out
}

#[test]
fn each_adapter_writes_the_files_its_host_reads() {
    // The static adapter writes nothing of its own.
    let plain = run(&with_host("adapter = \"static\""));
    for adapter in ADAPTERS {
        let mut host = format!("adapter = \"{adapter}\"");
        if *adapter == "static" {
            host.push_str("\nprecompress = true");
        }
        let build = run(&with_host(&host));
        assert!(!build.failed(), "{adapter}:\n{}", diagnostics(&build));
        insta::assert_snapshot!(format!("adapter_{adapter}"), show(&build, &plain));
    }
}

/// A `_headers` path, as Cloudflare and Netlify match one: `*` is any
/// characters.
fn glob(pattern: &str, url: &str) -> bool {
    match pattern.split_once('*') {
        Some((a, b)) => url.len() >= a.len() + b.len() && url.starts_with(a) && url.ends_with(b),
        None => pattern == url,
    }
}

/// Each `_headers` rule: its path, then each line under it.
fn rules(headers: &str) -> Vec<(&str, Vec<&str>)> {
    let mut rules: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in headers.lines().filter(|l| !l.trim().is_empty()) {
        match line.strip_prefix("  ") {
            Some(header) => rules.last_mut().unwrap().1.push(header),
            None => rules.push((line, Vec::new())),
        }
    }
    rules
}

/// The headers Workers static assets sends for `url`, as its asset worker
/// applies `_headers` (workers-sdk `attachCustomHeaders`): matching rules
/// in order, each `! name` deleting, and a name an earlier rule set
/// appended to with a comma.
fn cloudflare(headers: &str, url: &str) -> BTreeMap<String, String> {
    let mut response = BTreeMap::from([(
        "cache-control".to_string(),
        "public, max-age=0, must-revalidate".to_string(),
    )]);
    let mut set = Vec::new();
    for (path, lines) in rules(headers) {
        if !glob(path, url) {
            continue;
        }
        for line in &lines {
            if let Some(name) = line.strip_prefix("! ") {
                response.remove(&name.to_ascii_lowercase());
            }
        }
        for line in lines.iter().filter(|l| !l.starts_with('!')) {
            let (name, value) = line.split_once(": ").unwrap();
            let name = name.to_ascii_lowercase();
            if set.contains(&name) {
                let joined = match response.get(&name) {
                    Some(old) => format!("{old}, {value}"),
                    None => value.to_string(),
                };
                response.insert(name, joined);
            } else {
                response.insert(name.clone(), value.to_string());
                set.push(name);
            }
        }
    }
    response
}

/// Netlify's: a later section's header replaces an earlier one's
/// (netlify/build#2290), and Netlify's default `Cache-Control` is the same
/// as Cloudflare's.
fn netlify(headers: &str, url: &str) -> BTreeMap<String, String> {
    assert!(!headers.contains("  !"), "Netlify has no `!` lines");
    let mut response = BTreeMap::from([(
        "cache-control".to_string(),
        "public, max-age=0, must-revalidate".to_string(),
    )]);
    for (path, lines) in rules(headers) {
        if glob(path, url) {
            for line in lines {
                let (name, value) = line.split_once(": ").unwrap();
                response.insert(name.to_ascii_lowercase(), value.to_string());
            }
        }
    }
    response
}

/// What Vercel does with `url` before its filesystem phase: the headers
/// of every `continue` route that matches, later ones winning, then the
/// first other route that matches, which may redirect.
fn vercel(config: &str, url: &str) -> (BTreeMap<String, String>, Option<(u64, String)>) {
    let config: serde_json::Value = serde_json::from_str(config).unwrap();
    assert_eq!(config["version"], 3);
    let mut headers = BTreeMap::new();
    for route in config["routes"].as_array().unwrap() {
        if route.get("handle").is_some() {
            break;
        }
        let src = regex::Regex::new(route["src"].as_str().unwrap()).unwrap();
        let Some(captures) = src.captures(url) else {
            continue;
        };
        let route_headers = route["headers"].as_object();
        if route["continue"] == true {
            for (name, value) in route_headers.unwrap() {
                headers.insert(
                    name.to_ascii_lowercase(),
                    value.as_str().unwrap().to_string(),
                );
            }
            continue;
        }
        let redirect = route["status"].as_u64().map(|status| {
            let mut location = String::new();
            let template = route_headers.unwrap()["Location"].as_str().unwrap();
            captures.expand(template, &mut location);
            (status, location)
        });
        return (headers, redirect);
    }
    (headers, None)
}

#[test]
fn every_host_sends_the_cache_policy_as_it_reads_its_files() {
    for adapter in ["cloudflare", "cloudflare-pages", "netlify", "vercel"] {
        let build = run(&with_host(&format!("adapter = \"{adapter}\"")));
        assert!(!build.failed(), "{adapter}:\n{}", diagnostics(&build));
        let css = stylesheet(&build);
        let send = |url: &str| match adapter {
            "netlify" => netlify(&file(&build, "_headers"), url),
            "vercel" => vercel(&file(&build, "config.json"), url).0,
            _ => cloudflare(&file(&build, "_headers"), url),
        };
        for (url, cache) in [
            (css.as_str(), "public, max-age=31536000, immutable"),
            ("/", "public, max-age=0, must-revalidate"),
            ("/blog/pew/", "public, max-age=0, must-revalidate"),
            ("/404.html", "public, max-age=0, must-revalidate"),
            // The Author's rule replaces yuanme's.
            ("/resume.pdf", "public, max-age=3600"),
        ] {
            let headers = send(url);
            let got = |name: &str| headers.get(name).map(String::as_str);
            assert_eq!(got("cache-control"), Some(cache), "{adapter} {url}");
            assert_eq!(
                got("x-content-type-options"),
                Some("nosniff"),
                "{adapter} {url}"
            );
            assert_eq!(got("x-frame-options"), Some("DENY"), "{adapter} {url}");
        }
    }
}

#[test]
fn every_host_redirects_as_site_config_says_and_pages_need_no_redirect() {
    for adapter in ["cloudflare", "cloudflare-pages", "netlify", "vercel"] {
        let build = run(&with_host(&format!("adapter = \"{adapter}\"")));
        let redirect = |url: &str| -> Option<(u64, String)> {
            if adapter == "vercel" {
                return vercel(&file(&build, "config.json"), url).1;
            }
            file(&build, "_redirects").lines().find_map(|line| {
                let parts: Vec<&str> = line.split(' ').collect();
                let [from, to, status] = parts[..] else {
                    panic!("{line}")
                };
                (from == url).then(|| (status.parse().unwrap(), to.to_string()))
            })
        };
        assert_eq!(
            redirect("/old-post/"),
            Some((308, "/blog/pew/".into())),
            "{adapter}"
        );
        assert_eq!(
            redirect("/talk"),
            Some((302, "https://example.com/talk".into())),
            "{adapter}"
        );
        for url in ["/", "/blog/pew/", "/resume.pdf", "/404.html"] {
            assert_eq!(redirect(url), None, "{adapter} {url}");
        }
    }
    // Vercel needs its redirect to the trailing slash, which the others
    // make themselves, and its 404 Page named.
    let build = run(&with_host("adapter = \"vercel\""));
    let config = file(&build, "config.json");
    assert_eq!(
        vercel(&config, "/blog/pew").1,
        Some((308, "/blog/pew/".into()))
    );
    assert_eq!(vercel(&config, "/.well-known/security").1, None);
    assert!(config.contains(r#"{ "src": "/.*", "dest": "/404.html", "status": 404 }"#));

    // GitHub Pages: a page that refreshes, where GitHub serves it.
    let build = run(&with_host("adapter = \"github-pages\""));
    let stub = file(&build, "old-post/index.html");
    assert!(
        stub.contains(r#"<meta http-equiv="refresh" content="0; url=/blog/pew/">"#),
        "{stub}"
    );
    assert!(
        stub.contains(r#"<link rel="canonical" href="https://e.com/blog/pew/">"#),
        "{stub}"
    );
    assert!(file(&build, "talk.html").contains("url=https://example.com/talk"));
}

#[test]
fn precompressed_files_decode_to_their_text_and_others_get_none() {
    let build = run(&with_host("adapter = \"static\"\nprecompress = true"));
    assert!(!build.failed(), "{}", diagnostics(&build));
    let css = stylesheet(&build)[1..].to_string();
    for path in [
        "index.html",
        "blog/pew/index.html",
        "404.html",
        "sitemap-0.xml",
        &css,
    ] {
        let original = bytes(&build, path);
        let mut br = Vec::new();
        brotli::Decompressor::new(bytes(&build, &format!("{path}.br")), 4096)
            .read_to_end(&mut br)
            .unwrap();
        assert_eq!(br, original, "{path}.br");
        let mut gz = Vec::new();
        flate2::read::GzDecoder::new(bytes(&build, &format!("{path}.gz")))
            .read_to_end(&mut gz)
            .unwrap();
        assert_eq!(gz, original, "{path}.gz");
    }
    assert!(!build.files.contains_key("resume.pdf.br"));
}

/// A mistake: files over the fixture's, with `yuanme.toml+` appended to
/// Site config, and every Diagnostic it must make as (code, file, the text
/// its primary span covers).
struct Case {
    files: Vec<(&'static str, String)>,
    want: Vec<(&'static str, &'static str, &'static str)>,
}

fn case(
    files: &[(&'static str, &str)],
    want: &[(&'static str, &'static str, &'static str)],
) -> Case {
    Case {
        files: files.iter().map(|(p, t)| (*p, t.to_string())).collect(),
        want: want.to_vec(),
    }
}

const HOME: &str = "templates/home.html";
const ENTRY: &str = "templates/posts/entry.html";
const CONFIG: &str = "yuanme.toml";

fn cases() -> Vec<Case> {
    let page = |body: &str| {
        format!("<!doctype html><html><head><title>T</title></head><body>{body}</body></html>")
    };
    let many_rules: String = (0..99)
        .map(|i| format!("[[host.headers]]\npath = \"/r{i}\"\nheaders = {{ X-A = \"1\" }}\n"))
        .collect();
    let many_redirects: String = (0..2000)
        .map(|i| format!("[[host.redirects]]\nfrom = \"/r{i}/\"\nto = \"/blog/pew/\"\n"))
        .collect();
    let long = format!(
        "[[host.headers]]\npath = \"/long\"\nheaders = {{ X-A = \"{}\" }}\n",
        "a".repeat(2000)
    );
    vec![
        // A Page's URL needs its trailing slash, or the host redirects.
        case(
            &[(HOME, &page("<a href=\"/blog/pew\">Pew</a>"))],
            &[("link/redirected", HOME, "/blog/pew")],
        ),
        case(
            &[(HOME, &page("<a href=\"/blog/pew/index.html\">Pew</a>"))],
            &[("link/redirected", HOME, "/blog/pew/index.html")],
        ),
        case(
            &[(HOME, &page("<a href=\"https://www.e.com/\">Home</a>"))],
            &[("link/redirected", HOME, "https://www.e.com/")],
        ),
        case(
            &[(HOME, &page("<a href=\"http://e.com/blog/pew/\">Pew</a>"))],
            &[("link/redirected", HOME, "http://e.com/blog/pew/")],
        ),
        case(
            &[(HOME, &page("<a href=\"/old-post/\">Pew</a>"))],
            &[("link/redirected", HOME, "/old-post/")],
        ),
        // In a body, the Entry is where the link was written.
        case(
            &[(
                "content/posts/pew.md",
                "---\ntitle: Pew\n---\n[Home](/index.html)\n",
            )],
            &[("link/redirected", "content/posts/pew.md", "/index.html")],
        ),
        case(
            &[(
                "yuanme.toml+",
                "[[host.redirects]]\nfrom = \"/older/\"\nto = \"/blog/pew\"\n",
            )],
            &[("link/redirected", CONFIG, "\"/blog/pew\"")],
        ),
        case(
            &[(HOME, &page("<img src=\"/img/cat.png\" alt=\"\">"))],
            &[("link/broken", HOME, "/img/cat.png")],
        ),
        // Relative to the Page: `/blog/` has no Page.
        case(
            &[(ENTRY, &page("{{ entry.content }}<a href=\"../\">Blog</a>"))],
            &[("link/broken", ENTRY, "../")],
        ),
        case(
            &[(
                "yuanme.toml+",
                "[[host.redirects]]\nfrom = \"/older/\"\nto = \"/nope/\"\n",
            )],
            &[("link/broken", CONFIG, "\"/nope/\"")],
        ),
        // yuanme's two cache rules count.
        case(
            &[("yuanme.toml+", &many_rules)],
            &[("host/over-limit", CONFIG, "\"/r97\"")],
        ),
        case(
            &[("yuanme.toml+", &many_redirects)],
            &[("host/over-limit", CONFIG, "\"/r1998/\"")],
        ),
        case(
            &[("yuanme.toml+", &long)],
            &[("host/over-limit", CONFIG, "\"/long\"")],
        ),
        case(
            &[(
                "yuanme.toml+",
                "[[host.redirects]]\nfrom = \"/blog/pew/\"\nto = \"/\"\n",
            )],
            &[("host/redirect-conflict", CONFIG, "\"/blog/pew/\"")],
        ),
        // Hosts redirect it to `/blog/pew/` themselves.
        case(
            &[(
                "yuanme.toml+",
                "[[host.redirects]]\nfrom = \"/blog/pew\"\nto = \"/\"\n",
            )],
            &[("host/redirect-conflict", CONFIG, "\"/blog/pew\"")],
        ),
        case(
            &[("static/_headers", "/*\n  X-A: 1\n")],
            &[("host/output-collision", CONFIG, "\"cloudflare\"")],
        ),
        case(
            &[
                ("yuanme.toml=", "adapter = \"github-pages\""),
                (
                    "yuanme.toml+",
                    "[[host.redirects]]\nfrom = \"/cv.pdf\"\nto = \"/resume.pdf\"\n",
                ),
            ],
            &[
                ("host/unsupported", CONFIG, "\"github-pages\""),
                ("host/unsupported", CONFIG, "\"/resume.pdf\""),
                ("host/unsupported", CONFIG, "\"/*\""),
                ("host/unsupported", CONFIG, "\"/cv.pdf\""),
            ],
        ),
        case(
            &[("yuanme.toml=", "adapter = \"static\"")],
            &[
                ("host/unsupported", CONFIG, "\"/resume.pdf\""),
                ("host/unsupported", CONFIG, "\"/*\""),
                ("host/unsupported", CONFIG, "\"/old-post/\""),
                ("host/unsupported", CONFIG, "\"/talk\""),
            ],
        ),
        // A Page must revalidate, and a hashed asset stay immutable.
        case(
            &[(
                "yuanme.toml+",
                "[[host.headers]]\npath = \"/blog/*\"\nheaders = { Cache-Control = \"public, max-age=600\" }\n",
            )],
            &[("host/cache-rule", CONFIG, "\"/blog/*\"")],
        ),
        case(
            &[(
                "yuanme.toml+",
                "[[host.headers]]\npath = \"/assets/*\"\nheaders = { cache-control = \"public, max-age=600\" }\n",
            )],
            &[("host/cache-rule", CONFIG, "\"/assets/*\"")],
        ),
    ]
}

#[test]
fn each_mistake_is_reported_on_what_it_is_about() {
    let cases = cases();
    let tested: std::collections::BTreeSet<&str> = cases
        .iter()
        .flat_map(|c| c.want.iter().map(|w| w.0))
        .collect();
    let all: std::collections::BTreeSet<&str> = yuanme_diag::codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| n.starts_with("host/") || (n.starts_with("link/") && *n != "link/unresolved"))
        .filter(|n| *n != "host/wrangler-mismatch")
        .collect();
    assert_eq!(tested, all);

    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            let mut inputs = fixture();
            for (path, over) in &case.files {
                let over = match (path.strip_suffix('+'), path.strip_suffix('=')) {
                    (Some(path), _) => text(&inputs, path) + "\n" + over,
                    (_, Some(path)) => {
                        text(&inputs, path).replace("adapter = \"cloudflare\"", over)
                    }
                    _ => over.clone(),
                };
                let path = path.trim_end_matches(['+', '=']);
                inputs.insert(path, over.into_bytes());
            }
            let build = run(&inputs);
            let got: Vec<_> = build
                .diagnostics
                .iter()
                .map(|d| {
                    let span = d.primary();
                    let text = build.sources.text(span.file);
                    (
                        d.code.name,
                        build.sources.path(span.file),
                        &text[span.range.clone()],
                    )
                })
                .collect();
            let failed = case.want.iter().any(|w| {
                w.0.starts_with("host/") && w.0 != "host/unsupported" && w.0 != "host/cache-rule"
            });
            let ok = got == case.want && build.failed() == failed;
            (!ok).then(|| {
                format!(
                    "want {:?}\n  got {got:?}\n{}",
                    case.want,
                    diagnostics(&build)
                )
            })
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
