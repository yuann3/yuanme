use std::ops::Range;

/// One file of the project, by its index in [`Files`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(u32);

/// A byte range in one original file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub file: FileId,
    pub range: Range<usize>,
}

/// The project's files, by path relative to the project root.
#[derive(Default)]
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

/// Maps byte ranges in text the Engine derived from a file, such as a Template
/// rewritten for scoped styles, back to that file, so a Diagnostic always
/// points at what the Author wrote.
pub struct SourceMap {
    file: FileId,
    pieces: Vec<Piece>,
}

struct Piece {
    derived: usize,
    len: usize,
    original: usize,
    /// Inserted text has no original bytes: every offset in it maps to the
    /// point where it was inserted.
    copied: bool,
}

/// Builds derived text and its [`SourceMap`] together, so they can't disagree.
pub struct SourceMapBuilder<'a> {
    original: &'a str,
    file: FileId,
    out: String,
    pieces: Vec<Piece>,
    /// Where in the original the next inserted text sits.
    cursor: usize,
}

impl<'a> SourceMapBuilder<'a> {
    pub fn new(file: FileId, original: &'a str) -> Self {
        Self {
            original,
            file,
            out: String::new(),
            pieces: Vec::new(),
            cursor: 0,
        }
    }

    /// Appends `original[range]` unchanged.
    pub fn copy(&mut self, range: Range<usize>) {
        // `SourceMap::span` relies on pieces' original offsets only growing;
        // a copy from earlier in the file would give spans that end before they start.
        debug_assert!(
            range.start >= self.cursor,
            "copies must be in original order"
        );
        self.push(range.len(), range.start, true);
        self.out.push_str(&self.original[range.clone()]);
        self.cursor = range.end;
    }

    /// Appends text the Author didn't write.
    pub fn insert(&mut self, text: &str) {
        self.push(text.len(), self.cursor, false);
        self.out.push_str(text);
    }

    fn push(&mut self, len: usize, original: usize, copied: bool) {
        if len > 0 {
            self.pieces.push(Piece {
                derived: self.out.len(),
                len,
                original,
                copied,
            });
        }
    }

    pub fn finish(self) -> (String, SourceMap) {
        (
            self.out,
            SourceMap {
                file: self.file,
                pieces: self.pieces,
            },
        )
    }
}

impl SourceMap {
    pub fn span(&self, derived: Range<usize>) -> Span {
        let start = self.offset(derived.start, |p| derived.start < p.derived + p.len);
        let end = if derived.is_empty() {
            start
        } else {
            self.offset(derived.end, |p| derived.end <= p.derived + p.len)
        };
        Span {
            file: self.file,
            range: start..end,
        }
    }

    fn offset(&self, at: usize, ends_after: impl Fn(&Piece) -> bool) -> usize {
        match self.pieces.iter().find(|p| ends_after(p)) {
            Some(p) if p.copied => p.original + at.saturating_sub(p.derived),
            Some(p) => p.original,
            None => self.pieces.last().map_or(0, |p| {
                if p.copied {
                    p.original + p.len
                } else {
                    p.original
                }
            }),
        }
    }
}
