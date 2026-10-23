use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::multispace1;
use nom::character::complete::one_of;
use nom::character::complete::space1;
use nom::combinator::all_consuming;
use nom::combinator::consumed;
use nom::combinator::map;
use nom::combinator::map_parser;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many1;
use nom::multi::many_till;
use nom::sequence::terminated;

use super::object_parser::standard_set_object;
use super::org_source::OrgSource;
use super::radio_link::RematchObject;
use super::util::confine_context;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use super::util::start_of_line;
use super::util::text_until_exit;
use crate::context::parser_with_context;
use crate::context::Context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::List;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::radio_link::rematch_target;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::preceded_by_whitespace;
use crate::types::Bold;
use crate::types::Code;
use crate::types::Italic;
use crate::types::Object;
use crate::types::StrikeThrough;
use crate::types::Underline;
use crate::types::Verbatim;

pub(crate) fn text_markup<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    alt((
        map(parser_with_context!(bold)(context), Object::Bold),
        map(parser_with_context!(italic)(context), Object::Italic),
        map(parser_with_context!(underline)(context), Object::Underline),
        map(
            parser_with_context!(strike_through)(context),
            Object::StrikeThrough,
        ),
        map(parser_with_context!(verbatim)(context), Object::Verbatim),
        map(parser_with_context!(code)(context), Object::Code),
    ))(input)
}

fn bold<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Bold<'s>> {
    let (remaining, (contents, children, post_blank)) = text_markup_object("*")(context, input)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Bold {
            source: source.into(),
            contents: contents.into(),
            post_blank: post_blank.map(Into::<&str>::into),
            children,
        },
    ))
}

fn italic<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Italic<'s>> {
    let (remaining, (contents, children, post_blank)) = text_markup_object("/")(context, input)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Italic {
            source: source.into(),
            contents: contents.into(),
            post_blank: post_blank.map(Into::<&str>::into),
            children,
        },
    ))
}

fn underline<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Underline<'s>> {
    let (remaining, (contents, children, post_blank)) = text_markup_object("_")(context, input)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Underline {
            source: source.into(),
            contents: contents.into(),
            post_blank: post_blank.map(Into::<&str>::into),
            children,
        },
    ))
}

fn strike_through<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, StrikeThrough<'s>> {
    let (remaining, (contents, children, post_blank)) = text_markup_object("+")(context, input)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        StrikeThrough {
            source: source.into(),
            contents: contents.into(),
            post_blank: post_blank.map(Into::<&str>::into),
            children,
        },
    ))
}

fn verbatim<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Verbatim<'s>> {
    let (remaining, (contents, post_blank)) = text_markup_string("=")(context, input)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Verbatim {
            source: source.into(),
            contents: contents.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn code<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Code<'s>> {
    let (remaining, (contents, post_blank)) = text_markup_string("~")(context, input)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Code {
            source: source.into(),
            contents: contents.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

/// The opening marker, the contents and the closing marker.
type Markup<'s> = (OrgSource<'s>, Vec<Object<'s>>, Option<OrgSource<'s>>);

fn text_markup_object(
    marker_symbol: &str,
) -> impl for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, Markup<'s>>
       + '_ {
    move |context, input: OrgSource<'_>| _text_markup_object(context, input, marker_symbol)
}

