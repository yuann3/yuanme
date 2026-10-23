use std::borrow::Cow;

use nom::branch::alt;
use nom::bytes::complete::escaped;
use nom::bytes::complete::is_a;
use nom::bytes::complete::tag;
use nom::bytes::complete::take;
use nom::bytes::complete::take_till1;
use nom::bytes::complete::take_until;
use nom::character::complete::anychar;
use nom::combinator::all_consuming;
use nom::combinator::consumed;
use nom::combinator::eof;
use nom::combinator::flat_map;
use nom::combinator::map;
use nom::combinator::map_parser;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::rest;
use nom::combinator::verify;
use nom::multi::many1;
use nom::multi::many1_count;
use nom::multi::many_till;
use nom::sequence::tuple;
use nom::InputTake;

use super::object_parser::regular_link_description_set_object;
use super::org_source::BracketDepth;
use super::org_source::OrgSource;
use super::plain_link::parse_file_and_application;
use super::plain_link::protocol;
use super::util::confine_context;
use super::util::get_consumed;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
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
use crate::types::LinkType;
use crate::types::Object;
use crate::types::RegularLink;

pub(crate) fn regular_link<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, RegularLink<'s>> {
    alt((
        parser_with_context!(regular_link_without_description)(context),
        parser_with_context!(regular_link_with_description)(context),
    ))(input)
}

fn regular_link_without_description<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, RegularLink<'s>> {
    let (remaining, _opening_bracket) = tag("[[")(input)?;
    let (remaining, path) = pathreg(context, remaining)?;
    let (remaining, _closing_bracket) = tag("]]")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        RegularLink {
            source: source.into(),
            link_type: path.link_type,
            path: path.path,
            raw_link: path.raw_link,
            search_option: path.search_option,
            contents: None,
            post_blank: post_blank.map(Into::<&str>::into),
            children: Vec::new(),
            application: path.application,
        },
    ))
}

fn regular_link_with_description<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, RegularLink<'s>> {
    let (remaining, _opening_bracket) = tag("[[")(input)?;
    let (remaining, path) = pathreg(context, remaining)?;
    let (remaining, _closing_bracket) = tag("][")(remaining)?;
    let (remaining, (contents, description)) =
        consumed(parser_with_context!(description)(context))(remaining)?;
    let (remaining, _closing_bracket) = tag("]]")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        RegularLink {
            source: source.into(),
            link_type: path.link_type,
            path: path.path,
            raw_link: path.raw_link,
            search_option: path.search_option,
            contents: Some(Into::<&str>::into(contents)),
            post_blank: post_blank.map(Into::<&str>::into),
            children: description,
            application: path.application,
        },
    ))
}

#[derive(Debug)]
struct PathReg<'s> {
    link_type: LinkType<'s>,
    path: Cow<'s, str>,
    raw_link: Cow<'s, str>,
    search_option: Option<Cow<'s, str>>,
    application: Option<Cow<'s, str>>,
}

fn pathreg<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathReg<'s>> {
    let (remaining, path) = map_parser(
        escaped(take_till1(|c| matches!(c, '\\' | '[' | ']')), '\\', anychar),
        parser_with_context!(parse_path_reg)(context),
    )(input)?;
    Ok((remaining, path))
}

fn parse_path_reg<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathReg<'s>> {
    if let Some(replaced_link) = apply_link_templates(context, input) {
        let replaced_input = Into::<OrgSource<'_>>::into(replaced_link.as_str());
        let (_remaining, link) = alt((
            parser_with_context!(file_path_reg)(context),
            id_path_reg,
            custom_id_path_reg,
            code_ref_path_reg,
            parser_with_context!(protocol_path_reg)(context),
            fuzzy_path_reg,
        ))(replaced_input)
        .map_err(|_| nom::Err::Error(CustomError::Static("No pathreg match after replacement.")))?;
        let remaining = input.take(input.len());
        let link_type = match link.link_type {
            LinkType::Protocol(protocol) => LinkType::Protocol(protocol.into_owned().into()),
            LinkType::File => LinkType::File,
            LinkType::Id => LinkType::Id,
            LinkType::CustomId => LinkType::CustomId,
            LinkType::CodeRef => LinkType::CodeRef,
            LinkType::Fuzzy => LinkType::Fuzzy,
        };
        Ok((
            remaining,
            PathReg {
                link_type,
                path: link.path.into_owned().into(),
                raw_link: link.raw_link.into_owned().into(),
                search_option: link.search_option.map(|s| s.into_owned().into()),
                application: link.application.map(|s| s.into_owned().into()),
            },
        ))
    } else {
        alt((
            parser_with_context!(file_path_reg)(context),
            id_path_reg,
            custom_id_path_reg,
            code_ref_path_reg,
            parser_with_context!(protocol_path_reg)(context),
            fuzzy_path_reg,
        ))(input)
    }
}

enum ParserState {
    Normal,
    Percent,
}

