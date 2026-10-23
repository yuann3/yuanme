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
//! The ranges live in a thread-local because `OrgSource` is a `Copy` value
//! with nowhere to keep them.

use std::cell::RefCell;

use super::org_source::OrgSource;
use super::util::in_section;
use crate::context::RefContext;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Closer {
    /// A text markup marker that can end markup: after at least one
    /// character of contents, not after whitespace, and before a POST
    /// character or the end of a line.
    Markup(u8),
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

thread_local! {
    static SCANS: RefCell<Vec<Scan>> = const { RefCell::new(Vec::new()) };
}

/// Forgets every scan. A new document may sit at an old one's address.
pub(crate) fn reset() {
    SCANS.with_borrow_mut(Vec::clear);
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

/// The first closer at or after `from`, or the blank line or end of input
/// that ends the paragraph first.
fn scan(text: &str, from: usize, closer: Closer, to_blank_line: bool) -> (usize, bool) {
    let bytes = text.as_bytes();
    let start = match closer {
        Closer::Markup(_) => from + text[from..].chars().next().map_or(0, char::len_utf8),
    };
    for at in start..bytes.len() {
        let found = match closer {
            Closer::Markup(marker) => bytes[at] == marker && ends_markup(text, at),
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
