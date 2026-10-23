//! Whether an opener's closer occurs before its paragraph ends.
//!
//! Upstream parses an object such as `*bold*` or `[fn:: note]` by scanning
//! for its end and re-parsing what it scanned. When the closer never comes,
//! the attempt fails after that work, and plain text then tries the openers
//! inside it again, so a line of unclosed openers took exponential time
//! (#30). Each such parser now asks here first, and fails at once when its
//! closer can't be ahead. The answer may be a wrong "yes", which only costs
//! time; a wrong "no" would change the tree.
//!
//! Each answer is remembered as a scanned range, so plain text asking at
//! every character of a long paragraph scans it once, not once per opener.
//! A bracket's closer depends on depth, so a range can't be reused for it;
//! instead each input gets a table of where every bracket closes. Both live
//! in thread-locals because `OrgSource` is a `Copy` value with nowhere to
//! keep them.
//!
//! Blocks, drawers and LaTeX environments ask the same of their end lines;
//! see `end_follows`.

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;

use super::org_source::OrgSource;
use super::util::in_section;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Closer {
    /// A text markup marker that can end markup: after at least one
    /// character of contents, not after whitespace, and before a POST
    /// character or the end of a line.
    Markup(u8),
    /// This text.
    Text(&'static str),
}

/// The last scan for one closer: from `from`, the first closer or the end of
/// the paragraph was at `until`.
struct Scan {
    closer: Closer,
    to_blank_line: bool,
    source: (usize, usize),
    from: usize,
    until: usize,
    found: bool,
}

/// For one input and one kind of bracket, where each position's bracket
/// closes and where each position's paragraph ends.
struct Brackets {
    source: (usize, usize),
    open: u8,
    /// The first `close` at or after each position that isn't matched by an
    /// `open` after that position, or `NONE`.
    closes_at: Vec<u32>,
    /// The line break that starts the next blank line, or the input's end.
    blank_line_at: Vec<u32>,
}

const NONE: u32 = u32::MAX;

/// For one input, a line's start and the last tag divider on that line.
#[derive(Clone, Copy)]
struct LastDivider {
    source: (usize, usize),
    line_start: usize,
    at: Option<usize>,
}

/// Inputs re-parsed inside objects each get a table; this many are kept.
const KEEP_TABLES: usize = 8;

thread_local! {
    static SCANS: RefCell<Vec<Scan>> = const { RefCell::new(Vec::new()) };
    static TABLES: RefCell<Vec<Brackets>> = const { RefCell::new(Vec::new()) };
    static LAST_DIVIDER: Cell<Option<LastDivider>> = const { Cell::new(None) };
}

/// Forgets every scan. A new document may sit at an old one's address.
pub(crate) fn reset() {
    SCANS.with_borrow_mut(Vec::clear);
    TABLES.with_borrow_mut(Vec::clear);
    LAST_DIVIDER.set(None);
    ENDS.with_borrow_mut(Vec::clear);
    TITLE_ENDS.set(None);
    PUNCTUATION_RUN.set(None);
}

/// Whether `input` starts the last list-item tag divider on its line: a
/// space or tab, `::`, then a space, a tab or the line's end. Plain text
/// asks at every character of a tag, so the line's last divider is found
/// once and remembered.
pub(crate) fn is_last_tag_divider(input: OrgSource<'_>) -> bool {
    let (text, at) = input.full_source_and_offset();
    let source = (text.as_ptr() as usize, text.len());
    let line_start = at - input.text_since_line_break().len();
    let last = match LAST_DIVIDER.get() {
        Some(last) if last.source == source && last.line_start == line_start => last.at,
        _ => {
            let line_end = text[at..].find('\n').map_or(text.len(), |end| at + end);
            let line = &text.as_bytes()[line_start..line_end];
            let followed_by_newline = line_end < text.len();
            let last = (0..line.len())
                .rev()
                .find(|&i| is_tag_divider(&line[i..], followed_by_newline))
                .map(|i| line_start + i);
            LAST_DIVIDER.set(Some(LastDivider {
                source,
                line_start,
                at: last,
            }));
            last
        }
    };
    last == Some(at)
}

fn is_tag_divider(rest: &[u8], followed_by_newline: bool) -> bool {
    rest.len() >= 3
        && matches!(rest[0], b' ' | b'\t')
        && &rest[1..3] == b"::"
        && (rest.len() == 3
            || matches!(rest[3], b' ' | b'\t')
            || (rest[3..] == *b"\r" && followed_by_newline))
}

/// Whether there is a `close` ahead of `input` that returns to its bracket
/// depth, as upstream's depth-counting exit matchers require, and before
/// the next blank line when blank lines end objects there.
fn closes_balanced(
    context: RefContext<'_, '_, '_, '_>,
    input: OrgSource<'_>,
    open: u8,
    close: u8,
) -> bool {
    let to_blank_line = in_section(context, "section") && !in_section(context, "lesser block");
    let (text, at) = input.full_source_and_offset();
    let source = (text.as_ptr() as usize, text.len());
    TABLES.with_borrow_mut(|tables| {
        let index = match tables
            .iter()
            .position(|table| table.source == source && table.open == open)
        {
            Some(index) => index,
            None => {
                if tables.len() == KEEP_TABLES {
                    tables.remove(0);
                }
                tables.push(brackets(text, open, close));
                tables.len() - 1
            }
        };
        let table = &tables[index];
        let closes_at = table.closes_at[at];
        closes_at != NONE && (!to_blank_line || closes_at < table.blank_line_at[at])
    })
}

fn brackets(text: &str, open: u8, close: u8) -> Brackets {
    let bytes = text.as_bytes();
    // depth[i] is the depth before byte i. A position's closer is the byte
    // before the first later position whose depth is lower.
    let mut depth = Vec::with_capacity(bytes.len() + 1);
    let mut current = 0i64;
    depth.push(current);
    for &byte in bytes {
        current += i64::from(byte == open) - i64::from(byte == close);
        depth.push(current);
    }
    let mut closes_at = vec![NONE; bytes.len() + 1];
    let mut lower: Vec<usize> = Vec::new();
    for i in (0..depth.len()).rev() {
        while lower.last().is_some_and(|&j| depth[j] >= depth[i]) {
            lower.pop();
        }
        if let Some(&j) = lower.last() {
            closes_at[i] = (j - 1) as u32;
        }
        lower.push(i);
    }
    let mut blank_line_at = vec![bytes.len() as u32; bytes.len() + 1];
    for i in (0..bytes.len()).rev() {
        blank_line_at[i] = if bytes[i] == b'\n' && starts_blank_line(&bytes[i + 1..]) {
            i as u32
        } else {
            blank_line_at[i + 1]
        };
    }
    Brackets {
        source: (text.as_ptr() as usize, text.len()),
        open,
        closes_at,
        blank_line_at,
    }
}

/// Fails unless a `close` that returns to `input`'s depth is ahead; see
/// `closes_balanced`.
pub(crate) fn expect_balanced<'s>(
    context: RefContext<'_, '_, '_, 's>,
    input: OrgSource<'s>,
    open: u8,
    close: u8,
) -> Res<OrgSource<'s>, ()> {
    if closes_balanced(context, input, open, close) {
        Ok((input, ()))
    } else {
        Err(nom::Err::Error(CustomError::Static("Never closes.")))
    }
}

