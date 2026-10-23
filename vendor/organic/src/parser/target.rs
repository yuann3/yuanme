use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::one_of;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;

use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use super::util::text_until_exit;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::types::Target;

pub(crate) fn target<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Target<'s>> {
    let (remaining, _) = tag("<<")(input)?;
    let (remaining, _) = peek(verify(anychar, |c| {
        !c.is_whitespace() && !"<>\n".contains(*c)
    }))(remaining)?;

    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &target_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, body) = text_until_exit(&parser_context, remaining)?;

    let preceding_character = remaining
        .get_preceding_character()
        .expect("We cannot be at the start of the file because we are inside a target.");
    if preceding_character.is_whitespace() {
        return Err(nom::Err::Error(CustomError::Static(
            "Targets cannot end with whitespace.",
        )));
    }
    let (remaining, _) = tag(">>")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Target {
            source: source.into(),
            value: body.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn target_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(one_of("<>\n"))(input)
}
