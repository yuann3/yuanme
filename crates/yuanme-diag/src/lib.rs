//! The one Diagnostic type every stage reports with, shaped like rustc's
//! (ADR 0012), and the ways it is printed.

/// Every code, by name: a table, so it stays a module.
pub mod codes;
mod diagnostic;
mod render;
mod source;
#[cfg(test)]
mod tests;

pub use codes::Code;
pub use diagnostic::{Applicability, Diagnostic, Edit, Level, apply_fixes, catch_panic};
pub use source::{FileId, Files, Span};