/// Whether `closer` occurs at or after `input`, and before the next blank
/// line when blank lines end objects there.
pub(crate) fn closes(
    context: RefContext<'_, '_, '_, '_>,
    input: OrgSource<'_>,
    closer: Closer,
) -> bool {
    // Objects in a verse block can span blank lines, and so can those parsed
    // again inside another object's contents, which have no section above.
    let to_blank_line = in_section(context, "section") && !in_section(context, "lesser block");
    let (text, at) = input.full_source_and_offset();
    let source = (text.as_ptr() as usize, text.len());
    SCANS.with_borrow_mut(|scans| {
        let index = match scans
            .iter()
            .position(|scan| scan.closer == closer && scan.to_blank_line == to_blank_line)
        {
            Some(index) => {
                let scan = &scans[index];
                if scan.source == source && scan.from <= at && at <= scan.until {
                    return scan.found;
                }
                index
            }
            None => {
                scans.push(Scan {
                    closer,
                    to_blank_line,
                    source,
                    from: 0,
                    until: 0,
                    found: false,
                });
                scans.len() - 1
            }
        };
        let (until, found) = scan(text, at, closer, to_blank_line);
        scans[index] = Scan {
            closer,
            to_blank_line,
            source,
            from: at,
            until,
            found,
        };
        found
    })
}

