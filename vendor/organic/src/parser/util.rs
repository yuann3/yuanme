use std::fmt::Debug;

use nom::branch::alt;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::none_of;
use nom::character::complete::one_of;
use nom::character::complete::space0;
use nom::combinator::eof;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::multi::many0;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::types::IndentationLevel;

pub(crate) const WORD_CONSTITUENT_CHARACTERS: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// Check if we are below a section of the given section type regardless of depth
pub(crate) fn in_section(context: RefContext<'_, '_, '_, '_>, section_name: &str) -> bool {
    for thing in context.iter() {
        match thing {
            ContextElement::Context(name) if *name == section_name => return true,
            _ => {}
        }
    }
    false
}

/// Checks if we are currently an immediate child of the given section type
pub(crate) fn immediate_in_section(
    context: RefContext<'_, '_, '_, '_>,
    section_name: &str,
) -> bool {
    for thing in context.iter() {
        match thing {
            ContextElement::Context(name) if *name == section_name => return true,
            ContextElement::Context(name) if *name != section_name => return false,
            _ => {}
        }
    }
    false
}

/// Get a slice of the string that was consumed in a parser using the original input to the parser and the remaining input after the parser.
pub(crate) fn get_consumed<'s>(input: OrgSource<'s>, remaining: OrgSource<'s>) -> OrgSource<'s> {
    input.get_until(remaining)
}

/// A line containing only whitespace and then a line break
///
/// It is up to the caller to ensure this is called at the start of a line.
pub(crate) fn blank_line(input: OrgSource<'_>) -> Res<OrgSource<'_>, OrgSource<'_>> {
    not(eof)(input)?;
    recognize(tuple((space0, alt((line_ending, eof)))))(input)
}

fn element_trailing_whitespace<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    alt((eof, recognize(many0(blank_line))))(input)
}

pub(crate) fn maybe_consume_object_trailing_whitespace_if_not_exiting<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Option<OrgSource<'s>>> {
    // We have to check exit matcher after each character because description list tags need to end with a space unconsumed (" ::").
    let (remaining, post_blank) = recognize(many_till(
        one_of(" \t"),
        alt((
            peek(recognize(none_of(" \t"))),
            parser_with_context!(exit_matcher_parser)(context),
        )),
    ))(input)?;
    Ok((
        remaining,
        if post_blank.len() == 0 {
            None
        } else {
            Some(post_blank)
        },
    ))
}

pub(crate) fn maybe_consume_trailing_whitespace_if_not_exiting<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Option<OrgSource<'s>>> {
    if context.should_consume_trailing_whitespace() && exit_matcher_parser(context, input).is_err()
    {
        Ok(opt(element_trailing_whitespace)(input)?)
    } else {
        Ok((input, None))
    }
}

pub(crate) fn maybe_consume_trailing_whitespace<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Option<OrgSource<'s>>> {
    if context.should_consume_trailing_whitespace() {
        Ok(opt(element_trailing_whitespace)(input)?)
    } else {
        Ok((input, None))
    }
}

/// Check that we are at the start of a line
pub(crate) fn start_of_line<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    if input.is_at_start_of_line() {
        Ok((input, ()))
    } else {
        Err(nom::Err::Error(CustomError::Static("Not at start of line")))
    }
}

pub(crate) fn preceded_by_whitespace(
    allow_start_of_file: bool,
) -> impl for<'s> Fn(OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    move |input| _preceded_by_whitespace(allow_start_of_file, input)
}

/// Check that we are at the start of a line
fn _preceded_by_whitespace<'s>(
    allow_start_of_file: bool,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let preceding_character = input.get_preceding_character();
    if !preceding_character
        .map(|c| c.is_whitespace() || c == '\u{200B}') // 200B = Zero-width space
        .unwrap_or(allow_start_of_file)
    {
        return Err(nom::Err::Error(CustomError::Static(
            "Must be preceded by a whitespace character.",
        )));
    }
    Ok((input, ()))
}

