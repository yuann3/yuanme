use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::combinator::consumed;
use nom::combinator::not;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many0;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::keyword::table_formula_keyword;
use super::object_parser::table_cell_set_object;
use super::org_source::OrgSource;
use super::util::exit_matcher_parser;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::types::Keyword;
use crate::types::Table;
use crate::types::TableCell;
use crate::types::TableRow;

/// Parse an org-mode-style table
///
/// This is not the table.el style.
pub(crate) fn org_mode_table<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Table<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    start_of_line(remaining)?;
    peek(tuple((space0, tag("|"))))(remaining)?;

    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("table"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &table_end,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);

    let org_mode_table_row_matcher = parser_with_context!(org_mode_table_row)(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);

    let (remaining, (contents, (children, _exit_contents))) =
        consumed(many_till(org_mode_table_row_matcher, exit_matcher))(remaining)?;

    let (remaining, formulas) =
        many0(parser_with_context!(table_formula_keyword)(context))(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        Table {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            formulas,
            children,
            contents: Into::<&str>::into(contents),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn detect_table<'b, 'g, 'r, 's, AK>(
    _affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    tuple((start_of_line, space0, tag("|")))(remaining)?;
    Ok((input, ()))
}

fn table_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    recognize(tuple((space0, not(tag("|")))))(input)
}

fn org_mode_table_row<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, TableRow<'s>> {
    alt((
        parser_with_context!(org_mode_table_row_rule)(context),
        parser_with_context!(org_mode_table_row_regular)(context),
    ))(input)
}

fn org_mode_table_row_rule<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, TableRow<'s>> {
    start_of_line(input)?;
    let (remaining, _) = tuple((space0, tag("|-"), opt(is_not("\r\n")), org_line_ending))(input)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        TableRow {
            source: source.into(),
            children: Vec::new(),
            contents: None,
        },
    ))
}

fn org_mode_table_row_regular<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, TableRow<'s>> {
    start_of_line(input)?;
    let (remaining, _) = tuple((space0, tag("|")))(input)?;
    // A row can have no cells: upstream required one, so a lone `|` line
    // failed the whole document, where Emacs reads it as an empty row.
    let (remaining, (contents, children)) =
        consumed(many0(parser_with_context!(org_mode_table_cell)(context)))(remaining)?;
    let (remaining, _tail) = recognize(tuple((space0, org_line_ending)))(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        TableRow {
            source: source.into(),
            children,
            contents: if contents.len() > 0 {
                Some(Into::<&str>::into(contents))
            } else {
                None
            },
        },
    ))
}

fn org_mode_table_cell<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, TableCell<'s>> {
    let parser_context = ContextElement::ExitMatcherNode(ExitMatcherNode {
        class: ExitClass::Beta,
        exit_matcher: &org_mode_table_cell_end,
    });
    let parser_context = context.with_additional_node(&parser_context);
    let table_cell_set_object_matcher =
        parser_with_context!(table_cell_set_object)(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);
    let (remaining, _) = space0(input)?;
    let (remaining, (contents, (children, _exit_contents))) = consumed(verify(
        many_till(table_cell_set_object_matcher, exit_matcher),
        |(children, exit_contents)| {
            !children.is_empty() || Into::<&str>::into(exit_contents).ends_with('|')
        },
    ))(remaining)?;

    let (remaining, _tail) = org_mode_table_cell_end(&parser_context, remaining)?;

    let source = get_consumed(input, remaining);

    Ok((
        remaining,
        TableCell {
            source: source.into(),
            children,
            contents: Into::<&str>::into(contents),
        },
    ))
}

fn org_mode_table_cell_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(tuple((space0, alt((tag("|"), peek(line_ending))))))(input)
}