fn _text_markup_object<'b, 'g, 'r, 's, 'c>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    marker_symbol: &'c str,
) -> Res<OrgSource<'s>, Markup<'s>> {
    let (remaining, _) = pre(context, input)?;
    let (remaining, open) = tag(marker_symbol)(remaining)?;
    let (remaining, _peek_not_whitespace) =
        peek(verify(anychar, |c| !c.is_whitespace() && *c != '\u{200B}'))(remaining)?;
    let text_markup_end_specialized = text_markup_end(open.into(), remaining.get_byte_offset());
    let contexts = [ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &text_markup_end_specialized,
    })];
    let parser_context = context.with_additional_node(&contexts[0]);

    let initial_context = ContextElement::document_context();
    let initial_context = Context::new(context.get_global_settings(), List::new(&initial_context));

    let (remaining, (contents, children)) = consumed(map_parser(
        verify(
            parser_with_context!(text_until_exit)(&parser_context),
            |text| text.len() > 0,
        ),
        confine_context(|i| {
            all_consuming(many1(parser_with_context!(standard_set_object)(
                &initial_context,
            )))(i)
        }),
    ))(remaining)?;

    {
        if exit_matcher_parser(context, remaining).is_ok() {
            return Err(nom::Err::Error(CustomError::Static(
                "Parent exit matcher is triggering.",
            )));
        }
    }

    let (remaining, _close) = text_markup_end_specialized(context, remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    Ok((remaining, (contents, children, post_blank)))
}

fn text_markup_string(
    marker_symbol: &str,
) -> impl for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, (OrgSource<'s>, Option<OrgSource<'s>>)>
       + '_ {
    move |context, input: OrgSource<'_>| _text_markup_string(context, input, marker_symbol)
}

fn _text_markup_string<'b, 'g, 'r, 's, 'c>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    marker_symbol: &'c str,
) -> Res<OrgSource<'s>, (OrgSource<'s>, Option<OrgSource<'s>>)> {
    let (remaining, _) = pre(context, input)?;
    let (remaining, open) = tag(marker_symbol)(remaining)?;
    let (remaining, _peek_not_whitespace) =
        peek(verify(anychar, |c| !c.is_whitespace() && *c != '\u{200B}'))(remaining)?;
    let text_markup_end_specialized = text_markup_end(open.into(), remaining.get_byte_offset());
    let contexts = [ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &text_markup_end_specialized,
    })];
    let parser_context = context.with_additional_node(&contexts[0]);

    let (remaining, contents) = recognize(verify(
        many_till(
            anychar,
            parser_with_context!(exit_matcher_parser)(&parser_context),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    ))(remaining)?;

    {
        if exit_matcher_parser(context, remaining).is_ok() {
            return Err(nom::Err::Error(CustomError::Static(
                "Parent exit matcher is triggering.",
            )));
        }
    }

    let (remaining, _close) = text_markup_end_specialized(context, remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    Ok((remaining, (contents, post_blank)))
}

fn pre<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    if start_of_line(input).is_ok() {
        return Ok((input, ()));
    }
    if preceded_by_whitespace(true)(input).is_ok() {
        return Ok((input, ()));
    }
    let preceding_character = input.get_preceding_character();
    match preceding_character {
        // If None, we are at the start of the file which is technically the beginning of a line.
        Some('-') | Some('(') | Some('{') | Some('\'') | Some('"') => {}
        Some(_) => {
            return Err(nom::Err::Error(CustomError::Static(
                "Not a valid pre character for text markup.",
            )));
        }
        None => unreachable!(), // None is for start of file, which should already be handled by the start_of_line matcher above.
    };
    Ok((input, ()))
}

fn post<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let (remaining, _) =
        alt((recognize(one_of(" \r\n\t-.,;:!?')}[\"\\")), org_line_ending))(input)?;
    Ok((remaining, ()))
}

fn text_markup_end(marker_symbol: &str, contents_start_offset: usize) -> impl ContextMatcher + '_ {
    move |context, input: OrgSource<'_>| {
        _text_markup_end(context, input, marker_symbol, contents_start_offset)
    }
}

fn _text_markup_end<'b, 'g, 'r, 's, 'c>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    marker_symbol: &'c str,
    contents_start_offset: usize,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    if input.get_byte_offset() == contents_start_offset {
        return Err(nom::Err::Error(CustomError::Static(
            "Text markup cannot be empty",
        )));
    }
    not(preceded_by_whitespace(false))(input)?;
    let (remaining, _marker) = terminated(
        tag(marker_symbol),
        peek(parser_with_context!(post)(context)),
    )(input)?;
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

