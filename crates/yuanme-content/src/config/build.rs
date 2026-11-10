//! `[build]`, `[router]`, `[tailwind]`, `[typescript]` and `[agents]`:
//! how the Site is built.

use std::collections::BTreeMap;
use std::ops::Range;

use super::check::parse_browser_version;
use super::{Table, as_str, entries, string, table};

/// How a visitor moves between Pages (#20, ADR 0007).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouterMode {
    /// Full page loads.
    Off,
    /// Full page loads with cross-document View Transitions, and no JS.
    Native,
    /// The Router on every Page.
    Swap,
}

#[derive(Debug)]
pub struct Build {
    /// The oldest version of each browser the Site supports, as
    /// `[major, minor, patch]`, by its key in `[build.targets]`. A browser
    /// left out is not supported.
    pub targets: BTreeMap<String, [u32; 3]>,
}

/// `[tailwind]` (#20).
#[derive(Debug, Default)]
pub struct Tailwind {
    /// The Tailwind CLI release, such as `4.3.3`, and where its value is.
    /// `None` runs the one the Engine pins.
    pub version: Option<(String, Range<usize>)>,
}

/// `[typescript]` (#20).
#[derive(Debug, Default)]
pub struct TypeScript {
    /// Where `check = true` is, when it is: `tsc` checks the Scripts.
    pub check: Option<Range<usize>>,
}

/// `[agents]` (#21).
#[derive(Debug)]
pub struct Agents {
    /// `dev`, `build` and `check` rewrite the yuanme block in `AGENTS.md`
    /// when another version wrote it.
    pub manage_agents_md: bool,
}

/// Tailwind v4's baseline (#20), so a Site with Tailwind gets the same
/// output whichever runs lightningcss.
const DEFAULT_TARGETS: &[(&str, [u32; 3])] = &[
    ("chrome", [111, 0, 0]),
    ("firefox", [128, 0, 0]),
    ("safari", [16, 4, 0]),
];

pub(super) fn read(build: Option<&Table<'_>>) -> Build {
    let targets = match build.and_then(|b| table(b, "targets")) {
        Some(targets) => entries(Some(targets))
            .iter()
            .map(|(browser, version)| {
                let version = as_str(version);
                (
                    browser.get_ref().to_string(),
                    parse_browser_version(version).expect("the first pass checked the version"),
                )
            })
            .collect(),
        None => DEFAULT_TARGETS
            .iter()
            .map(|(browser, version)| (browser.to_string(), *version))
            .collect(),
    };
    Build { targets }
}

pub(super) fn router(router: Option<&Table<'_>>) -> RouterMode {
    match router.and_then(|r| string(r, "mode")).as_deref() {
        None | Some("off") => RouterMode::Off,
        Some("native") => RouterMode::Native,
        Some("swap") => RouterMode::Swap,
        Some(_) => unreachable!("checked as a choice"),
    }
}

pub(super) fn tailwind(tailwind: Option<&Table<'_>>) -> Tailwind {
    Tailwind {
        version: tailwind
            .and_then(|t| t.get("version"))
            .map(|v| (as_str(v).to_string(), v.span())),
    }
}

pub(super) fn typescript(typescript: Option<&Table<'_>>) -> TypeScript {
    TypeScript {
        check: typescript
            .and_then(|t| t.get("check"))
            .filter(|v| v.get_ref().as_bool() == Some(true))
            .map(|v| v.span()),
    }
}

pub(super) fn agents(agents: Option<&Table<'_>>) -> Agents {
    Agents {
        manage_agents_md: agents
            .and_then(|t| t.get("manage_agents_md"))
            .is_none_or(|v| v.get_ref().as_bool() == Some(true)),
    }
}
