use std::borrow::Cow;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::combinator::map;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::object::Object;
use super::AffiliatedKeywords;
use super::GetAffiliatedKeywords;
use super::PlainText;
use super::PostBlank;
use super::StandardProperties;
use super::Timestamp;
use crate::error::CustomError;
use crate::error::Res;

#[derive(Debug)]
pub struct Paragraph<'s> {
    pub source: &'s str,
    pub contents: Option<&'s str>,
    pub post_blank: Option<&'s str>,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub children: Vec<Object<'s>>,
}

#[derive(Debug)]
pub struct Comment<'s> {
    pub source: &'s str,
    pub value: Vec<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct TableCell<'s> {
    pub source: &'s str,
    pub children: Vec<Object<'s>>,
    pub contents: &'s str,
}

#[derive(Debug)]
pub struct VerseBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub data: Option<&'s str>,
    pub children: Vec<Object<'s>>,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct CommentBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub contents: &'s str,
    pub post_blank: Option<&'s str>,
}

pub type CharOffsetInLine = u16;

#[derive(Debug)]
pub enum RetainLabels {
    No,
    Yes,
    /// When adding -k to the switches on an example or src block, the labels are kept in the source code and links will use line numbers.
    Keep(CharOffsetInLine),
}

#[derive(Debug)]
pub struct ExampleBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub switches: Option<&'s str>,
    pub number_lines: Option<SwitchNumberLines>,
    pub preserve_indent: Option<CharOffsetInLine>,
    pub retain_labels: RetainLabels,
    pub use_labels: bool,
    pub label_format: Option<&'s str>,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct ExportBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub export_type: Option<&'s str>,
    pub data: Option<&'s str>,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct SrcBlock<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub language: Option<&'s str>,
    pub switches: Option<&'s str>,
    pub parameters: Option<&'s str>,
    pub number_lines: Option<SwitchNumberLines>,
    pub preserve_indent: Option<CharOffsetInLine>,
    pub retain_labels: RetainLabels,
    pub use_labels: bool,
    pub label_format: Option<&'s str>,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub enum ClockStatus {
    Running,
    Closed,
}

#[derive(Debug)]
pub struct Clock<'s> {
    pub source: &'s str,
    pub timestamp: Timestamp<'s>,
    pub duration: Option<&'s str>,
    pub status: ClockStatus,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct DiarySexp<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct Planning<'s> {
    pub source: &'s str,
    pub scheduled: Option<Timestamp<'s>>,
    pub deadline: Option<Timestamp<'s>>,
    pub closed: Option<Timestamp<'s>>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct FixedWidthArea<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub value: Vec<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct HorizontalRule<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct Keyword<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub key: &'s str,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct BabelCall<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub value: &'s str,
    pub call: Option<&'s str>,
    pub inside_header: Option<&'s str>,
    pub arguments: Option<&'s str>,
    pub end_header: Option<&'s str>,
    pub post_blank: Option<&'s str>,
}

#[derive(Debug)]
pub struct LatexEnvironment<'s> {
    pub source: &'s str,
    pub affiliated_keywords: AffiliatedKeywords<'s>,
    pub value: &'s str,
    pub post_blank: Option<&'s str>,
}

/// A line number used in switches to lesser blocks.
///
/// This must be signed because emacs subtracts 1 from the actual value in the org-mode text, which makes a 0 turn into a -1.
pub type LineNumber = isize;

#[derive(Debug)]
pub enum SwitchNumberLines {
    New(LineNumber),
    Continued(LineNumber),
}

impl<'s> Paragraph<'s> {
    /// Generate a paragraph of the passed in text with no additional properties.
    ///
    /// This is used for elements that support an "empty" content like greater blocks.
    pub(crate) fn of_text(
        source: &'s str,
        body: &'s str,
        contents: Option<&'s str>,
        post_blank: Option<&'s str>,
    ) -> Self {
        Paragraph {
            source,
            contents,
            post_blank,
            affiliated_keywords: AffiliatedKeywords::default(),
            children: vec![Object::PlainText(PlainText { source: body })],
        }
    }
}

impl<'s> StandardProperties<'s> for Paragraph<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        self.contents
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for TableCell<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        0
    }
}

impl<'s> StandardProperties<'s> for Comment<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for VerseBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        Some(self.contents)
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}
impl<'s> StandardProperties<'s> for CommentBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}
impl<'s> StandardProperties<'s> for ExampleBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}
impl<'s> StandardProperties<'s> for ExportBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}
impl<'s> StandardProperties<'s> for SrcBlock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Clock<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for DiarySexp<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Planning<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for FixedWidthArea<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for HorizontalRule<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for Keyword<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for BabelCall<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> StandardProperties<'s> for LatexEnvironment<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        self.source
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        None
    }

    fn get_post_blank(&self) -> PostBlank {
        self.post_blank
            .map(|text| text.lines().count())
            .unwrap_or(0)
            .try_into()
            .expect("Too much post-blank to fit into a PostBlank.")
    }
}

