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
