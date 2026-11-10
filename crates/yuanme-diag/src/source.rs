use std::ops::Range;

/// One file of the project, by its index in [`Files`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct FileId(u32);

/// A byte range in one original file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub file: FileId,
    pub range: Range<usize>,
}

/// The project's files, by path relative to the project root.
#[derive(Clone, Default)]
pub struct Files {
    files: Vec<(String, String)>,
}

/// A 1-based line and a 1-based column counted in Unicode scalar values, as rustc's are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Files {
    pub fn add(&mut self, path: impl Into<String>, text: impl Into<String>) -> FileId {
        self.files.push((path.into(), text.into()));
        FileId(u32::try_from(self.files.len() - 1).expect("fewer than 2^32 files"))
    }

    pub fn path(&self, file: FileId) -> &str {
        &self.files[file.0 as usize].0
    }

    pub fn text(&self, file: FileId) -> &str {
        &self.files[file.0 as usize].1
    }

    pub fn position(&self, file: FileId, offset: usize) -> Position {
        let before = &self.text(file)[..offset];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        Position {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }

    /// The whole line `offset` is on, without its line ending.
    pub fn line_text(&self, file: FileId, offset: usize) -> &str {
        let text = self.text(file);
        let start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
        let end = text[offset..].find('\n').map_or(text.len(), |i| offset + i);
        text[start..end].trim_end_matches('\r')
    }
}
