use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::consumed;
use nom::combinator::eof;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::util::exit_matcher_parser;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::parser::util::immediate_in_section;
use crate::parser::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::parser::util::start_of_line;
use crate::types::NodeProperty;
use crate::types::PropertyDrawer;

pub(crate) fn property_drawer<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PropertyDrawer<'s>> {
    if immediate_in_section(context, "property-drawer") {
        return Err(nom::Err::Error(CustomError::Static(
            "Cannot nest objects of the same element",
        )));
    }
    let (
        remaining,
        (_start_of_line, _leading_whitespace, _open_tag, _trailing_whitespace, _line_ending),
    ) = tuple((
        start_of_line,
        space0,
        tag_no_case(":PROPERTIES:"),
        space0,
        line_ending,
    ))(input)?;

    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("property-drawer"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &property_drawer_end,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let (remaining, (contents, children)) =
        consumed(parser_with_context!(children)(&parser_context))(remaining)?;
    let (remaining, _end) = property_drawer_end(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        PropertyDrawer {
            source: source.into(),
            children,
            contents: if contents.len() > 0 {
                Some(contents.into())
            } else {
                None
            },
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn children<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<NodeProperty<'s>>> {
    let node_property_matcher = parser_with_context!(node_property)(context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(context);
    let (remaining, (children, _exit_contents)) =
        many_till(node_property_matcher, exit_matcher)(input)?;
    Ok((remaining, children))
}

fn property_drawer_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(tuple((
        start_of_line,
        space0,
        tag_no_case(":end:"),
        space0,
        alt((line_ending, eof)),
    )))(input)
}

fn node_property<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, NodeProperty<'s>> {
    let (remaining, (_start_of_line, _leading_whitespace, _open_colon, name, _close_colon)) =
        tuple((
            start_of_line,
            space0,
            tag(":"),
            parser_with_context!(node_property_name)(context),
            tag(":"),
        ))(input)?;
    match tuple((
        space0::<OrgSource<'_>, nom::error::Error<OrgSource<'_>>>,
        line_ending,
    ))(remaining)
    {
        Ok((remaining, _ws)) => {
            let source = get_consumed(input, remaining);
            Ok((
                remaining,
                NodeProperty {
                    source: source.into(),
                    property_name: Into::<&str>::into(name),
                    value: None,
                },
            ))
        }
        Err(_) => {
            let (remaining, (_ws, value, _line_ending)) =
                tuple((space1, is_not("\r\n"), line_ending))(remaining)?;
            let source = get_consumed(input, remaining);
            Ok((
                remaining,
                NodeProperty {
                    source: source.into(),
                    property_name: Into::<&str>::into(name),
                    value: Some(value.into()),
                },
            ))
        }
    }
}

fn node_property_name<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &node_property_name_end,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, name) = recognize(tuple((
        verify(
            many_till(
                anychar,
                parser_with_context!(exit_matcher_parser)(&parser_context),
            ),
            |(children, _exit_contents)| !children.is_empty(),
        ),
        opt(tag("+")),
    )))(input)?;

    Ok((remaining, name))
}

fn node_property_name_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(tuple((
        alt((tag("+:"), tag(":"))),
        alt((space1, line_ending, eof)),
    )))(input)
}
