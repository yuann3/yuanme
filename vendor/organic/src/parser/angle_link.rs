use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take;
use nom::bytes::complete::take_until;
use nom::combinator::consumed;
use nom::combinator::flat_map;
use nom::combinator::map;
use nom::combinator::map_parser;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::rest;
use nom::combinator::verify;
use nom::multi::many1_count;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::util::maybe_consume_object_trailing_whitespace_if_not_exiting;
use super::util::text_until_exit;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::plain_link::parse_file_and_application;
use crate::parser::plain_link::protocol;
use crate::parser::util::get_consumed;
use crate::types::AngleLink;
use crate::types::LinkType;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn angle_link<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, AngleLink<'s>> {
    let (remaining, _) = tag("<")(input)?;
    let (remaining, (raw_link, parsed_link)) = consumed(map_parser(
        recognize(tuple((
            parser_with_context!(protocol)(context),
            tag(":"),
            parser_with_context!(path_angle)(context),
        ))),
        parser_with_context!(parse_angle_link)(context),
    ))(remaining)?;
    let (remaining, _) = tag(">")(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_object_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        AngleLink {
            source: source.into(),
            link_type: parsed_link.link_type,
            path: parsed_link.path,
            raw_link: raw_link.into(),
            search_option: parsed_link.search_option,
            application: parsed_link.application,
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[derive(Debug)]
struct PathAngle<'s> {
    link_type: LinkType<'s>,
    path: &'s str,
    search_option: Option<&'s str>,
    application: Option<&'s str>,
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn path_angle<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &path_angle_end,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, path) = text_until_exit(&parser_context, input)?;
    Ok((remaining, path))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn path_angle_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    tag(">")(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn parse_angle_link<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathAngle<'s>> {
    alt((
        parser_with_context!(parse_file_angle_link)(context),
        parser_with_context!(parse_protocol_angle_link)(context),
    ))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn parse_file_angle_link<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathAngle<'s>> {
    let (remaining, application) = map(
        tuple((
            peek(tag("file")),
            map_parser(
                parser_with_context!(protocol)(context),
                parse_file_and_application,
            ),
            tag(":"),
        )),
        |(_, application, _)| application,
    )(input)?;
    let (remaining, _) = opt(flat_map(
        peek(map(verify(many1_count(tag("/")), |c| *c >= 3), |c| c - 1)),
        take,
    ))(remaining)?;
    let (remaining, path) = alt((take_until("::"), rest))(remaining)?;
    let (remaining, search_option) = opt(map(tuple((tag("::"), rest)), |(_, search_option)| {
        search_option
    }))(remaining)?;
    Ok((
        remaining,
        PathAngle {
            link_type: LinkType::File,
            path: path.into(),
            search_option: search_option.map(Into::<&str>::into),
            application: application.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn parse_protocol_angle_link<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PathAngle<'s>> {
    let (remaining, link_type) = map(
        tuple((parser_with_context!(protocol)(context), tag(":"))),
        |(protocol, _)| LinkType::Protocol(protocol.into()),
    )(input)?;
    let (remaining, path) = rest(remaining)?;
    Ok((
        remaining,
        PathAngle {
            link_type,
            path: path.into(),
            search_option: None,
            application: None,
        },
    ))
}