impl<'s> Comment<'s> {
    pub fn get_value(&self) -> String {
        let final_size = self.value.iter().map(|line| line.len()).sum();
        let mut ret = String::with_capacity(final_size);
        for line in &self.value {
            ret.push_str(line);
        }

        ret
    }
}

impl<'s> FixedWidthArea<'s> {
    pub fn get_value(&self) -> String {
        self.value.join("\n")
    }
}

impl<'s> ExampleBlock<'s> {
    /// Gets the contents of the lesser block, handling the escaping of lines with leading commas.
    pub fn get_value(&self) -> Cow<'s, str> {
        lesser_block_content(self.value).expect("This parser should never fail.")
    }
}

impl<'s> ExportBlock<'s> {
    /// Gets the export type capitalized.
    ///
    /// Upstream Emacs Org-mode capitalizes the export type.
    pub fn get_export_type(&self) -> Option<String> {
        self.export_type.map(|s| s.to_uppercase())
    }

    /// Gets the contents of the lesser block, handling the escaping of lines with leading commas.
    pub fn get_value(&self) -> Cow<'s, str> {
        lesser_block_content(self.value).expect("This parser should never fail.")
    }
}

impl<'s> SrcBlock<'s> {
    /// Gets the contents of the lesser block, handling the escaping of lines with leading commas.
    pub fn get_value(&self) -> Cow<'s, str> {
        lesser_block_content(self.value).expect("This parser should never fail.")
    }
}

impl<'s> GetAffiliatedKeywords<'s> for Paragraph<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for LatexEnvironment<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for BabelCall<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for Keyword<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for HorizontalRule<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for FixedWidthArea<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for DiarySexp<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for SrcBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for ExportBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for ExampleBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for CommentBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

impl<'s> GetAffiliatedKeywords<'s> for VerseBlock<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s> {
        &self.affiliated_keywords
    }
}

enum ContentState {
    Normal,
    Modified(String),
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn lesser_block_content<'s>(input: &'s str) -> Result<Cow<'s, str>, CustomError> {
    let mut state = ContentState::Normal;
    let mut remaining = input;
    loop {
        if remaining.is_empty() {
            break;
        }

        let (remain, (pre_escape_whitespace, line)) =
            content_line(remaining).map_err(|err| match err {
                nom::Err::Incomplete(_) => panic!("This parser does not support streaming."),
                nom::Err::Error(e) => e,
                nom::Err::Failure(e) => e,
            })?;
        if let Some(val) = pre_escape_whitespace {
            if let ContentState::Modified(ref mut ret) = state {
                ret.push_str(val);
            } else {
                let mut ret = String::new();
                ret.push_str(get_str_until(input, remaining));
                ret.push_str(val);
                state = ContentState::Modified(ret);
            }
        }
        if let ContentState::Modified(ref mut ret) = state {
            ret.push_str(line);
        }
        remaining = remain;
    }

    match state {
        ContentState::Normal => Ok(Cow::Borrowed(get_str_until(input, remaining))),
        ContentState::Modified(ret) => Ok(Cow::Owned(ret)),
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn content_line<'s>(input: &'s str) -> Res<&'s str, (Option<&'s str>, &'s str)> {
    let (remaining, pre_escape_whitespace) = opt(map(
        tuple((
            recognize(tuple((
                space0,
                many_till(
                    tag(","),
                    peek(tuple((tag(","), alt((tag("#+"), tag("*")))))),
                ),
            ))),
            tag(","),
        )),
        |(pre_comma, _)| pre_comma,
    ))(input)?;
    let (remaining, line_post_escape) = recognize(many_till(anychar, line_ending))(remaining)?;
    Ok((remaining, (pre_escape_whitespace, line_post_escape)))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
/// Check if the child string slice is a slice of the parent string slice.
pub(crate) fn is_slice_of(parent: &str, child: &str) -> bool {
    let parent_start = parent.as_ptr() as usize;
    let parent_end = parent_start + parent.len();
    let child_start = child.as_ptr() as usize;
    let child_end = child_start + child.len();
    child_start >= parent_start && child_end <= parent_end
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn get_str_until<'s>(parent: &'s str, child: &'s str) -> &'s str {
    debug_assert!(is_slice_of(parent, child));
    let parent_start = parent.as_ptr() as usize;
    let child_start = child.as_ptr() as usize;
    &parent[..(child_start - parent_start)]
}
