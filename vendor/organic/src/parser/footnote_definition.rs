use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::bytes::complete::take_while;
use nom::character::complete::space0;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::many1;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::util::include_input;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::WORD_CONSTITUENT_CHARACTERS;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::element_parser::element;
use crate::parser::util::blank_line;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::immediate_in_section;
use crate::parser::util::maybe_consume_trailing_whitespace;
use crate::parser::util::start_of_line;
use crate::types::FootnoteDefinition;
use crate::types::Keyword;

pub(crate) fn footnote_definition<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, FootnoteDefinition<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    if immediate_in_section(context, "footnote definition") {
        return Err(nom::Err::Error(CustomError::Static(
            "Cannot nest objects of the same element",
        )));
    }
    start_of_line(remaining)?;
    // Cannot be indented.
    let (remaining, (_, lbl, _, _, _)) = tuple((
        tag_no_case("[fn:"),
        label,
        tag("]"),
        space0,
        opt(verify(many0(blank_line), |lines: &Vec<OrgSource<'_>>| {
            lines.len() <= 2
        })),
    ))(remaining)?;
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("footnote definition"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &footnote_definition_end,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let element_matcher = parser_with_context!(element(true))(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);
    let before_contents = remaining;
    let (mut remaining, (mut children, _exit_contents)) =
        many_till(include_input(element_matcher), exit_matcher)(remaining)?;

    // Re-parse the last element of the footnote definition with consume trailing whitespace off because the trailing whitespace needs to belong to the footnote definition, not the contents.
    if context.should_consume_trailing_whitespace() {
        if let Some((final_item_input, _)) = children.pop() {
            let final_item_context = ContextElement::ConsumeTrailingWhitespace(false);
            let final_item_context = parser_context.with_additional_node(&final_item_context);
            let (remain, reparsed_final_item) =
                parser_with_context!(element(true))(&final_item_context)(final_item_input)?;
            children.push((final_item_input, reparsed_final_item));
            remaining = remain;
        }
    }

    let contents = get_consumed(before_contents, remaining);
    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        FootnoteDefinition {
            source: source.into(),
            contents: Some(contents.into()),
            post_blank: post_blank.map(Into::<&str>::into),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            label: lbl.into(),
            children: children.into_iter().map(|(_, item)| item).collect(),
        },
    ))
}

pub(crate) fn label<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    take_while(|c| WORD_CONSTITUENT_CHARACTERS.contains(c) || "-_".contains(c))(input)
}

fn footnote_definition_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, source) = alt((
        recognize(tuple((
            parser_with_context!(maybe_consume_trailing_whitespace)(context),
            |i| detect_footnote_definition(std::iter::empty(), i, context, i),
        ))),
        recognize(tuple((
            start_of_line,
            verify(many1(blank_line), |lines: &Vec<OrgSource<'_>>| {
                lines.len() >= 2
            }),
        ))),
    ))(input)?;

    Ok((remaining, source))
}

pub(crate) fn detect_footnote_definition<'b, 'g, 'r, 's, AK>(
    _affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    tuple((start_of_line, tag_no_case("[fn:"), label, tag("]")))(remaining)?;
    Ok((input, ()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::bind_context;
    use crate::context::Context;
    use crate::context::GlobalSettings;
    use crate::context::List;
    use crate::types::StandardProperties;

    #[test]
    fn two_paragraphs() {
        let input = OrgSource::new(
            "[fn:1] A footnote.

[fn:2] A multi-

line footnote.",
        );
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let footnote_definition_matcher = bind_context!(element(true), &initial_context);
        let (remaining, first_footnote_definition) =
            footnote_definition_matcher(input).expect("Parse first footnote_definition");
        let (remaining, second_footnote_definition) =
            footnote_definition_matcher(remaining).expect("Parse second footnote_definition.");
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(
            first_footnote_definition.get_source(),
            "[fn:1] A footnote.

"
        );
        assert_eq!(
            second_footnote_definition.get_source(),
            "[fn:2] A multi-

line footnote."
        );
    }

    #[test]
    fn multiline_break() {
        let input = OrgSource::new(
            "[fn:2] A multi-

line footnote.


not in the footnote.",
        );
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let footnote_definition_matcher = bind_context!(element(true), &initial_context);
        let (remaining, first_footnote_definition) =
            footnote_definition_matcher(input).expect("Parse first footnote_definition");
        assert_eq!(Into::<&str>::into(remaining), "not in the footnote.");
        assert_eq!(
            first_footnote_definition.get_source(),
            "[fn:2] A multi-

line footnote.


"
        );
    }
}
