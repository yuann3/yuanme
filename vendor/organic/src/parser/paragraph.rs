use nom::branch::alt;
use nom::character::complete::space1;
use nom::combinator::consumed;
use nom::combinator::eof;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many1;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::element_parser::detect_element;
use super::org_source::OrgSource;
use super::util::blank_line;
use super::util::get_consumed;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::object_parser::standard_set_object;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::start_of_line;
use crate::types::Keyword;
use crate::types::Paragraph;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, affiliated_keywords))
)]
pub(crate) fn paragraph<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Paragraph<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let contexts = [ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &paragraph_end,
    })];
    let parser_context = context.with_additional_node(&contexts[0]);
    let standard_set_object_matcher = parser_with_context!(standard_set_object)(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);

    let (remaining, (contents, (children, _exit_contents))) = consumed(verify(
        many_till(standard_set_object_matcher, exit_matcher),
        |(children, _exit_contents)| !children.is_empty(),
    ))(remaining)?;

    // Not checking parent exit matcher because if there are any children matched then we have a valid paragraph.

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Paragraph {
            source: source.into(),
            contents: Some(contents.into()),
            post_blank: post_blank.map(Into::<&str>::into),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            children,
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn empty_paragraph<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Paragraph<'s>> {
    // If it is just a single newline then source, contents, and post-blank are "\n".
    // If it has multiple newlines then contents is the first "\n" and post-blank is all the new lines.
    // If there are any spaces on the first line then post-blank excludes the first line.

    let exit_matcher = parser_with_context!(exit_matcher_parser)(context);

    let (remaining, first_line_with_spaces) =
        opt(recognize(tuple((space1, org_line_ending))))(input)?;

    let post_blank_begin = remaining;

    if let Some(first_line_with_spaces) = first_line_with_spaces {
        let (remaining, _additional_lines) =
            recognize(many_till(blank_line, exit_matcher))(remaining)?;
        let post_blank = get_consumed(post_blank_begin, remaining);
        let source = get_consumed(input, remaining);
        Ok((
            remaining,
            Paragraph::of_text(
                Into::<&str>::into(source),
                Into::<&str>::into(first_line_with_spaces),
                Some(Into::<&str>::into(first_line_with_spaces)),
                Some(Into::<&str>::into(post_blank)),
            ),
        ))
    } else {
        let (remaining, first_line) = blank_line(remaining)?;
        let (remaining, _additional_lines) =
            recognize(many_till(blank_line, exit_matcher))(remaining)?;
        let post_blank = get_consumed(post_blank_begin, remaining);
        let source = get_consumed(input, remaining);
        Ok((
            remaining,
            Paragraph::of_text(
                Into::<&str>::into(source),
                Into::<&str>::into(first_line),
                Some(Into::<&str>::into(first_line)),
                Some(Into::<&str>::into(post_blank)),
            ),
        ))
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn paragraph_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let regular_end = recognize(tuple((start_of_line, many1(blank_line))))(input);
    if regular_end.is_ok() {
        return regular_end;
    }
    alt((
        recognize(parser_with_context!(detect_element(false))(context)),
        eof,
    ))(input)
}

#[cfg(test)]
mod tests {
    use crate::context::bind_context;
    use crate::context::Context;
    use crate::context::ContextElement;
    use crate::context::GlobalSettings;
    use crate::context::List;
    use crate::parser::element_parser::element;
    use crate::parser::org_source::OrgSource;
    use crate::parser::paragraph::empty_paragraph;
    use crate::types::StandardProperties;

    #[test]
    fn two_paragraphs() {
        let input = OrgSource::new("foo bar baz\n\nlorem ipsum");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let paragraph_matcher = bind_context!(element(true), &initial_context);
        let (remaining, first_paragraph) = paragraph_matcher(input).expect("Parse first paragraph");
        let (remaining, second_paragraph) =
            paragraph_matcher(remaining).expect("Parse second paragraph.");
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(first_paragraph.get_source(), "foo bar baz\n\n");
        assert_eq!(second_paragraph.get_source(), "lorem ipsum");
    }

    #[test]
    fn paragraph_whitespace() {
        let input = OrgSource::new("\n");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let paragraph_matcher = bind_context!(empty_paragraph, &initial_context);
        let (remaining, paragraph) = paragraph_matcher(input).expect("Parse paragraph");
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(paragraph.get_source(), "\n");
        assert_eq!(paragraph.get_contents(), Some("\n"));
    }
}
