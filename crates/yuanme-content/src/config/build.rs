//! `[build]`, `[router]`, `[tailwind]`, `[typescript]` and `[agents]`:
//! how the Site is built.

use std::collections::BTreeMap;
use std::ops::Range;

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
pub(super) const DEFAULT_TARGETS: &[(&str, [u32; 3])] = &[
    ("chrome", [111, 0, 0]),
    ("firefox", [128, 0, 0]),
    ("safari", [16, 4, 0]),
];
