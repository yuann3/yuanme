//! Math at build time (#17, #50), converted to MathML Core by math-core.

use math_core::{LatexToMathML, MathCoreConfig};

/// A converter for one Entry, with `macros` defined, each a name without
/// its backslash and a definition. The error is the index of the macro
/// that can't be read, and why.
pub(crate) fn converter(macros: &[(String, String)]) -> Result<LatexToMathML, (usize, String)> {
    let config = MathCoreConfig {
        // A `\newcommand` stays defined for the rest of the Entry (#17).
        global_group: true,
        ..Default::default()
    };
    LatexToMathML::new(config, macros.to_vec()).map_err(|(e, i, _)| (i, message(&e)))
}

/// math-core's message as a Diagnostic's: `Unknown command "\x".` reads
/// `unknown command "\x"`.
pub(crate) fn message(e: &math_core::LatexError) -> String {
    let m = e.error_message();
    let m = m.strip_suffix('.').unwrap_or(&m);
    let mut chars = m.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
}