/// Fails unless `closer` is ahead of `input`; see `closes`.
pub(crate) fn expect<'s>(
    context: RefContext<'_, '_, '_, 's>,
    input: OrgSource<'s>,
    closer: &'static str,
) -> Res<OrgSource<'s>, ()> {
    if closes(context, input, Closer::Text(closer)) {
        Ok((input, ()))
    } else {
        Err(nom::Err::Error(CustomError::Static("Never closes.")))
    }
}

/// The first closer at or after `from`, or the blank line or end of input
/// that ends the paragraph first.
fn scan(text: &str, from: usize, closer: Closer, to_blank_line: bool) -> (usize, bool) {
    let bytes = text.as_bytes();
    let start = match closer {
        Closer::Markup(_) => from + text[from..].chars().next().map_or(0, char::len_utf8),
        Closer::Text(_) => from,
    };
    for at in start..bytes.len() {
        let found = match closer {
            Closer::Markup(marker) => bytes[at] == marker && ends_markup(text, at),
            Closer::Text(closer) => bytes[at..].starts_with(closer.as_bytes()),
        };
        if found {
            return (at, true);
        }
        if to_blank_line && bytes[at] == b'\n' && starts_blank_line(&bytes[at + 1..]) {
            return (at, false);
        }
    }
    (bytes.len(), false)
}

/// The rule text_markup_end applies to the marker at `at`.
fn ends_markup(text: &str, at: usize) -> bool {
    let before = text[..at].chars().next_back();
    let after = text.as_bytes().get(at + 1);
    before.is_some_and(|c| !c.is_whitespace() && c != '\u{200B}')
        && after.is_none_or(|b| b" \r\n\t-.,;:!?')}[\"\\".contains(b))
}

fn starts_blank_line(line: &[u8]) -> bool {
    let rest = &line[line
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count()..];
    rest.is_empty() || rest[0] == b'\n' || rest.starts_with(b"\r\n")
}

