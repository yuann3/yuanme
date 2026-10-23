use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::combinator::eof;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many1_count;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::util::get_consumed;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::start_of_line;
use crate::types::HorizontalRule;
use crate::types::Keyword;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, affiliated_keywords))
)]
pub(crate) fn horizontal_rule<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, HorizontalRule<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    start_of_line(remaining)?;
    let (remaining, _rule) = recognize(tuple((
        space0,
        verify(many1_count(tag("-")), |dashes| *dashes >= 5),
        space0,
        alt((line_ending, eof)),
    )))(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        HorizontalRule {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}
