use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::eof;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::closer::expect_end;
use super::closer::End;
use super::greater_block::leading_blank_lines_end;
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
use crate::types::DynamicBlock;
use crate::types::Element;
use crate::types::Keyword;

pub(crate) fn dynamic_block<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, DynamicBlock<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    if immediate_in_section(context, "dynamic block") {
        return Err(nom::Err::Error(CustomError::Static(
            "Cannot nest objects of the same element",
        )));
    }

    start_of_line(remaining)?;
    let (remaining, _leading_whitespace) = space0(remaining)?;
    let (remaining, (_, name, parameters, _, _)) = tuple((
        recognize(tuple((tag_no_case("#+begin:"), space1))),
        name,
        opt(tuple((space1, parameters))),
        space0,
        line_ending,
    ))(remaining)?;
    expect_end(remaining, End::DynamicBlock)?;
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("dynamic block"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &dynamic_block_end,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let parameters = parameters.map(|(_ws, parameters)| parameters);
    let element_matcher = parser_with_context!(element(true))(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);
    not(exit_matcher)(remaining)?;
    let contents_begin = remaining;
    let blank_line_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Alpha,
        exit_matcher: &leading_blank_lines_end,
    });
    let blank_line_context = parser_context.with_additional_node(&blank_line_context);

    let (remaining, leading_blank_lines) =
        opt(bind_context!(empty_paragraph, &blank_line_context))(remaining)?;
    let (remaining, (mut children, _exit_contents)) =
        many_till(element_matcher, exit_matcher)(remaining)?;
    if let Some(lines) = leading_blank_lines {
        children.insert(0, Element::Paragraph(lines));
    }
    let contents = get_consumed(contents_begin, remaining);

    let (remaining, _end) = dynamic_block_end(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        DynamicBlock {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            block_name: name.into(),
            parameters: parameters.map(|val| val.into()),
            children,
            contents: if contents.len() > 0 {
                Some(Into::<&str>::into(contents))
            } else {
                None
            },
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn name<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    is_not(" \t\r\n")(input)
}

fn parameters<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(many_till(anychar, peek(tuple((space0, line_ending)))))(input)
}

fn dynamic_block_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    let (remaining, source) = recognize(tuple((
        space0,
        tag_no_case("#+end"),
        opt(tag(":")),
        alt((eof, line_ending)),
    )))(input)?;
    Ok((remaining, source))
}
