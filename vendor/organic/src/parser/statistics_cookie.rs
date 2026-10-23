use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::util::get_consumed;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::RefContext;
use crate::error::Res;
use crate::types::StatisticsCookie;

pub(crate) fn statistics_cookie<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, StatisticsCookie<'s>> {
    alt((
        parser_with_context!(percent_statistics_cookie)(context),
        parser_with_context!(fraction_statistics_cookie)(context),
    ))(input)
}

fn percent_statistics_cookie<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, StatisticsCookie<'s>> {
    let (remaining, _) = recognize(tuple((
        tag("["),
        opt(nom::character::complete::u64),
        tag("%]"),
    )))(input)?;
    let value = get_consumed(input, remaining);
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        StatisticsCookie {
            source: source.into(),
            value: value.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn fraction_statistics_cookie<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, StatisticsCookie<'s>> {
    let (remaining, _) = recognize(tuple((
        tag("["),
        opt(nom::character::complete::u64),
        tag("/"),
        opt(nom::character::complete::u64),
        tag("]"),
    )))(input)?;
    let value = get_consumed(input, remaining);
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        StatisticsCookie {
            source: source.into(),
            value: value.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}
