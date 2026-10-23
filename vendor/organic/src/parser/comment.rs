use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::combinator::eof;
use nom::combinator::not;
use nom::combinator::recognize;
use nom::multi::many0;
use nom::multi::many_till;
use nom::sequence::preceded;
use nom::sequence::tuple;

use super::org_source::OrgSource;
use super::util::get_consumed;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::immediate_in_section;
use crate::parser::util::start_of_line;
use crate::types::Comment;

pub(crate) fn comment<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Comment<'s>> {
    if immediate_in_section(context, "comment") {
        return Err(nom::Err::Error(CustomError::Static(
            "Cannot nest objects of the same element",
        )));
    }
    let parser_context = ContextElement::Context("comment");
    let parser_context = context.with_additional_node(&parser_context);
    let comment_line_matcher = parser_with_context!(comment_line)(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);
    let (remaining, first_line) = comment_line_matcher(input)?;
    let (remaining, mut remaining_lines) =
        many0(preceded(not(exit_matcher), comment_line_matcher))(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    let mut value = Vec::with_capacity(remaining_lines.len() + 1);
    let last_line = remaining_lines.pop();
    if let Some(last_line) = last_line {
        value.push(Into::<&str>::into(first_line));
        value.extend(remaining_lines.into_iter().map(Into::<&str>::into));
        let last_line = Into::<&str>::into(last_line);
        // Trim the line ending from the final line.
        value.push(&last_line[..(last_line.len() - 1)])
    } else {
        // Trim the line ending from the only line.
        let only_line = Into::<&str>::into(first_line);
        value.push(&only_line[..(only_line.len() - 1)])
    }
    Ok((
        remaining,
        Comment {
            source: source.into(),
            value,
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn comment_line<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    let (remaining, _) = tuple((space0, tag("#")))(input)?;
    if let Ok((remaining, line_break)) = org_line_ending(remaining) {
        return Ok((remaining, line_break));
    }
    let (remaining, _) = tag(" ")(remaining)?;
    let (remaining, value) = recognize(many_till(anychar, org_line_ending))(remaining)?;
    Ok((remaining, value))
}

pub(crate) fn detect_comment<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ()> {
    tuple((
        start_of_line,
        space0,
        tag("#"),
        alt((tag(" "), line_ending, eof)),
    ))(input)?;
    Ok((input, ()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::bind_context;
    use crate::context::Context;
    use crate::context::GlobalSettings;
    use crate::context::List;

    #[test]
    fn require_space_after_hash() {
        let input = OrgSource::new(
            "# Comment line
#not a comment
# Comment again",
        );
        let global_settings = GlobalSettings::default();
        let initial_context = ContextElement::document_context();
        let initial_context = Context::new(&global_settings, List::new(&initial_context));
        let comment_matcher = bind_context!(comment, &initial_context);
        let (remaining, first_comment) = comment_matcher(input).expect("Parse first comment");
        assert_eq!(
            Into::<&str>::into(remaining),
            r#"#not a comment
# Comment again"#
        );
        assert_eq!(
            first_comment.source,
            "# Comment line
"
        );
    }
}
