use std::borrow::Cow;
use std::ops::RangeBounds;

use nom::Compare;
use nom::FindSubstring;
use nom::InputIter;
use nom::InputLength;
use nom::InputTake;
use nom::InputTakeAtPosition;
use nom::Offset;
use nom::Slice;

// Upstream used i16, which overflowed past 32,767 unclosed brackets in one
// document and panicked in debug builds.
pub(crate) type BracketDepth = i32;

#[derive(Copy, Clone)]
pub(crate) struct OrgSource<'s> {
    full_source: &'s str,
    start: usize,
    end: usize, // exclusive
    start_of_line: usize,
    bracket_depth: BracketDepth,     // []
    brace_depth: BracketDepth,       // {}
    parenthesis_depth: BracketDepth, // ()
    preceding_character: Option<char>,
}

impl<'s> std::fmt::Debug for OrgSource<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Org")
            .field(&Into::<&str>::into(self))
            .finish()
    }
}

impl<'s> OrgSource<'s> {
    /// Returns a wrapped string that keeps track of values we need for parsing org-mode.
    ///
    /// Only call this on the full original string. Calling this on a substring can result in invalid values.
    pub(crate) fn new(input: &'s str) -> Self {
        OrgSource {
            full_source: input,
            start: 0,
            end: input.len(),
            start_of_line: 0,
            preceding_character: None,
            bracket_depth: 0,
            brace_depth: 0,
            parenthesis_depth: 0,
        }
    }

    /// Get the text since the line break preceding the start of this WrappedInput.
    pub(crate) fn text_since_line_break(&self) -> &'s str {
        &self.full_source[self.start_of_line..self.start]
    }

    pub(crate) fn len(&self) -> usize {
        self.end - self.start
    }

    pub(crate) fn get_byte_offset(&self) -> usize {
        self.start
    }

    pub(crate) fn get_preceding_character(&self) -> Option<char> {
        self.preceding_character
    }

    pub(crate) fn is_at_start_of_line(&self) -> bool {
        self.start == self.start_of_line
    }

    pub(crate) fn get_until(&self, other: OrgSource<'s>) -> OrgSource<'s> {
        debug_assert!(other.start >= self.start);
        debug_assert!(other.end <= self.end);
        self.slice(..(other.start - self.start))
    }

    pub(crate) fn get_until_end_of(&self, other: OrgSource<'s>) -> OrgSource<'s> {
        debug_assert!(other.start >= self.start);
        debug_assert!(other.end <= self.end);
        self.slice(..(other.end - self.start))
    }

    pub(crate) fn get_until_end_of_str(&self, other: &'s str) -> OrgSource<'s> {
        let full_source_start = self.full_source.as_ptr() as usize;
        let other_start = other.as_ptr() as usize - full_source_start;
        let other_end = other_start + other.len();
        debug_assert!(other_start >= self.start);
        debug_assert!(other_end <= self.end);
        self.slice(..(other_end - self.start))
    }

    pub(crate) fn get_start_of_line(&self) -> OrgSource<'s> {
        let skipped_text = self.text_since_line_break();
        let mut bracket_depth = self.bracket_depth;
        let mut brace_depth = self.brace_depth;
        let mut parenthesis_depth = self.parenthesis_depth;
        // Since we're going backwards, this does the opposite.
        for byte in skipped_text.bytes() {
            match byte {
                b'\n' => {
                    panic!("Should not hit a line break when only going back to the start of the line.");
                }
                b'[' => {
                    bracket_depth -= 1;
                }
                b']' => {
                    bracket_depth += 1;
                }
                b'{' => {
                    brace_depth -= 1;
                }
                b'}' => {
                    brace_depth += 1;
                }
                b'(' => {
                    parenthesis_depth -= 1;
                }
                b')' => {
                    parenthesis_depth += 1;
                }
                _ => {}
            };
        }

        OrgSource {
            full_source: self.full_source,
            start: self.start_of_line,
            end: self.end,
            start_of_line: self.start_of_line,
            preceding_character: if self.start_of_line > 0 {
                Some('\n')
            } else {
                None
            },
            bracket_depth,
            brace_depth,
            parenthesis_depth,
        }
    }

    pub(crate) fn get_bracket_depth(&self) -> BracketDepth {
        self.bracket_depth
    }

    pub(crate) fn get_brace_depth(&self) -> BracketDepth {
        self.brace_depth
    }

    pub(crate) fn get_parenthesis_depth(&self) -> BracketDepth {
        self.parenthesis_depth
    }
}

impl<'s> InputTake for OrgSource<'s> {
    fn take(&self, count: usize) -> Self {
        self.slice(..count)
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        (self.slice(count..), self.slice(..count))
    }
}

impl<'s, 'o, O: Into<&'o str>> Compare<O> for OrgSource<'s> {
    fn compare(&self, t: O) -> nom::CompareResult {
        (&self.full_source[self.start..self.end]).compare(t.into())
    }

    fn compare_no_case(&self, t: O) -> nom::CompareResult {
        (&self.full_source[self.start..self.end]).compare_no_case(t.into())
    }
}

impl<'s> From<&'s str> for OrgSource<'s> {
    fn from(value: &'s str) -> Self {
        OrgSource::new(value)
    }
}

