//! Stylesheets, Scripts, files and Static files that Pages name (#56, #58),
//! built from fixture projects through the Engine's one seam.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use yuanme_engine::{Body, Build, Cache, Inputs, Mode, build};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/sites")
        .join(name)
}

/// The project at `root`, loaded as the binary loads it: `node_modules`
/// left out, images hashed as a stream.
fn load(root: &Path) -> Inputs {
    let mut inputs = Inputs::at(root.to_path_buf());
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
            if path.is_dir() {
                if relative != "node_modules" {
                    stack.push(path);
                }
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            if relative.ends_with(".png") {
                inputs.insert_streamed(relative, blake3::hash(&bytes));
            } else {
                inputs.insert(relative, bytes);
            }
        }
    }
    inputs
}

fn run(inputs: &Inputs) -> Build {
    build(inputs, &mut Cache::memory("1.0.0"), Mode::Build)
}

/// Every output file: text as it is, other bytes by size and hash.
fn show(build: &Build) -> String {
    assert!(!build.failed(), "{}", diagnostics(build));
    let mut out = String::new();
    for (path, output) in &build.files {
        match &output.body {
            Body::Bytes(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) if !text.contains('\0') => writeln!(out, "=== {path}\n{text}").unwrap(),
                _ => writeln!(out, "=== {path}\n<{} bytes>", bytes.len()).unwrap(),
            },
            Body::Input(input) => writeln!(out, "=== {path}\n<copied from {input}>").unwrap(),
            Body::Cached(_) => unreachable!("no asset is cached yet"),
        }
    }
    out
}

