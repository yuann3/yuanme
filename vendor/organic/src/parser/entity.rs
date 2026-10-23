use std::cell::RefCell;

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
    let Some(&first) = text.as_bytes().first() else {
        return Err(nom::Err::Error(CustomError::Static("NoEntity")));
    };
    let entities = context.get_global_settings().entities;
    // Even comparing the first byte of each of Org's ~400 entities made a
    // run of backslashes cost half a microsecond each (#15), so only those
    // that start with the right byte are tried, in their order.
    INDEX.with_borrow_mut(|index| {
        let key = (entities.as_ptr() as usize, entities.len());
        let index = match index {
            Some(index) if index.entities == key => index,
            _ => index.insert(Index {
                entities: key,
                by_first_byte: by_first_byte(entities),
            }),
        };
        for entity in index.by_first_byte[usize::from(first)]
            .iter()
            .map(|&i| &entities[i])
        {
            if !text.starts_with(entity.name) {
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
    })
}

/// For one list of entities, by first byte, where those that start with it
/// are in the list.
struct Index {
    entities: (usize, usize),
    by_first_byte: Vec<Vec<usize>>,
}

thread_local! {
    static INDEX: RefCell<Option<Index>> = const { RefCell::new(None) };
}

/// Forgets the index. Another list may sit at an old one's address.
pub(crate) fn reset() {
    INDEX.set(None);
}

fn by_first_byte(entities: &[EntityDefinition<'_>]) -> Vec<Vec<usize>> {
    let mut index = vec![Vec::new(); 256];
    for (i, entity) in entities.iter().enumerate() {
        if let Some(&first) = entity.name.as_bytes().first() {
            index[usize::from(first)].push(i);
        }
    }
    index
}

fn entity_end<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    let (remaining, _) = alt((eof, recognize(satisfy(|c| !c.is_alphabetic()))))(input)?;

    Ok((remaining, ()))
}
