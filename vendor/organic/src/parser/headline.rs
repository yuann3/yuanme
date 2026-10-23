use nom::branch::alt;
use nom::bytes::complete::is_a;
use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::consumed;
use nom::combinator::map;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::many1;
use nom::multi::separated_list1;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::section::section;
use super::util::exit_matcher_parser;
use super::util::get_consumed;
use super::util::org_line_ending;
use super::util::org_space;
use super::util::org_space_or_line_ending;
use super::util::start_of_line;
use crate::context::bind_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::object_parser::standard_set_object;
use crate::parser::util::blank_line;
use crate::types::DocumentElement;
use crate::types::Element;
use crate::types::Heading;
use crate::types::HeadlineLevel;
use crate::types::Object;
use crate::types::PriorityCookie;
use crate::types::TodoKeywordType;

pub(crate) const fn heading(
    parent_level: HeadlineLevel,
) -> impl for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, Heading<'s>> {
    move |context, input: OrgSource<'_>| _heading(context, input, parent_level)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn _heading<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    parent_star_count: HeadlineLevel,
) -> Res<OrgSource<'s>, Heading<'s>> {
    let mut scheduled = None;
    let mut deadline = None;
    let mut closed = None;
    not(bind_context!(exit_matcher_parser, context))(input)?;
    let (remaining, pre_headline) = headline(context, input, parent_star_count)?;
    let section_matcher = bind_context!(section, context);
    let heading_matcher = bind_context!(heading(pre_headline.star_count), context);
    let (contents_begin, _) = opt(many0(blank_line))(remaining)?;
    let maybe_post_blank = get_consumed(remaining, contents_begin);
    let (remaining, maybe_section) =
        opt(map(section_matcher, DocumentElement::Section))(remaining)?;
    let (remaining, _ws) = opt(tuple((start_of_line, many0(blank_line))))(remaining)?;
    let (remaining, mut children) =
        many0(map(heading_matcher, DocumentElement::Heading))(remaining)?;
    if let Some(section) = maybe_section {
        // If the section has a planning then the timestamp values are copied to the heading.
        if let DocumentElement::Section(inner_section) = &section {
            if let Some(Element::Planning(planning)) = inner_section.children.first() {
                scheduled.clone_from(&planning.scheduled);
                deadline.clone_from(&planning.deadline);
                closed.clone_from(&planning.closed);
            }
        }
        children.insert(0, section);
    }
    let has_children = !children.is_empty();
    let remaining = if !has_children {
        // Support empty headings
        let (remain, _ws) = many0(blank_line)(remaining)?;
        remain
    } else {
        remaining
    };
    let is_archived = pre_headline.tags.contains(&"ARCHIVE");

    let contents = get_consumed(contents_begin, remaining);
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Heading {
            source: source.into(),
            level: pre_headline.headline_level,
            todo_keyword: pre_headline
                .todo_keyword
                .map(|(todo_keyword_type, todo_keyword)| {
                    (todo_keyword_type, Into::<&str>::into(todo_keyword))
                }),
            priority_cookie: pre_headline.priority_cookie.map(|(_, priority)| priority),
            title: pre_headline.title,
            tags: pre_headline.tags,
            children,
            is_comment: pre_headline.comment.is_some(),
            is_archived,
            is_footnote_section: pre_headline.is_footnote_section,
            scheduled,
            deadline,
            closed,
            contents: if contents.len() > 0 {
                Some(Into::<&str>::into(contents))
            } else {
                None
            },
            post_blank: if has_children {
                None
            } else {
                Some(Into::<&str>::into(maybe_post_blank))
            },
        },
    ))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn detect_headline<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    tuple((start_of_line, many1(tag("*")), space1))(input)?;
    Ok((input, ()))
}

