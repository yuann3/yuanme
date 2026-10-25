//! `yuanme docs` and `.yuanme/docs/` (#21): one page per Diagnostic code,
//! made from the code registry, and an index. #71 adds the topic pages.

use std::fs;
use std::io;
use std::path::Path;

use yuanme_diag::codes;

/// The index, `yuanme docs` with no argument.
pub fn index() -> String {
    let mut out = String::from(
        "# yuanme docs\n\nRun `yuanme docs CODE` for a page; each one is also in `.yuanme/docs/CODE.md`.\n\n## Diagnostic codes\n\n",
    );
    for code in codes::ALL {
        let first = code
            .meaning
            .split_once(". ")
            .map_or(code.meaning, |(s, _)| s);
        let first = first.trim_end_matches('.');
        out.push_str(&format!("- `{}`: {first}.\n", code.name));
    }
    out
}

/// A page by its name: a code such as `math/unknown-label`, or `index`.
pub fn page(name: &str) -> Option<String> {
    if name == "index" {
        return Some(index());
    }
    let code = codes::ALL.iter().find(|c| c.name == name)?;
    Some(format!(
        "# {}\n\n{}\n\n## Example\n\n```\n{}\n```\n\n## Fix\n\n{}\n",
        code.name, code.meaning, code.example, code.fix
    ))
}

/// Every page name, for "did you mean".
pub fn names() -> impl Iterator<Item = &'static str> {
    std::iter::once("index").chain(codes::ALL.iter().map(|c| c.name))
}

/// Writes every page to `dir` unless its `VERSION` already says `version`,
/// so the files always match the binary that runs in the project (#21).
pub fn write(dir: &Path, version: &str) -> io::Result<()> {
    let stamp = dir.join("VERSION");
    if fs::read_to_string(&stamp).is_ok_and(|v| v.trim() == version) {
        return Ok(());
    }
    // A page an older Engine had and this one doesn't must go too.
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    for name in names() {
        let path = dir.join(format!("{name}.md"));
        fs::create_dir_all(path.parent().expect("in the docs folder"))?;
        fs::write(&path, page(name).expect("a page of every name"))?;
    }
    fs::write(stamp, format!("{version}\n"))
}
