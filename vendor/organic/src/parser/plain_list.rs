use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::digit1;
use nom::character::complete::line_ending;
use nom::character::complete::multispace1;
use nom::character::complete::one_of;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::consumed;
use nom::combinator::eof;
use nom::combinator::map;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::many1;
use nom::multi::many_till;
use nom::sequence::tuple;
use nom::InputTake;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::element_parser::element;
use super::object_parser::standard_set_object;
use super::org_source::OrgSource;
use super::util::include_input;
use super::util::indentation_level;
use super::util::non_whitespace_character;
use crate::context::bind_context;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::blank_line;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::parser::util::org_space;
use crate::parser::util::start_of_line;
use crate::types::CheckboxType;
use crate::types::IndentationLevel;
use crate::types::Keyword;
use crate::types::Object;
use crate::types::PlainList;
use crate::types::PlainListItem;
use crate::types::PlainListItemCounter;
use crate::types::PlainListItemPreBlank;
use crate::types::PlainListType;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, _affiliated_keywords))
)]
pub(crate) fn detect_plain_list<'b, 'g, 'r, 's, AK>(
    _affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    if verify(
        tuple((
            start_of_line,
            parser_with_context!(indentation_level)(context),
            parser_with_context!(bullet)(context),
            alt((space1, line_ending, eof)),
        )),
        |(_start, (indent_level, _), (_bullet_type, bull), _after_whitespace)| {
            !Into::<&str>::into(bull).starts_with('*') || *indent_level > 0
        },
    )(remaining)
    .is_ok()
    {
        return Ok((input, ()));
    }
    return Err(nom::Err::Error(CustomError::Static("No element detected.")));
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
pub(crate) fn detect_not_plain_list_item_indent<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (u16, OrgSource<'s>)> {
    if let Ok((_remaining, (_, indent, _))) = tuple((
        start_of_line,
        parser_with_context!(indentation_level)(context),
        not(tuple((
            parser_with_context!(bullet)(context),
            alt((space1, line_ending, eof)),
        ))),
    ))(input)
    {
        return Ok((input, indent));
    }

    // Headlines are not plain list items.
    if let Ok((_remaining, (_, indent, _))) = verify(
        tuple((
            start_of_line,
            parser_with_context!(indentation_level)(context),
            tuple((
                parser_with_context!(bullet)(context),
                alt((space1, line_ending, eof)),
            )),
        )),
        |(_, (depth, _), ((_, bullet), _))| {
            *depth == 0 && Into::<&str>::into(bullet).starts_with('*')
        },
    )(input)
    {
        return Ok((input, indent));
    }
    return Err(nom::Err::Error(CustomError::Static("No element detected.")));
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, affiliated_keywords))
)]
pub(crate) fn plain_list<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, PlainList<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let contexts = [
        ContextElement::Context("plain list"),
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Beta,
            exit_matcher: &plain_list_end,
        }),
    ];

    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    // children stores tuple of (input string, parsed object) so we can re-parse the final item
    let mut children = Vec::new();
    let mut first_item_indentation: Option<IndentationLevel> = None;
    let mut first_item_list_type: Option<PlainListType> = None;
    let contents_begin = remaining;
    let mut remaining = remaining;

    // The final list item does not consume trailing blank lines (which instead get consumed by the list). We have three options here:
    //
    // 1. Parse all items while consuming trailing whitespace, then edit the final item to remove trailing whitespace.
    // 2. Parse all items without consuming trailing whitespace, then edit all but the final one to add in the trailing whitespace.
    // 3. Re-parse the final item with consume trailing whitespace disabled.
    //
    // While #3 is the most slow, it also seems to cleanest and involves the least manual mutation of already-parsed objects so I am going with #3 for now, but we should revisit #1 or #2 when the parser is more developed.

    loop {
        let list_item = plain_list_item(&parser_context, remaining);
        match (&first_item_list_type, &list_item) {
            (None, Ok((_remain, (list_type, _item)))) => {
                let _ = first_item_list_type.insert(*list_type);
            }
            (None, Err(_)) => {}
            (Some(_), _) => {}
        };
        match list_item {
            Ok((remain, (_list_type, item)))
                if item.indentation == *first_item_indentation.get_or_insert(item.indentation) =>
            {
                children.push((remaining, item));
                remaining = remain;
            }
            Ok(_) | Err(_) => {
                break;
            }
        };

        let maybe_exit = exit_matcher_parser(&parser_context, remaining);
        if maybe_exit.is_ok() {
            break;
        }
    }

    if children.is_empty() {
        return Err(nom::Err::Error(CustomError::Static(
            "Plain lists require at least one element.",
        )));
    }

    let contents = get_consumed(contents_begin, remaining);
    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        PlainList {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            list_type: first_item_list_type.expect("Plain lists require at least one element."),
            children: children.into_iter().map(|(_start, item)| item).collect(),
            contents: Some(Into::<&str>::into(contents)),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn plain_list_item<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (PlainListType, PlainListItem<'s>)> {
    start_of_line(input)?;
    let (remaining, (indent_level, _leading_whitespace)) = indentation_level(context, input)?;
    let (remaining, (bullet_type, bull)) =
        verify(bind_context!(bullet, context), |(_bullet_type, bull)| {
            !Into::<&str>::into(bull).starts_with('*') || indent_level > 0
        })(remaining)?;

    let (remaining, maybe_counter_set) =
        opt(tuple((space1, tag("[@"), counter_set_value, tag("]"))))(remaining)?;
    let maybe_counter_set = maybe_counter_set.map(|(_, _, val, _)| val);

    let (remaining, maybe_checkbox) = opt(tuple((space1, item_checkbox)))(remaining)?;

    let (remaining, maybe_tag) = if let BulletType::Unordered = bullet_type {
        opt(tuple((space1, bind_context!(item_tag, context))))(remaining)?
    } else {
        (remaining, None)
    };
    let list_type = match (&maybe_tag, bullet_type) {
        (None, BulletType::Ordered) => PlainListType::Ordered,
        (None, BulletType::Unordered) => PlainListType::Unordered,
        (Some(_), BulletType::Ordered) => unreachable!(),
        (Some(_), BulletType::Unordered) => PlainListType::Descriptive,
    };

    let exit_matcher = plain_list_item_end(indent_level);
    let final_item_whitespace_cutoff = final_item_whitespace_cutoff(indent_level);
    let final_whitespace_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &final_item_whitespace_cutoff,
    });
    let final_whitespace_context = context.with_additional_node(&final_whitespace_context);
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Beta,
            exit_matcher: &exit_matcher,
        }),
    ];
    let parser_context = final_whitespace_context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);

    let maybe_contentless_item: Res<OrgSource<'_>, ()> =
        detect_contentless_item_contents(&parser_context, remaining);
    if let Ok((_rem, _ws)) = maybe_contentless_item {
        let (remaining, post_blank) = if tuple((
            blank_line,
            bind_context!(final_item_whitespace_cutoff, context),
        ))(remaining)
        .is_ok()
        {
            recognize(alt((blank_line, eof)))(remaining)?
        } else {
            recognize(alt((recognize(many1(blank_line)), eof)))(remaining)?
        };
        let source = get_consumed(input, remaining);
        return Ok((
            remaining,
            (
                list_type,
                PlainListItem {
                    source: source.into(),
                    indentation: indent_level,
                    bullet: bull.into(),
                    counter: maybe_counter_set,
                    checkbox: None,
                    tag: maybe_tag
                        .map(|(_ws, item_tag)| item_tag)
                        .unwrap_or(Vec::new()),
                    pre_blank: 0,
                    children: Vec::new(),
                    contents: None,
                    post_blank: if post_blank.len() > 0 {
                        Some(Into::<&str>::into(post_blank))
                    } else {
                        None
                    },
                },
            ),
        ));
    }
    let (remaining, pre_blank) = item_tag_post_gap(&parser_context, remaining)?;
    let pre_blank = Into::<&str>::into(pre_blank)
        .bytes()
        .filter(|b| *b == b'\n')
        .count();

    let (remaining, (contents, (children, _exit_contents))) = consumed(many_till(
        include_input(bind_context!(element(true), &parser_context)),
        bind_context!(exit_matcher_parser, &parser_context),
    ))(remaining)?;

    // We have to use the parser_context here to include the whitespace cut-off
    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(&final_whitespace_context, remaining)?;

    let source = get_consumed(input, remaining);
    return Ok((
        remaining,
        (
            list_type,
            PlainListItem {
                source: source.into(),
                indentation: indent_level,
                bullet: bull.into(),
                counter: maybe_counter_set,
                checkbox: maybe_checkbox.map(|(_, (checkbox_type, source))| {
                    (checkbox_type, Into::<&str>::into(source))
                }),
                tag: maybe_tag
                    .map(|(_ws, item_tag)| item_tag)
                    .unwrap_or(Vec::new()),
                pre_blank: PlainListItemPreBlank::try_from(pre_blank)
                    .expect("pre-blank cannot be larger than 2."),
                children: children.into_iter().map(|(_start, item)| item).collect(),
                contents: if contents.len() > 0 {
                    Some(contents.into())
                } else {
                    None
                },
                post_blank: post_blank.map(Into::<&str>::into),
            },
        ),
    ));
}