/// A line that can end a block, a drawer or a LaTeX environment.
#[derive(Clone, Copy)]
pub(crate) enum End<'a> {
    /// `#+end_NAME`, for a greater or lesser block.
    Block(&'a str),
    /// `#+end:` or `#+end`, for a dynamic block.
    DynamicBlock,
    /// `:end:`, for a drawer.
    Drawer,
    /// `\end{NAME}`.
    Latex(&'a str),
}

/// For one input, where the last line that might end each kind of block,
/// drawer and LaTeX environment starts, or 0 for none.
struct Ends {
    source: (usize, usize),
    /// By lowercased name.
    blocks: HashMap<String, usize>,
    /// The last block end whose name isn't ASCII. Upstream compares such a
    /// name by Unicode case folding, where `K` (U+212A) matches `k`, so it
    /// may end a block of any name.
    any_block: usize,
    latex: HashMap<String, usize>,
    any_latex: usize,
    dynamic_block: usize,
    drawer: usize,
}

thread_local! {
    static ENDS: RefCell<Vec<Ends>> = const { RefCell::new(Vec::new()) };
}

/// Whether a line that may be `end` starts after `input`.
///
/// A block, drawer or LaTeX environment parses its contents as elements
/// before it looks for its end, and fails after all that work when the end
/// never comes; the paragraph that takes its place then tries every opener
/// inside it again. Unclosed blocks nested in each other took exponential
/// time that way (#15). Emacs looks for the end line first and makes an
/// unclosed block a paragraph straight away, which this lets each parser do
/// too. Lines that match every end matcher are found, and maybe more, so the
/// answer may be a wrong "yes" but never a wrong "no".
fn end_follows(input: OrgSource<'_>, end: End<'_>) -> bool {
    let (text, at) = input.full_source_and_offset();
    let source = (text.as_ptr() as usize, text.len());
    ENDS.with_borrow_mut(|tables| {
        let index = match tables.iter().position(|table| table.source == source) {
            Some(index) => index,
            None => {
                if tables.len() == KEEP_TABLES {
                    tables.remove(0);
                }
                tables.push(ends(text));
                tables.len() - 1
            }
        };
        let table = &tables[index];
        let last = match end {
            End::Block(name) => last_named(&table.blocks, table.any_block, name),
            End::DynamicBlock => table.dynamic_block,
            End::Drawer => table.drawer,
            End::Latex(name) => last_named(&table.latex, table.any_latex, name),
        };
        last > at
    })
}

fn last_named(lines: &HashMap<String, usize>, any: usize, name: &str) -> usize {
    if !name.is_ascii() {
        return usize::MAX;
    }
    let named = lines.get(&name.to_ascii_lowercase()).copied();
    named.unwrap_or(0).max(any)
}

fn ends(text: &str) -> Ends {
    let mut ends = Ends {
        source: (text.as_ptr() as usize, text.len()),
        blocks: HashMap::new(),
        any_block: 0,
        latex: HashMap::new(),
        any_latex: 0,
        dynamic_block: 0,
        drawer: 0,
    };
    let mut start = 0;
    for line in text.split('\n') {
        let rest = line.trim_start_matches([' ', '\t']);
        if let Some(rest) = strip_prefix_ignore_case(rest, "#+end") {
            if matches!(rest, "" | ":" | "\r" | ":\r") {
                ends.dynamic_block = start;
            }
            if let Some(name) = rest.strip_prefix('_') {
                let name = name.split([' ', '\t', '\r']).next().unwrap_or_default();
                record(&mut ends.blocks, &mut ends.any_block, name, start);
            }
        } else if let Some(rest) = strip_prefix_ignore_case(rest, ":end:") {
            if rest.trim_matches([' ', '\t', '\r']).is_empty() {
                ends.drawer = start;
            }
        } else if let Some(rest) = strip_prefix_ignore_case(rest, "\\end{") {
            if let Some((name, _)) = rest.split_once('}') {
                record(&mut ends.latex, &mut ends.any_latex, name, start);
            }
        }
        start += line.len() + 1;
    }
    ends
}

fn strip_prefix_ignore_case<'s>(text: &'s str, prefix: &str) -> Option<&'s str> {
    text.get(..prefix.len())
        .filter(|start| start.eq_ignore_ascii_case(prefix))
        .map(|_| &text[prefix.len()..])
}

fn record(lines: &mut HashMap<String, usize>, any: &mut usize, name: &str, start: usize) {
    if name.is_ascii() {
        lines.insert(name.to_ascii_lowercase(), start);
    } else {
        *any = start;
    }
}

/// Fails unless a line that may be `end` is ahead; see `end_follows`.
pub(crate) fn expect_end<'s>(input: OrgSource<'s>, end: End<'_>) -> Res<OrgSource<'s>, ()> {
    if end_follows(input, end) {
        Ok((input, ()))
    } else {
        Err(nom::Err::Error(CustomError::Static("Never ends.")))
    }
}

/// For one input, a line's start and which of its positions end a
/// headline's title.
struct TitleEnds {
    source: (usize, usize),
    line_start: usize,
    end: usize,
    /// By byte offset from `line_start`, up to where the line ending starts.
    ends: Vec<bool>,
}

thread_local! {
    static TITLE_ENDS: RefCell<Option<TitleEnds>> = const { RefCell::new(None) };
}