impl<'s> From<&OrgSource<'s>> for &'s str {
    fn from(value: &OrgSource<'s>) -> Self {
        &value.full_source[value.start..value.end]
    }
}

impl<'s> From<OrgSource<'s>> for &'s str {
    fn from(value: OrgSource<'s>) -> Self {
        &value.full_source[value.start..value.end]
    }
}

impl<'s> From<&OrgSource<'s>> for Cow<'s, str> {
    fn from(value: &OrgSource<'s>) -> Self {
        (&value.full_source[value.start..value.end]).into()
    }
}

impl<'s> From<OrgSource<'s>> for Cow<'s, str> {
    fn from(value: OrgSource<'s>) -> Self {
        (&value.full_source[value.start..value.end]).into()
    }
}

impl<'s, R> Slice<R> for OrgSource<'s>
where
    R: RangeBounds<usize>,
{
    fn slice(&self, range: R) -> Self {
        let new_start = match range.start_bound() {
            std::ops::Bound::Included(idx) => self.start + idx,
            std::ops::Bound::Excluded(idx) => self.start + idx - 1,
            std::ops::Bound::Unbounded => self.start,
        };
        let new_end = match range.end_bound() {
            std::ops::Bound::Included(idx) => self.start + idx + 1,
            std::ops::Bound::Excluded(idx) => self.start + idx,
            std::ops::Bound::Unbounded => self.end,
        };
        if new_start < self.start {
            panic!("Attempted to extend before the start of the WrappedInput.")
        }
        if new_end > self.end {
            panic!("Attempted to extend past the end of the WrappedInput.")
        }
        if new_start == self.start && new_end == self.end {
            return *self;
        }

        let skipped_text = &self.full_source[self.start..new_start];
        let mut start_of_line = self.start_of_line;
        let mut bracket_depth = self.bracket_depth;
        let mut brace_depth = self.brace_depth;
        let mut parenthesis_depth = self.parenthesis_depth;
        for (offset, byte) in skipped_text.bytes().enumerate() {
            match byte {
                b'\n' => {
                    start_of_line = self.start + offset + 1;
                }
                b'[' => {
                    bracket_depth += 1;
                }
                b']' => {
                    bracket_depth -= 1;
                }
                b'{' => {
                    brace_depth += 1;
                }
                b'}' => {
                    brace_depth -= 1;
                }
                b'(' => {
                    parenthesis_depth += 1;
                }
                b')' => {
                    parenthesis_depth -= 1;
                }
                _ => {}
            };
        }

        OrgSource {
            full_source: self.full_source,
            start: new_start,
            end: new_end,
            start_of_line,
            preceding_character: skipped_text.chars().last().or(self.preceding_character),
            bracket_depth,
            brace_depth,
            parenthesis_depth,
        }
    }
}

impl<'s> std::fmt::Display for OrgSource<'s> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Into::<&str>::into(self).fmt(f)
    }
}

impl<'s> InputLength for OrgSource<'s> {
    fn input_len(&self) -> usize {
        self.end - self.start
    }
}

impl<'s> InputIter for OrgSource<'s> {
    type Item = <&'s str as InputIter>::Item;

    type Iter = <&'s str as InputIter>::Iter;

    type IterElem = <&'s str as InputIter>::IterElem;

    fn iter_indices(&self) -> Self::Iter {
        Into::<&str>::into(self).char_indices()
    }

    fn iter_elements(&self) -> Self::IterElem {
        Into::<&str>::into(self).iter_elements()
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        Into::<&str>::into(self).position(predicate)
    }

    fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
        Into::<&str>::into(self).slice_index(count)
    }
}

impl<'s> Offset for OrgSource<'s> {
    fn offset(&self, second: &Self) -> usize {
        second.start - self.start
    }
}

impl<'s> InputTakeAtPosition for OrgSource<'s> {
    type Item = <&'s str as InputTakeAtPosition>::Item;

    fn split_at_position<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match Into::<&str>::into(self).position(predicate) {
            Some(idx) => Ok(self.take_split(idx)),
            None => Err(nom::Err::Incomplete(nom::Needed::new(1))),
        }
    }

    fn split_at_position1<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
        e: nom::error::ErrorKind,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match Into::<&str>::into(self).position(predicate) {
            Some(0) => Err(nom::Err::Error(E::from_error_kind(*self, e))),
            Some(idx) => Ok(self.take_split(idx)),
            None => Err(nom::Err::Incomplete(nom::Needed::new(1))),
        }
    }

    fn split_at_position_complete<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.split_at_position(predicate) {
            Err(nom::Err::Incomplete(_)) => Ok(self.take_split(self.input_len())),
            res => res,
        }
    }

    fn split_at_position1_complete<P, E: nom::error::ParseError<Self>>(
        &self,
        predicate: P,
        e: nom::error::ErrorKind,
    ) -> nom::IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        let window = Into::<&str>::into(self);
        match window.position(predicate) {
            Some(0) => Err(nom::Err::Error(E::from_error_kind(*self, e))),
            Some(n) => Ok(self.take_split(n)),
            None => {
                if window.input_len() == 0 {
                    Err(nom::Err::Error(E::from_error_kind(*self, e)))
                } else {
                    Ok(self.take_split(self.input_len()))
                }
            }
        }
    }
}

