/// A Diagnostic code and its docs page. Codes never change meaning and are
/// never reused (ADR 0012).
///
/// `non_exhaustive` keeps codes from being created outside this crate, so
/// the `codes!` macro below is the only place a code can come from.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Code {
    pub name: &'static str,
    /// What the code means, as the first paragraph of its docs page.
    pub meaning: &'static str,
    /// A file that triggers the code, shown on the docs page.
    pub example: &'static str,
    /// How to fix it.
    pub fix: &'static str,
}

/// Defining a code here is the only way to create one, and the fields make
/// its docs page part of defining it.
macro_rules! codes {
    ($($id:ident = $name:literal { meaning: $meaning:literal, example: $example:literal, fix: $fix:literal $(,)? })*) => {
        $(pub const $id: Code = Code { name: $name, meaning: $meaning, example: $example, fix: $fix };)*
        /// Every code, for the docs and for the uniqueness test.
        pub const ALL: &[&Code] = &[$(&$id),*];
    };
}

codes! {
    CONFIG_SYNTAX = "config/syntax" {
        meaning: "`yuanme.toml` is not valid TOML, so nothing else in it can be read.",
        example: "[site]\nurl = https://example.com",
        fix: "Fix the TOML at the reported place. Here the string needs quotes: `url = \"https://example.com\"`.",
    }
    CONFIG_UNKNOWN_KEY = "config/unknown-key" {
        meaning: "Site config has a key yuanme doesn't know. Unknown keys are errors, not ignored, so a typo can't silently do nothing. Only `[site.extra]` takes keys of the Author's own.",
        example: "[site]\nurl = \"https://example.com\"\ntitel = \"My site\"",
        fix: "Use the key the help suggests, such as `title`, or move a key of your own into `[site.extra]`.",
    }
    CONFIG_MISSING_KEY = "config/missing-key" {
        meaning: "Site config leaves out a key that has no default.",
        example: "[site]\ntitle = \"My site\"",
        fix: "Add the key. Here every Site needs its address: `url = \"https://example.com\"`.",
    }
    CONFIG_WRONG_TYPE = "config/wrong-type" {
        meaning: "A key in Site config has a value of the wrong TOML type, such as a string where a number goes.",
        example: "[collections.posts]\nroute = \"/blog/{slug}/\"\npaginate = \"10\"",
        fix: "Write the value as the type the message names. Here `paginate = 10`, without quotes.",
    }
    CONFIG_OUT_OF_RANGE = "config/out-of-range" {
        meaning: "A number in Site config is outside the range its key allows.",
        example: "[collections.posts]\nroute = \"/blog/{slug}/\"\nlist_route = \"/blog/\"\npaginate = 0",
        fix: "Use a number inside the range the message names. Here `paginate` is at least 1.",
    }
    CONFIG_INVALID_VALUE = "config/invalid-value" {
        meaning: "A value in Site config has the right type but means nothing to yuanme: a URL that isn't absolute, a Route without its placeholder, an unknown field type, or a name that refers to nothing.",
        example: "[collections.posts]\nroute = \"/blog/\"",
        fix: "Change the value as the message says. Here an Entry's Route needs `{slug}`: `route = \"/blog/{slug}/\"`.",
    }
    CONFIG_ENGINE_VERSION = "config/engine-version" {
        meaning: "The project needs a different version of yuanme than the one running: `yuanme = \"X.Y\"` is the oldest compatible version, with Cargo's caret meaning, so 1.2 accepts any 1.x from 1.2 on and 0.3 accepts only 0.3.x.",
        example: "yuanme = \"9.0\"\n\n[site]\nurl = \"https://example.com\"",
        fix: "Install a version of yuanme that the project accepts, with `yuanme self update X.Y.Z`, or change `yuanme = ` if the project really works with the version you have.",
    }
    CONFIG_DEPRECATED = "config/deprecated" {
        meaning: "A key in Site config has been renamed. The old name keeps working until the next major release.",
        example: "Any key the release notes list as renamed.",
        fix: "Apply the machine-applicable fix, which renames the key; `yuanme check --json` reports it.",
    }
    INTERNAL_PANIC = "internal/panic" {
        meaning: "yuanme crashed while processing a file. This is a bug in yuanme, not in the project.",
        example: "Any file can trigger it, if it reaches a bug in a parser or a renderer.",
        fix: "Report it at https://github.com/yuann3/yuanme/issues with the file named in the message. Until it is fixed, changing the construct near the reported place usually avoids it.",
    }
}

/// The areas from #21. A code outside them is a typo.
pub const AREAS: &[&str] = &[
    "config", "entry", "org", "markdown", "math", "link", "template", "asset", "image", "budget",
    "host", "internal",
];
