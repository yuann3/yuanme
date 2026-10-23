use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_while;
use nom::character::complete::anychar;
use nom::character::complete::one_of;
use nom::combinator::consumed;
use nom::combinator::map;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::closer;
use super::object_parser::standard_set_object;
use super::org_source::BracketDepth;
use super::org_source::OrgSource;
use super::util::exit_matcher_parser;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use super::util::preceded_by_whitespace;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::Matcher;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::types::Object;
use crate::types::PlainText;
use crate::types::Subscript;
use crate::types::Superscript;

pub(crate) fn detect_subscript_or_superscript<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    // This does not have to detect all valid subscript/superscript but all that it detects must be valid.
    let (remaining, _) = one_of("_^")(input)?;
    pre(input)?;
    if tag::<_, _, CustomError>("*")(remaining).is_ok() {
        return Ok((input, ()));
    }
    let (remaining, _) = opt(one_of("+-"))(remaining)?;
    let (_remaining, _) = verify(anychar, |c| c.is_alphanumeric())(remaining)?;
    Ok((input, ()))
}

pub(crate) fn subscript<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Subscript<'s>> {
    let (remaining, _) = tag("_")(input)?;
    pre(input)?;
    let (remaining, body) = script_body(context, remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    let (use_brackets, contents, body) = match body {
        ScriptBody::Braceless(text) => (
            false,
            text,
            vec![Object::PlainText(PlainText { source: text })],
        ),
        ScriptBody::WithBraces(contents, body) => (true, contents, body),
    };

    Ok((
        remaining,
        Subscript {
            source: source.into(),
            use_brackets,
            contents,
            post_blank: post_blank.map(Into::<&str>::into),
            children: body,
        },
    ))
}

pub(crate) fn superscript<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Superscript<'s>> {
    // We check for the circumflex first before checking the pre-character as a minor optimization to avoid walking up the context tree to find the document root unnecessarily.
    let (remaining, _) = tag("^")(input)?;
    pre(input)?;
    let (remaining, body) = script_body(context, remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    let (use_brackets, contents, body) = match body {
        ScriptBody::Braceless(text) => (
            false,
            text,
            vec![Object::PlainText(PlainText { source: text })],
        ),
        ScriptBody::WithBraces(contents, body) => (true, contents, body),
    };

    Ok((
        remaining,
        Superscript {
            source: source.into(),
            use_brackets,
            contents,
            post_blank: post_blank.map(Into::<&str>::into),
            children: body,
        },
    ))
}

fn pre<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    not(preceded_by_whitespace(true))(input)?;
    Ok((input, ()))
}

#[derive(Debug)]
enum ScriptBody<'s> {
    Braceless(&'s str),
    WithBraces(&'s str, Vec<Object<'s>>),
}

fn script_body<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ScriptBody<'s>> {
    alt((
        map(parser_with_context!(script_asterisk)(context), |body| {
            ScriptBody::Braceless(body.into())
        }),
        map(parser_with_context!(script_alphanum)(context), |body| {
            ScriptBody::Braceless(body.into())
        }),
        map(
            parser_with_context!(script_with_braces)(context),
            |(contents, body)| ScriptBody::WithBraces(Into::<&str>::into(contents), body),
        ),
        map(
            parser_with_context!(script_with_parenthesis)(context),
            |body| ScriptBody::Braceless(body.into()),
        ),
    ))(input)
}

fn script_asterisk<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    tag("*")(input)
}

fn script_alphanum<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _sign) = opt(recognize(one_of("+-")))(input)?;
    let (remaining, _script) =
        many_till(script_alphanum_character, end_script_alphanum_character)(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn script_alphanum_character<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(verify(anychar, |c| {
        c.is_alphanumeric() || r",.\".contains(*c)
    }))(input)
}

fn end_script_alphanum_character<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, final_char) = recognize(verify(anychar, |c| c.is_alphanumeric()))(input)?;
    peek(tuple((
        take_while(|c| r",.\".contains(c)),
        not(script_alphanum_character),
    )))(remaining)?;
    Ok((remaining, final_char))
}

fn script_with_braces<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (OrgSource<'s>, Vec<Object<'s>>)> {
    let (remaining, _) = tag("{")(input)?;
    closer::expect_balanced(context, remaining, b'{', b'}')?;
    let exit_with_depth = script_with_braces_end(remaining.get_brace_depth());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &exit_with_depth,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, (contents, (children, _exit_contents))) = consumed(many_till(
        parser_with_context!(standard_set_object)(&parser_context),
        parser_with_context!(exit_matcher_parser)(&parser_context),
    ))(remaining)?;

    let (remaining, _) = tag("}")(remaining)?;
    Ok((remaining, (contents, children)))
}

fn script_with_braces_end(starting_brace_depth: BracketDepth) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| {
        _script_with_braces_end(context, input, starting_brace_depth)
    }
}

fn _script_with_braces_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_brace_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_brace_depth() - starting_brace_depth;
    if current_depth > 0 {
        // Its impossible for the next character to end the subscript or superscript if we're any amount of braces deep
        return Err(nom::Err::Error(CustomError::Static(
            "Not a valid end for subscript or superscript.",
        )));
    }
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing brace should end the subscript or superscript.
        unreachable!("Exceeded subscript or superscript brace depth.")
    }
    tag("}")(input)
}

fn script_with_parenthesis<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _) = tag("(")(input)?;
    closer::expect_balanced(context, remaining, b'(', b')')?;
    let exit_with_depth = script_with_parenthesis_end(remaining.get_parenthesis_depth());

    let (remaining, _) = many_till(
        anychar,
        alt((
            peek(exit_with_depth),
            parser_with_context!(exit_matcher_parser)(context),
        )),
    )(remaining)?;

    let (remaining, _) = tag(")")(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn script_with_parenthesis_end(starting_parenthesis_depth: BracketDepth) -> impl Matcher {
    move |input: OrgSource<'_>| _script_with_parenthesis_end(input, starting_parenthesis_depth)
}

fn _script_with_parenthesis_end<'s>(
    input: OrgSource<'s>,
    starting_parenthesis_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_parenthesis_depth() - starting_parenthesis_depth;
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing bracket should end the citation.
        unreachable!("Exceeded citation key suffix bracket depth.")
    }
    if current_depth == 0 {
        let close_parenthesis = tag::<_, _, CustomError>(")")(input);
        if close_parenthesis.is_ok() {
            return close_parenthesis;
        }
    }
    Err(nom::Err::Error(CustomError::Static(
        "No script parenthesis end.",
    )))
}