impl<'n, 's> FindSubstring<&'n str> for OrgSource<'s> {
    fn find_substring(&self, substr: &'n str) -> Option<usize> {
        Into::<&str>::into(self).find(substr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range() {
        let input = OrgSource::new("foo bar baz");
        let output = input.slice(4..7);
        assert_eq!(output.to_string(), "bar");
    }

    #[test]
    fn range_to() {
        let input = OrgSource::new("foo bar baz");
        let output = input.slice(..7);
        assert_eq!(output.to_string(), "foo bar");
    }

    #[test]
    fn range_from() {
        let input = OrgSource::new("foo bar baz");
        let output = input.slice(4..);
        assert_eq!(output.to_string(), "bar baz");
    }

    #[test]
    fn full_range() {
        let input = OrgSource::new("foo bar baz");
        let output = input.slice(..);
        assert_eq!(output.to_string(), "foo bar baz");
    }

    #[test]
    fn nested_range() {
        let input = OrgSource::new("lorem foo bar baz ipsum");
        let first_cut = input.slice(6..17);
        let output = first_cut.slice(4..7);
        assert_eq!(first_cut.to_string(), "foo bar baz");
        assert_eq!(output.to_string(), "bar");
    }

    #[test]
    #[should_panic]
    fn out_of_bounds() {
        let input = OrgSource::new("lorem foo bar baz ipsum");
        input.slice(6..30);
    }

    #[test]
    #[should_panic]
    fn out_of_nested_bounds() {
        let input = OrgSource::new("lorem foo bar baz ipsum");
        let first_cut = input.slice(6..17);
        first_cut.slice(4..14);
    }

    #[test]
    fn line_break() {
        let input = OrgSource::new("lorem\nfoo\nbar\nbaz\nipsum");
        assert_eq!(input.slice(5..).start_of_line, 0);
        assert_eq!(input.slice(6..).start_of_line, 6);
        assert_eq!(input.slice(6..).slice(10..).start_of_line, 14);
    }

    #[test]
    fn text_since_line_break() {
        let input = OrgSource::new("lorem\nfoo\nbar\nbaz\nipsum");
        assert_eq!(input.text_since_line_break(), "");
        assert_eq!(input.slice(5..).text_since_line_break(), "lorem");
        assert_eq!(input.slice(6..).text_since_line_break(), "");
        assert_eq!(input.slice(6..).slice(10..).text_since_line_break(), "ba");
    }

    #[test]
    fn preceding_character() {
        let input = OrgSource::new("lorem\nfoo\nbar\nbaz\nipsum");
        assert_eq!(input.get_preceding_character(), None);
        assert_eq!(input.slice(5..).get_preceding_character(), Some('m'));
        assert_eq!(input.slice(6..).get_preceding_character(), Some('\n'));
        assert_eq!(
            input.slice(6..).slice(10..).get_preceding_character(),
            Some('a')
        );
    }

    #[test]
    fn is_at_start_of_line() {
        let input = OrgSource::new("lorem\nfoo\nbar\nbaz\nipsum");
        assert_eq!(input.is_at_start_of_line(), true);
        assert_eq!(input.slice(5..).is_at_start_of_line(), false);
        assert_eq!(input.slice(6..).is_at_start_of_line(), true);
        assert_eq!(input.slice(6..).slice(10..).is_at_start_of_line(), false);
    }

    #[test]
    fn preceding_character_unicode() {
        let input = OrgSource::new("🧡💛💚💙💜");
        assert_eq!(input.get_preceding_character(), None);
        assert_eq!(input.slice(8..).get_preceding_character(), Some('💛'));
    }

    #[test]
    fn depth() {
        let input = OrgSource::new("[][()][({)]}}}}");
        assert_eq!(input.get_bracket_depth(), 0);
        assert_eq!(input.get_brace_depth(), 0);
        assert_eq!(input.get_parenthesis_depth(), 0);
        assert_eq!(input.slice(4..).get_bracket_depth(), 1);
        assert_eq!(input.slice(4..).get_brace_depth(), 0);
        assert_eq!(input.slice(4..).get_parenthesis_depth(), 1);
        assert_eq!(input.slice(4..).slice(6..).get_bracket_depth(), 1);
        assert_eq!(input.slice(4..).slice(6..).get_brace_depth(), 1);
        assert_eq!(input.slice(4..).slice(6..).get_parenthesis_depth(), 0);
        assert_eq!(input.slice(14..).get_brace_depth(), -2);
    }
}