#[derive(Debug)]
enum BulletType {
    Ordered,
    Unordered,
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn bullet<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, (BulletType, OrgSource<'s>)> {
    let (remaining, ((bullet_type, _without_space), peek_trailing_space)) = tuple((
        alt((
            map(tag("*"), |bull| (BulletType::Unordered, bull)),
            map(tag("-"), |bull| (BulletType::Unordered, bull)),
            map(tag("+"), |bull| (BulletType::Unordered, bull)),
            map(
                recognize(tuple((
                    bind_context!(counter, context),
                    alt((tag("."), tag(")"))),
                ))),
                |bull| (BulletType::Ordered, bull),
            ),
        )),
        peek(space0),
    ))(input)?;
    let with_space = input.get_until_end_of(peek_trailing_space);
    Ok((remaining, (bullet_type, with_space)))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn counter<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    if context.get_global_settings().list_allow_alphabetical {
        alt((
            recognize(one_of(
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
            )),
            digit1,
        ))(input)
    } else {
        digit1(input)
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn counter_set_value<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, PlainListItemCounter> {
    alt((
        map(
            one_of("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"),
            |letter| {
                let num = match letter {
                    'a'..='z' => (letter as u32) - ('a' as u32) + 1,
                    'A'..='Z' => (letter as u32) - ('A' as u32) + 1,
                    _ => unreachable!(),
                };
                PlainListItemCounter::try_from(num)
                    .expect("Counter set value should be between 1 and 26 inclusive.")
            },
        ),
        map(digit1, |num: OrgSource<'_>| {
            Into::<&str>::into(num)
                .parse()
                .expect("digit1 must parse to a number.")
        }),
    ))(input)
}

const fn final_item_whitespace_cutoff(indent_level: IndentationLevel) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| {
        impl_final_item_whitespace_cutoff(context, input, indent_level)
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn impl_final_item_whitespace_cutoff<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    indent_level: IndentationLevel,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    // element!(plain_list_end, context, input);

    if let Ok((_remaining, _)) = verify(
        tuple((
            opt(blank_line),
            bind_context!(indentation_level, context),
            not(multispace1),
        )),
        |(_, (depth, _stars), _not_whitespace)| *depth < indent_level,
    )(input)
    {
        return Ok((input, input.take(0)));
    }

    if let Ok((_remaining, _)) = tuple((
        opt(blank_line),
        verify(
            bind_context!(detect_not_plain_list_item_indent, context),
            |(depth, _)| *depth == indent_level,
        ),
    ))(input)
    {
        return Ok((input, input.take(0)));
    }

    Err(nom::Err::Error(CustomError::Static(
        "No whitespace cut-off.",
    )))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn plain_list_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(tuple((
        start_of_line,
        verify(many1(blank_line), |lines: &Vec<OrgSource<'_>>| {
            lines.len() >= 2
        }),
    )))(input)
}

const fn plain_list_item_end(indent_level: IndentationLevel) -> impl ContextMatcher {
    let line_indented_lte_matcher = line_indented_lte(indent_level);
    move |context, input: OrgSource<'_>| {
        _plain_list_item_end(context, input, &line_indented_lte_matcher)
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, line_indented_lte_matcher))
)]
fn _plain_list_item_end<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    line_indented_lte_matcher: impl ContextMatcher,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    recognize(tuple((
        opt(blank_line),
        bind_context!(line_indented_lte_matcher, context),
    )))(input)
}

