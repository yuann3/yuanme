use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::digit1;
use nom::character::complete::one_of;
use nom::character::complete::space1;
use nom::combinator::map;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::util::exit_matcher_parser;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::types::Date;
use crate::types::DayOfMonth;
use crate::types::Hour;
use crate::types::Minute;
use crate::types::Month;
use crate::types::Repeater;
use crate::types::RepeaterType;
use crate::types::Time;
use crate::types::TimeUnit;
use crate::types::Timestamp;
use crate::types::TimestampRangeType;
use crate::types::TimestampType;
use crate::types::WarningDelay;
use crate::types::WarningDelayType;
use crate::types::Year;

pub(crate) fn timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    // TODO: This would be more efficient if we didn't throw away the parse result of the first half of an active/inactive date range timestamp if the parse fails (as in, the first thing active_date_range_timestamp parses is a active_timestamp but then we throw that away if it doesn't turn out to be a full active_date_range_timestamp despite the active_timestamp parse being completely valid). I am going with the simplest/cleanest approach for the first implementation.
    alt((
        // Order matters here. If its a date range, we need to parse the entire date range instead of just the first timestamp. If its a time range, we need to make sure thats parsed as a time range instead of as the "rest" portion of a single timestamp.
        parser_with_context!(diary_timestamp)(context),
        parser_with_context!(active_time_range_timestamp)(context),
        parser_with_context!(inactive_time_range_timestamp)(context),
        parser_with_context!(active_date_range_timestamp)(context),
        parser_with_context!(inactive_date_range_timestamp)(context),
        parser_with_context!(active_timestamp(true))(context),
        parser_with_context!(inactive_timestamp(true))(context),
    ))(input)
}

fn diary_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    let (remaining, _) = tag("<%%(")(input)?;
    let (remaining, _body) = sexp(context, remaining)?;
    let (remaining, _) = tag(")>")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Timestamp {
            source: source.into(),
            timestamp_type: TimestampType::Diary,
            range_type: TimestampRangeType::None,
            start: None,
            end: None,
            start_time: None,
            end_time: None,
            repeater: None,
            warning_delay: None,
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn sexp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &sexp_end,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, body) = recognize(verify(
        many_till(
            anychar,
            parser_with_context!(exit_matcher_parser)(&parser_context),
        ),
        |(body, _end_contents)| !body.is_empty(),
    ))(input)?;

    Ok((remaining, body))
}

fn sexp_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alt((tag(")>"), recognize(one_of(">\n"))))(input)
}

const fn active_timestamp(
    allow_post_blank: bool,
) -> impl for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    move |context, input| impl_active_timestamp(context, input, allow_post_blank)
}

fn impl_active_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    allow_post_blank: bool,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    let (remaining, _) = tag("<")(input)?;
    let (remaining, start) = date(context, remaining)?;
    let time_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &active_time_rest_end,
    });
    let time_context = context.with_additional_node(&time_context);
    let (remaining, time) = opt(tuple((
        space1,
        parser_with_context!(time(true))(&time_context),
    )))(remaining)?;
    let remaining = if time.is_none() {
        // Upstream org-mode accepts malformed timestamps. For example '<2016-02-14 Sun ++y>'.
        let (remain, _) = opt(parser_with_context!(time_rest)(&time_context))(remaining)?;
        remain
    } else {
        remaining
    };
    let (remaining, repeater) =
        opt(tuple((space1, parser_with_context!(repeater)(context))))(remaining)?;
    let (remaining, warning_delay) = opt(tuple((
        space1,
        parser_with_context!(warning_delay)(context),
    )))(remaining)?;
    let (remaining, _) = tag(">")(remaining)?;

    let (remaining, post_blank) = if allow_post_blank {
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?
    } else {
        (remaining, None)
    };
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Timestamp {
            source: source.into(),
            timestamp_type: TimestampType::Active,
            range_type: TimestampRangeType::None,
            start: Some(start.clone()),
            end: Some(start),
            start_time: time.as_ref().map(|(_, time)| time.clone()),
            end_time: time.map(|(_, time)| time),
            repeater: repeater.map(|(_, repeater)| repeater),
            warning_delay: warning_delay.map(|(_, warning_delay)| warning_delay),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) const fn inactive_timestamp(
    allow_post_blank: bool,
) -> impl for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    move |context, input| impl_inactive_timestamp(context, input, allow_post_blank)
}

