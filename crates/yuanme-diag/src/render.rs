use annotate_snippets::{AnnotationKind, Group, Level as Lvl, Patch, Renderer, Snippet};
use serde_json::{Value, json};

use crate::{Applicability, ChildLevel, Diagnostic, Files, Level, Span};

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warning => "warning",
        }
    }
}

impl Diagnostic {
    /// One `--json` line, in the shape #21 fixed (ADR 0012).
    pub fn to_json(&self, files: &Files) -> Value {
        let spans: Vec<Value> = self
            .spans
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let mut span = span_json(files, &l.span);
                span["is_primary"] = json!(i == 0);
                span["label"] = json!(l.label);
                // rustc lists every line a span touches; #21 fixed one string,
                // the line the span starts on.
                span["text"] = json!(files.line_text(l.span.file, l.span.range.start));
                span
            })
            .collect();
        let children: Vec<Value> = self
            .children
            .iter()
            .map(|c| {
                let suggestions: Vec<Value> = c
                    .suggestions
                    .iter()
                    .map(|s| {
                        let edits: Vec<Value> = s
                            .edits
                            .iter()
                            .map(|e| {
                                json!({
                                    "file": files.path(e.span.file),
                                    "byte_start": e.span.range.start,
                                    "byte_end": e.span.range.end,
                                    "replacement": e.replacement,
                                })
                            })
                            .collect();
                        json!({ "applicability": applicability(s.applicability), "edits": edits })
                    })
                    .collect();
                json!({ "level": child_level(c.level), "message": c.message, "suggestions": suggestions })
            })
            .collect();
        let mut json = json!({
            "type": "diagnostic",
            "level": self.level.as_str(),
            "code": self.code.name,
            "message": self.message,
            "spans": spans,
            "children": children,
            "docs": format!("yuanme docs {}", self.code.name),
            "rendered": self.render(files, false),
        });
        // Sorted, as without serde_json's `preserve_order`, which a
        // dependency (oxc_resolver) may turn on for the whole build.
        json.sort_all_objects();
        json
    }

    /// rustc-style output with source snippets, for a terminal.
    pub fn render(&self, files: &Files, color: bool) -> String {
        let level = match self.level {
            Level::Error => Lvl::ERROR,
            Level::Warning => Lvl::WARNING,
        };
        let mut main = Group::with_title(
            level
                .primary_title(self.message.as_str())
                .id(self.code.name),
        );
        // One snippet per file, in the order files first appear among the spans.
        let mut seen = Vec::new();
        for label in &self.spans {
            if seen.contains(&label.span.file) {
                continue;
            }
            seen.push(label.span.file);
            let mut snippet =
                Snippet::source(files.text(label.span.file)).path(files.path(label.span.file));
            for (i, l) in self
                .spans
                .iter()
                .enumerate()
                .filter(|(_, l)| l.span.file == label.span.file)
            {
                let kind = if i == 0 {
                    AnnotationKind::Primary
                } else {
                    AnnotationKind::Context
                };
                let mut annotation = kind.span(l.span.range.clone());
                if let Some(text) = &l.label {
                    annotation = annotation.label(text.as_str());
                }
                snippet = snippet.annotation(annotation);
            }
            main = main.element(snippet);
        }
        let mut report = Vec::new();
        let mut fixes = Vec::new();
        for child in &self.children {
            let level = match child.level {
                ChildLevel::Help => Lvl::HELP,
                ChildLevel::Note => Lvl::NOTE,
            };
            if child.suggestions.is_empty() {
                main = main.element(level.message(child.message.as_str()));
                continue;
            }
            let mut fix = Group::with_title(level.secondary_title(child.message.as_str()));
            for suggestion in &child.suggestions {
                let first = &suggestion.edits[0].span;
                let mut snippet =
                    Snippet::source(files.text(first.file)).path(files.path(first.file));
                for edit in &suggestion.edits {
                    snippet = snippet.patch(Patch::new(
                        edit.span.range.clone(),
                        edit.replacement.as_str(),
                    ));
                }
                fix = fix.element(snippet);
            }
            fixes.push(fix);
        }
        main = main
            .element(Lvl::NOTE.message(format!("run `yuanme docs {}` for more", self.code.name)));
        report.push(main);
        report.extend(fixes);
        let renderer = if color {
            Renderer::styled()
        } else {
            Renderer::plain()
        };
        renderer.render(&report)
    }

    /// One line per Diagnostic, for output that isn't a terminal (#21).
    pub fn render_short(&self, files: &Files) -> String {
        let span = self.primary();
        let at = files.position(span.file, span.range.start);
        format!(
            "{}:{}:{}: {}[{}]: {}",
            files.path(span.file),
            at.line,
            at.column,
            self.level.as_str(),
            self.code.name,
            self.message
        )
    }
}

fn span_json(files: &Files, span: &Span) -> Value {
    let start = files.position(span.file, span.range.start);
    let end = files.position(span.file, span.range.end);
    json!({
        "file": files.path(span.file),
        "byte_start": span.range.start,
        "byte_end": span.range.end,
        "line_start": start.line,
        "column_start": start.column,
        "line_end": end.line,
        "column_end": end.column,
    })
}

fn child_level(level: ChildLevel) -> &'static str {
    match level {
        ChildLevel::Help => "help",
        ChildLevel::Note => "note",
    }
}

fn applicability(a: Applicability) -> &'static str {
    match a {
        Applicability::MachineApplicable => "machine-applicable",
        Applicability::MaybeIncorrect => "maybe-incorrect",
        Applicability::HasPlaceholders => "has-placeholders",
        Applicability::Unspecified => "unspecified",
    }
}