impl<'x> RematchObject<'x> for Bold<'x> {
    fn rematch_object<'b, 'g, 'r, 's>(
        &'x self,
        _context: RefContext<'b, 'g, 'r, 's>,
        input: OrgSource<'s>,
    ) -> Res<OrgSource<'s>, Object<'s>> {
        let (remaining, (contents, children, post_blank)) =
            _rematch_text_markup_object(_context, input, "*", &self.children)?;
        let source = get_consumed(input, remaining);
        Ok((
            remaining,
            Object::Bold(Bold {
                source: source.into(),
                contents: contents.into(),
                post_blank: post_blank.map(Into::<&str>::into),
                children,
            }),
        ))
    }
}

impl<'x> RematchObject<'x> for Italic<'x> {
    fn rematch_object<'b, 'g, 'r, 's>(
        &'x self,
        _context: RefContext<'b, 'g, 'r, 's>,
        input: OrgSource<'s>,
    ) -> Res<OrgSource<'s>, Object<'s>> {
        let (remaining, (contents, children, post_blank)) =
            _rematch_text_markup_object(_context, input, "/", &self.children)?;
        let source = get_consumed(input, remaining);
        Ok((
            remaining,
            Object::Italic(Italic {
                source: source.into(),
                contents: contents.into(),
                post_blank: post_blank.map(Into::<&str>::into),
                children,
            }),
        ))
    }
}

impl<'x> RematchObject<'x> for Underline<'x> {
    fn rematch_object<'b, 'g, 'r, 's>(
        &'x self,
        _context: RefContext<'b, 'g, 'r, 's>,
        input: OrgSource<'s>,
    ) -> Res<OrgSource<'s>, Object<'s>> {
        let (remaining, (contents, children, post_blank)) =
            _rematch_text_markup_object(_context, input, "_", &self.children)?;
        let source = get_consumed(input, remaining);
        Ok((
            remaining,
            Object::Underline(Underline {
                source: source.into(),
                contents: contents.into(),
                post_blank: post_blank.map(Into::<&str>::into),
                children,
            }),
        ))
    }
}

impl<'x> RematchObject<'x> for StrikeThrough<'x> {
    fn rematch_object<'b, 'g, 'r, 's>(
        &'x self,
        _context: RefContext<'b, 'g, 'r, 's>,
        input: OrgSource<'s>,
    ) -> Res<OrgSource<'s>, Object<'s>> {
        let (remaining, (contents, children, post_blank)) =
            _rematch_text_markup_object(_context, input, "+", &self.children)?;
        let source = get_consumed(input, remaining);
        Ok((
            remaining,
            Object::StrikeThrough(StrikeThrough {
                source: source.into(),
                contents: contents.into(),
                post_blank: post_blank.map(Into::<&str>::into),
                children,
            }),
        ))
    }
}

fn _rematch_text_markup_object<'b, 'g, 'r, 's, 'x>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    marker_symbol: &'static str,
    original_match_children: &'x Vec<Object<'x>>,
) -> Res<OrgSource<'s>, Markup<'s>> {
    let (remaining, _) = pre(context, input)?;
    let (remaining, open) = tag(marker_symbol)(remaining)?;
    let (remaining, _peek_not_whitespace) = peek(not(multispace1))(remaining)?;
    let text_markup_end_specialized = text_markup_end(open.into(), remaining.get_byte_offset());
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &text_markup_end_specialized,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let contents_begin = remaining;
    let (remaining, children) =
        // TODO: This doesn't really check the exit matcher between each object. I think it may be possible to construct an org document that parses incorrectly with the current code.
        rematch_target(&parser_context, original_match_children, remaining)?;

    {
        if exit_matcher_parser(context, remaining).is_ok() {
            return Err(nom::Err::Error(CustomError::Static(
                "Parent exit matcher is triggering.",
            )));
        }
    }
    let contents_end = remaining;
    let contents = contents_begin.get_until(contents_end);

    let (remaining, _close) = text_markup_end_specialized(context, remaining)?;
    let (remaining, post_blank) = opt(space1)(remaining)?;
    Ok((remaining, (contents, children, post_blank)))
}