fn impl_inactive_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    allow_post_blank: bool,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    let (remaining, _) = tag("[")(input)?;
    let (remaining, start) = date(context, remaining)?;
    let time_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &inactive_time_rest_end,
    });
    let time_context = context.with_additional_node(&time_context);
    let (remaining, time) = opt(tuple((
        space1,
        parser_with_context!(time(true))(&time_context),
    )))(remaining)?;
    let remaining = if time.is_none() {
        // Upstream org-mode accepts malformed timestamps. For example '<2016-02-14 Sun ++y>'.
        let (remain, _) = opt(parser_with_context!(time_rest)(&time_context))(remaining)?;
        remain
    } else {
        remaining
    };
    let (remaining, repeater) =
        opt(tuple((space1, parser_with_context!(repeater)(context))))(remaining)?;
    let (remaining, warning_delay) = opt(tuple((
        space1,
        parser_with_context!(warning_delay)(context),
    )))(remaining)?;
    let (remaining, _) = tag("]")(remaining)?;

    let (remaining, post_blank) = if allow_post_blank {
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?
    } else {
        (remaining, None)
    };
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Timestamp {
            source: source.into(),
            timestamp_type: TimestampType::Inactive,
            range_type: TimestampRangeType::None,
            start: Some(start.clone()),
            end: Some(start),
            start_time: time.as_ref().map(|(_, time)| time.clone()),
            end_time: time.map(|(_, time)| time),
            repeater: repeater.map(|(_, repeater)| repeater),
            warning_delay: warning_delay.map(|(_, warning_delay)| warning_delay),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn active_date_range_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    let (remaining, first_timestamp) = impl_active_timestamp(context, input, false)?;
    // TODO: Does the space0 at the end of the active/inactive timestamp parsers cause this to be incorrect? I could use a look-behind to make sure the preceding character is not whitespace
    let (remaining, _separator) = tag("--")(remaining)?;
    let (remaining, second_timestamp) = impl_active_timestamp(context, remaining, false)?;

    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Timestamp {
            source: source.into(),
            timestamp_type: TimestampType::ActiveRange,
            range_type: TimestampRangeType::DateRange,
            start: first_timestamp.start,
            end: second_timestamp.end,
            start_time: first_timestamp.start_time,
            end_time: second_timestamp.end_time,
            repeater: first_timestamp.repeater.or(second_timestamp.repeater),
            warning_delay: first_timestamp
                .warning_delay
                .or(second_timestamp.warning_delay),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn active_time_range_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    let (remaining, _) = tag("<")(input)?;
    let (remaining, start_date) = date(context, remaining)?;
    let time_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &active_time_rest_end,
    });
    let time_context = context.with_additional_node(&time_context);
    let first_time_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &time_range_rest_end,
    });
    let first_time_context = time_context.with_additional_node(&first_time_context);
    let (remaining, (_, first_time)) = tuple((
        space1,
        parser_with_context!(time(false))(&first_time_context),
    ))(remaining)?;
    let (remaining, _) = tag("-")(remaining)?;
    let (remaining, second_time) = parser_with_context!(time(true))(&time_context)(remaining)?;
    let (remaining, repeater) =
        opt(tuple((space1, parser_with_context!(repeater)(context))))(remaining)?;
    let (remaining, warning_delay) = opt(tuple((
        space1,
        parser_with_context!(warning_delay)(context),
    )))(remaining)?;
    let (remaining, _) = tag(">")(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Timestamp {
            source: source.into(),
            timestamp_type: TimestampType::ActiveRange,
            range_type: TimestampRangeType::TimeRange,
            start: Some(start_date.clone()),
            end: Some(start_date),
            start_time: Some(first_time),
            end_time: Some(second_time),
            repeater: repeater.map(|(_, repeater)| repeater),
            warning_delay: warning_delay.map(|(_, warning_delay)| warning_delay),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn inactive_date_range_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    let (remaining, first_timestamp) = impl_inactive_timestamp(context, input, false)?;
    // TODO: Does the space0 at the end of the active/inactive timestamp parsers cause this to be incorrect? I could use a look-behind to make sure the preceding character is not whitespace
    let (remaining, _separator) = tag("--")(remaining)?;
    let (remaining, second_timestamp) = impl_inactive_timestamp(context, remaining, false)?;

    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Timestamp {
            source: source.into(),

            timestamp_type: TimestampType::InactiveRange,
            range_type: TimestampRangeType::DateRange,
            start: first_timestamp.start,
            end: second_timestamp.end,
            start_time: first_timestamp.start_time,
            end_time: second_timestamp.end_time,
            repeater: first_timestamp.repeater.or(second_timestamp.repeater),
            warning_delay: first_timestamp
                .warning_delay
                .or(second_timestamp.warning_delay),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn inactive_time_range_timestamp<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Timestamp<'s>> {
    let (remaining, _) = tag("[")(input)?;
    let (remaining, start_date) = date(context, remaining)?;
    let time_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &inactive_time_rest_end,
    });
    let time_context = context.with_additional_node(&time_context);
    let first_time_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &time_range_rest_end,
    });
    let first_time_context = time_context.with_additional_node(&first_time_context);
    let (remaining, (_, first_time)) = tuple((
        space1,
        parser_with_context!(time(false))(&first_time_context),
    ))(remaining)?;
    let (remaining, _) = tag("-")(remaining)?;
    let (remaining, second_time) = parser_with_context!(time(true))(&time_context)(remaining)?;
    let (remaining, repeater) =
        opt(tuple((space1, parser_with_context!(repeater)(context))))(remaining)?;
    let (remaining, warning_delay) = opt(tuple((
        space1,
        parser_with_context!(warning_delay)(context),
    )))(remaining)?;
    let (remaining, _) = tag("]")(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Timestamp {
            source: source.into(),
            timestamp_type: TimestampType::InactiveRange,
            range_type: TimestampRangeType::TimeRange,
            start: Some(start_date.clone()),
            end: Some(start_date),
            start_time: Some(first_time),
            end_time: Some(second_time),
            repeater: repeater.map(|(_, repeater)| repeater),
            warning_delay: warning_delay.map(|(_, warning_delay)| warning_delay),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn date<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Date<'s>> {
    let (remaining, year) = verify(digit1, |year: &OrgSource<'_>| year.len() == 4)(input)?;
    let (remaining, _) = tag("-")(remaining)?;
    let (remaining, month) = verify(digit1, |month: &OrgSource<'_>| month.len() == 2)(remaining)?;
    let (remaining, _) = tag("-")(remaining)?;
    let (remaining, day_of_month) = verify(digit1, |day_of_month: &OrgSource<'_>| {
        day_of_month.len() == 2
    })(remaining)?;
    let (remaining, day_name) =
        opt(tuple((space1, parser_with_context!(dayname)(context))))(remaining)?;

    let year = Year::new(Into::<&str>::into(year))
        .expect("TODO: I should be able to return CustomError from nom parsers.");
    let month = Month::new(Into::<&str>::into(month))
        .expect("TODO: I should be able to return CustomError from nom parsers.");
    let day_of_month = DayOfMonth::new(Into::<&str>::into(day_of_month))
        .expect("TODO: I should be able to return CustomError from nom parsers.");

    let date = Date::new(
        year,
        month,
        day_of_month,
        day_name.map(|(_, day_name)| Into::<&str>::into(day_name)),
    )
    .expect("TODO: I should be able to return CustomError from nom parsers.");

    Ok((remaining, date))
}

fn dayname<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &dayname_end,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, body) = recognize(verify(
        many_till(
            anychar,
            parser_with_context!(exit_matcher_parser)(&parser_context),
        ),
        |(body, _end_contents)| !body.is_empty(),
    ))(input)?;

    Ok((remaining, body))
}