/// Whether a headline's title ends at `input`: blanks, then maybe tags
/// such as `:a:b:` and blanks, then the line's end. Plain text asks at every
/// character of a title, and the tags parser ran to the line's end each
/// time, so a title of many `:`-separated words took quadratic time (#15).
/// Each line's answers are found in one pass from its end and remembered.
pub(crate) fn ends_headline_title(input: OrgSource<'_>) -> bool {
    let (text, at) = input.full_source_and_offset();
    let source = (text.as_ptr() as usize, text.len());
    let line_start = at - input.text_since_line_break().len();
    // Contents parsed again inside an object end before the full source.
    let end = at + input.len();
    TITLE_ENDS.with_borrow_mut(|memo| {
        let ends = match memo {
            Some(memo)
                if memo.source == source && memo.line_start == line_start && memo.end == end =>
            {
                &memo.ends
            }
            _ => {
                let line = &text[line_start..end];
                let line = match line.find('\n') {
                    Some(newline) => line[..newline]
                        .strip_suffix('\r')
                        .unwrap_or(&line[..newline]),
                    None => line,
                };
                &memo
                    .insert(TitleEnds {
                        source,
                        line_start,
                        end,
                        ends: title_ends(line),
                    })
                    .ends
            }
        };
        // Past the ends is the line ending, which ends the title too.
        ends.get(at - line_start).copied().unwrap_or(true)
    })
}

fn title_ends(line: &str) -> Vec<bool> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let n = chars.len();
    let is_blank = |c: char| c == ' ' || c == '\t';
    let is_tag = |c: char| c.is_alphanumeric() || "_@#%".contains(c);
    // By char index: whether only blanks follow; whether `(TAG:)+` and
    // blanks follow; the first non-blank at or after it.
    let mut blank_to_end = vec![true; n + 1];
    let mut tags_to_end = vec![false; n + 1];
    let mut non_blank = vec![n; n + 1];
    let mut run_end = vec![n; n + 1];
    let mut ends = vec![false; line.len() + 1];
    ends[line.len()] = true;
    for i in (0..n).rev() {
        let c = chars[i].1;
        blank_to_end[i] = is_blank(c) && blank_to_end[i + 1];
        non_blank[i] = if is_blank(c) { non_blank[i + 1] } else { i };
        if is_tag(c) {
            run_end[i] = if i + 1 < n && is_tag(chars[i + 1].1) {
                run_end[i + 1]
            } else {
                i + 1
            };
            let colon = run_end[i];
            tags_to_end[i] = colon < n
                && chars[colon].1 == ':'
                && (blank_to_end[colon + 1] || tags_to_end[colon + 1]);
        }
        let first = non_blank[i];
        ends[chars[i].0] = first == n || (chars[first].1 == ':' && tags_to_end[first + 1]);
    }
    ends
}

/// For one input, the last run of plain-link path punctuation found.
#[derive(Clone, Copy)]
struct PunctuationRun {
    source: (usize, usize),
    end: usize,
    from: usize,
    to: usize,
}

thread_local! {
    static PUNCTUATION_RUN: Cell<Option<PunctuationRun>> = const { Cell::new(None) };
}

/// Where the run of punctuation that may end a plain link's path, starting
/// at `input`, ends. A path asks at each of its characters, and scanning
/// the run each time made a long run quadratic (#15), so the run is
/// remembered and every position in it gets the same answer.
pub(crate) fn path_punctuation_end(input: OrgSource<'_>) -> usize {
    let (text, at) = input.full_source_and_offset();
    let source = (text.as_ptr() as usize, text.len());
    let end = at + input.len();
    if let Some(run) = PUNCTUATION_RUN.get() {
        if run.source == source && run.end == end && run.from <= at && at <= run.to {
            return run.to;
        }
    }
    let to = text[at..end]
        .char_indices()
        .find(|&(_, c)| !c.is_ascii_punctuation() || " \t\r\n[]<>()/".contains(c))
        .map_or(end, |(offset, _)| at + offset);
    PUNCTUATION_RUN.set(Some(PunctuationRun {
        source,
        end,
        from: at,
        to,
    }));
    to
}