/// Fields from a not-yet-fully-parsed Headline.
///
/// This struct exists to give names to the fields of a partially-parsed Headline to avoid returning a large tuple of nameless fields.
#[derive(Debug)]
struct PreHeadline<'s> {
    headline_level: HeadlineLevel,
    star_count: HeadlineLevel,
    todo_keyword: Option<(TodoKeywordType, OrgSource<'s>)>,
    priority_cookie: Option<(OrgSource<'s>, PriorityCookie)>,
    comment: Option<OrgSource<'s>>,
    title: Vec<Object<'s>>,
    tags: Vec<&'s str>,
    is_footnote_section: bool,
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn headline<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    parent_star_count: HeadlineLevel,
) -> Res<OrgSource<'s>, PreHeadline<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Document,
        exit_matcher: &headline_title_end,
    });
    let parser_context = context.with_additional_node(&parser_context);

    let (remaining, (_, (headline_level, star_count, _), _)) = tuple((
        start_of_line,
        verify(
            bind_context!(headline_level, &parser_context),
            |(_, count, _)| *count > parent_star_count,
        ),
        peek(org_space),
    ))(input)?;

    let (remaining, maybe_todo_keyword) = opt(tuple((
        space1,
        bind_context!(heading_keyword, &parser_context),
        peek(org_space_or_line_ending),
    )))(remaining)?;

    let (remaining, maybe_priority) = opt(tuple((space1, priority_cookie)))(remaining)?;

    let (remaining, maybe_comment) = opt(tuple((
        space1,
        tag("COMMENT"),
        peek(org_space_or_line_ending),
    )))(remaining)?;

    let (remaining, maybe_title) = opt(tuple((
        space1,
        consumed(many1(bind_context!(standard_set_object, &parser_context))),
    )))(remaining)?;

    let (remaining, maybe_tags) = opt(tuple((space0, tags)))(remaining)?;

    let (remaining, _) = tuple((space0, org_line_ending))(remaining)?;

    let is_footnote_section = maybe_title
        .as_ref()
        .map(|(_, (raw_title, _))| raw_title)
        .map(|raw_title| {
            Into::<&str>::into(raw_title) == context.get_global_settings().footnote_section
        })
        .unwrap_or(false);

    Ok((
        remaining,
        PreHeadline {
            headline_level,
            star_count,
            todo_keyword: maybe_todo_keyword.map(|(_, todo, _)| todo),
            priority_cookie: maybe_priority,
            comment: maybe_comment.map(|(_, comment, _)| comment),
            title: maybe_title
                .map(|(_, (_, title))| title)
                .unwrap_or(Vec::new()),
            tags: maybe_tags
                .map(|(_ws, tags)| tags.into_iter().map(Into::<&str>::into).collect())
                .unwrap_or(Vec::new()),
            is_footnote_section,
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn headline_title_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(tuple((space0, opt(tuple((tags, space0))), org_line_ending)))(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn tags<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, Vec<OrgSource<'s>>> {
    let (remaining, (_open, tags, _close)) =
        tuple((tag(":"), separated_list1(tag(":"), single_tag), tag(":")))(input)?;
    Ok((remaining, tags))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn single_tag<'r, 's>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(many1(verify(anychar, |c| {
        c.is_alphanumeric() || "_@#%".contains(*c)
    })))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn heading_keyword<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (TodoKeywordType, OrgSource<'s>)> {
    let global_settings = context.get_global_settings();
    if global_settings.in_progress_todo_keywords.is_empty()
        && global_settings.complete_todo_keywords.is_empty()
    {
        alt((
            map(tag("TODO"), |capture| (TodoKeywordType::Todo, capture)),
            map(tag("DONE"), |capture| (TodoKeywordType::Done, capture)),
        ))(input)
    } else {
        for todo_keyword in global_settings
            .in_progress_todo_keywords
            .iter()
            .map(String::as_str)
        {
            let result = tag::<_, _, CustomError>(todo_keyword)(input);
            if let Ok((remaining, ent)) = result {
                return Ok((remaining, (TodoKeywordType::Todo, ent)));
            }
        }
        for todo_keyword in global_settings
            .complete_todo_keywords
            .iter()
            .map(String::as_str)
        {
            let result = tag::<_, _, CustomError>(todo_keyword)(input);
            if let Ok((remaining, ent)) = result {
                return Ok((remaining, (TodoKeywordType::Done, ent)));
            }
        }
        Err(nom::Err::Error(CustomError::Static("NoTodoKeyword")))
    }
}

fn priority_cookie(input: OrgSource<'_>) -> Res<OrgSource<'_>, PriorityCookie> {
    let (remaining, (_, priority_character, _)) = tuple((
        tag("[#"),
        verify(anychar, |c| c.is_alphanumeric()),
        tag("]"),
    ))(input)?;
    let cookie = PriorityCookie::try_from(priority_character).map_err(|_| {
        nom::Err::Error(CustomError::Static(
            "Failed to cast priority cookie to number.",
        ))
    })?;
    Ok((remaining, cookie))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn headline_level<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (HeadlineLevel, HeadlineLevel, OrgSource<'s>)> {
    let (remaining, stars) = is_a("*")(input)?;
    let count = stars.len().try_into().unwrap();
    let level = match context.get_global_settings().odd_levels_only {
        crate::context::HeadlineLevelFilter::Odd => {
            if count % 2 == 0 {
                (count + 2) / 2
            } else {
                (count + 1) / 2
            }
        }
        crate::context::HeadlineLevelFilter::OddEven => count,
    };
    Ok((remaining, (level, count, stars)))
}
