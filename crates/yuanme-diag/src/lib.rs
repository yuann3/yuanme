//! The one Diagnostic type every stage reports with, shaped like rustc's
//! (ADR 0012), and the ways it is printed.

pub mod codes;
mod render;
mod source;
#[cfg(test)]
mod tests;

use std::panic::{AssertUnwindSafe, catch_unwind};

pub use codes::Code;
pub use source::{FileId, Files, Position, SourceMap, SourceMapBuilder, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildLevel {
    Help,
    Note,
}

/// rustc's applicability: whether a tool may apply a fix without a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub level: Level,
    pub code: &'static Code,
    pub message: String,
    /// The first is the primary span.
    pub spans: Vec<Label>,
    pub children: Vec<Child>,
}

#[derive(Clone, Debug)]
pub struct Label {
    pub span: Span,
    pub label: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Child {
    pub level: ChildLevel,
    pub message: String,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub applicability: Applicability,
    pub edits: Vec<Edit>,
}

#[derive(Clone, Debug)]
pub struct Edit {
    pub span: Span,
    pub replacement: String,
}

impl Diagnostic {
    pub fn error(code: &'static Code, message: impl Into<String>, span: Span) -> Self {
        Self::new(Level::Error, code, message.into(), span)
    }

    pub fn warning(code: &'static Code, message: impl Into<String>, span: Span) -> Self {
        Self::new(Level::Warning, code, message.into(), span)
    }

    fn new(level: Level, code: &'static Code, message: String, span: Span) -> Self {
        Self {
            level,
            code,
            message,
            spans: vec![Label { span, label: None }],
            children: Vec::new(),
        }
    }

    /// Labels the primary span.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.spans[0].label = Some(label.into());
        self
    }

    pub fn secondary(mut self, span: Span, label: impl Into<String>) -> Self {
        self.spans.push(Label {
            span,
            label: Some(label.into()),
        });
        self
    }

    pub fn note(self, message: impl Into<String>) -> Self {
        self.child(ChildLevel::Note, message.into(), Vec::new())
    }

    pub fn help(self, message: impl Into<String>) -> Self {
        self.child(ChildLevel::Help, message.into(), Vec::new())
    }

    /// Help that carries one fix: `edits` applied together.
    pub fn suggest(
        self,
        message: impl Into<String>,
        applicability: Applicability,
        edits: Vec<Edit>,
    ) -> Self {
        // The rendered fix shows one file's snippet, so a fix that spanned
        // files would show a person half of what `--json` applies.
        assert!(
            !edits.is_empty() && edits.iter().all(|e| e.span.file == edits[0].span.file),
            "a suggestion edits one file"
        );
        self.child(
            ChildLevel::Help,
            message.into(),
            vec![Suggestion {
                applicability,
                edits,
            }],
        )
    }

    fn child(mut self, level: ChildLevel, message: String, suggestions: Vec<Suggestion>) -> Self {
        self.children.push(Child {
            level,
            message,
            suggestions,
        });
        self
    }

    pub fn primary(&self) -> &Span {
        &self.spans[0].span
    }
}

/// Runs one task, such as parsing or rendering one file, and turns a panic
/// into an `internal/panic` Diagnostic on that file, so one bad file can't
/// take the build down and every other error is still reported (#24).
pub fn catch_panic<T>(
    files: &Files,
    file: FileId,
    task: impl FnOnce() -> T,
) -> Result<T, Diagnostic> {
    // A task owns everything it writes and returns it, so nothing half-updated
    // outlives the unwind; that is why AssertUnwindSafe is sound here. The
    // default panic hook has already printed the message and location to
    // stderr by the time this returns, so the panic is never silent.
    catch_unwind(AssertUnwindSafe(task)).map_err(|payload| {
        let reason = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        Diagnostic::error(
            &codes::INTERNAL_PANIC,
            format!("yuanme crashed while processing `{}`", files.path(file)),
            Span { file, range: 0..0 },
        )
        .note(format!("the panic was: {reason}"))
        .help(
            "this is a bug in yuanme: please report it at https://github.com/yuann3/yuanme/issues",
        )
    })
}

/// Two edits in one file that touch the same bytes, so they can't both apply.
#[derive(Debug, PartialEq, Eq)]
pub struct OverlappingEdits;

/// Applies every `machine-applicable` edit for `file`. Tests use it to prove
/// that a fix leaves the file valid (ADR 0012): apply, then check again.
pub fn apply_fixes(
    files: &Files,
    file: FileId,
    diagnostics: &[Diagnostic],
) -> Result<String, OverlappingEdits> {
    let mut edits: Vec<&Edit> = diagnostics
        .iter()
        .flat_map(|d| &d.children)
        .flat_map(|c| &c.suggestions)
        .filter(|s| s.applicability == Applicability::MachineApplicable)
        .flat_map(|s| &s.edits)
        .filter(|e| e.span.file == file)
        .collect();
    edits.sort_by_key(|e| (e.span.range.start, e.span.range.end));
    // `check` renders every Page, so one Template mistake reaches here once per
    // Page with the same fix; identical edits are one edit, not a conflict.
    edits.dedup_by(|a, b| a.span == b.span && a.replacement == b.replacement);
    if edits
        .windows(2)
        .any(|w| w[0].span.range.end > w[1].span.range.start)
    {
        return Err(OverlappingEdits);
    }
    let text = files.text(file);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for edit in edits {
        out.push_str(&text[at..edit.span.range.start]);
        out.push_str(&edit.replacement);
        at = edit.span.range.end;
    }
    out.push_str(&text[at..]);
    Ok(out)
}