fn diagnostics(build: &Build) -> String {
    build
        .diagnostics
        .iter()
        .map(|d| d.render(&build.sources, false))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The output file whose path starts with `assets/<name>-`, as text.
fn bundle(build: &Build, name: &str) -> String {
    let prefix = format!("assets/{name}-");
    let (_, output) = build
        .files
        .iter()
        .find(|(path, _)| path.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no {name} bundle"));
    let Body::Bytes(bytes) = &output.body else {
        panic!("a bundle is in memory")
    };
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn pages_name_hashed_bundles_and_copies_and_get_the_sidecar_stylesheet() {
    insta::assert_snapshot!(show(&run(&load(&fixture("assets")))));
}

/// Copies the folder `from` to `to`.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

#[test]
fn a_stylesheet_imports_packages_through_their_style_condition_or_field() {
    let hoisted = run(&load(&fixture("packages")));
    insta::assert_snapshot!(show(&hoisted));

    // Bun's isolated layout links each package from `node_modules/.bun`,
    // so its files resolve to paths there. The bundle is the same.
    #[cfg(unix)]
    {
        let dir = tempfile::tempdir().unwrap();
        isolated(
            &fixture("packages"),
            dir.path(),
            &[
                ("@fontsource/geist", "@fontsource+geist@5.0.0", &[]),
                ("plain", "plain@1.0.0", &[]),
            ],
        );
        assert_eq!(show(&run(&load(dir.path()))), show(&hoisted));
    }
}

/// The project `from` copied to `to`, with its packages laid out as Bun's
/// isolated linker does: each one in `node_modules/.bun/<store>`, beside
/// links to its own dependencies, and only the project's dependencies
/// linked from `node_modules`. (name, store, dependencies)
#[cfg(unix)]
fn isolated(from: &Path, to: &Path, packages: &[(&str, &str, &[&str])]) {
    copy_dir(from, to);
    let modules = to.join("node_modules");
    let store = |store: &str, name: &str| {
        Path::new(".bun")
            .join(store)
            .join("node_modules")
            .join(name)
    };
    for (name, at, _) in packages {
        let real = modules.join(store(at, name));
        std::fs::create_dir_all(real.parent().unwrap()).unwrap();
        std::fs::rename(modules.join(name), &real).unwrap();
    }
    let up = |name: &str| if name.contains('/') { "../" } else { "" };
    for (name, at, dependencies) in packages {
        for dependency in *dependencies {
            let (_, dependency_at, _) = packages.iter().find(|(n, ..)| n == dependency).unwrap();
            // From `.bun/<at>/node_modules/<dependency>` to its store.
            let target = Path::new("../..")
                .join(up(dependency))
                .join(dependency_at)
                .join("node_modules")
                .join(dependency);
            let link = modules
                .join(".bun")
                .join(at)
                .join("node_modules")
                .join(dependency);
            std::fs::create_dir_all(link.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(target, link).unwrap();
        }
        let direct = packages.iter().all(|(_, _, deps)| !deps.contains(name));
        if direct {
            let target = Path::new(up(name)).join(store(at, name));
            std::os::unix::fs::symlink(target, modules.join(name)).unwrap();
        }
    }
}

/// Every Script a Page loads, then every chunk it preloads.
fn script_urls(page: &str) -> (Vec<&str>, Vec<&str>) {
    let attribute = |tag: &str, name: &str| -> Vec<&str> {
        page.split(tag)
            .skip(1)
            .filter_map(|rest| {
                let rest = &rest[..rest.find('>').unwrap()];
                let value = rest.split(&format!("{name}=\"")).nth(1)?;
                // Without the `?query` the Template wrote.
                Some(&value[..value.find(['"', '?']).unwrap()])
            })
            .collect()
    };
    (
        attribute("<script type=\"module\"", "src"),
        attribute("<link rel=\"modulepreload\"", "href"),
    )
}

/// The chunks a chunk's code imports: statically, then with `import()`.
fn chunk_imports(code: &str) -> (Vec<String>, Vec<String>) {
    let mut statics = Vec::new();
    let mut dynamics = Vec::new();
    for (i, _) in code.match_indices("./") {
        let quote = code.as_bytes()[i - 1];
        if !b"\"'`".contains(&quote) {
            continue;
        }
        let end = code[i..].find(quote as char).unwrap();
        let file = format!("assets/{}", &code[i + 2..i + end]);
        if code[..i - 1].ends_with("import(") {
            dynamics.push(file);
        } else {
            statics.push(file);
        }
    }
    (statics, dynamics)
}

#[test]
fn scripts_are_one_build_with_shared_chunks_and_transitive_preloads() {
    let build = run(&load(&fixture("scripts")));
    insta::assert_snapshot!(show(&build));

    let text = |path: &str| match &build.files[path].body {
        Body::Bytes(bytes) => std::str::from_utf8(bytes).unwrap().to_string(),
        other => panic!("{path} is {other:?}"),
    };
    // Every chunk a chunk imports is a file of the Site.
    let js: Vec<&String> = build.files.keys().filter(|p| p.ends_with(".js")).collect();
    for path in &js {
        let (statics, dynamics) = chunk_imports(&text(path));
        for file in statics.iter().chain(&dynamics) {
            assert!(
                build.files.contains_key(file),
                "{path} imports {file}, which isn't written"
            );
        }
    }
    // A Page preloads exactly the chunks its Scripts import statically,
    // through every chunk between, and no chunk it loads as a Script.
    for page in ["index.html", "about/index.html"] {
        let html = text(page);
        let (scripts, preloads) = script_urls(&html);
        let mut want = std::collections::BTreeSet::new();
        let mut stack: Vec<String> = scripts.iter().map(|s| s[1..].to_string()).collect();
        while let Some(path) = stack.pop() {
            for file in chunk_imports(&text(&path)).0 {
                if want.insert(file.clone()) {
                    stack.push(file);
                }
            }
        }
        for script in &scripts {
            want.remove(&script[1..]);
        }
        let got: BTreeSet<String> = preloads.iter().map(|p| p[1..].to_string()).collect();
        assert!(!want.is_empty(), "{page} preloads nothing");
        assert_eq!(got, want, "{page}");
        assert_eq!(got.len(), preloads.len(), "{page} preloads a chunk twice");
    }
    // The home Page renders the photo Template, so its Page Script holds
    // the Sidecar script, whose `mount` it registers (ADR 0013). The about
    // Page renders no Template with one, so it gets no Page Script.
    let (home, about) = (text("index.html"), text("about/index.html"));
    let ((home, _), (about, _)) = (script_urls(&home), script_urls(&about));
    let page: Vec<_> = home
        .iter()
        .filter(|s| s.starts_with("/assets/page-"))
        .collect();
    let [page] = page.as_slice() else {
        panic!("{home:?}")
    };
    assert!(
        !about.iter().any(|s| s.starts_with("/assets/page-")),
        "{about:?}"
    );
    let bundle = build.scripts.iter().find(|b| b.file == page[1..]).unwrap();
    assert!(
        bundle
            .modules
            .iter()
            .any(|(m, _)| m == "templates/components/photo.ts"),
        "{:?}",
        bundle.modules
    );
    assert!(text(&page[1..]).contains(".mount"), "{}", text(&page[1..]));

    // What went into each bundle, for the build report.
    let site = build
        .scripts
        .iter()
        .find(|b| b.modules.iter().any(|(m, _)| m == "scripts/site.ts"))
        .expect("a bundle of site.ts");
    assert_eq!(site.size, text(&site.file).len());
    let modules: Vec<&str> = site.modules.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        modules,
        [
            "yuanme:router",
            "node_modules/tiny-alphabet/index.js",
            "node_modules/tiny-id/index.browser.js",
            "data/film.toml",
            "scripts/site.ts",
        ]
    );
    assert!(site.modules.iter().all(|(_, size)| *size > 0));
}

/// The router fixture with `[router] mode` set to `mode`.
fn router_site(mode: &str) -> Build {
    let root = fixture("router");
    let mut inputs = load(&root);
    let config = std::fs::read_to_string(root.join("yuanme.toml")).unwrap();
    inputs.insert(
        "yuanme.toml",
        config
            .replace("mode = \"swap\"", &format!("mode = \"{mode}\""))
            .into_bytes(),
    );
    run(&inputs)
}

fn text(build: &Build, path: &str) -> String {
    match &build.files[path].body {
        Body::Bytes(bytes) => std::str::from_utf8(bytes).unwrap().to_string(),
        other => panic!("{path} is {other:?}"),
    }
}

/// Each Page's `<script type="module">` tags, whole.
fn script_tags(page: &str) -> Vec<&str> {
    page.match_indices("<script type=\"module\"")
        .map(|(at, _)| &page[at..at + page[at..].find('>').unwrap() + 1])
        .collect()
}

#[test]
fn each_page_mounts_its_sidecar_scripts_through_one_page_script() {
    for mode in ["off", "native", "swap"] {
        let build = router_site(mode);
        assert!(!build.failed(), "{}", diagnostics(&build));
        // Pages that render the same Templates' Sidecar scripts share a
        // Page Script, which holds them or imports them.
        let page_script = |page: &str| -> String {
            let html = text(&build, page);
            let tags: Vec<&str> = script_tags(&html)
                .into_iter()
                .filter(|t| t.contains("/assets/page-"))
                .collect();
            let [tag] = tags.as_slice() else {
                panic!("{mode} {page}: {tags:?}")
            };
            assert_eq!(
                tag.contains(" data-yuanme-router"),
                mode == "swap",
                "{mode} {tag}"
            );
            assert_eq!(
                html.contains("<style>@view-transition{navigation:auto}</style>"),
                mode == "native",
                "{mode} {page}"
            );
            let src = tag.split("src=\"/").nth(1).unwrap();
            src[..src.find('"').unwrap()].to_string()
        };
        let home = page_script("index.html");
        assert_eq!(page_script("projects/index.html"), home, "{mode}");
        let about = page_script("about/index.html");
        assert_ne!(about, home, "{mode}");
        assert_eq!(page_script("posts/long/index.html"), about, "{mode}");
        let (statics, _) = chunk_imports(&text(&build, &home));
        let bundle = |file: &str| build.scripts.iter().find(|b| b.file == file).unwrap();
        let modules: Vec<&str> = statics
            .iter()
            .chain([&home])
            .flat_map(|c| bundle(c).modules.iter().map(|(m, _)| m.as_str()))
            .collect();
        for sidecar in [
            "templates/layouts/base.ts",
            "templates/components/counter.ts",
        ] {
            assert!(modules.contains(&sidecar), "{mode}: {modules:?}");
        }
        let router = build
            .scripts
            .iter()
            .find(|b| b.modules.iter().any(|(m, _)| m == "yuanme:swap"));
        assert_eq!(router.is_some(), mode == "swap", "{mode}");
    }
}

/// #26's sub-budget: the Router, with the swap core and the Script
/// lifecycle, is at most 2 KB br, as a Site serves it.
#[test]
fn the_router_is_one_chunk_of_at_most_2_kb_br() {
    let build = router_site("swap");
    assert!(!build.failed(), "{}", diagnostics(&build));
    let router = build
        .scripts
        .iter()
        .find(|b| b.modules.iter().any(|(m, _)| m == "yuanme:router"))
        .unwrap();
    let mut modules: Vec<&str> = router.modules.iter().map(|(m, _)| m.as_str()).collect();
    modules.sort();
    assert_eq!(modules, ["yuanme:mount", "yuanme:router", "yuanme:swap"]);
    let code = text(&build, &router.file);
    let mut br = Vec::new();
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        ..Default::default()
    };
    brotli::BrotliCompress(&mut code.as_bytes(), &mut br, &params).unwrap();
    assert!(br.len() <= 2048, "the Router is {} B br", br.len());
}

#[test]
fn with_the_router_on_a_page_without_sidecar_scripts_starts_it() {
    let root = fixture("scripts");
    let mut inputs = load(&root);
    let config = std::fs::read_to_string(root.join("yuanme.toml")).unwrap();
    inputs.insert(
        "yuanme.toml",
        format!("{config}\n[router]\nmode = \"swap\"\n").into_bytes(),
    );
    let build = run(&inputs);
    assert!(!build.failed(), "{}", diagnostics(&build));
    let about = text(&build, "about/index.html");
    let tags = script_tags(&about);
    assert!(
        tags.iter()
            .any(|t| t.contains("/assets/page-") && t.contains(" data-yuanme-router")),
        "{tags:?}"
    );
}

#[test]
#[cfg(unix)]
fn bun_layouts_give_the_same_bundles() {
    let hoisted = show(&run(&load(&fixture("scripts"))));
    let dir = tempfile::tempdir().unwrap();
    isolated(
        &fixture("scripts"),
        dir.path(),
        &[
            ("tiny-id", "tiny-id@1.0.0", &["tiny-alphabet"]),
            ("tiny-alphabet", "tiny-alphabet@1.0.0", &[]),
        ],
    );
    // Only the project's own dependency is at the top.
    assert!(!dir.path().join("node_modules/tiny-alphabet").exists());
    assert_eq!(show(&run(&load(dir.path()))), hoisted);
}

/// Files by path, over a fixture's.
type Over = &'static [(&'static str, &'static [u8])];

/// (code, files over the `assets` fixture, the file and text its primary
/// span covers)
const ERRORS: &[(&str, Over, &str, &str)] = &[
    (
        "asset/not-found",
        &[(
            "templates/home.html",
            b"{% extends \"layouts/base.html\" %}{% block head %}<link rel=\"stylesheet\" href=\"/styles/sit.css\">{% endblock %}",
        )],
        "templates/home.html",
        "/styles/sit.css",
    ),
    // A Static file's URL has no `static/`.
    (
        "asset/not-found",
        &[(
            "templates/home.html",
            b"{% extends \"layouts/base.html\" %}{% block head %}<link rel=\"icon\" href=\"/static/favicon.svg\">{% endblock %}",
        )],
        "templates/home.html",
        "/static/favicon.svg",
    ),
    (
        "asset/not-found",
        &[("styles/post.css", b"article {\n  background: url(../img/nope.png);\n}\n")],
        "styles/post.css",
        "../img/nope.png",
    ),
    (
        "asset/not-found",
        &[("styles/post.css", b"\n@import \"missing.css\";\n")],
        "styles/post.css",
        "missing.css",
    ),
    (
        "asset/not-found",
        &[("styles/post.css", b"@import \"no-such-package\";\n")],
        "styles/post.css",
        "no-such-package",
    ),
    (
        "asset/dependencies-not-installed",
        &[
            ("package.json", b"{}"),
            ("styles/post.css", b"@import \"no-such-package\";\n"),
        ],
        "styles/post.css",
        "no-such-package",
    ),
    (
        "asset/ambiguous-url",
        &[("static/styles/post.css", b"article {}")],
        "templates/posts/entry.html",
        "../../styles/post.css?v=2",
    ),
    (
        "asset/classic-script",
        &[(
            "templates/home.html",
            b"{% extends \"layouts/base.html\" %}{% block body %}<script src=\"/scripts/site.ts\"></script>{% endblock %}",
        )],
        "templates/home.html",
        "/scripts/site.ts",
    ),
    (
        "asset/invalid-css",
        &[("styles/parts/type.css", b"p { color: red }\n\n  a$b { color: red }\n")],
        "styles/parts/type.css",
        "",
    ),
    (
        "asset/invalid-css",
        &[("templates/components/row.css", b".row { content: \"caf\xe9\" }")],
        "templates/components/row.css",
        "",
    ),
    (
        "asset/invalid-css",
        &[("templates/components/row.css", b".row {}\n\n.row :local(.a) { color: red }\n")],
        "templates/components/row.css",
        "",
    ),
    (
        "asset/invalid-css",
        &[("templates/components/row.css", b".row :global() { color: red }\n")],
        "templates/components/row.css",
        "",
    ),
    // Two Sidecars whose paths hash alike.
    (
        "asset/scope-collision",
        &[
            ("templates/components/c388214.html", b"<p class=\"a\">a</p>"),
            ("templates/components/c388214.css", b".a {}"),
            ("templates/components/c517339.html", b"<p class=\"b\">b</p>"),
            ("templates/components/c517339.css", b".b {}"),
            (
                "templates/home.html",
                b"{% extends \"layouts/base.html\" %}{% block body %}{% include \"components/c388214.html\" %}{% include \"components/c517339.html\" %}{% endblock %}",
            ),
        ],
        "templates/components/c517339.css",
        "",
    ),
    (
        "asset/not-found",
        &[("scripts/site.ts", b"import { x } from \"./nope\";\nconsole.log(x);\n")],
        "scripts/site.ts",
        "./nope",
    ),
    (
        "asset/not-found",
        &[("scripts/site.ts", b"// left-pad\nimport pad from 'left-pad';\npad();\n")],
        "scripts/site.ts",
        "left-pad",
    ),
    (
        "asset/dependencies-not-installed",
        &[
            ("package.json", b"{}"),
            ("scripts/site.ts", b"import pad from \"left-pad\";\npad();\n"),
        ],
        "scripts/site.ts",
        "left-pad",
    ),
    (
        "asset/css-in-script",
        &[("scripts/site.ts", b"import \"../styles/site.css\";\n")],
        "scripts/site.ts",
        "../styles/site.css",
    ),
    (
        "asset/jsx",
        &[
            ("scripts/site.ts", b"import \"./el.js\";\n"),
            ("scripts/el.js", b"export const el = <div />;\n"),
        ],
        "scripts/el.js",
        "",
    ),
    (
        "asset/jsx",
        &[
            ("scripts/app.tsx", b"export {};\n"),
            (
                "templates/home.html",
                b"{% extends \"layouts/base.html\" %}{% block body %}<script type=\"module\" src=\"/scripts/app.tsx\"></script>{% endblock %}",
            ),
        ],
        "templates/home.html",
        "/scripts/app.tsx",
    ),
    (
        "asset/invalid-script",
        &[("scripts/site.ts", b"export {};\nlet x = ;\n")],
        "scripts/site.ts",
        "",
    ),
    (
        "asset/invalid-script",
        &[("scripts/site.ts", b"import { nope } from \"./other.ts\";\nnope();\n"), ("scripts/other.ts", b"export const yes = 1;\n")],
        "scripts/site.ts",
        "",
    ),
    // Warnings: the build goes on.
    (
        "asset/unmatched-selector",
        &[("templates/components/row.css", b".row { color: red }\n\n.row .titel,\n.row:hover { color: blue }\n")],
        "templates/components/row.css",
        ".row .titel,\n.row:hover",
    ),
    (
        "asset/script-warning",
        &[("scripts/site.ts", b"export const two = eval(\"1 + 1\");\n")],
        "scripts/site.ts",
        "",
    ),
];

#[test]
fn each_mistake_is_one_diagnostic_on_the_url_it_is_about() {
    let tested: BTreeSet<&str> = ERRORS.iter().map(|(code, ..)| *code).collect();
    let all: BTreeSet<&str> = yuanme_diag::codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| n.starts_with("asset/"))
        .collect();
    assert_eq!(tested, all);

    let base = load(&fixture("assets"));
    let wrong: Vec<String> = ERRORS
        .iter()
        .filter_map(|(code, files, file, under)| {
            let mut inputs = base.clone();
            for (path, bytes) in *files {
                inputs.insert(*path, *bytes);
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
            let fails = !["asset/script-warning", "asset/unmatched-selector"].contains(code);
            let ok = build.failed() == fails
                && build.files.is_empty() == fails
                && got == [(*code, *file, *under)];
            (!ok).then(|| {
                format!(
                    "want {code} in {file} on {under:?}\n  got {got:?}\n{}",
                    diagnostics(&build)
                )
            })
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn rendered_asset_diagnostics() {
    let mut inputs = load(&fixture("assets"));
    inputs.insert(
        "templates/home.html",
        b"{% extends \"layouts/base.html\" %}\n{% block head %}<link rel=\"stylesheet\" href=\"/styles/sit.css\">\n<script src=\"/scripts/site.ts\"></script>{% endblock %}\n".as_slice(),
    );
    let build = run(&inputs);
    insta::assert_snapshot!(diagnostics(&build));

    // The suggested `type="module"` makes it a Script, which builds.
    let fix = build
        .diagnostics
        .iter()
        .find(|d| d.code.name == "asset/classic-script")
        .and_then(|d| d.children.iter().flat_map(|c| &c.suggestions).next())
        .expect("a fix");
    let [edit] = fix.edits.as_slice() else {
        panic!("one edit")
    };
    let mut text = build.sources.text(edit.span.file).to_string();
    text.replace_range(edit.span.range.clone(), &edit.replacement);
    let text = text.replace("/styles/sit.css", "/styles/site.css");
    inputs.insert("templates/home.html", text.into_bytes());
    let build = run(&inputs);
    assert!(!build.failed(), "{}", diagnostics(&build));
}

#[test]
fn rendered_script_diagnostics() {
    let mut inputs = load(&fixture("assets"));
    inputs.insert(
        "scripts/site.ts",
        b"import { greet } from \"./greet\";\nimport \"../styles/site.css\";\n\ngreet();\n"
            .as_slice(),
    );
    inputs.insert(
        "scripts/greet.ts",
        b"export const greet = () => {};\n".as_slice(),
    );
    let build = run(&inputs);
    insta::assert_snapshot!(diagnostics(&build));

    // The suggested edit deletes the import, which then builds.
    let [d] = build.diagnostics.as_slice() else {
        panic!("{}", diagnostics(&build))
    };
    let fix = d
        .children
        .iter()
        .flat_map(|c| &c.suggestions)
        .next()
        .expect("a fix");
    let [edit] = fix.edits.as_slice() else {
        panic!("one edit")
    };
    let mut text = build.sources.text(edit.span.file).to_string();
    text.replace_range(edit.span.range.clone(), &edit.replacement);
    assert_eq!(text, "import { greet } from \"./greet\";\n\ngreet();\n");
    inputs.insert("scripts/site.ts", text.into_bytes());
    let build = run(&inputs);
    assert!(!build.failed(), "{}", diagnostics(&build));
}

/// The Reference site's two stylesheets as Astro and Tailwind 3 built them,
/// through the default targets: everything the parity inventory says must
/// survive does, and only `css-targets`' fallbacks go (#20, #23).
#[test]
fn the_reference_sites_css_keeps_what_the_parity_inventory_lists() {
    let root = fixture("reference-css");
    let build = run(&load(&root));
    assert!(!build.failed(), "{}", diagnostics(&build));
    // Minifying drops whitespace, so both sides are compared without it.
    let squeeze = |css: &str| css.split_whitespace().collect::<String>();
    let count = |css: &str, what: &str| css.matches(&squeeze(what)).count();

    // (stylesheet, what must survive)
    let kept: &[(&str, &[&str])] = &[
        (
            "global",
            &[
                "@property --",
                ":has(",
                "linear-gradient(in oklab",
                "color-mix(in oklab",
                "@supports (color: oklab(from red l a b))",
                "oklab(from color-mix(",
                "text-wrap:pretty",
                "100dvh",
                "100svh",
                "35svh",
                "::selection",
                // The droplet's fallbacks, which no target needs but a
                // custom property's value can't prove.
                "background:#f27baa;background:var(--droplet);background:linear-gradient(",
                "-webkit-background-clip:text",
                "-webkit-text-fill-color:transparent",
                "@media (prefers-reduced-motion:",
                "@media (hover:hover) and (pointer:fine)",
                "@keyframes",
            ],
        ),
        (
            "about",
            &[
                "@container",
                "container-type:size",
                "cqw",
                "cqh",
                "@media (scripting:enabled)",
                "translate:",
                "rotate:",
                "scale:",
                "@keyframes",
            ],
        ),
    ];
    let mut wrong = Vec::new();
    for (name, features) in kept {
        let source =
            squeeze(&std::fs::read_to_string(root.join(format!("styles/{name}.css"))).unwrap());
        let built = squeeze(&bundle(&build, name));
        for feature in *features {
            let (before, after) = (count(&source, feature), count(&built, feature));
            if before == 0 || before != after {
                wrong.push(format!("{name}: `{feature}` {before} -> {after}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));

    // `css-targets`: every target supports dvh, svh and `::selection`.
    let global = squeeze(&bundle(&build, "global"));
    assert_eq!(count(&global, "100vh"), 0);
    assert_eq!(count(&global, "::-moz-selection"), 0);
}

/// Every selector of `css` that names an attribute starting with `prefix`,
/// grouped by that attribute: each one with the at-rules it is in, the
/// attribute written `[S]`, and each compound's simple selectors sorted,
/// so that two builds' spellings of the same selector compare equal.
fn scoped_selectors(css: &str, prefix: &str) -> BTreeSet<BTreeSet<String>> {
    use lightningcss::rules::CssRule;
    use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
    use lightningcss::traits::ToCss;

    fn walk(
        rules: &[CssRule<'_>],
        context: &str,
        attribute: &regex::Regex,
        groups: &mut BTreeMap<String, BTreeSet<String>>,
    ) {
        for rule in rules {
            let printed = rule.to_css_string(PrinterOptions::default()).unwrap();
            let prelude = printed.split('{').next().unwrap().trim();
            let inner = match rule {
                CssRule::Style(style) => {
                    for selector in &style.selectors.0 {
                        let text = selector.to_css_string(PrinterOptions::default()).unwrap();
                        let Some(found) = attribute.captures(&text) else {
                            continue;
                        };
                        let text = attribute.replace_all(&text, "[S]");
                        groups
                            .entry(found[1].to_string())
                            .or_default()
                            .insert(format!("{context}{}", canonical(&text)));
                    }
                    continue;
                }
                CssRule::Media(r) => &r.rules,
                CssRule::Container(r) => &r.rules,
                CssRule::Supports(r) => &r.rules,
                _ => continue,
            };
            walk(&inner.0, &format!("{context}{prelude} "), attribute, groups);
        }
    }

    /// `a.b:hover[S] > c` and `a[S].b:hover > c` are one selector.
    fn canonical(selector: &str) -> String {
        let mut out = String::new();
        let mut depth = 0;
        let mut compound = String::new();
        let flush = |compound: &mut String, out: &mut String| {
            let mut parts: Vec<String> = Vec::new();
            let mut d = 0;
            for (i, c) in compound.char_indices() {
                let starts = d == 0
                    && ".[:#".contains(c)
                    && !(c == ':' && compound[..i].ends_with(':'))
                    && i > 0;
                if starts {
                    parts.push(String::new());
                }
                if parts.is_empty() {
                    parts.push(String::new());
                }
                match c {
                    '(' | '[' => d += 1,
                    ')' | ']' => d -= 1,
                    _ => {}
                }
                parts.last_mut().unwrap().push(c);
            }
            let (mut pseudo, mut rest): (Vec<String>, Vec<String>) =
                parts.into_iter().partition(|p| p.starts_with("::"));
            // The type selector first, then the rest.
            rest.sort_by_key(|p| (p.starts_with(['.', '[', ':', '#']), p.clone()));
            rest.append(&mut pseudo);
            out.push_str(&rest.concat());
            compound.clear();
        };
        for c in selector.chars() {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth -= 1,
                _ => {}
            }
            if depth == 0 && " >+~".contains(c) {
                flush(&mut compound, &mut out);
                out.push(c);
            } else {
                compound.push(c);
            }
        }
        flush(&mut compound, &mut out);
        out
    }

    let sheet = StyleSheet::parse(css, ParserOptions::default()).unwrap();
    let attribute = regex::Regex::new(&format!(r"\[({prefix}[a-z0-9]+)\]")).unwrap();
    let mut groups = BTreeMap::new();
    walk(&sheet.rules.0, "", &attribute, &mut groups);
    groups.into_values().collect()
}

/// The Reference site's six Sidecars, scoped by the Engine, are scoped as
/// Astro scoped them: the same selectors, once the attribute's name and
/// the order within each compound are set aside, in the same at-rules and
/// grouped by Template the same way (ADR 0006: the spike's 68 of 68).
#[test]
fn the_reference_sidecars_are_scoped_as_astro_scoped_them() {
    let root = fixture("eyuan-sidecars");
    let build = run(&load(&root));
    // The fixture has no images and leaves out Pages its links name.
    let codes: Vec<&str> = build.diagnostics.iter().map(|d| d.code.name).collect();
    assert!(
        codes.iter().all(|c| *c == "link/broken"),
        "{}",
        diagnostics(&build)
    );
    let ours = scoped_selectors(&bundle(&build, "sidecars"), "data-y-");
    let astro = std::fs::read_to_string(root.join("astro.css")).unwrap()
        + &std::fs::read_to_string(fixture("reference-css").join("styles/about.css")).unwrap();
    let theirs = scoped_selectors(&astro, "data-astro-cid-");
    assert_eq!(theirs.len(), 6);
    assert_eq!(theirs.iter().map(BTreeSet::len).sum::<usize>(), 68);
    let only = |a: &BTreeSet<BTreeSet<String>>, b: &BTreeSet<BTreeSet<String>>| {
        let b: BTreeSet<&String> = b.iter().flatten().collect();
        a.iter()
            .flatten()
            .filter(|s| !b.contains(s))
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ours,
        theirs,
        "only ours: {:#?}\nonly Astro's: {:#?}",
        only(&ours, &theirs),
        only(&theirs, &ours)
    );
}
