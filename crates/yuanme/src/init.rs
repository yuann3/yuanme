//! `yuanme init` (#21, #36, #25, #37): a Site from a Starter, the files
//! coding agents read in it, and with `--host`, the files that deploy it.
//! It asks nothing and overwrites nothing.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use clap::ValueEnum;
use yuanme_content::config::Adapter;
use yuanme_engine::Mode;

use crate::events::Event;
use crate::{ENGINE_VERSION, Error, Out, docs, schemas};

/// The Starters embedded in the binary (#36). Each is its files, from the
/// project root; `init` adds the `#:schema` line and `yuanme = "X.Y"` to
/// the top of its `yuanme.toml`.
#[derive(Clone, Copy, ValueEnum)]
pub enum Starter {
    /// `yuanme.toml`, one Layout, a home Page, a 404 Page and a stylesheet.
    Minimal,
}

impl Starter {
    fn name(self) -> &'static str {
        match self {
            Starter::Minimal => "minimal",
        }
    }

    fn files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Starter::Minimal => &[
                ("yuanme.toml", include_str!("init/minimal/yuanme.toml")),
                (
                    "templates/layouts/base.html",
                    include_str!("init/minimal/templates/layouts/base.html"),
                ),
                (
                    "templates/home.html",
                    include_str!("init/minimal/templates/home.html"),
                ),
                (
                    "templates/404.html",
                    include_str!("init/minimal/templates/404.html"),
                ),
                (
                    "styles/site.css",
                    include_str!("init/minimal/styles/site.css"),
                ),
            ],
        }
    }
}

const BEGIN: &str = "<!-- BEGIN:yuanme -->\n";
const END: &str = "<!-- END:yuanme -->\n";
/// The line after `BEGIN`, up to the version that wrote the block.
const WRITTEN_BY: &str = "<!-- Written by yuanme ";

/// The managed block of `AGENTS.md` (#21): at most 8 KB, as Vercel's
/// docs index that scored best in their evals.
fn block(version: &str) -> String {
    let mut out = format!(
        "{BEGIN}{WRITTEN_BY}{version}, which rewrites this block when another version runs here. Write your own notes outside it. -->\n\n{}",
        include_str!("init/agents-block.md")
    );
    for (name, description) in docs::topics() {
        out.push_str(&format!("- `.yuanme/docs/{name}.md`: {description}\n"));
    }
    out.push_str("\nEach Diagnostic code has a page too, such as `.yuanme/docs/entry/unknown-key.md` for `entry/unknown-key`.\n");
    out.push_str(END);
    out
}

/// `text` with the yuanme block replaced, or added at its end.
fn with_block(text: &str, version: &str) -> String {
    if let Some((before, rest)) = text.split_once(BEGIN)
        && let Some((_, after)) = rest.split_once(END)
    {
        return format!("{before}{}{after}", block(version));
    }
    match text {
        "" => block(version),
        t if t.ends_with('\n') => format!("{t}\n{}", block(version)),
        t => format!("{t}\n\n{}", block(version)),
    }
}

/// The version that wrote `text`'s yuanme block, if it has a whole one.
fn block_version(text: &str) -> Option<&str> {
    let (_, rest) = text.split_once(BEGIN)?;
    let (block, _) = rest.split_once(END)?;
    block
        .strip_prefix(WRITTEN_BY)?
        .split_once(',')
        .map(|(v, _)| v)
}

/// The two workflow skills (#21), in `.agents/skills/` for every client
/// that reads the Agent Skills standard, and in `.claude/skills/`, the
/// only place Claude Code reads.
const SKILLS: &[(&str, &str)] = &[
    ("yuanme-verify", include_str!("init/yuanme-verify.md")),
    ("yuanme-migrate", include_str!("init/yuanme-migrate.md")),
];
const SKILL_DIRS: &[&str] = &[".agents/skills", ".claude/skills"];
const HASH_KEY: &str = "  yuanme-hash: ";

/// A skill as yuanme writes it, with the version that wrote it and a hash
/// of the rest of the file in its `metadata`, so a later version can tell
/// whether the Author edited it.
fn skill(source: &str, version: &str) -> String {
    let (front, body) = source
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---\n"))
        .expect("a skill starts with front matter");
    let unhashed = format!("---\n{front}\nmetadata:\n  yuanme-version: \"{version}\"\n---\n{body}");
    let hash = blake3::hash(unhashed.as_bytes()).to_hex();
    unhashed.replacen("\n---\n", &format!("\n{HASH_KEY}\"{hash}\"\n---\n"), 1)
}

