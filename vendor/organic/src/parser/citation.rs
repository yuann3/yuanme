use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::combinator::map;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many1;
use nom::multi::many_till;
use nom::multi::separated_list1;
use nom::sequence::tuple;
use nom::InputTake;

use super::citation_reference::must_balance_bracket;
use super::closer;
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
use crate::parser::citation_reference::citation_reference;
use crate::parser::citation_reference::citation_reference_key;
use crate::parser::object_parser::standard_set_object;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::types::Citation;
use crate::types::Object;

pub(crate) fn citation<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Citation<'s>> {
    // TODO: Despite being a standard object, citations cannot exist inside the global prefix/suffix for other citations because citations must contain something that matches @key which is forbidden inside the global prefix/suffix. This TODO is to evaluate if its worth putting in an explicit check for this (which can be easily accomplished by checking the output of `get_bracket_depth()`). I suspect its not worth it because I expect, outside of intentionally crafted inputs, this parser will exit immediately inside a citation since it is unlikely to find the "[cite" substring inside a citation global prefix/suffix.
    let (remaining, _) = tag_no_case("[cite")(input)?;
    closer::expect(context, remaining, "]")?;
    let (remaining, style) = opt(citestyle)(remaining)?;
    let (remaining, _) = tag(":")(remaining)?;
    let (remaining, prefix) =
        must_balance_bracket(opt(parser_with_context!(global_prefix)(context)))(remaining)?;

    let contents_begin = remaining;
    let (remaining, references) =
        separated_list1(tag(";"), parser_with_context!(citation_reference)(context))(remaining)?;
    let contents_end = {
        let (rem, _) = opt(tag(";"))(remaining)?;
        rem
    };
    let (remaining, suffix) = must_balance_bracket(opt(map(
        tuple((tag(";"), parser_with_context!(global_suffix)(context))),
        |(_, suffix)| suffix,
    )))(remaining)?;
    let (remaining, _) = tag("]")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    let contents = contents_begin.get_until(contents_end);
    Ok((
        remaining,
        Citation {
            source: source.into(),
            style: style.map(Into::<&str>::into),
            prefix: prefix.unwrap_or(Vec::new()),
            suffix: suffix.unwrap_or(Vec::new()),
            children: references,
            contents: Into::<&str>::into(contents),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn citestyle<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    map(
        tuple((
            tag("/"),
            recognize(tuple((style, opt(tuple((tag("/"), variant)))))),
        )),
        |(_, style)| style,
    )(input)
}

fn style<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(many1(verify(anychar, |c| {
        c.is_alphanumeric() || "_-".contains(*c)
    })))(input)
}

fn variant<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(many1(verify(anychar, |c| {
        c.is_alphanumeric() || "_-/".contains(*c)
    })))(input)
}

fn global_prefix<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Object<'s>>> {
    let exit_with_depth = global_prefix_end(input.get_bracket_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, (children, _exit_contents)) = verify(
        many_till(
            parser_with_context!(standard_set_object)(&parser_context),
            parser_with_context!(exit_matcher_parser)(&parser_context),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    )(input)?;
    let (remaining, _) = tag(";")(remaining)?;
    Ok((remaining, children))
}

fn global_prefix_end(starting_bracket_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _global_prefix_end(context, input, starting_bracket_depth)
}

fn _global_prefix_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_bracket_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_bracket_depth() - starting_bracket_depth;
    if current_depth < 0 {
        // An object inside went past this closer. Upstream panicked here;
        // ending now makes that object fail and be read again as text.
        return Ok((input, input.take(0)));
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

fn global_suffix<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Object<'s>>> {
    let exit_with_depth = global_suffix_end(input.get_bracket_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, (children, _exit_contents)) = verify(
        many_till(
            parser_with_context!(standard_set_object)(&parser_context),
            parser_with_context!(exit_matcher_parser)(&parser_context),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    )(input)?;
    Ok((remaining, children))
}

fn global_suffix_end(starting_bracket_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _global_suffix_end(context, input, starting_bracket_depth)
}

fn _global_suffix_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_bracket_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_bracket_depth() - starting_bracket_depth;
    if current_depth < 0 {
        // An object inside went past this closer. Upstream panicked here;
        // ending now makes that object fail and be read again as text.
        return Ok((input, input.take(0)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::bind_context;
    use crate::context::Context;
    use crate::context::GlobalSettings;
    use crate::context::List;
    use crate::parser::element_parser::element;
    use crate::types::Element;
    use crate::types::StandardProperties;

    #[test]
    fn citation_simple() -> Result<(), Box<dyn std::error::Error>> {
        let input = OrgSource::new("[cite:@foo]");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let paragraph_matcher = bind_context!(element(true), &initial_context);
        let (remaining, first_paragraph) = paragraph_matcher(input).expect("Parse first paragraph");
        let first_paragraph = match first_paragraph {
            Element::Paragraph(paragraph) => paragraph,
            _ => panic!("Should be a paragraph!"),
        };
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(first_paragraph.get_source(), "[cite:@foo]");
        assert_eq!(first_paragraph.children.len(), 1);

        match first_paragraph
            .children
            .first()
            .expect("Len already asserted to be 1.")
        {
            Object::Citation(inner) => {
                assert_eq!(inner.get_source(), "[cite:@foo]");
                assert_eq!(inner.children.len(), 1);
                assert!(inner.prefix.is_empty());
                assert!(inner.suffix.is_empty());
                assert!(inner.style.is_none());
                let citation_reference = inner
                    .children
                    .first()
                    .expect("Len already asserted to be 1.");
                assert_eq!(citation_reference.get_source(), "@foo");
                assert_eq!(citation_reference.key, "foo");
                assert!(citation_reference.prefix.is_empty());
                assert!(citation_reference.suffix.is_empty());
            }
            _ => {
                return Err("Child should be a citation.".into());
            }
        };
        Ok(())
    }
}
