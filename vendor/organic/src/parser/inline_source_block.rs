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
#[cfg(feature = "tracing")]
use tracing::span;

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
use crate::types::InlineSourceBlock;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn inline_source_block<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, InlineSourceBlock<'s>> {
    let (remaining, _) = tag_no_case("src_")(input)?;
    let (remaining, language) = lang(context, remaining)?;
    let (remaining, parameters) = opt(parser_with_context!(header)(context))(remaining)?;
    let (remaining, value) = body(context, remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        InlineSourceBlock {
            source: source.into(),
            language: language.into(),
            parameters: parameters.map(Into::<&str>::into),
            value: value.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn lang<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &lang_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, lang) = recognize(many_till(
        verify(anychar, |c| !(c.is_whitespace() || "[{".contains(*c))),
        parser_with_context!(exit_matcher_parser)(&parser_context),
    ))(input)?;
    Ok((remaining, lang))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn lang_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(one_of("[{"))(input)
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
        class: ExitClass::Beta,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, header_contents) = recognize(many_till(
        anychar,
        parser_with_context!(exit_matcher_parser)(&parser_context),
    ))(remaining)?;
    let (remaining, _) = tag("]")(remaining)?;
    Ok((remaining, header_contents))
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
fn body<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tag("{")(input)?;

    let exit_with_depth = body_end(remaining.get_brace_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, body_contents) = recognize(many_till(
        anychar,
        parser_with_context!(exit_matcher_parser)(&parser_context),
    ))(remaining)?;
    let (remaining, _) = {
        #[cfg(feature = "tracing")]
        let span = span!(
            tracing::Level::DEBUG,
            "outside end body",
            remaining = Into::<&str>::into(remaining)
        );
        #[cfg(feature = "tracing")]
        let _enter = span.enter();
        tag("}")(remaining)?
    };
    Ok((remaining, body_contents))
}

fn body_end(starting_brace_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _body_end(context, input, starting_brace_depth)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn _body_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_brace_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_brace_depth() - starting_brace_depth;
    if current_depth > 0 {
        // Its impossible for the next character to end the body if we're any amount of brace deep
        return Err(nom::Err::Error(CustomError::Static("NoBodyEnd")));
    }
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing brace should end the body.
        unreachable!("Exceeded body brace depth.")
    }
    alt((tag("}"), line_ending))(input)
}
