use nom::branch::alt;
use nom::bytes::complete::is_not;
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
use nom::combinator::verify;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::paragraph::empty_paragraph;
use super::util::in_section;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::context::bind_context;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::element_parser::element;
use crate::parser::util::blank_line;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::types::CenterBlock;
use crate::types::Element;
use crate::types::Keyword;
use crate::types::QuoteBlock;
use crate::types::SpecialBlock;

pub(crate) fn greater_block<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    input: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    pre_affiliated_keywords_input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Element<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    start_of_line(input)?;
    let (remaining, _leading_whitespace) = space0(input)?;
    let (remaining, (_begin, name)) = tuple((
        tag_no_case("#+begin_"),
        verify(name, |name: &OrgSource<'_>| {
            !matches!(
                Into::<&str>::into(name).to_lowercase().as_str(),
                "comment" | "example" | "export" | "src" | "verse",
            )
        }),
    ))(remaining)?;
    let name = Into::<&str>::into(name);
    let (remaining, element) = match name.to_lowercase().as_str() {
        "center" => center_block(
            context,
            remaining,
            pre_affiliated_keywords_input,
            affiliated_keywords,
        )?,
        "quote" => quote_block(
            context,
            remaining,
            pre_affiliated_keywords_input,
            affiliated_keywords,
        )?,
        _ => special_block(name)(
            context,
            remaining,
            pre_affiliated_keywords_input,
            affiliated_keywords,
        )?,
    };
    Ok((remaining, element))
}

fn center_block<'b, 'g, 'r, 's, AK>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    pre_affiliated_keywords_input: OrgSource<'s>,
    affiliated_keywords: AK,
) -> Res<OrgSource<'s>, Element<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, body) = greater_block_body(
        context,
        input,
        pre_affiliated_keywords_input,
        "center",
        "center block",
    )?;
    Ok((
        remaining,
        Element::CenterBlock(CenterBlock {
            source: body.source,
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            children: body.children,
            contents: body.contents,
            post_blank: body.post_blank,
        }),
    ))
}

fn quote_block<'b, 'g, 'r, 's, AK>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    pre_affiliated_keywords_input: OrgSource<'s>,
    affiliated_keywords: AK,
) -> Res<OrgSource<'s>, Element<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, body) = greater_block_body(
        context,
        input,
        pre_affiliated_keywords_input,
        "quote",
        "quote block",
    )?;
    Ok((
        remaining,
        Element::QuoteBlock(QuoteBlock {
            source: body.source,
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            children: body.children,
            contents: body.contents,
            post_blank: body.post_blank,
        }),
    ))
}

fn special_block<'s, AK>(
    name: &'s str,
) -> impl for<'b, 'g, 'r> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
    OrgSource<'s>,
    AK,
) -> Res<OrgSource<'s>, Element<'s>>
       + 's
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let context_name = format!("special block {}", name);
    move |context, input, pre_affiliated_keywords_input, affiliated_keywords| {
        _special_block(
            context,
            input,
            pre_affiliated_keywords_input,
            name,
            context_name.as_str(),
            affiliated_keywords,
        )
    }
}

fn _special_block<'c, 'b, 'g, 'r, 's, AK>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    pre_affiliated_keywords_input: OrgSource<'s>,
    name: &'s str,
    context_name: &'c str,
    affiliated_keywords: AK,
) -> Res<OrgSource<'s>, Element<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, parameters) = opt(tuple((space1, parameters)))(input)?;
    let (remaining, body) = greater_block_body(
        context,
        remaining,
        pre_affiliated_keywords_input,
        name,
        context_name,
    )?;
    Ok((
        remaining,
        Element::SpecialBlock(SpecialBlock {
            source: body.source,
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            children: body.children,
            block_type: name,
            parameters: parameters.map(|(_, parameters)| Into::<&str>::into(parameters)),
            contents: body.contents,
            post_blank: body.post_blank,
        }),
    ))
}

#[derive(Debug)]
struct GreaterBlockBody<'s> {
    source: &'s str,
    children: Vec<Element<'s>>,
    contents: Option<&'s str>,
    post_blank: Option<&'s str>,
}

fn greater_block_body<'c, 'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    pre_affiliated_keywords_input: OrgSource<'s>,
    name: &'c str,
    context_name: &'c str,
) -> Res<OrgSource<'s>, GreaterBlockBody<'s>> {
    if in_section(context, context_name) {
        return Err(nom::Err::Error(CustomError::Static(
            "Cannot nest objects of the same element",
        )));
    }
    let exit_with_name = greater_block_end(name);
    let (remaining, _nl) = tuple((space0, line_ending))(input)?;
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context(context_name),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &exit_with_name,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
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

    let (remaining, _end) = exit_with_name(&parser_context, remaining)?;

    // Not checking if parent exit matcher is causing exit because the greater_block_end matcher asserts we matched a full greater block

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(pre_affiliated_keywords_input, remaining);
    Ok((
        remaining,
        GreaterBlockBody {
            source: Into::<&str>::into(source),
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

fn greater_block_end(name: &str) -> impl ContextMatcher + '_ {
    move |context, input: OrgSource<'_>| _greater_block_end(context, input, name)
}

fn _greater_block_end<'b, 'g, 'r, 's, 'c>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    name: &'c str,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    let (remaining, _leading_whitespace) = space0(input)?;
    let (remaining, (_begin, _name, _ws)) = tuple((
        tag_no_case("#+end_"),
        tag_no_case(name),
        alt((eof, line_ending)),
    ))(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

pub(crate) fn leading_blank_lines_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(not(blank_line))(input)
}
