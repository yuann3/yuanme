//! Tailwind as a Managed tool (#59), built from fixture projects through
//! the Engine's one seam and compared with the official CLI's own output.
//! Each test runs the real CLI, which the Engine downloads on first use,
//! so they are ignored in the normal run; CI's tailwind job runs them.

use std::path::{Path, PathBuf};
use std::process::Command;

use yuanme_assets::tailwind::{self, Watch};
use yuanme_assets::tool;
use yuanme_engine::{Body, Build, Cache, Inputs, Mode, build};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/sites")
        .join(name)
}

/// The fixture `name`, copied to a folder of its own, so the official CLI's
/// files can be written beside it.
fn project(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture(name), dir.path());
    dir
}

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

/// The project at `root`, as the binary loads it: `node_modules` left out.
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
            inputs.insert(relative, std::fs::read(&path).unwrap());
        }
    }
    inputs
}

fn diagnostics(build: &Build) -> String {
    build
        .diagnostics
        .iter()
        .map(|d| d.render(&build.sources, false))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The CSS bundle the build wrote for `styles/<name>.css`.
fn stylesheet(build: &Build, name: &str) -> String {
    assert!(!build.failed(), "{}", diagnostics(build));
    let (_, output) = build
        .files
        .iter()
        .find(|(path, _)| path.starts_with(&format!("assets/{name}-")) && path.ends_with(".css"))
        .unwrap_or_else(|| panic!("no {name} bundle in {:?}", build.files.keys()));
    let Body::Bytes(bytes) = &output.body else {
        panic!("a bundle is held in memory")
    };
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// The pinned CLI, downloaded as a build would.
fn cli() -> PathBuf {
    tailwind::tool(tailwind::VERSION)
        .expect("a platform Tailwind has a binary for")
        .binary(&tool::cache_dir().expect("a cache folder"))
        .unwrap()
}

/// The official CLI's own build of `input`, written beside `styles/`'s
/// stylesheets, with its flags, such as `--optimize`.
fn official(root: &Path, input: &str, flags: &[&str]) -> String {
    let path = root.join("styles/official.css");
    std::fs::write(&path, input).unwrap();
    // Its own temporary folder, as the Engine gives each run.
    let tmp = tempfile::tempdir().unwrap();
    let output = Command::new(cli())
        .current_dir(root.join("styles"))
        .env("TMPDIR", tmp.path())
        .args(["--input", "official.css", "--silent"])
        .args(flags)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// CSS as the official CLI minifies it, so that whitespace and the
/// spelling of values compare equal.
fn normalized(root: &Path, css: &str) -> String {
    official(root, css, &["--minify"])
}

/// `a` and `b` are the same, or the first place they differ is shown.
fn assert_same(a: &str, b: &str) {
    let Some(at) = a
        .bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .or_else(|| (a.len() != b.len()).then(|| a.len().min(b.len())))
    else {
        return;
    };
    let from = a.floor_char_boundary(at.saturating_sub(150));
    let near = |s: &str| s[from..s.ceil_char_boundary((at + 150).min(s.len()))].to_string();
    panic!(
        "they differ at byte {at}:\n  ours: …{}…\ntheirs: …{}…",
        near(a),
        near(b)
    );
}

/// The tokens the fixture uses, found by hand: its class attributes, its
/// Script's strings and its Sidecar's text.
const TOKENS: &str = "bg-white dark:bg-slate-900 mx-auto max-w-2xl p-4 card text-3xl font-bold underline text-brand badge
main (pointer: fine) ring-2
";

#[test]
#[ignore = "downloads the Tailwind CLI"]
fn a_tailwind_stylesheet_matches_the_official_clis_optimize() {
    let dir = project("tailwind");
    let root = dir.path();
    let built = build(&load(root), &mut Cache::memory("1.0.0"), Mode::Build);
    let ours = stylesheet(&built, "site");

    let sidecar = std::fs::read_to_string(root.join("templates/components/badge.css")).unwrap();
    std::fs::write(root.join("tokens.txt"), format!("{TOKENS}{sidecar}")).unwrap();
    let input = std::fs::read_to_string(root.join("styles/site.css"))
        .unwrap()
        .replace(
            "@import \"tailwindcss\";",
            "@import \"tailwindcss\" source(none);\n@source \"../tokens.txt\";",
        );
    let theirs = official(root, &input, &["--optimize"]);

    // What each source gave: a class, a Script's string, the theme
    // variable only a Sidecar uses, and the Author's own `@apply`.
    for found in [".text-brand", ".ring-2", "--color-rose-600:", ".card{"] {
        assert!(ours.contains(found), "{found} isn't in {ours}");
    }
    assert_same(&normalized(root, &ours), &normalized(root, &theirs));
}

#[test]
#[ignore = "downloads the Tailwind CLI, and daisyui with Bun"]
fn daisyui_works_from_a_bun_isolated_install() {
    let dir = project("daisyui");
    let root = dir.path();
    let status = Command::new("bun")
        .current_dir(root)
        .args(["install", "--frozen-lockfile", "--linker", "isolated"])
        .status()
        .expect("bun is installed");
    assert!(status.success());
    // The isolated layout: the package lives in the store, linked from
    // `node_modules`.
    let link = std::fs::read_link(root.join("node_modules/daisyui")).unwrap();
    assert!(link.starts_with(".bun"), "{}", link.display());

    let built = build(&load(root), &mut Cache::memory("1.0.0"), Mode::Build);
    let ours = stylesheet(&built, "site");

    std::fs::write(root.join("tokens.txt"), "btn btn-primary\n").unwrap();
    let theirs = official(
        root,
        "@import \"tailwindcss\" source(none);\n@source \"../tokens.txt\";\n@plugin \"daisyui\";\n",
        &["--optimize"],
    );
    assert!(ours.contains(".btn-primary"), "{ours}");
    assert_same(&normalized(root, &ours), &normalized(root, &theirs));
}

#[test]
#[ignore = "downloads the Tailwind CLI"]
fn tailwinds_errors_are_diagnostics_in_the_stylesheet() {
    let dir = project("tailwind");
    let root = dir.path();
    std::fs::write(
        root.join("styles/site.css"),
        "@import \"tailwindcss\";\n\n.card {\n  @apply rounded-lgg;\n}\n",
    )
    .unwrap();
    let built = build(&load(root), &mut Cache::memory("1.0.0"), Mode::Build);
    assert!(built.failed());
    let [d] = built.diagnostics.as_slice() else {
        panic!("{}", diagnostics(&built))
    };
    assert_eq!(d.code.name, "asset/tailwind-error");
    assert_eq!(
        d.message,
        "Cannot apply unknown utility class `rounded-lgg`"
    );
    let span = d.primary();
    assert_eq!(built.sources.path(span.file), "styles/site.css");
    assert_eq!(
        &built.sources.text(span.file)[span.range.clone()],
        "rounded-lgg"
    );
}

#[test]
#[ignore = "downloads the Tailwind CLI"]
fn the_watch_process_builds_again_only_when_the_tokens_change() {
    let dir = project("tailwind");
    let root = dir.path();
    let css = "@import \"tailwindcss\";\n";
    std::fs::create_dir(root.join("watch")).unwrap();
    let tokens = root.join("watch/tokens.txt");
    let input = tailwind::input(css, &tokens).unwrap();
    let (mut watch, first) = Watch::start(
        &cli(),
        &root.join("styles"),
        &input,
        tokens,
        "flex\n",
        root.join("out.css"),
    )
    .unwrap();
    assert!(
        first.contains(".flex") && !first.contains(".grid"),
        "{first}"
    );

    let second = watch.update("flex\ngrid\n").unwrap();
    assert!(
        second.contains(".flex") && second.contains(".grid"),
        "{second}"
    );

    // The same tokens don't wait for a build that never comes.
    let start = std::time::Instant::now();
    assert_eq!(watch.update("flex\ngrid\n").unwrap(), second);
    assert!(start.elapsed().as_millis() < 50);
}

/// The folders `yuanme dev`'s running Tailwind CLIs keep, one each.
fn watching() -> Vec<PathBuf> {
    let prefix = format!("yuanme-tailwind-watch-{}-", std::process::id());
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(&prefix)
        })
        .collect()
}

/// `yuanme dev` keeps a Tailwind CLI running for each stylesheet (#59),
/// and what it builds is what a fresh CLI builds: as classes come, as
/// they go, which the running CLI would keep, and as the stylesheet
/// changes, which it reads only once.
#[test]
#[ignore = "downloads the Tailwind CLI"]
fn dev_builds_with_a_running_cli_what_a_fresh_one_builds() {
    let dir = project("tailwind");
    let root = dir.path();
    let edit = |path: &str, from: &str, to: &str| {
        let path = root.join(path);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{from}");
        std::fs::write(&path, text.replacen(from, to, 1)).unwrap();
    };
    let mut cache = Cache::memory("1.0.0");
    let mut started = None;
    let steps: [(&str, &dyn Fn()); 4] = [
        ("the first build", &|| {}),
        ("a class added", &|| {
            edit("templates/home.html", "p-4", "p-4 grid")
        }),
        ("a class removed", &|| {
            edit("templates/home.html", " underline", "")
        }),
        ("the stylesheet edited", &|| {
            edit("styles/site.css", "shadow-md", "shadow-md ring-1")
        }),
    ];
    for (step, change) in steps {
        change();
        let dev = build(&load(root), &mut cache, Mode::Dev);
        let fresh = build(&load(root), &mut Cache::memory("1.0.0"), Mode::Drafts);
        let (ours, theirs) = (stylesheet(&dev, "site"), stylesheet(&fresh, "site"));
        assert!(ours == theirs, "{step}:\n{ours}\n---\n{theirs}");
        let [folder] = watching().try_into().unwrap();
        // Only a class going away or a new stylesheet starts a new CLI.
        if step == "a class added" {
            assert_eq!(Some(&folder), started.as_ref(), "{step}");
        }
        started = Some(folder);
    }
    assert!(!stylesheet(&build(&load(root), &mut cache, Mode::Dev), "site").contains(".underline"));
    drop(cache);
    assert_eq!(
        watching(),
        Vec::<PathBuf>::new(),
        "a CLI outlived its cache"
    );
}