fn dayname_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(verify(anychar, |c| {
        c.is_whitespace() || "+-]>0123456789\n".contains(*c)
    }))(input)
}

const fn time(
    allow_rest: bool,
) -> impl for<'b, 'g, 'r, 's> Fn(RefContext<'b, 'g, 'r, 's>, OrgSource<'s>) -> Res<OrgSource<'s>, Time<'s>>
{
    move |context, input| _time(context, input, allow_rest)
}

fn _time<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    allow_rest: bool,
) -> Res<OrgSource<'s>, Time<'s>> {
    let (remaining, hour) = verify(digit1, |hour: &OrgSource<'_>| {
        hour.len() >= 1 && hour.len() <= 2
    })(input)?;
    let (remaining, _) = tag(":")(remaining)?;
    let (remaining, minute) =
        verify(digit1, |minute: &OrgSource<'_>| minute.len() == 2)(remaining)?;
    let (remaining, time_rest) = if allow_rest {
        opt(parser_with_context!(time_rest)(context))(remaining)?
    } else {
        (remaining, None)
    };

    let hour = Hour::new(Into::<&str>::into(hour))
        .expect("TODO: I should be able to return CustomError from nom parsers.");
    let minute = Minute::new(Into::<&str>::into(minute))
        .expect("TODO: I should be able to return CustomError from nom parsers.");
    let time = Time::new(hour, minute, time_rest.map(Into::<&str>::into))
        .expect("TODO: I should be able to return CustomError from nom parsers.");

    Ok((remaining, time))
}

