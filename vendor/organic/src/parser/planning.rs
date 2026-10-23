use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::multi::many1;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::timestamp::timestamp;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use crate::context::parser_with_context;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::types::Planning;
use crate::types::Timestamp;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn planning<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Planning<'s>> {
    start_of_line(input)?;
    let (remaining, _leading_whitespace) = space0(input)?;
    let (remaining, planning_parameters) =
        many1(parser_with_context!(planning_parameter)(context))(remaining)?;
    let (remaining, _trailing_ws) = tuple((space0, org_line_ending))(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    let mut scheduled = None;
    let mut deadline = None;
    let mut closed = None;

    for (timestamp_type, timestamp) in planning_parameters.into_iter() {
        match timestamp_type {
            PlanningTimestampType::Scheduled => {
                scheduled = Some(timestamp);
            }
            PlanningTimestampType::Deadline => {
                deadline = Some(timestamp);
            }
            PlanningTimestampType::Closed => {
                closed = Some(timestamp);
            }
        }
    }

    Ok((
        remaining,
        Planning {
            source: source.into(),
            scheduled,
            deadline,
            closed,
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[derive(Debug)]
enum PlanningTimestampType {
    Scheduled,
    Deadline,
    Closed,
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn planning_parameter<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (PlanningTimestampType, Timestamp<'s>)> {
    let (remaining, planning_type) = alt((
        map(tag_no_case("DEADLINE"), |_| PlanningTimestampType::Deadline),
        map(tag_no_case("SCHEDULED"), |_| {
            PlanningTimestampType::Scheduled
        }),
        map(tag_no_case("CLOSED"), |_| PlanningTimestampType::Closed),
    ))(input)?;
    let (remaining, _gap) = tuple((tag(":"), space1))(remaining)?;
    let (remaining, timestamp) = timestamp(context, remaining)?;
    Ok((remaining, (planning_type, timestamp)))
}
