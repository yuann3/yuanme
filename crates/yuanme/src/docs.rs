//! `yuanme docs` and `.yuanme/docs/` (#21, #38): the topic pages, an index,
//! and one page per Diagnostic code, made from the code registry.
//!
//! The topic pages are Markdown in `docs/` beside this file, embedded at
//! compile time. #38 put them in `site/content/docs/`, but a published
//! crate ships only its own sources (`cargo xtask package-check`), and a
//! symlink isn't one on Windows, which releases build on. So the crate
//! holds them, and `cargo xtask docs` writes the docs site's copies.

use std::fs;
use std::io;
use std::path::Path;

use yuanme_diag::codes::{self, Code};

/// The hand-written pages by name, in the index's order (#21). Each starts
/// with `title:` and `description:` lines between `---` lines.
const TOPICS: &[(&str, &str)] = &[("getting-started", include_str!("docs/getting-started.md"))];

pub struct Page {
    pub title: String,
    /// One sentence, for the index.
    pub description: String,
    pub body: String,
}

impl Page {
    /// The page as `yuanme docs` prints it and `.yuanme/docs/` holds it.
    pub fn markdown(&self) -> String {
        format!("# {}\n\n{}", self.title, self.body)
    }
}

fn topic(name: &str, source: &str) -> Page {
    // A Windows checkout may have turned the file's line ends into CRLF.
    let source = source.replace("\r\n", "\n");
    let (front, body) = source
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---\n"))
        .unwrap_or_else(|| panic!("docs/{name}.md starts with front matter"));
    let field = |key: &str| {
        front
            .lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix(": "))
            .unwrap_or_else(|| panic!("docs/{name}.md has `{key}:`"))
            .to_string()
    };
    let body = body.trim_start().to_string();
    Page {
        title: field("title"),
        description: field("description"),
        body,
    }
}

/// The first sentence of a code's meaning, for the index.
fn summary(code: &Code) -> String {
    let first = code
        .meaning
        .split_once(". ")
        .map_or(code.meaning, |(s, _)| s);
    format!("{}.", first.trim_end_matches('.'))
}

fn code_page(code: &Code) -> Page {
    Page {
        title: code.name.to_string(),
        description: summary(code),
        body: format!(
            "{}\n\n## Example\n\n```\n{}\n```\n\n## Fix\n\n{}\n",
            code.meaning, code.example, code.fix
        ),
    }
}

/// A topic or code page by its name. The index is made from these.
pub fn get(name: &str) -> Option<Page> {
    if let Some((name, source)) = TOPICS.iter().find(|(n, _)| *n == name) {
        return Some(topic(name, source));
    }
    codes::ALL
        .iter()
        .find(|c| c.name == name)
        .map(|c| code_page(c))
}

/// The index, `yuanme docs` with no argument.
pub fn index() -> String {
    let mut out = String::from(
        "# yuanme docs\n\nRun `yuanme docs NAME` for a page; each one is also in `.yuanme/docs/NAME.md`.\n\n## Topics\n\n",
    );
    for (name, source) in TOPICS {
        out.push_str(&format!(
            "- `{name}`: {}\n",
            topic(name, source).description
        ));
    }
    out.push_str("\n## Diagnostic codes\n\n");
    for code in codes::ALL {
        out.push_str(&format!("- `{}`: {}\n", code.name, summary(code)));
    }
    out
}

/// A page by its name: a topic such as `templates`, a code such as
/// `math/unknown-label`, or `index`.
pub fn page(name: &str) -> Option<String> {
    match name {
        "index" => Some(index()),
        _ => get(name).map(|p| p.markdown()),
    }
}

/// Every page name, for "did you mean".
pub fn names() -> impl Iterator<Item = &'static str> {
    std::iter::once("index")
        .chain(TOPICS.iter().map(|(n, _)| *n))
        .chain(codes::ALL.iter().map(|c| c.name))
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