fn time_rest<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, body) = recognize(verify(
        many_till(anychar, parser_with_context!(exit_matcher_parser)(context)),
        |(body, _end_contents)| !body.is_empty(),
    ))(input)?;

    Ok((remaining, body))
}

fn active_time_rest_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alt((
        recognize(verify(anychar, |c| ">\n".contains(*c))),
        recognize(tuple((space1, parser_with_context!(repeater)(context)))),
        recognize(tuple((
            space1,
            parser_with_context!(warning_delay)(context),
        ))),
    ))(input)
}

fn inactive_time_rest_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alt((
        recognize(verify(anychar, |c| "]\n".contains(*c))),
        recognize(tuple((space1, parser_with_context!(repeater)(context)))),
        recognize(tuple((
            space1,
            parser_with_context!(warning_delay)(context),
        ))),
    ))(input)
}

fn time_range_rest_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    // We pop off the most recent context element to get a context tree with just the active/inactive_time_rest_end exit matcher (removing this function from the exit matcher chain) because the 2nd time in the range does not end when a "-TIME" pattern is found.
    let parent_node = context.get_parent().expect("Two context elements are added to the tree when adding this exit matcher, so it should be impossible for this to return None.");
    let exit_contents = recognize(tuple((
        tag("-"),
        parser_with_context!(time(true))(&parent_node),
    )))(input);
    #[allow(clippy::let_and_return)] // otherwise parent_node does not live long enough.
    exit_contents
}

fn repeater<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Repeater> {
    // + for cumulative type
    // ++ for catch-up type
    // .+ for restart type
    let (remaining, repeater_type) = alt((
        map(tag("++"), |_| RepeaterType::CatchUp),
        map(tag("+"), |_| RepeaterType::Cumulative),
        map(tag(".+"), |_| RepeaterType::Restart),
    ))(input)?;
    let (remaining, value) = digit1(remaining)?;
    let value = Into::<&str>::into(value)
        .parse()
        .expect("digit1 ensures this will parse as a number.");
    // h = hour, d = day, w = week, m = month, y = year
    let (remaining, unit) = alt((
        map(tag("h"), |_| TimeUnit::Hour),
        map(tag("d"), |_| TimeUnit::Day),
        map(tag("w"), |_| TimeUnit::Week),
        map(tag("m"), |_| TimeUnit::Month),
        map(tag("y"), |_| TimeUnit::Year),
    ))(remaining)?;
    Ok((
        remaining,
        Repeater {
            repeater_type,
            value,
            unit,
        },
    ))
}

fn warning_delay<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, WarningDelay> {
    // - for all type
    // -- for first type
    let (remaining, warning_delay_type) = alt((
        map(tag("--"), |_| WarningDelayType::First),
        map(tag("-"), |_| WarningDelayType::All),
    ))(input)?;
    let (remaining, value) = digit1(remaining)?;
    let value = Into::<&str>::into(value)
        .parse()
        .expect("digit1 ensures this will parse as a number.");
    // h = hour, d = day, w = week, m = month, y = year
    let (remaining, unit) = alt((
        map(tag("h"), |_| TimeUnit::Hour),
        map(tag("d"), |_| TimeUnit::Day),
        map(tag("w"), |_| TimeUnit::Week),
        map(tag("m"), |_| TimeUnit::Month),
        map(tag("y"), |_| TimeUnit::Year),
    ))(remaining)?;
    Ok((
        remaining,
        WarningDelay {
            warning_delay_type,
            value,
            unit,
        },
    ))
}