/// Whether `text` is a skill another version of yuanme wrote and no one
/// changed since: the hash it keeps is still that of the rest of it.
fn stale_skill(text: &str) -> bool {
    let text = text.replace("\r\n", "\n");
    let mut hash = None;
    let mut rest = String::new();
    for line in text.split_inclusive('\n') {
        match line.strip_prefix(HASH_KEY) {
            Some(h) => hash = Some(h.trim().trim_matches('"')),
            None => rest.push_str(line),
        }
    }
    let current = format!("  yuanme-version: \"{ENGINE_VERSION}\"\n");
    hash.is_some_and(|h| blake3::hash(rest.as_bytes()).to_hex().as_str() == h)
        && !rest.contains(&current)
}

/// Rewrites the files `init` wrote for agents that another version of
/// yuanme wrote, as `dev`, `build` and `check` do (#21): the block of
/// `AGENTS.md` unless `[agents] manage_agents_md = false`, and each skill
/// the Author hasn't edited. It never makes one that isn't there.
pub fn refresh(root: &Path, manage_agents_md: bool, out: &Out) -> io::Result<()> {
    let agents = root.join("AGENTS.md");
    if manage_agents_md
        && let Ok(text) = fs::read_to_string(&agents)
        && block_version(&text).is_some_and(|v| v != ENGINE_VERSION)
    {
        fs::write(&agents, with_block(&text, ENGINE_VERSION))?;
        out.say(&format!(
            "updated AGENTS.md's yuanme block for yuanme {ENGINE_VERSION}"
        ));
    }
    for dir in SKILL_DIRS {
        for (name, source) in SKILLS {
            let path = format!("{dir}/{name}/SKILL.md");
            if fs::read_to_string(root.join(&path)).is_ok_and(|t| stale_skill(&t)) {
                fs::write(root.join(&path), skill(source, ENGINE_VERSION))?;
                out.say(&format!("updated {path} for yuanme {ENGINE_VERSION}"));
            }
        }
    }
    Ok(())
}

/// A Worker's or a Pages project's name, from the Site's folder: lowercase
/// letters, digits and `-`, at most 58, which Pages allows and Workers
/// allows more of.
fn host_name(root: &Path) -> String {
    let folder = root
        .file_name()
        .map_or(String::new(), |f| f.to_string_lossy().to_lowercase());
    let mut name = String::new();
    for c in folder.chars() {
        match c {
            'a'..='z' | '0'..='9' => name.push(c),
            _ if !name.is_empty() && !name.ends_with('-') => name.push('-'),
            _ => {}
        }
    }
    name.truncate(58);
    match name.trim_end_matches('-') {
        "" => "site".into(),
        n => n.into(),
    }
}

/// `wrangler.jsonc`'s, fixed rather than the day `init` ran, so that what
/// `init` writes depends only on the Engine. A static-assets Worker runs
/// no code that a later date would change.
const COMPATIBILITY_DATE: &str = "2026-10-30";

/// The files `--host` writes besides `[host] adapter`, by path (#25, #37).
fn host_files(adapter: Adapter, root: &Path) -> Vec<(&'static str, String)> {
    let workflow = match adapter {
        Adapter::Cloudflare => include_str!("init/deploy/cloudflare.yml"),
        Adapter::Netlify => include_str!("init/deploy/netlify.yml"),
        Adapter::Vercel => include_str!("init/deploy/vercel.yml"),
        Adapter::GithubPages => include_str!("init/deploy/github-pages.yml"),
        Adapter::CloudflarePages => include_str!("init/deploy/cloudflare-pages.yml"),
        // A server of the Author's own, which no workflow can know.
        Adapter::Static => return Vec::new(),
    };
    let name = host_name(root);
    let fill = |text: &str| {
        text.replace("@BUILD@\n", include_str!("init/deploy/build.yml"))
            .replace("@VERSION@", release())
            .replace("@NAME@", &name)
            .replace("@DATE@", COMPATIBILITY_DATE)
    };
    let mut files = vec![
        (".github/workflows/deploy.yml", fill(workflow)),
        (
            ".github/dependabot.yml",
            include_str!("init/dependabot.yml").to_string(),
        ),
    ];
    if adapter == Adapter::Cloudflare {
        files.push(("wrangler.jsonc", fill(include_str!("init/wrangler.jsonc"))));
    }
    files
}

/// The release the Engine is or was built from: its version without the
/// commit a checkout adds.
fn release() -> &'static str {
    ENGINE_VERSION.split('+').next().expect("a version")
}

/// What `init` does to one file.
enum Write {
    /// The file mustn't exist.
    Create(String),
    /// The file may exist, and this is its new text.
    Update(String),
}

