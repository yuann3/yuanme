use nom::branch::alt;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::character::complete::space0;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::paragraph::paragraph;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use super::util::start_of_line;
use super::OrgSource;
use crate::context::bind_context;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::macros::element;
use crate::types::Object;
use crate::types::Paragraph;
use crate::types::PlainText;

pub(crate) fn bullshitium<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Paragraph<'s>> {
    alt((
        bind_context!(broken_end, context),
        bind_context!(broken_dynamic_block, context),
    ))(input)
}

pub(crate) fn detect_bullshitium<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    element!(detect_broken_end, context, input);
    element!(detect_broken_dynamic_block, context, input);
    Err(nom::Err::Error(CustomError::Static("No bullshitium.")))
}

pub(crate) fn broken_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Paragraph<'s>> {
    start_of_line(input)?;
    let (remaining, _) = space0(input)?;
    let (remaining, _) = tag_no_case(":end:")(remaining)?;
    let (lead_in_remaining, _) = tuple((space0, org_line_ending))(remaining)?;
    if let Ok((remaining, mut paragraph)) =
        paragraph(std::iter::empty(), lead_in_remaining, context, input)
    {
        take_lead_in(input, lead_in_remaining, &mut paragraph);
        Ok((remaining, paragraph))
    } else {
        let (remaining, post_blank) =
            maybe_consume_trailing_whitespace_if_not_exiting(context, lead_in_remaining)?;

        let body = Into::<&str>::into(input.get_until(lead_in_remaining));

        Ok((
            remaining,
            Paragraph::of_text(
                input.get_until(remaining).into(),
                body,
                if !body.is_empty() { Some(body) } else { None },
                post_blank.map(Into::<&str>::into),
            ),
        ))
    }
}

/// Makes the line that starts a bullshitium part of the paragraph after it.
fn take_lead_in<'s>(
    input: OrgSource<'s>,
    lead_in_remaining: OrgSource<'s>,
    paragraph: &mut Paragraph<'s>,
) {
    match paragraph.children.first_mut() {
        Some(Object::PlainText(plain_text)) => {
            plain_text.source = input.get_until_end_of_str(plain_text.source).into();
            paragraph.contents = Some(input.get_until_end_of_str(plain_text.source).into());
        }
        // Upstream panicked here. Emacs reads the line as plain text before
        // the paragraph's first object, such as a timestamp or a link.
        Some(_) => {
            let lead_in = PlainText {
                source: input.get_until(lead_in_remaining).into(),
            };
            paragraph.children.insert(0, Object::PlainText(lead_in));
            let contents = paragraph
                .contents
                .expect("a paragraph with children has contents");
            paragraph.contents = Some(input.get_until_end_of_str(contents).into());
        }
        None => unreachable!("Paragraph must have children."),
    }
}

pub(crate) fn detect_broken_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    start_of_line(input)?;
    let (remaining, _) = space0(input)?;
    let (remaining, _) = tag_no_case(":end:")(remaining)?;
    let (_remaining, _) = tuple((space0, org_line_ending))(remaining)?;
    Ok((input, ()))
}

pub(crate) fn broken_dynamic_block<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Paragraph<'s>> {
    start_of_line(input)?;
    let (remaining, _) = space0(input)?;
    let (remaining, _) = tag_no_case("#+BEGIN:")(remaining)?;
    let (lead_in_remaining, _) = many_till(anychar, org_line_ending)(remaining)?;
    if let Ok((remaining, mut paragraph)) =
        paragraph(std::iter::empty(), lead_in_remaining, context, input)
    {
        take_lead_in(input, lead_in_remaining, &mut paragraph);
        Ok((remaining, paragraph))
    } else {
        let (remaining, post_blank) =
            maybe_consume_trailing_whitespace_if_not_exiting(context, lead_in_remaining)?;

        let body = Into::<&str>::into(input.get_until(lead_in_remaining));

        Ok((
            remaining,
            Paragraph::of_text(
                input.get_until(remaining).into(),
                body,
                if !body.is_empty() { Some(body) } else { None },
                post_blank.map(Into::<&str>::into),
            ),
        ))
    }
}

pub(crate) fn detect_broken_dynamic_block<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    start_of_line(input)?;
    let (remaining, _) = space0(input)?;
    let (_remaining, _) = tag_no_case("#+BEGIN:")(remaining)?;
    Ok((input, ()))
}
