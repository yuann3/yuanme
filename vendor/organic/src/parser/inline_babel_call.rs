use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::one_of;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many_till;

use super::org_source::BracketDepth;
use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::types::InlineBabelCall;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn inline_babel_call<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, InlineBabelCall<'s>> {
    let (remaining, _) = tag_no_case("call_")(input)?;
    let (remaining, name) = name(context, remaining)?;
    let (remaining, inside_header) = opt(parser_with_context!(header)(context))(remaining)?;
    let (remaining, arguments) = argument(context, remaining)?;
    let (remaining, end_header) = opt(parser_with_context!(header)(context))(remaining)?;
    let value = get_consumed(input, remaining);
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        InlineBabelCall {
            source: source.into(),
            value: value.into(),
            call: name.into(),
            inside_header: inside_header.map(Into::<&str>::into),
            arguments: if arguments.len() > 0 {
                Some(arguments.into())
            } else {
                None
            },
            end_header: end_header.map(Into::<&str>::into),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn name<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &name_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, name) = recognize(many_till(
        verify(anychar, |c| !(c.is_whitespace() || "[]()".contains(*c))),
        parser_with_context!(exit_matcher_parser)(&parser_context),
    ))(input)?;
    Ok((remaining, name))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn name_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(one_of("[("))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn header<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tag("[")(input)?;

    let exit_with_depth = header_end(remaining.get_bracket_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, name) = recognize(many_till(
        anychar,
        parser_with_context!(exit_matcher_parser)(&parser_context),
    ))(remaining)?;
    let (remaining, _) = tag("]")(remaining)?;
    Ok((remaining, name))
}

fn header_end(starting_bracket_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _header_end(context, input, starting_bracket_depth)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn _header_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_bracket_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_bracket_depth() - starting_bracket_depth;
    if current_depth > 0 {
        // Its impossible for the next character to end the header if we're any amount of bracket deep
        return Err(nom::Err::Error(CustomError::Static("NoHeaderEnd")));
    }
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing bracket should end the header.
        unreachable!("Exceeded header bracket depth.")
    }
    alt((tag("]"), line_ending))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn argument<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tag("(")(input)?;

    let exit_with_depth = argument_end(remaining.get_parenthesis_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, name) = recognize(many_till(
        anychar,
        parser_with_context!(exit_matcher_parser)(&parser_context),
    ))(remaining)?;
    let (remaining, _) = tag(")")(remaining)?;
    Ok((remaining, name))
}

fn argument_end(starting_parenthesis_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _argument_end(context, input, starting_parenthesis_depth)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn _argument_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_parenthesis_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_parenthesis_depth() - starting_parenthesis_depth;
    if current_depth > 0 {
        // Its impossible for the next character to end the argument if we're any amount of parenthesis deep
        return Err(nom::Err::Error(CustomError::Static("NoArgumentEnd")));
    }
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing parenthesis should end the argument.
        unreachable!("Exceeded argument parenthesis depth.")
    }
    alt((tag(")"), line_ending))(input)
}
