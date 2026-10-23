use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::digit1;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::timestamp::inactive_date_range_timestamp;
use super::timestamp::inactive_time_range_timestamp;
use super::timestamp::inactive_timestamp;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use crate::context::parser_with_context;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::types::Clock;
use crate::types::ClockStatus;
use crate::types::Timestamp;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn clock<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Clock<'s>> {
    start_of_line(input)?;
    let (remaining, _leading_whitespace) = space0(input)?;
    let (remaining, _clock) = tag_no_case("clock:")(remaining)?;
    let (remaining, _gap_whitespace) = space1(remaining)?;

    let (remaining, (timestamp, duration)) = clock_timestamp(context, remaining)?;
    let (remaining, _) = tuple((space0, org_line_ending))(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Clock {
            source: source.into(),
            timestamp,
            duration,
            status: if duration.is_some() {
                ClockStatus::Closed
            } else {
                ClockStatus::Running
            },
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn clock_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (Timestamp<'s>, Option<&'s str>)> {
    // TODO: This would be more efficient if we didn't throw away the parse result of the first half of an active/inactive date range timestamp if the parse fails (as in, the first thing active_date_range_timestamp parses is a active_timestamp but then we throw that away if it doesn't turn out to be a full active_date_range_timestamp despite the active_timestamp parse being completely valid). I am going with the simplest/cleanest approach for the first implementation.
    alt((
        // Order matters here. If its a date range, we need to parse the entire date range instead of just the first timestamp. If its a time range, we need to make sure thats parsed as a time range instead of as the "rest" portion of a single timestamp.
        map(
            parser_with_context!(inactive_time_range_timestamp)(context),
            |timestamp| (timestamp, None),
        ),
        map(
            tuple((
                parser_with_context!(inactive_date_range_timestamp)(context),
                opt(duration),
            )),
            |(timestamp, duration)| (timestamp, duration.map(Into::<&str>::into)),
        ),
        map(
            parser_with_context!(inactive_timestamp(true))(context),
            |timestamp| (timestamp, None),
        ),
    ))(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn duration<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tuple((tag("=>"), space1))(input)?;
    let (remaining, duration) = recognize(tuple((digit1, tag(":"), digit1)))(remaining)?;
    Ok((remaining, duration))
}
