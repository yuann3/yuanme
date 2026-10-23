use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::bytes::complete::take;
use nom::bytes::complete::take_until;
use nom::character::complete::anychar;
use nom::character::complete::none_of;
use nom::character::complete::one_of;
use nom::combinator::all_consuming;
use nom::combinator::consumed;
use nom::combinator::eof;
use nom::combinator::flat_map;
use nom::combinator::map;
use nom::combinator::map_parser;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::rest;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::many1;
use nom::multi::many1_count;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::org_source::BracketDepth;
use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::Matcher;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::WORD_CONSTITUENT_CHARACTERS;
use crate::types::LinkType;
use crate::types::PlainLink;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn plain_link<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PlainLink<'s>> {
    let (remaining, _) = pre(context, input)?;
    let (remaining, path_plain) = parse_path_plain(context, remaining)?;
    peek(parser_with_context!(post)(context))(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        PlainLink {
            source: source.into(),
            link_type: path_plain.link_type,
            path: path_plain.path,
            raw_link: path_plain.raw_link,
            search_option: path_plain.search_option,
            application: path_plain.application,
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[derive(Debug)]
struct PathPlain<'s> {
    link_type: LinkType<'s>,
    path: &'s str,
    raw_link: &'s str,
    search_option: Option<&'s str>,
    application: Option<&'s str>,
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn pre<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let preceding_character = input.get_preceding_character();
    match preceding_character {
        // If None, we are at the start of the file which is fine
        None => {}
        Some(x) if !WORD_CONSTITUENT_CHARACTERS.contains(x) => {}
        Some(_) => {
            // Not at start of line, cannot be a heading
            return Err(nom::Err::Error(CustomError::Static(
                "Not a valid pre character for plain link.",
            )));
        }
    };
    Ok((input, ()))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn post<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let (remaining, _) = alt((eof, recognize(none_of(WORD_CONSTITUENT_CHARACTERS))))(input)?;
    Ok((remaining, ()))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn parse_path_plain<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathPlain<'s>> {
    alt((
        parser_with_context!(file_path_plain)(context),
        parser_with_context!(protocol_path_plain)(context),
    ))(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn parse_file_and_application<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Option<OrgSource<'s>>> {
    let (remaining, _) = tag("file")(input)?;
    let (remaining, application) =
        opt(map(tuple((tag("+"), rest)), |(_, application)| application))(remaining)?;
    // Assert we consumed the entire protocol.
    not(anychar)(remaining)?;
    Ok((remaining, application))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn parse_path_and_search_option<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (OrgSource<'s>, Option<OrgSource<'s>>)> {
    alt((
        all_consuming(parse_path_and_search_option_with_search_option),
        all_consuming(parse_path_and_search_option_without_search_option),
    ))(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn parse_path_and_search_option_with_search_option<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (OrgSource<'s>, Option<OrgSource<'s>>)> {
    let (remaining, path) = take_until("::")(input)?;
    let (remaining, search_option) = opt(map(
        tuple((
            tag("::"),
            verify(is_not(" \t\r\n"), |search_option| {
                Into::<&str>::into(search_option)
                    .chars()
                    .any(char::is_alphanumeric)
            }),
        )),
        |(_, search_option)| search_option,
    ))(remaining)?;
    // Assert we consumed the entire protocol.
    not(anychar)(remaining)?;
    Ok((remaining, (path, search_option)))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn parse_path_and_search_option_without_search_option<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (OrgSource<'s>, Option<OrgSource<'s>>)> {
    map(rest, |path| (path, None))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn file_path_plain<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathPlain<'s>> {
    let path_plain_end = path_plain_end(input.get_parenthesis_depth(), true);
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &path_plain_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, (raw_link, (_, application, _, _, (path, search_option)))) =
        consumed(tuple((
            peek(tag("file")),
            map_parser(
                parser_with_context!(protocol)(&parser_context),
                parse_file_and_application,
            ),
            tag(":"),
            opt(flat_map(
                peek(map(verify(many1_count(tag("/")), |c| *c >= 3), |c| c - 1)),
                take,
            )),
            map_parser(
                parser_with_context!(path_plain)(&parser_context),
                parse_path_and_search_option,
            ),
        )))(input)?;
    Ok((
        remaining,
        PathPlain {
            link_type: LinkType::File,
            path: path.into(),
            raw_link: raw_link.into(),
            search_option: search_option.map(Into::<&str>::into),
            application: application.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn protocol_path_plain<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathPlain<'s>> {
    let path_plain_end = path_plain_end(input.get_parenthesis_depth(), false);
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &path_plain_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, (raw_link, (protocol, _, path))) = consumed(tuple((
        parser_with_context!(protocol)(&parser_context),
        tag(":"),
        parser_with_context!(path_plain)(&parser_context),
    )))(input)?;
    Ok((
        remaining,
        PathPlain {
            link_type: LinkType::Protocol(protocol.into()),
            path: path.into(),
            raw_link: raw_link.into(),
            search_option: None,
            application: None,
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn protocol<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    for link_parameter in context.get_global_settings().link_parameters {
        let result = tag_no_case::<_, _, CustomError>(*link_parameter)(input);
        if let Ok((remaining, ent)) = result {
            return Ok((remaining, ent));
        }
    }

    Err(nom::Err::Error(CustomError::Static("NoLinkProtocol")))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn path_plain<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    // The caller needs to put an instance of path_plain_end on the context before calling this.
    let (remaining, _components) = many1(alt((
        parser_with_context!(path_plain_no_parenthesis)(context),
        parser_with_context!(path_plain_parenthesis)(context),
    )))(input)?;

    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

fn path_plain_end(
    starting_parenthesis_depth: BracketDepth,
    enable_search_option: bool,
) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| {
        impl_path_plain_end(
            context,
            input,
            starting_parenthesis_depth,
            enable_search_option,
        )
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn impl_path_plain_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    starting_parenthesis_depth: BracketDepth,
    _enable_search_option: bool,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_parenthesis_depth() - starting_parenthesis_depth;

    let (remaining, _leading_punctuation) = many0(verify(anychar, |c| {
        !" \t\r\n[]<>()/".contains(*c) && c.is_ascii_punctuation()
    }))(input)?;

    let disallowed_character = alt((recognize(one_of(" \t\r\n[]<>")), eof))(remaining);
    if disallowed_character.is_ok() {
        return disallowed_character;
    }

    if current_depth == 0 {
        let close_parenthesis = tag::<_, _, CustomError>(")")(remaining);
        if close_parenthesis.is_ok() {
            return close_parenthesis;
        }

        let open_parenthesis_without_match = recognize(tuple((
            peek(tag("(")),
            not(parser_with_context!(path_plain_parenthesis)(context)),
        )))(remaining);
        if open_parenthesis_without_match.is_ok() {
            return open_parenthesis_without_match;
        }
    }

    Err(nom::Err::Error(CustomError::Static("No path plain end")))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn path_plain_no_parenthesis<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, path) = recognize(verify(
        many_till(
            anychar,
            alt((
                peek(path_plain_no_parenthesis_disallowed_character),
                parser_with_context!(exit_matcher_parser)(context),
            )),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    ))(input)?;

    Ok((remaining, path))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn path_plain_no_parenthesis_disallowed_character<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(verify(anychar, |c| {
        c.is_whitespace() || "()[]<>".contains(*c)
    }))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn path_plain_parenthesis<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let (remaining, _opening) = tag("(")(input)?;
    let starting_depth = remaining.get_parenthesis_depth();

    let (remaining, _path) = recognize(verify(
        many_till(
            anychar,
            alt((
                peek(path_plain_parenthesis_end(starting_depth)),
                parser_with_context!(exit_matcher_parser)(context),
            )),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    ))(remaining)?;
    let (remaining, _opening) = tag(")")(remaining)?;
    let source = get_consumed(input, remaining);

    Ok((remaining, source))
}

fn path_plain_parenthesis_end(starting_parenthesis_depth: BracketDepth) -> impl Matcher {
    move |input: OrgSource<'_>| _path_plain_parenthesis_end(input, starting_parenthesis_depth)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn _path_plain_parenthesis_end<'s>(
    input: OrgSource<'s>,
    starting_parenthesis_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_parenthesis_depth() - starting_parenthesis_depth;
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing parenthesis should end the link.
        unreachable!("Exceeded plain link parenthesis depth.")
    }
    if current_depth == 0 {
        let close_parenthesis = tag::<_, _, CustomError>(")")(input);
        if close_parenthesis.is_ok() {
            return close_parenthesis;
        }
    }
    if current_depth == 1 {
        let open_parenthesis = tag::<_, _, CustomError>("(")(input);
        if open_parenthesis.is_ok() {
            return open_parenthesis;
        }
    }
    Err(nom::Err::Error(CustomError::Static(
        "No closing parenthesis",
    )))
}