pub fn run(
    cwd: &Path,
    dir: Option<&Path>,
    starter: Starter,
    host: Option<Adapter>,
    out: &Out,
) -> Result<u8, Error> {
    let shown = dir.map_or(".".to_string(), |d| d.display().to_string());
    let root = cwd.join(dir.unwrap_or(Path::new(".")));
    if root.exists() && !root.is_dir() {
        return Err(Error::BadCommand(format!(
            "{shown} is a file, not a folder"
        )));
    }
    let read = |path: &str| match fs::read_to_string(root.join(path)) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    };

    let mut plan: BTreeMap<String, Write> = BTreeMap::new();
    for (path, text) in starter.files() {
        let text = match *path {
            "yuanme.toml" => {
                let minor = release().rsplit_once('.').expect("X.Y.Z").0;
                let mut text = format!(
                    "#:schema ./.yuanme/schemas/yuanme.toml.json\nyuanme = \"{minor}\"\n\n{text}"
                );
                if let Some(adapter) = host {
                    text.push_str(&format!("\n[host]\nadapter = \"{}\"\n", adapter.name()));
                }
                text
            }
            _ => text.to_string(),
        };
        plan.insert(path.to_string(), Write::Create(text));
    }
    for (path, text) in host.map_or(Vec::new(), |a| host_files(a, &root)) {
        plan.insert(path.to_string(), Write::Create(text));
    }
    for dir in SKILL_DIRS {
        for (name, source) in SKILLS {
            let text = skill(source, ENGINE_VERSION);
            plan.insert(format!("{dir}/{name}/SKILL.md"), Write::Create(text));
        }
    }
    let agents = read("AGENTS.md")?;
    plan.insert(
        "AGENTS.md".into(),
        Write::Update(with_block(agents.as_deref().unwrap_or(""), ENGINE_VERSION)),
    );
    if read("CLAUDE.md")?.is_none() {
        plan.insert("CLAUDE.md".into(), Write::Create("@AGENTS.md\n".into()));
    }
    // Everything under `.yuanme/` is made again, and the output directory
    // is the build's.
    let ignore = match host {
        // `vercel link` and `vercel pull` write there too.
        Some(Adapter::Vercel) => [".yuanme/", ".vercel/"],
        _ => [".yuanme/", "dist/"],
    };
    let gitignore = match read(".gitignore")? {
        None => format!(
            "# What yuanme writes: its caches, docs and schemas, and the built Site.\n{}\n",
            ignore.join("\n")
        ),
        Some(text) => {
            let mut text = text;
            for line in ignore {
                if !text.lines().any(|l| l.trim() == line) {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push_str(&format!("{line}\n"));
                }
            }
            text
        }
    };
    plan.insert(".gitignore".into(), Write::Update(gitignore));

    let conflicts: Vec<&str> = plan
        .iter()
        .filter(|(path, write)| matches!(write, Write::Create(_)) && root.join(path).exists())
        .map(|(path, _)| path.as_str())
        .collect();
    if !conflicts.is_empty() {
        let files = match conflicts.len() {
            1 => "1 file".to_string(),
            n => format!("{n} files"),
        };
        return Err(Error::BadCommand(format!(
            "yuanme init would overwrite {files}, so it changed nothing:\n  {}",
            conflicts.join("\n  ")
        )));
    }

    let mut created = Vec::new();
    let mut updated = Vec::new();
    for (path, write) in plan {
        let file = root.join(&path);
        let text = match write {
            Write::Create(text) => {
                created.push(path);
                text
            }
            Write::Update(text) => {
                match read(&path)? {
                    Some(old) if old == text => continue,
                    Some(_) => updated.push(path),
                    None => created.push(path),
                }
                text
            }
        };
        fs::create_dir_all(file.parent().expect("in the project"))?;
        fs::write(file, text)?;
    }

    // What an agent reads before its first build.
    docs::write(&root.join(".yuanme/docs"), ENGINE_VERSION)?;
    yuanme_engine::write_types(&root)?;
    let run = crate::engine_build(&root, None, Mode::Check { drafts: false }, false, false)?;
    schemas::write(
        &root.join(".yuanme/schemas"),
        run.build.config.as_ref(),
        run.build.site.as_ref(),
    )?;

    if out.json {
        out.event(Event::SiteCreated {
            dir: shown,
            starter: starter.name(),
            host: host.map(Adapter::name),
            created,
            updated,
        });
        return Ok(0);
    }
    let mut text = match shown.as_str() {
        "." => format!("created a Site from the {} Starter:\n", starter.name()),
        d => format!(
            "created a Site in {d} from the {} Starter:\n",
            starter.name()
        ),
    };
    for path in &created {
        text.push_str(&format!("  {path}\n"));
    }
    if !updated.is_empty() {
        text.push_str("updated:\n");
        for path in &updated {
            text.push_str(&format!("  {path}\n"));
        }
    }
    text.push_str(&match shown.as_str() {
        "." => "next: yuanme dev".to_string(),
        d => format!("next: cd {d} && yuanme dev"),
    });
    out.say(&text);
    Ok(0)
}
