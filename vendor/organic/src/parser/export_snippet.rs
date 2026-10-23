use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::combinator::opt;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many1;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::closer;
use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::types::ExportSnippet;

pub(crate) fn export_snippet<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ExportSnippet<'s>> {
    let (remaining, _) = tag("@@")(input)?;
    closer::expect(context, remaining, "@@")?;
    let (remaining, backend_name) = backend(context, remaining)?;
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &export_snippet_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, backend_contents) = opt(tuple((
        tag(":"),
        parser_with_context!(contents)(&parser_context),
    )))(remaining)?;
    let (remaining, _) = tag("@@")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        ExportSnippet {
            source: source.into(),
            backend: backend_name.into(),
            contents: backend_contents.map(|(_colon, backend_contents)| backend_contents.into()),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn backend<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, backend_name) =
        recognize(many1(verify(anychar, |c| c.is_alphanumeric() || *c == '-')))(input)?;

    Ok((remaining, backend_name))
}

fn contents<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, source) = recognize(verify(
        many_till(anychar, parser_with_context!(exit_matcher_parser)(context)),
        |(children, _exit_contents)| !children.is_empty(),
    ))(input)?;
    Ok((remaining, source))
}

fn export_snippet_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    tag("@@")(input)
}
