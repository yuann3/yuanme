//! Org into the Document (#49): the organic fork reads Org as Org 9.8.7's
//! org-element does (ADR 0008), and its tree is lowered here and dropped.
//! Org outside the Org subset is an error at its span, and the rest of the
//! file is still read, so every error in it is reported at once.

mod keywords;
mod lower;

use std::ops::Range;

use yuanme_diag::{Diagnostic, FileId, Files, Span, codes};
use yuanme_org::parser::{ParseError, parse_with_settings};
use yuanme_org::settings::{FileAccessInterface, GlobalSettings};

use crate::document::Document;

pub(crate) use keywords::head;

/// An Org keyword outside any element, as written: `#+KEY: value`.
#[derive(Debug)]
pub(crate) struct Keyword {
    pub key: String,
    pub key_at: Range<usize>,
    pub value: String,
    pub value_at: Range<usize>,
    /// Before the first headline or body text, where metadata goes.
    pub preamble: bool,
}

/// An Org file read: its Document, unless it couldn't be parsed, its
/// keywords, and what went wrong.
pub(crate) struct Read {
    pub document: Option<Document>,
    pub keywords: Vec<Keyword>,
    /// The file's preamble, where its fields are written.
    pub preamble: Range<usize>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses `file`, a whole Org file, outside any Site: for tests and tools.
/// The Document is there even when the file has errors, as long as it
/// parsed; an Entry with an error fails.
pub fn document(files: &Files, file: FileId) -> (Option<Document>, Vec<Diagnostic>) {
    let read = read(files.text(file), file, &[]);
    (read.document, read.diagnostics)
}

/// organic parses `#+SETUPFILE` itself, which reads a file. The Engine
/// reports the keyword as an error instead (#16), so the file is never read.
struct NoFiles;

impl FileAccessInterface for NoFiles {
    fn read_file(&self, _path: &str) -> Result<String, std::io::Error> {
        Ok(String::new())
    }
}

/// organic's recursion stops at 1 MiB of stack with `TooDeep`; this leaves
/// room above that for lowering the tree it built.
const STACK: usize = 8 << 20;

/// Reads `text`, an Org file, with the Site's math `macros`, on a thread of
/// its own: its stack is big enough for the deepest tree organic builds,
/// whatever thread the caller is on. A panic there goes on unwinding here,
/// for the caller's `catch_panic`.
pub(crate) fn read(text: &str, file: FileId, macros: &[(String, String)]) -> Read {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("org".into())
            .stack_size(STACK)
            .spawn_scoped(scope, || read_here(text, file, macros))
            .expect("start a thread to parse Org");
        worker
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

fn read_here(text: &str, file: FileId, macros: &[(String, String)]) -> Read {
    let settings = GlobalSettings {
        file_access: &NoFiles,
        ..GlobalSettings::default()
    };
    match parse_with_settings(text, &settings) {
        Ok(parsed) => lower::lower(text, file, &parsed, macros),
        Err(e) => {
            let d = match e {
                ParseError::OutOfFuel { opener } => Diagnostic::error(
                    &codes::ORG_OUT_OF_FUEL,
                    "this paragraph takes too long to parse",
                    paragraph(text, file, opener),
                )
                .help("constructs opened many times and never closed are almost always a mistake, such as `[[` or `*` repeated, or blocks that never end"),
                ParseError::TooDeep { opener } => Diagnostic::error(
                    &codes::ORG_TOO_DEEP,
                    "this paragraph nests too deeply to parse",
                    paragraph(text, file, opener),
                )
                .help("nest the blocks, lists or markup that start here less deeply"),
                // Org has no syntax errors, so this is a bug in the parser.
                ParseError::Failed(message) => panic!("organic failed to parse: {message}"),
            };
            Read {
                document: None,
                keywords: Vec::new(),
                preamble: 0..0,
                diagnostics: vec![d],
            }
        }
    }
}

/// From `opener` to the blank line that ends its paragraph, which a
/// runaway parse can't say itself.
fn paragraph(text: &str, file: FileId, opener: usize) -> Span {
    let start = opener.min(text.len());
    let mut end = text.len();
    let mut line = start;
    while line < text.len() {
        let next = text[line..].find('\n').map_or(text.len(), |i| line + i + 1);
        if line > start && text[line..next].trim().is_empty() {
            end = line;
            break;
        }
        line = next;
    }
    Span {
        file,
        range: start..text[start..end].trim_end().len() + start,
    }
}
