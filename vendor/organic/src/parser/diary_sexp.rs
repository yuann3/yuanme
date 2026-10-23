use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::combinator::recognize;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::OrgSource;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use crate::context::RefContext;
use crate::error::Res;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::types::DiarySexp;
use crate::types::Keyword;

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, affiliated_keywords))
)]
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
    let (remaining, value) = recognize(tuple((tag("%%("), is_not("\r\n"))))(remaining)?;
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

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context, _affiliated_keywords))
)]
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