const fn line_indented_lte(indent_level: IndentationLevel) -> impl ContextMatcher {
    move |context, input: OrgSource<'_>| _line_indented_lte(context, input, indent_level)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn _line_indented_lte<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    indent_level: IndentationLevel,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let matched = recognize(verify(
        tuple((
            bind_context!(indentation_level, context),
            non_whitespace_character,
        )),
        // It is fine that we get the indent level using the number of bytes rather than the number of characters because nom's space0 only matches space and tab (0x20 and 0x09)
        |((indentation_level, _leading_whitespace), _anychar)| *indentation_level <= indent_level,
    ))(input)?;

    Ok(matched)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn item_tag<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Object<'s>>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Gamma,
        exit_matcher: &item_tag_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let (remaining, (children, _exit_contents)) = verify(
        many_till(
            // TODO: Should this be using a different set like the minimal set?
            bind_context!(standard_set_object, &parser_context),
            bind_context!(exit_matcher_parser, &parser_context),
        ),
        |(children, _exit_contents)| !children.is_empty(),
    )(input)?;
    let (remaining, _) = item_tag_divider(remaining)?;
    Ok((remaining, children))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
fn item_tag_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alt((item_tag_divider, line_ending))(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn item_tag_divider<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(tuple((
        one_of(" \t"),
        tag("::"),
        peek(tuple((
            opt(tuple((
                peek(one_of(" \t")),
                many_till(anychar, peek(alt((item_tag_divider, line_ending, eof)))),
            ))),
            alt((line_ending, eof)),
        ))),
    )))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn item_tag_post_gap<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    verify(
        recognize(tuple((
            alt((blank_line, space0)),
            many_till(
                blank_line,
                alt((
                    peek(recognize(not(blank_line))),
                    peek(recognize(tuple((many0(blank_line), eof)))),
                    bind_context!(exit_matcher_parser, context),
                )),
            ),
        ))),
        |gap| gap.len() > 0,
    )(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn item_checkbox<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, (CheckboxType, OrgSource<'s>)> {
    alt((
        map(
            recognize(tuple((tag("["), org_space, tag("]")))),
            |capture| (CheckboxType::Off, capture),
        ),
        map(tag("[-]"), |capture| (CheckboxType::Trans, capture)),
        map(tag("[X]"), |capture| (CheckboxType::On, capture)),
    ))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context))
)]
fn detect_contentless_item_contents<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    let (remaining, _) = recognize(many_till(
        blank_line,
        bind_context!(exit_matcher_parser, context),
    ))(input)?;
    Ok((remaining, ()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::bind_context;
    use crate::context::Context;
    use crate::context::GlobalSettings;
    use crate::context::List;
    use crate::types::StandardProperties;

    #[test]
    fn plain_list_item_empty() {
        let input = OrgSource::new("1.");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let plain_list_item_matcher = bind_context!(plain_list_item, &initial_context);
        let (remaining, (_, result)) = plain_list_item_matcher(input).unwrap();
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(result.get_source(), "1.");
    }

    #[test]
    fn plain_list_item_simple() {
        let input = OrgSource::new("1. foo");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let plain_list_item_matcher = bind_context!(plain_list_item, &initial_context);
        let (remaining, (_, result)) = plain_list_item_matcher(input).unwrap();
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(result.get_source(), "1. foo");
    }

    #[test]
    fn plain_list_empty() {
        let input = OrgSource::new("1.");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let (remaining, result) =
            plain_list(std::iter::empty(), input, &initial_context, input).unwrap();
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(result.get_source(), "1.");
    }

    #[test]
    fn plain_list_simple() {
        let input = OrgSource::new("1. foo");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let (remaining, result) =
            plain_list(std::iter::empty(), input, &initial_context, input).unwrap();
        assert_eq!(Into::<&str>::into(remaining), "");
        assert_eq!(result.get_source(), "1. foo");
    }

    #[test]
    fn plain_list_cant_start_line_with_asterisk() {
        // Plain lists with an asterisk bullet must be indented or else they would be a headline
        let input = OrgSource::new("* foo");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let result = plain_list(std::iter::empty(), input, &initial_context, input);
        assert!(result.is_err());
    }

    #[test]
    fn indented_can_start_line_with_asterisk() {
        // Plain lists with an asterisk bullet must be indented or else they would be a headline
        let input = OrgSource::new(" * foo");
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let result = plain_list(std::iter::empty(), input, &initial_context, input);
        assert!(result.is_ok());
    }

    #[test]
    fn two_blank_lines_ends_list() {
        let input = OrgSource::new(
            r#"1. foo
2. bar
   baz
3. lorem


   ipsum
"#,
        );
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let plain_list_matcher = bind_context!(element(true), &initial_context);
        let (remaining, result) =
            plain_list_matcher(input).expect("Should parse the plain list successfully.");
        assert_eq!(Into::<&str>::into(remaining), "   ipsum\n");
        assert_eq!(
            result.get_source(),
            r#"1. foo
2. bar
   baz
3. lorem


"#
        );
    }

    #[test]
    fn two_blank_lines_ends_nested_list() {
        let input = OrgSource::new(
            r#"1. foo
   1. bar


baz"#,
        );
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let plain_list_matcher = bind_context!(element(true), &initial_context);
        let (remaining, result) =
            plain_list_matcher(input).expect("Should parse the plain list successfully.");
        assert_eq!(Into::<&str>::into(remaining), "baz");
        assert_eq!(
            result.get_source(),
            r#"1. foo
   1. bar


"#
        );
    }

    #[test]
    fn interior_trailing_whitespace() {
        let input = OrgSource::new(
            r#"1. foo

   bar

   1. baz

      lorem

   ipsum


dolar"#,
        );
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let plain_list_matcher = bind_context!(element(true), &initial_context);
        let (remaining, result) =
            plain_list_matcher(input).expect("Should parse the plain list successfully.");
        assert_eq!(Into::<&str>::into(remaining), "dolar");
        assert_eq!(
            result.get_source(),
            r#"1. foo

   bar

   1. baz

      lorem

   ipsum


"#
        );
    }

    #[test]
    fn detect_line_break() {
        let input = OrgSource::new(
            r#"+
"#,
        );
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let result = detect_plain_list(std::iter::empty(), input, &initial_context, input);
        assert!(result.is_ok());
    }

    #[test]
    fn detect_eof() {
        let input = OrgSource::new(r#"+"#);
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let result = detect_plain_list(std::iter::empty(), input, &initial_context, input);
        assert!(result.is_ok());
    }

    #[test]
    fn detect_no_gap() {
        let input = OrgSource::new(r#"+foo"#);
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let result = detect_plain_list(std::iter::empty(), input, &initial_context, input);
        // Since there is no whitespace after the '+' this is a paragraph, not a plain list.
        assert!(result.is_err());
    }

    #[test]
    fn detect_with_gap() {
        let input = OrgSource::new(r#"+ foo"#);
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let result = detect_plain_list(std::iter::empty(), input, &initial_context, input);
        assert!(result.is_ok());
    }
}
