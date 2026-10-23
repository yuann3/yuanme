use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::satisfy;
use nom::combinator::cond;
use nom::combinator::eof;
use nom::combinator::map;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::EntityDefinition;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::types::Entity;

pub(crate) fn entity<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Entity<'s>> {
    let (remaining, _) = tag("\\")(input)?;
    let (remaining, (entity_definition, entity_name, use_brackets)) = name(context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;

    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Entity {
            source: source.into(),
            name: entity_name.into(),
            latex_math_mode: entity_definition.latex_math_mode,
            latex: entity_definition.latex,
            html: entity_definition.html,
            ascii: entity_definition.ascii,
            utf8: entity_definition.utf8,
            use_brackets,
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn name<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (&'g EntityDefinition<'s>, OrgSource<'s>, bool)> {
    let text: &str = input.into();
    let first = text.as_bytes().first();
    for entity in context.get_global_settings().entities {
        // A failed `tag`, or even a memcmp, for each of Org's ~400 entities
        // made every backslash cost a microsecond (#15).
        if entity.name.as_bytes().first() != first || !text.starts_with(entity.name) {
            continue;
        }
        let result = tuple((
            tag::<_, _, CustomError>(entity.name),
            cond(
                !entity.name.ends_with(' '),
                alt((
                    map(tag("{}"), |_| true),
                    map(peek(recognize(entity_end)), |_| false),
                )),
            ),
        ))(input);
        if let Ok((remaining, (ent, use_brackets))) = result {
            return Ok((remaining, (entity, ent, use_brackets.unwrap_or(false))));
        }
    }

    Err(nom::Err::Error(CustomError::Static("NoEntity")))
}

fn entity_end<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    let (remaining, _) = alt((eof, recognize(satisfy(|c| !c.is_alphabetic()))))(input)?;

    Ok((remaining, ()))
}
