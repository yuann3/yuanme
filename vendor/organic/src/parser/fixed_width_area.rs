use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::space0;
use nom::combinator::map;
use nom::combinator::not;
use nom::multi::many0;
use nom::sequence::preceded;
use nom::sequence::tuple;
use nom::InputTake;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use super::util::rest_of_line;
use crate::context::parser_with_context;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::types::FixedWidthArea;
use crate::types::Keyword;

pub(crate) fn fixed_width_area<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, FixedWidthArea<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let exit_matcher = parser_with_context!(exit_matcher_parser)(context);
    let (remaining, first_line) = fixed_width_area_line(remaining)?;
    let (remaining, remaining_lines) = many0(preceded(
        not(tuple((org_line_ending, exit_matcher))),
        map(
            tuple((org_line_ending, fixed_width_area_line)),
            |(_line_ending, line_contents)| line_contents,
        ),
    ))(remaining)?;

    let post_blank_begin = remaining;
    let (remaining, _first_line_break) = org_line_ending(remaining)?;
    let (remaining, _additional_post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let post_blank = get_consumed(post_blank_begin, remaining);
    let source = get_consumed(input, remaining);
    let mut value = Vec::with_capacity(remaining_lines.len() + 1);
    value.push(Into::<&str>::into(first_line));
    value.extend(remaining_lines.into_iter().map(Into::<&str>::into));
    Ok((
        remaining,
        FixedWidthArea {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            value,
            post_blank: if post_blank.len() > 0 {
                Some(Into::<&str>::into(post_blank))
            } else {
                None
            },
        },
    ))
}

fn fixed_width_area_line<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    let (remaining, _) = tuple((space0, tag(":")))(input)?;
    if let Ok((_remain, _line_break)) = org_line_ending(remaining) {
        return Ok((remaining, remaining.take(0)));
    }
    let (remaining, _) = tag(" ")(remaining)?;
    // As in `comment_line`: a character at a time was slow enough to break
    // the time bound when unclosed blocks re-read the line.
    let (remaining, value) = rest_of_line(remaining)?;
    Ok((remaining, value))
}

pub(crate) fn detect_fixed_width_area<'b, 'g, 'r, 's, AK>(
    _affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    tuple((
        start_of_line,
        space0,
        tag(":"),
        alt((tag(" "), org_line_ending)),
    ))(remaining)?;
    Ok((input, ()))
}
