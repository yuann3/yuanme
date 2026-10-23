use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::headline::detect_headline;
use super::org_source::OrgSource;
use super::util::exit_matcher_parser;
use super::util::get_consumed;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::comment::comment;
use crate::parser::element_parser::element;
use crate::parser::planning::planning;
use crate::parser::property_drawer::property_drawer;
use crate::parser::util::blank_line;
use crate::parser::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::types::Element;
use crate::types::Section;

pub(crate) fn zeroth_section<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Section<'s>> {
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("section"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Document,
            exit_matcher: &section_end,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let without_consuming_whitespace_context = ContextElement::ConsumeTrailingWhitespace(false);
    let without_consuming_whitespace_context =
        parser_context.with_additional_node(&without_consuming_whitespace_context);

    let element_matcher = parser_with_context!(element(true))(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);

    let (remaining, comment_and_property_drawer_element) = opt(tuple((
        opt(parser_with_context!(comment)(
            &without_consuming_whitespace_context,
        )),
        parser_with_context!(property_drawer)(context),
        many0(blank_line),
    )))(input)?;

    let (remaining, (mut children, _exit_contents)) = verify(
        many_till(element_matcher, exit_matcher),
        |(children, _exit_contents)| {
            !children.is_empty() || comment_and_property_drawer_element.is_some()
        },
    )(remaining)?;

    if let Some((comment, property_drawer, _ws)) = comment_and_property_drawer_element {
        children.insert(0, Element::PropertyDrawer(property_drawer));
        if let Some(ele) = comment.map(Element::Comment) {
            children.insert(0, ele);
        }
    }

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;

    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Section {
            source: source.into(),
            post_blank: post_blank.map(Into::<&str>::into),
            children,
        },
    ))
}

pub(crate) fn section<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    mut input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Section<'s>> {
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("section"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Document,
            exit_matcher: &section_end,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let element_matcher = parser_with_context!(element(true))(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);
    let (mut remaining, (planning_element, property_drawer_element)) = tuple((
        opt(parser_with_context!(planning)(&parser_context)),
        opt(parser_with_context!(property_drawer)(&parser_context)),
    ))(input)?;
    if planning_element.is_none() && property_drawer_element.is_none() {
        let (remain, _ws) = many0(blank_line)(remaining)?;
        remaining = remain;
        input = remain;
    }
    let (remaining, (mut children, _exit_contents)) = verify(
        many_till(element_matcher, exit_matcher),
        |(children, _exit_contents)| {
            !children.is_empty() || property_drawer_element.is_some() || planning_element.is_some()
        },
    )(remaining)?;
    if let Some(ele) = property_drawer_element.map(Element::PropertyDrawer) {
        children.insert(0, ele);
    }
    if let Some(ele) = planning_element.map(Element::Planning) {
        children.insert(0, ele)
    }

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;

    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Section {
            source: source.into(),
            post_blank: post_blank.map(Into::<&str>::into),
            children,
        },
    ))
}

fn section_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(detect_headline)(input)
}
