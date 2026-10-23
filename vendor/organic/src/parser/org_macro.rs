use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::space0;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::separated_list0;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::types::OrgMacro;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn org_macro<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgMacro<'s>> {
    let (remaining, _) = tag("{{{")(input)?;
    let (remaining, macro_name) = org_macro_name(context, remaining)?;
    let (remaining, macro_args) = opt(parser_with_context!(org_macro_args)(context))(remaining)?;
    let (remaining, _) = tag("}}}")(remaining)?;
    let macro_value = get_consumed(input, remaining);
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;

    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        OrgMacro {
            source: source.into(),
            key: macro_name.into(),
            args: macro_args
                .unwrap_or_else(|| Vec::with_capacity(0))
                .into_iter()
                .map(|arg| arg.into())
                .collect(),
            value: Into::<&str>::into(macro_value),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn org_macro_name<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = verify(anychar, |c| c.is_alphabetic())(input)?;
    let (remaining, _) = many0(verify(anychar, |c| {
        c.is_alphanumeric() || *c == '-' || *c == '_'
    }))(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn org_macro_args<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<OrgSource<'s>>> {
    let (remaining, _) = tag("(")(input)?;
    let (remaining, args) =
        separated_list0(tag(","), parser_with_context!(org_macro_arg)(context))(remaining)?;
    let (remaining, _) = tuple((space0, tag(")")))(remaining)?;

    Ok((remaining, args))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn org_macro_arg<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let mut remaining = input;
    let mut escaping: bool = false;
    loop {
        not(parser_with_context!(exit_matcher_parser)(context))(remaining)?;
        not(peek(tag("}}}")))(remaining)?;
        if peek(tuple((space0::<_, CustomError>, tag(")"))))(remaining).is_ok() {
            break;
        }

        let (new_remaining, next_char) = anychar(remaining)?;
        if escaping {
            remaining = new_remaining;
            escaping = false;
            continue;
        }
        if next_char == '\\' {
            escaping = true;
            if peek(tag::<_, _, CustomError>(")"))(new_remaining).is_ok() {
                // Special case for backslash at the end of a macro
                remaining = new_remaining;
                break;
            }
        }
        if next_char == ',' || next_char == ')' {
            break;
        }
        remaining = new_remaining;
    }
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}
