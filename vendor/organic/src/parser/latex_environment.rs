use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::bytes::complete::take_while1;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::combinator::eof;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::util::get_consumed;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::start_of_line;
use crate::types::Keyword;
use crate::types::LatexEnvironment;

pub(crate) fn latex_environment<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, LatexEnvironment<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let value_start = remaining;
    start_of_line(remaining)?;
    let (remaining, _leading_whitespace) = space0(remaining)?;
    let (remaining, (_opening, name, _open_close_brace, _ws, _line_ending)) =
        tuple((tag_no_case(r"\begin{"), name, tag("}"), space0, line_ending))(remaining)?;

    let latex_environment_end_specialized = latex_environment_end(name.into());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &latex_environment_end_specialized,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, _contents) = contents(&latex_environment_end_specialized)(context, remaining)?;
    let (remaining, _end) = latex_environment_end_specialized(&parser_context, remaining)?;
    let value_end = remaining;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    let value = get_consumed(value_start, value_end);
    Ok((
        remaining,
        LatexEnvironment {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            value: value.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn name<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    take_while1(|c: char| c.is_alphanumeric() || c == '*')(input)
}

fn contents<F: ContextMatcher>(end_matcher: F) -> impl ContextMatcher {
    move |context, input| _contents(&end_matcher, context, input)
}

fn _contents<'b, 'g, 'r, 's, F: ContextMatcher>(
    end_matcher: F,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, source) = recognize(many_till(
        anychar,
        peek(alt((
            parser_with_context!(exit_matcher_parser)(context),
            parser_with_context!(&end_matcher)(context),
        ))),
    ))(input)?;

    Ok((remaining, source))
}

fn latex_environment_end(current_name: &str) -> impl ContextMatcher {
    let current_name_lower = current_name.to_lowercase();
    move |context, input: OrgSource<'_>| {
        _latex_environment_end(context, input, current_name_lower.as_str())
    }
}

fn _latex_environment_end<'b, 'g, 'r, 's, 'c>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    current_name_lower: &'c str,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    let (remaining, _leading_whitespace) = space0(input)?;
    let (remaining, (_begin, _name, _close_brace, _ws, _line_ending)) = tuple((
        tag_no_case(r"\end{"),
        tag_no_case(current_name_lower),
        tag("}"),
        space0,
        alt((eof, line_ending)),
    ))(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}
