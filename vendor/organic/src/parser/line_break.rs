use nom::bytes::complete::tag;
use nom::character::complete::line_ending;
use nom::character::complete::one_of;
use nom::combinator::recognize;
use nom::multi::many0;

use super::org_source::OrgSource;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::types::LineBreak;

pub(crate) fn line_break<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, LineBreak<'s>> {
    let (remaining, _) = pre(context, input)?;
    let (remaining, _) = tag(r"\\")(remaining)?;
    let (remaining, _) = recognize(many0(one_of(" \t")))(remaining)?;
    let (remaining, _) = line_ending(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        LineBreak {
            source: source.into(),
        },
    ))
}

fn pre<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let preceding_character = input.get_preceding_character();
    match preceding_character {
        // If None, we are at the start of the file
        None | Some('\\') => {
            return Err(nom::Err::Error(CustomError::Static(
                "Not a valid pre character for line break.",
            )));
        }
        _ => {}
    };

    let current_line = input.text_since_line_break();
    let is_non_empty_line = current_line.chars().any(|c| !c.is_whitespace());
    if !is_non_empty_line {
        return Err(nom::Err::Error(CustomError::Static(
            "Not a valid pre line for line break.",
        )));
    }

    Ok((input, ()))
}
