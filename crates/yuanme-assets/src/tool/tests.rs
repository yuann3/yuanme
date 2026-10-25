use std::fs;
use std::path::Path;
use std::process::Command;

use super::*;

/// SHA-256 of `hello\n`, from `printf 'hello\n' | shasum -a 256`.
const HELLO: &str = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";

fn url(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn tool(url: String, sha256: Sha256) -> Tool {
    Tool {
        name: "Example",
        file: "example".into(),
        version: "1.2.3".into(),
        // Unset, so a developer's own override can't reach the test.
        env: "YUANME_TEST_TOOL_BIN_NEVER_SET",
        urls: vec![url],
        sha256,
        archive: None,
    }
}

#[test]
fn a_download_is_checked_cached_and_then_run_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let release = dir.path().join("release");
    fs::write(&release, "hello\n").unwrap();
    let cache = dir.path().join("cache");

    let tool = tool(url(&release), Sha256::Known(HELLO.into()));
    let binary = tool.binary(&cache).unwrap();
    assert!(binary.starts_with(cache.join("example-1.2.3")));
    assert_eq!(fs::read_to_string(&binary).unwrap(), "hello\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&binary).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "{mode:o}");
    }

    // Offline, the cached copy is what runs.
    fs::remove_file(&release).unwrap();
    assert_eq!(tool.binary(&cache).unwrap(), binary);
}

/// TypeScript's `tsc` needs its `lib.*.d.ts` beside it, so its release is
/// a `.tgz` of a folder, which is unpacked whole.
#[test]
fn an_archive_is_unpacked_with_the_files_beside_its_binary() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("package/lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("example"), "hello\n").unwrap();
    fs::write(lib.join("lib.d.ts"), "declare var x: number;\n").unwrap();
    fs::write(dir.path().join("package/README.md"), "not kept\n").unwrap();
    let release = dir.path().join("example.tgz");
    let status = Command::new("tar")
        .current_dir(dir.path())
        .arg("-czf")
        .arg(&release)
        .arg("package")
        .status()
        .unwrap();
    assert!(status.success());
    let sha256 = hex(&Hasher::digest(fs::read(&release).unwrap()));
    let cache = dir.path().join("cache");

    let tool = Tool {
        archive: Some("package/lib"),
        ..tool(url(&release), Sha256::Known(sha256))
    };
    let binary = tool.binary(&cache).unwrap();
    assert_eq!(binary, cache.join("example-1.2.3/example"));
    assert_eq!(fs::read_to_string(&binary).unwrap(), "hello\n");
    let folder = cache.join("example-1.2.3");
    let mut kept = walk(&cache);
    kept.sort();
    assert_eq!(
        kept,
        [folder.join("example"), folder.join("lib.d.ts")].map(|p| p.display().to_string())
    );

    fs::remove_file(&release).unwrap();
    assert_eq!(tool.binary(&cache).unwrap(), binary);
}

/// `tsc`'s GitHub release layout isn't promised (#27), so the npm
/// registry's copy of the same file is a second place to get it.
#[test]
fn a_download_that_fails_is_tried_from_the_next_url() {
    let dir = tempfile::tempdir().unwrap();
    let release = dir.path().join("release");
    fs::write(&release, "hello\n").unwrap();
    let missing = dir.path().join("moved");

    let tool = Tool {
        urls: vec![url(&missing), url(&release)],
        ..tool(String::new(), Sha256::Known(HELLO.into()))
    };
    let binary = tool.binary(&dir.path().join("cache")).unwrap();
    assert_eq!(fs::read_to_string(binary).unwrap(), "hello\n");

    // With neither there, the error names both.
    let tool = Tool {
        urls: vec![url(&missing), url(&dir.path().join("gone"))],
        ..tool
    };
    let error = tool.binary(&dir.path().join("other")).unwrap_err();
    assert!(
        error.message.contains("/moved") && error.message.contains("/gone"),
        "{error:?}"
    );
}

#[test]
fn a_release_the_engine_pins_no_checksum_for_is_checked_against_its_list() {
    let dir = tempfile::tempdir().unwrap();
    let release = dir.path().join("example");
    fs::write(&release, "hello\n").unwrap();
    let sums = dir.path().join("sha256sums.txt");
    let other = "0".repeat(64);
    fs::write(&sums, format!("{other}  ./example-x\n{HELLO}  ./example\n")).unwrap();

    let tool = tool(url(&release), Sha256::Listed(url(&sums)));
    let binary = tool.binary(&dir.path().join("cache")).unwrap();
    assert_eq!(fs::read_to_string(binary).unwrap(), "hello\n");
}

#[test]
fn a_download_with_another_checksum_is_refused_and_not_kept() {
    let dir = tempfile::tempdir().unwrap();
    let release = dir.path().join("release");
    fs::write(&release, "hello, tampered\n").unwrap();
    let cache = dir.path().join("cache");

    let tool = tool(url(&release), Sha256::Known(HELLO.into()));
    let error = tool.binary(&cache).unwrap_err();
    assert!(
        error.message.contains(HELLO) && error.message.contains("SHA-256"),
        "{error:?}"
    );
    let kept: Vec<_> = walk(&cache);
    assert!(kept.is_empty(), "{kept:?}");
}

#[test]
fn offline_the_error_says_so_and_names_the_override() {
    let dir = tempfile::tempdir().unwrap();
    // Nothing listens on the discard port.
    let tool = tool(
        "http://127.0.0.1:9/example".into(),
        Sha256::Known(HELLO.into()),
    );
    let error = tool.binary(dir.path()).unwrap_err();
    assert!(
        error
            .message
            .starts_with("can't download Example 1.2.3 from http://127.0.0.1:9/example: "),
        "{error:?}"
    );
    assert!(
        error.help.contains("YUANME_TEST_TOOL_BIN_NEVER_SET"),
        "{error:?}"
    );
}

fn walk(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path.display().to_string());
        }
    }
    out
}
