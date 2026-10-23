use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::combinator::all_consuming;
use nom::combinator::consumed;
use nom::combinator::map_parser;
use nom::combinator::verify;
use nom::multi::many1;

use super::org_source::BracketDepth;
use super::org_source::OrgSource;
use super::util::confine_context;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use super::util::text_until_exit;
use crate::context::parser_with_context;
use crate::context::Context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::List;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::footnote_definition::label;
use crate::parser::object_parser::standard_set_object;
use crate::parser::util::get_consumed;
use crate::types::FootnoteReference;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn footnote_reference<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, FootnoteReference<'s>> {
    alt((
        parser_with_context!(anonymous_footnote)(context),
        parser_with_context!(footnote_reference_only)(context),
        parser_with_context!(inline_footnote)(context),
    ))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn anonymous_footnote<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, FootnoteReference<'s>> {
    let (remaining, _) = tag_no_case("[fn::")(input)?;
    let exit_with_depth = footnote_definition_end(remaining.get_bracket_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let initial_context = ContextElement::document_context();
    let initial_context = Context::new(context.get_global_settings(), List::new(&initial_context));

    let (remaining, (contents, children)) = consumed(map_parser(
        verify(
            parser_with_context!(text_until_exit)(&parser_context),
            |text| text.len() > 0,
        ),
        confine_context(|i| {
            all_consuming(many1(parser_with_context!(standard_set_object)(
                &initial_context,
            )))(i)
        }),
    ))(remaining)?;

    let (remaining, _) = tag("]")(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        FootnoteReference {
            source: source.into(),
            contents: Some(contents.into()),
            post_blank: post_blank.map(Into::<&str>::into),
            label: None,
            definition: children,
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn inline_footnote<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, FootnoteReference<'s>> {
    let (remaining, _) = tag_no_case("[fn:")(input)?;
    let (remaining, label_contents) = label(remaining)?;
    let (remaining, _) = tag(":")(remaining)?;
    let exit_with_depth = footnote_definition_end(remaining.get_bracket_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let initial_context = ContextElement::document_context();
    let initial_context = Context::new(context.get_global_settings(), List::new(&initial_context));

    let (remaining, (contents, children)) = consumed(map_parser(
        verify(
            parser_with_context!(text_until_exit)(&parser_context),
            |text| text.len() > 0,
        ),
        confine_context(|i| {
            all_consuming(many1(parser_with_context!(standard_set_object)(
                &initial_context,
            )))(i)
        }),
    ))(remaining)?;

    let (remaining, _) = tag("]")(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        FootnoteReference {
            source: source.into(),
            contents: Some(contents.into()),
            post_blank: post_blank.map(Into::<&str>::into),
            label: Some(label_contents.into()),
            definition: children,
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn footnote_reference_only<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, FootnoteReference<'s>> {
    let (remaining, _) = tag_no_case("[fn:")(input)?;
    let (remaining, label_contents) = label(remaining)?;
    let (remaining, _) = tag("]")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        FootnoteReference {
            source: source.into(),
            contents: None,
            post_blank: post_blank.map(Into::<&str>::into),
            label: Some(label_contents.into()),
            definition: Vec::with_capacity(0),
        },
    ))
}

fn footnote_definition_end(starting_bracket_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| {
        _footnote_definition_end(context, input, starting_bracket_depth)
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn _footnote_definition_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_bracket_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_bracket_depth() - starting_bracket_depth;
    if current_depth > 0 {
        // Its impossible for the next character to end the footnote reference definition if we're any amount of brackets deep
        return Err(nom::Err::Error(CustomError::Static(
            "NoFootnoteReferenceDefinitionEnd",
        )));
    }
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing bracket should end the footnote definition.
        unreachable!("Exceeded footnote reference definition bracket depth.")
    }
    tag("]")(input)
}