/// Pull one non-whitespace character.
///
/// This function only operates on spaces, tabs, carriage returns, and line feeds. It does not handle fancy unicode whitespace.
pub(crate) fn non_whitespace_character(input: OrgSource<'_>) -> Res<OrgSource<'_>, char> {
    none_of(" \t\r\n")(input)
}

/// Check that we are at the start of a line
pub(crate) fn exit_matcher_parser<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    peek(|i| context.check_exit_matcher(i))(input)
}

pub(crate) fn text_until_exit<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(many_till(
        anychar,
        parser_with_context!(exit_matcher_parser)(context),
    ))(input)
}

#[allow(dead_code)]
fn not_yet_implemented() -> Res<OrgSource<'static>, ()> {
    Err(nom::Err::Error(CustomError::Static("Not implemented yet.")))
}

#[allow(dead_code)]
/// Text from the current point until the next line break or end of file
///
/// Useful for debugging.
fn text_until_eol<'s>(input: OrgSource<'s>) -> Result<&'s str, nom::Err<CustomError>> {
    let line = recognize(many_till(anychar, alt((line_ending, eof))))(input)
        .map(|(_remaining, line)| Into::<&str>::into(line))?;
    Ok(line.trim())
}

/// Return a tuple of (input, output) from a nom parser.
///
/// This is similar to recognize except it returns the input instead of the portion of the input that was consumed.
pub(crate) fn include_input<'s, F, O>(
    mut inner: F,
) -> impl FnMut(OrgSource<'s>) -> Res<OrgSource<'s>, (OrgSource<'s>, O)>
where
    F: FnMut(OrgSource<'s>) -> Res<OrgSource<'s>, O>,
{
    move |input: OrgSource<'_>| {
        let (remaining, output) = inner(input)?;
        Ok((remaining, (input, output)))
    }
}

/// Match single space or tab.
///
/// In org-mode syntax, spaces and tabs are often (but not always!) interchangeable.
pub(crate) fn org_space(input: OrgSource<'_>) -> Res<OrgSource<'_>, char> {
    one_of(" \t")(input)
}

/// Matches a single space, tab, line ending, or end of file.
///
/// In org-mode syntax there are often delimiters that could be any whitespace at all or the end of file.
pub(crate) fn org_space_or_line_ending(input: OrgSource<'_>) -> Res<OrgSource<'_>, OrgSource<'_>> {
    alt((recognize(org_space), org_line_ending))(input)
}

/// Match a line break or the end of the file.
///
/// In org-mode syntax, the end of the file can serve the same purpose as a line break syntactically.
pub(crate) fn org_line_ending(input: OrgSource<'_>) -> Res<OrgSource<'_>, OrgSource<'_>> {
    alt((line_ending, eof))(input)
}

/// Match the whitespace at the beginning of a line and give it an indentation level.
pub(crate) fn indentation_level<'s>(
    context: RefContext<'_, '_, '_, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (IndentationLevel, OrgSource<'s>)> {
    let (remaining, leading_whitespace) = space0(input)?;
    let indentation_level = Into::<&str>::into(leading_whitespace)
        .chars()
        .map(|c| match c {
            ' ' => 1,
            '\t' => context.get_global_settings().tab_width,
            _ => unreachable!(),
        })
        .sum();
    Ok((remaining, (indentation_level, leading_whitespace)))
}

/// Reset the input OrgSource as if it was starting a fresh document.
///
/// This is important for making start-of-document, end-of-document, and other context-dependent tests succeed.
pub(crate) fn confine_context<'s, O: Debug, I: Fn(OrgSource<'s>) -> Res<OrgSource<'s>, O>>(
    inner: I,
) -> impl Fn(OrgSource<'s>) -> Res<OrgSource<'s>, O> {
    move |input| impl_confine_context(input, &inner)
}

/// Reset the input OrgSource as if it was starting a fresh document.
///
/// This is important for making start-of-document, end-of-document, and other context-dependent tests succeed.
fn impl_confine_context<'s, O: Debug, I: Fn(OrgSource<'s>) -> Res<OrgSource<'s>, O>>(
    input: OrgSource<'s>,
    inner: I,
) -> Res<OrgSource<'s>, O> {
    let raw_str = Into::<&str>::into(input);
    let back_to_org_source = Into::<OrgSource<'_>>::into(raw_str);
    inner(back_to_org_source)
}
