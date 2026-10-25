//! #15's static output checks (#64), built from the `hosts` fixture
//! through the Engine's one seam.

use std::path::Path;

use yuanme_engine::{Build, Cache, Inputs, Mode, build};

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
        .filter(|n| n.starts_with("link/") && *n != "link/unresolved")
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