fn apply_link_templates<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Option<String> {
    let (remaining, key) = opt(map(
        tuple((
            recognize(take_until::<_, _, nom::error::Error<_>>(":")),
            is_a(":"),
        )),
        |(key, _)| key,
    ))(input)
    .expect("opt ensures this cannot error.");
    let key = match key {
        Some(key) => key,
        None => {
            return None;
        }
    };
    let replacement_template = match context.get_global_settings().link_templates.get(key.into()) {
        Some(template) => template,
        None => {
            return None;
        }
    };

    let inject_value = Into::<&str>::into(remaining);
    let mut ret = String::with_capacity(replacement_template.len() + inject_value.len());
    let mut state = ParserState::Normal;
    let mut injected_value = false;
    for c in replacement_template.chars() {
        state = match (&state, c) {
            (ParserState::Normal, '%') => ParserState::Percent,
            (ParserState::Normal, _) => {
                ret.push(c);
                ParserState::Normal
            }
            (ParserState::Percent, 's') => {
                ret.push_str(inject_value);
                injected_value = true;
                ParserState::Normal
            }
            (ParserState::Percent, _) => {
                panic!("Unhandled percent value: {}", c)
            }
        };
    }
    // Handle lingering state
    if let ParserState::Percent = state {
        ret.push('%');
    }
    if !injected_value {
        ret.push_str(inject_value);
    }
    Some(ret)
}

fn file_path_reg<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathReg<'s>> {
    let path_reg_end = path_reg_end(input.get_parenthesis_depth(), true);
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &path_reg_end,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, (raw_link, (application, _, path, search_option))) = consumed(tuple((
        alt((
            map(
                tuple((
                    peek(tag("file")),
                    map_parser(
                        parser_with_context!(protocol)(&parser_context),
                        parse_file_and_application,
                    ),
                    tag(":"),
                )),
                |(_, application, _)| application,
            ),
            map(peek(tag(".")), |_| None),
            map(peek(tag("/")), |_| None),
        )),
        opt(flat_map(
            peek(map(verify(many1_count(tag("/")), |c| *c >= 3), |c| c - 1)),
            take,
        )),
        parser_with_context!(text_until_exit)(&parser_context),
        opt(map(tuple((tag("::"), rest)), |(_, search_option)| {
            search_option
        })),
    )))(input)?;

    Ok((
        remaining,
        PathReg {
            link_type: LinkType::File,
            path: path.into(),
            raw_link: raw_link.into(),
            search_option: search_option
                .map(Into::<&str>::into)
                .map(Into::<Cow<str>>::into),
            application: application
                .map(Into::<&str>::into)
                .map(Into::<Cow<str>>::into),
        },
    ))
}

fn id_path_reg<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, PathReg<'s>> {
    let (remaining, (raw_link, (_, path))) = consumed(tuple((tag("id:"), rest)))(input)?;
    Ok((
        remaining,
        PathReg {
            link_type: LinkType::Id,
            path: path.into(),
            raw_link: raw_link.into(),
            search_option: None,
            application: None,
        },
    ))
}

fn custom_id_path_reg<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, PathReg<'s>> {
    let (remaining, (raw_link, (_, path))) = consumed(tuple((tag("#"), rest)))(input)?;
    Ok((
        remaining,
        PathReg {
            link_type: LinkType::CustomId,
            path: path.into(),
            raw_link: raw_link.into(),
            search_option: None,
            application: None,
        },
    ))
}

fn code_ref_path_reg<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, PathReg<'s>> {
    let (remaining, (raw_link, (_, path, _))) = consumed(tuple((
        tag("("),
        recognize(many_till(anychar, peek(tuple((tag(")"), eof))))),
        tag(")"),
    )))(input)?;
    Ok((
        remaining,
        PathReg {
            link_type: LinkType::CodeRef,
            path: path.into(),
            raw_link: raw_link.into(),
            search_option: None,
            application: None,
        },
    ))
}

fn protocol_path_reg<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathReg<'s>> {
    let (remaining, (raw_link, (protocol, _, path))) = consumed(tuple((
        parser_with_context!(protocol)(context),
        tag(":"),
        rest,
    )))(input)?;
    Ok((
        remaining,
        PathReg {
            link_type: LinkType::Protocol(protocol.into()),
            path: path.into(),
            raw_link: raw_link.into(),
            search_option: None,
            application: None,
        },
    ))
}

fn fuzzy_path_reg<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, PathReg<'s>> {
    let (remaining, body) = rest(input)?;
    Ok((
        remaining,
        PathReg {
            link_type: LinkType::Fuzzy,
            path: body.into(),
            raw_link: body.into(),
            search_option: None,
            application: None,
        },
    ))
}

fn description<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Object<'s>>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &description_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let initial_context = ContextElement::document_context();
    let initial_context = Context::new(context.get_global_settings(), List::new(&initial_context));

    let (remaining, children) = map_parser(
        verify(
            parser_with_context!(text_until_exit)(&parser_context),
            |text| text.len() > 0,
        ),
        confine_context(|i| {
            all_consuming(many1(parser_with_context!(
                regular_link_description_set_object
            )(&initial_context)))(i)
        }),
    )(input)?;

    Ok((remaining, children))
}

fn description_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    tag("]]")(input)
}

fn path_reg_end(
    starting_parenthesis_depth: BracketDepth,
    enable_search_option: bool,
) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| {
        impl_path_reg_end(
            context,
            input,
            starting_parenthesis_depth,
            enable_search_option,
        )
    }
}

fn impl_path_reg_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    _starting_parenthesis_depth: BracketDepth,
    enable_search_option: bool,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    if enable_search_option {
        let search_option = peek(tag("::"))(input);
        if search_option.is_ok() {
            return search_option;
        }
    }

    Err(nom::Err::Error(CustomError::Static("No path reg end")))
}
