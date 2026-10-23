use nom::bytes::complete::tag;
use nom::combinator::recognize;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use super::util::rest_of_line;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::types::DiarySexp;
use crate::types::Keyword;

pub(crate) fn diary_sexp<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, DiarySexp<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    start_of_line(remaining)?;
    // `%%(` alone is a diary sexp too, as `detect_diary_sexp` says.
    let (remaining, value) = recognize(tuple((tag("%%("), rest_of_line)))(remaining)?;
    let (remaining, _eol) = org_line_ending(remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        DiarySexp {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            value: Into::<&str>::into(value),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn detect_diary_sexp<'b, 'g, 'r, 's, AK>(
    _affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    tuple((start_of_line, tag("%%(")))(remaining)?;
    Ok((input, ()))
}
