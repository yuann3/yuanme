//! TypeScript checking with `tsc` 7 as a Managed tool (#60), through the
//! Engine's one seam. Each test runs the real `tsc`, which the Engine
//! downloads on first use, so they are ignored in the normal run; CI's
//! typescript job runs them.

use std::path::{Path, PathBuf};

use yuanme_engine::{Build, Cache, Inputs, Mode, build};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/sites")
        .join(name)
}

/// The fixture `name`, copied to a folder of its own, since the check
/// writes `.yuanme/` into the project.
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

/// The project at `root`, as the binary loads it.
fn load(root: &Path) -> Inputs {
    let mut inputs = Inputs::at(root.to_path_buf());
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
            if path.is_dir() {
                if !relative.starts_with('.') {
                    stack.push(path);
                }
                continue;
            }
            inputs.insert(relative, std::fs::read(&path).unwrap());
        }
    }
    inputs
}

/// Each Diagnostic as (code, file, the text its span covers, message).
fn found(build: &Build) -> Vec<(String, String, String, String)> {
    build
        .diagnostics
        .iter()
        .map(|d| {
            let span = d.primary();
            (
                d.code.name.to_string(),
                build.sources.path(span.file).to_string(),
                build.sources.text(span.file)[span.range.clone()].to_string(),
                d.message.clone(),
            )
        })
        .collect()
}

fn row(file: &str, text: &str, message: &str) -> (String, String, String, String) {
    let code = "asset/type-error".to_string();
    (code, file.into(), text.into(), message.into())
}

/// The fixture's four mistakes, each at the name or value `tsc` puts it
/// on, one of them after a character that is two UTF-16 units. Everything
/// else in it type-checks: a root-relative import, a JS module, a Data
/// file, a global from the Author's own `.d.ts`, `yuanme:router`, a
/// Router event and a Sidecar's `mount(ctx)`.
#[test]
#[ignore = "downloads tsc"]
fn type_errors_are_diagnostics_at_their_spans() {
    let dir = project("typescript");
    let built = build(&load(dir.path()), &mut Cache::memory("1.0.0"), Mode::Build);
    assert!(built.failed());
    assert_eq!(
        found(&built),
        [
            row(
                "scripts/site.ts",
                "year",
                "TS2322: Type 'string' is not assignable to type 'number'."
            ),
            row(
                "scripts/site.ts",
                "42",
                "TS2345: Argument of type 'number' is not assignable to parameter of type 'string'."
            ),
            row(
                "scripts/site.ts",
                "loud",
                "TS2322: Type 'string' is not assignable to type 'number'."
            ),
            row(
                "templates/partials/clock.ts",
                "toFixed",
                "TS2551: Property 'toFixed' does not exist on type 'string'. Did you mean 'fixed'?"
            ),
        ]
    );
}

/// The types yuanme writes are the ones the Scripts are checked against,
/// and `check` reports the same.
#[test]
#[ignore = "downloads tsc"]
fn a_project_without_type_errors_builds_and_checks() {
    let dir = project("typescript");
    let root = dir.path();
    let fix = |path: &str, from: &str, to: &str| {
        let path = root.join(path);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{from}");
        std::fs::write(&path, text.replace(from, to)).unwrap();
    };
    fix("scripts/site.ts", "\"2026\"", "2026");
    fix("scripts/site.ts", "greet(42)", "greet(\"42\")");
    fix("scripts/site.ts", "loud: number", "loud: string");
    fix("templates/partials/clock.ts", ".toFixed()", "");
    for mode in [Mode::Build, Mode::Check { drafts: false }] {
        let built = build(&load(root), &mut Cache::memory("1.0.0"), mode);
        assert_eq!(found(&built), []);
    }
    let types = std::fs::read_to_string(root.join(".yuanme/types/yuanme.d.ts")).unwrap();
    assert!(types.contains("declare module 'yuanme:router'"), "{types}");
}
