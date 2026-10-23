use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::alpha1;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::none_of;
use nom::character::complete::one_of;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::closer;
use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::types::LatexFragment;

pub(crate) fn latex_fragment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, LatexFragment<'s>> {
    let (remaining, _) = alt((
        parser_with_context!(raw_latex_fragment)(context),
        parser_with_context!(escaped_parenthesis_fragment)(context),
        parser_with_context!(escaped_bracket_fragment)(context),
        parser_with_context!(double_dollar_fragment)(context),
        parser_with_context!(dollar_char_fragment)(context),
        parser_with_context!(bordered_dollar_fragment)(context),
    ))(input)?;
    let value = get_consumed(input, remaining);
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        LatexFragment {
            source: source.into(),
            value: value.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn raw_latex_fragment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tag("\\")(input)?;
    let (remaining, _) = name(context, remaining)?;
    let (remaining, _) = many0(parser_with_context!(brackets)(context))(remaining)?;

    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn name<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alpha1(input)
}

fn brackets<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, body) = alt((
        recognize(tuple((
            tag("["),
            many_till(
                anychar,
                peek(alt((
                    parser_with_context!(exit_matcher_parser)(context),
                    alt((recognize(one_of("{}[]")), line_ending)),
                ))),
            ),
            tag("]"),
        ))),
        recognize(tuple((
            tag("{"),
            many_till(
                anychar,
                peek(alt((
                    parser_with_context!(exit_matcher_parser)(context),
                    alt((recognize(one_of("{}")), line_ending)),
                ))),
            ),
            tag("}"),
        ))),
    ))(input)?;
    Ok((remaining, body))
}

fn escaped_parenthesis_fragment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tag("\\(")(input)?;
    closer::expect(context, remaining, "\\)")?;
    let (remaining, _) = recognize(many_till(
        anychar,
        peek(alt((
            parser_with_context!(exit_matcher_parser)(context),
            tag("\\)"),
        ))),
    ))(remaining)?;
    let (remaining, _) = tag("\\)")(remaining)?;

    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn escaped_bracket_fragment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tag("\\[")(input)?;
    closer::expect(context, remaining, "\\]")?;
    let (remaining, _) = recognize(many_till(
        anychar,
        peek(alt((
            parser_with_context!(exit_matcher_parser)(context),
            tag("\\]"),
        ))),
    ))(remaining)?;
    let (remaining, _) = tag("\\]")(remaining)?;

    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn double_dollar_fragment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    // TODO: The documentation on the dollar sign versions is incomplete. Test to figure out what the real requirements are. For example, can this span more than 3 lines and can this contain a single $ since its terminated by $$?
    let (remaining, _) = tag("$$")(input)?;
    closer::expect(context, remaining, "$$")?;
    let (remaining, _) = recognize(many_till(
        anychar,
        peek(alt((
            parser_with_context!(exit_matcher_parser)(context),
            tag("$"),
        ))),
    ))(remaining)?;
    let (remaining, _) = tag("$$")(remaining)?;

    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn dollar_char_fragment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (_, _) = pre(context, input)?;
    let (remaining, _) = tag("$")(input)?;
    let (remaining, _) = verify(none_of(".,?;\""), |c| !c.is_whitespace())(remaining)?;
    let (remaining, _) = tag("$")(remaining)?;
    let (_, _) = peek(parser_with_context!(post)(context))(remaining)?;

    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn pre<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let preceding_character = input.get_preceding_character();
    if let Some('$') = preceding_character {
        return Err(nom::Err::Error(CustomError::Static(
            "Not a valid pre character for dollar char fragment.",
        )));
    }
    Ok((input, ()))
}

fn post<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    // TODO: What about eof? Test to find out.

    // TODO: Figure out which punctuation characters should be included.
    let (remaining, _) = alt((recognize(one_of(" \t-.,;:!?'\"")), line_ending))(input)?;
    Ok((remaining, ()))
}

fn bordered_dollar_fragment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (_, _) = pre(context, input)?;
    let (remaining, _) = tag("$")(input)?;
    // TODO: I'm assuming I should be peeking at the borders but the documentation is not clear. Test to figure out.
    let (_, _) = peek(parser_with_context!(open_border)(context))(remaining)?;

    let (remaining, _) = recognize(many_till(
        anychar,
        peek(alt((
            parser_with_context!(exit_matcher_parser)(context),
            tag("$"),
        ))),
    ))(remaining)?;

    let (_, _) = peek(parser_with_context!(close_border)(context))(remaining)?;
    let (remaining, _) = tag("$")(remaining)?;
    let (_, _) = peek(parser_with_context!(post)(context))(remaining)?;

    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn open_border<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(verify(none_of(".,;$"), |c| !c.is_whitespace()))(input)
}

fn close_border<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let preceding_character = input.get_preceding_character();
    match preceding_character {
        Some(c) if !c.is_whitespace() && !".,;$".contains(c) => Ok((input, ())),
        _ => Err(nom::Err::Error(CustomError::Static(
            "Not a valid pre character for dollar char fragment.",
        ))),
    }
}
