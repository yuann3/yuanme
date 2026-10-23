use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::combinator::map;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many1;
use nom::multi::many_till;
use nom::sequence::preceded;
use nom::sequence::tuple;

use super::org_source::BracketDepth;
use super::org_source::OrgSource;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::object_parser::minimal_set_object;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::WORD_CONSTITUENT_CHARACTERS;
use crate::types::CitationReference;
use crate::types::Object;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn citation_reference<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, CitationReference<'s>> {
    let (remaining, prefix) =
        must_balance_bracket(opt(parser_with_context!(key_prefix)(context)))(input)?;
    let (remaining, key) = parser_with_context!(citation_reference_key)(context)(remaining)?;
    let (remaining, suffix) =
        must_balance_bracket(opt(parser_with_context!(key_suffix)(context)))(remaining)?;
    let without_closing_semi_remaining = remaining;
    let (remaining, _closing_semi) = opt(tag(";"))(remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        without_closing_semi_remaining,
        CitationReference {
            source: source.into(),
            key: key.into(),
            prefix: prefix.unwrap_or(Vec::new()),
            suffix: suffix.unwrap_or(Vec::new()),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn citation_reference_key<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, source) = map(
        tuple((
            tag("@"),
            recognize(many1(verify(
                preceded(
                    not(parser_with_context!(exit_matcher_parser)(context)),
                    anychar,
                ),
                |c| {
                    WORD_CONSTITUENT_CHARACTERS.contains(*c)
                        || "-.:?~`'/*@+|(){}<>&_^$#%~".contains(*c)
                },
            ))),
        )),
        |(_, key)| key,
    )(input)?;
    Ok((remaining, source))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn key_prefix<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Object<'s>>> {
    let exit_with_depth = key_prefix_end(input.get_bracket_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, (children, _exit_contents)) = verify(
        many_till(
            parser_with_context!(minimal_set_object)(&parser_context),
            parser_with_context!(exit_matcher_parser)(&parser_context),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    )(input)?;
    Ok((remaining, children))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn key_suffix<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Object<'s>>> {
    let exit_with_depth = key_suffix_end(input.get_bracket_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, (children, _exit_contents)) = verify(
        many_till(
            parser_with_context!(minimal_set_object)(&parser_context),
            parser_with_context!(exit_matcher_parser)(&parser_context),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    )(input)?;
    Ok((remaining, children))
}

fn key_prefix_end(starting_bracket_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _key_prefix_end(context, input, starting_bracket_depth)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn _key_prefix_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_bracket_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_bracket_depth() - starting_bracket_depth;
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing bracket should end the citation.
        unreachable!("Exceeded citation key prefix bracket depth.")
    }
    if current_depth == 0 {
        let close_bracket = tag::<_, _, CustomError>("]")(input);
        if close_bracket.is_ok() {
            return close_bracket;
        }
    }
    alt((
        tag(";"),
        recognize(parser_with_context!(citation_reference_key)(context)),
    ))(input)
}

fn key_suffix_end(starting_bracket_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _key_suffix_end(context, input, starting_bracket_depth)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn _key_suffix_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_bracket_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_bracket_depth() - starting_bracket_depth;
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing bracket should end the citation.
        unreachable!("Exceeded citation key suffix bracket depth.")
    }
    if current_depth == 0 {
        let close_bracket = tag::<_, _, CustomError>("]")(input);
        if close_bracket.is_ok() {
            return close_bracket;
        }
    }
    tag(";")(input)
}

pub(crate) fn must_balance_bracket<'s, F, O>(
    mut inner: F,
) -> impl FnMut(OrgSource<'s>) -> Res<OrgSource<'s>, O>
where
    F: FnMut(OrgSource<'s>) -> Res<OrgSource<'s>, O>,
{
    move |input: OrgSource<'_>| {
        let pre_bracket_depth = input.get_bracket_depth();
        let (remaining, output) = inner(input)?;
        if remaining.get_bracket_depth() - pre_bracket_depth != 0 {
            return Err(nom::Err::Error(CustomError::Static("UnbalancedBrackets")));
        }
        Ok((remaining, output))
    }
}
