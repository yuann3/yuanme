use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::character::complete::one_of;
use nom::character::complete::space0;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many_till;
use nom::sequence::tuple;
use nom::InputTake;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::BracketDepth;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::start_of_line;
use super::OrgSource;
use crate::context::Matcher;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::parser::util::org_line_ending;
use crate::types::BabelCall;
use crate::types::Keyword;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, affiliated_keywords))
)]
pub(crate) fn babel_call<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, BabelCall<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    start_of_line(remaining)?;
    let (remaining, _) = tuple((space0, tag("#+"), tag_no_case("call"), tag(":")))(remaining)?;

    let (remaining, _ws) = space0(remaining)?;
    let (remaining, babel_call_value) = babel_call_value(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        BabelCall {
            source: Into::<&str>::into(source),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            value: Into::<&str>::into(babel_call_value.value),
            call: babel_call_value.call.map(Into::<&str>::into),
            inside_header: babel_call_value.inside_header.map(Into::<&str>::into),
            arguments: babel_call_value.arguments.map(Into::<&str>::into),
            end_header: babel_call_value.end_header.map(Into::<&str>::into),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[derive(Debug)]
struct BabelCallValue<'s> {
    /// The entire string to the right of "#+call: " without the trailing line break.
    value: OrgSource<'s>,

    /// The function name which may contain a line break if there are no headers/arguments.
    call: Option<OrgSource<'s>>,
    inside_header: Option<OrgSource<'s>>,
    arguments: Option<OrgSource<'s>>,
    end_header: Option<OrgSource<'s>>,
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn babel_call_value<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, BabelCallValue<'s>> {
    alt((
        babel_call_value_without_headers,
        babel_call_value_with_headers,
    ))(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn babel_call_value_without_headers<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, BabelCallValue<'s>> {
    let (remaining, value) = babel_call_call_with_headers(input)?;
    let (remaining, _ws) = tuple((space0, org_line_ending))(remaining)?;
    let call = get_consumed(input, remaining);
    Ok((
        remaining,
        BabelCallValue {
            value,
            call: Some(call),
            inside_header: None,
            arguments: None,
            end_header: None,
        },
    ))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn babel_call_value_with_headers<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, BabelCallValue<'s>> {
    let (remaining, call) = opt(babel_call_call_with_headers)(input)?;
    let (remaining, inside_header) = opt(inside_header)(remaining)?;
    let (remaining, arguments) = opt(arguments)(remaining)?;
    let (remaining, end_header) = opt(end_header)(remaining)?;
    let value = get_consumed(input, remaining);
    let (remaining, _ws) = tuple((space0, org_line_ending))(remaining)?;
    Ok((
        remaining,
        BabelCallValue {
            value,
            call,
            inside_header,
            arguments: arguments.flatten(),
            end_header,
        },
    ))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn babel_call_call_with_headers<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    // When babel call contains no arguments or headers (for example: "#+call: lorem ipsum\n") then the trailing line break is part of the call. Otherwise, it is not.
    verify(
        recognize(many_till(
            anychar,
            peek(alt((
                recognize(one_of("[(")),
                recognize(tuple((space0, org_line_ending))),
            ))),
        )),
        |s| s.len() > 0,
    )(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn inside_header<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, contents) = balanced_bracket(
        |i| tag("[")(i),
        |i| peek(tag("]"))(i),
        |i| recognize(tuple((space0, org_line_ending)))(i),
        |i| tag("]")(i),
        |s| s.get_bracket_depth(),
    )(input)?;
    let (contents_start, _) = tag("[")(input)?;
    Ok((remaining, contents.unwrap_or(contents_start.take(0))))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn arguments<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, Option<OrgSource<'s>>> {
    balanced_bracket(
        |i| tag("(")(i),
        |i| peek(tag(")"))(i),
        |i| recognize(tuple((space0, org_line_ending)))(i),
        |i| tag(")")(i),
        |s| s.get_parenthesis_depth(),
    )(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn end_header<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = space0(input)?;
    verify(
        recognize(many_till(anychar, peek(tuple((space0, org_line_ending))))),
        |s| s.len() > 0,
    )(remaining)
}

fn balanced_bracket<
    O: Matcher,
    S: Matcher,
    F: Matcher,
    E: Matcher,
    D: for<'ss> Fn(OrgSource<'ss>) -> BracketDepth,
>(
    opening_parser: O,
    stop_parser: S,
    fail_parser: F,
    end_parser: E,
    depth_function: D,
) -> impl for<'s> Fn(OrgSource<'s>) -> Res<OrgSource<'s>, Option<OrgSource<'s>>> {
    move |input| {
        impl_balanced_bracket::<&O, &S, &F, &E, &D>(
            input,
            &opening_parser,
            &stop_parser,
            &fail_parser,
            &end_parser,
            &depth_function,
        )
    }
}

fn impl_balanced_bracket<
    's,
    O: Matcher,
    S: Matcher,
    F: Matcher,
    E: Matcher,
    D: for<'ss> Fn(OrgSource<'ss>) -> BracketDepth,
>(
    input: OrgSource<'s>,
    opening_parser: O,
    stop_parser: S,
    fail_parser: F,
    end_parser: E,
    depth_function: D,
) -> Res<OrgSource<'s>, Option<OrgSource<'s>>> {
    let (mut remaining, _) = opening_parser(input)?;
    let contents_start = remaining;
    let original_depth = depth_function(remaining);

    loop {
        let bracket_depth = depth_function(remaining);
        if bracket_depth == original_depth {
            let (remain, stop_result) = opt(&stop_parser)(remaining)?;
            remaining = remain;
            if stop_result.is_some() {
                break;
            }
        }

        if fail_parser(remaining).is_ok() {
            return Err(nom::Err::Error(CustomError::Static("Fail parser matched.")));
        }

        let (remain, _) = anychar(remaining)?;
        remaining = remain;
    }
    let contents_end = remaining;

    let (remaining, _) = end_parser(remaining)?;
    let contents = if Into::<&str>::into(contents_start) != Into::<&str>::into(contents_end) {
        Some(contents_start.get_until(contents_end))
    } else {
        None
    };
    Ok((remaining, contents))
}
