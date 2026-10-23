use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::bytes::complete::take_while;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::combinator::consumed;
use nom::combinator::eof;
use nom::combinator::not;
use nom::combinator::recognize;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::paragraph::empty_paragraph;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::context::bind_context;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::element_parser::element;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::immediate_in_section;
use crate::parser::util::start_of_line;
use crate::parser::util::WORD_CONSTITUENT_CHARACTERS;
use crate::types::Drawer;
use crate::types::Element;
use crate::types::Keyword;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, affiliated_keywords))
)]
pub(crate) fn drawer<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Drawer<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    if immediate_in_section(context, "drawer") {
        return Err(nom::Err::Error(CustomError::Static(
            "Cannot nest objects of the same element",
        )));
    }
    start_of_line(remaining)?;
    let (remaining, _leading_whitespace) = space0(remaining)?;
    let (remaining, (_open_colon, drawer_name, _close_colon, _new_line)) = tuple((
        tag(":"),
        name,
        tag(":"),
        recognize(tuple((space0, line_ending))),
    ))(remaining)?;

    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("drawer"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &drawer_end,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let (remaining, (contents, children)) =
        consumed(parser_with_context!(children)(&parser_context))(remaining)?;

    let (remaining, _end) = drawer_end(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Drawer {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            drawer_name: drawer_name.into(),
            children,
            contents: Some(contents.into()),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn children<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Element<'s>>> {
    let element_matcher = parser_with_context!(element(true))(context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(context);

    if let Ok((remaining, (_not_exit, empty_para))) =
        tuple((not(exit_matcher), bind_context!(empty_paragraph, context)))(input)
    {
        return Ok((remaining, vec![Element::Paragraph(empty_para)]));
    }

    let (remaining, (children, _exit_contents)) = many_till(element_matcher, exit_matcher)(input)?;

    Ok((remaining, children))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn name<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    take_while(|c| WORD_CONSTITUENT_CHARACTERS.contains(c) || "-_".contains(c))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn drawer_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    recognize(tuple((
        space0,
        tag_no_case(":end:"),
        space0,
        alt((line_ending, eof)),
    )))(input)
}
