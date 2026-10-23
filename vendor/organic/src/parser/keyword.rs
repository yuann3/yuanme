use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::bytes::complete::take_while1;
use nom::character::complete::anychar;
use nom::character::complete::one_of;
use nom::character::complete::space0;
use nom::combinator::consumed;
use nom::combinator::map;
use nom::combinator::not;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::org_source::BracketDepth;
use super::org_source::OrgSource;
use super::util::get_consumed;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use super::util::org_line_ending;
use crate::context::constants::ORG_ELEMENT_AFFILIATED_KEYWORDS;
use crate::context::constants::ORG_ELEMENT_DUAL_KEYWORDS;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::macros::element;
use crate::parser::util::start_of_line;
use crate::types::AffiliatedKeywords;
use crate::types::Keyword;

pub(crate) fn filtered_keyword<'s, F: Fn(OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>>>(
    key_parser: F,
) -> impl Fn(OrgSource<'s>) -> Res<OrgSource<'s>, Keyword<'s>> {
    move |input| _filtered_keyword(&key_parser, input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(key_parser))
)]
fn _filtered_keyword<'s, F: Fn(OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>>>(
    key_parser: F,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Keyword<'s>> {
    start_of_line(input)?;
    // TODO: When key is a member of org-element-parsed-keywords, value can contain the standard set objects, excluding footnote references.
    let (remaining, (consumed_input, (_, _, parsed_key, _))) =
        consumed(tuple((space0, tag("#+"), key_parser, tag(":"))))(input)?;
    let (remaining, _ws) = space0(remaining)?;
    if let Ok((remaining, _)) = org_line_ending(remaining) {
        return Ok((
            remaining,
            Keyword {
                source: consumed_input.into(),
                affiliated_keywords: AffiliatedKeywords::default(), // To be populated by the caller if this keyword is in a context to support affiliated keywords.
                key: parsed_key.into(),
                value: "",
                post_blank: None,
            },
        ));
    }
    let (remaining, parsed_value) =
        recognize(many_till(anychar, peek(tuple((space0, org_line_ending)))))(remaining)?;
    let (remaining, _ws) = tuple((space0, org_line_ending))(remaining)?;
    Ok((
        remaining,
        Keyword {
            source: consumed_input.into(),
            affiliated_keywords: AffiliatedKeywords::default(), // To be populated by the caller if this keyword is in a context to support affiliated keywords.
            key: parsed_key.into(),
            value: parsed_value.into(),
            post_blank: None,
        },
    ))
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(context, affiliated_keywords))
)]
pub(crate) fn keyword<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Keyword<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, mut kw) = filtered_keyword(regular_keyword_key)(remaining)?;
    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    kw.affiliated_keywords =
        parse_affiliated_keywords(context.get_global_settings(), affiliated_keywords);
    kw.source = Into::<&str>::into(source);
    kw.post_blank = post_blank.map(Into::<&str>::into);
    Ok((remaining, kw))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn affiliated_keyword<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, Keyword<'s>> {
    filtered_keyword(affiliated_key)(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(ret, level = "debug", skip(_context))
)]
pub(crate) fn table_formula_keyword<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Keyword<'s>> {
    verify(filtered_keyword(table_formula_key), |kw| {
        !kw.value.is_empty()
    })(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn table_formula_key<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    tag_no_case("tblfm")(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn regular_keyword_key<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    not(peek(alt((tag_no_case("call"), tag_no_case("begin")))))(input)?;
    recognize(many_till(
        anychar,
        peek(alt((
            recognize(one_of(" \t\r\n")), // Give up if we hit whitespace
            recognize(tuple((tag(":"), one_of(" \t\r\n")))), // Stop if we see a colon followed by whitespace
            recognize(tuple((tag(":"), is_not(" \t\r\n:"), not(tag(":"))))), // Stop if we see a colon that is the last colon before whitespace. This is for keywords like "#+foo:bar:baz: lorem: ipsum" which would have the key "foo:bar:baz".
        ))),
    ))(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn affiliated_key<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    element!(dual_affiliated_key, input);
    element!(plain_affiliated_key, input);
    element!(export_keyword, input);
    Err(nom::Err::Error(CustomError::Static("No affiliated key.")))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn plain_affiliated_key<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    for keyword in ORG_ELEMENT_AFFILIATED_KEYWORDS {
        let result = map(
            tuple((tag_no_case::<_, _, CustomError>(keyword), peek(tag(":")))),
            |(key, _)| key,
        )(input);
        if let Ok((remaining, ent)) = result {
            return Ok((remaining, ent));
        }
    }

    Err(nom::Err::Error(CustomError::Static("NoKeywordKey")))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn dual_affiliated_key<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    for keyword in ORG_ELEMENT_DUAL_KEYWORDS {
        let result = recognize(tuple((
            tag_no_case::<_, _, CustomError>(keyword),
            tag("["),
            optval,
            tag("]"),
            peek(tag(":")),
        )))(input);
        if let Ok((remaining, ent)) = result {
            return Ok((remaining, ent));
        }
    }

    Err(nom::Err::Error(CustomError::Static("NoKeywordKey")))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn optval<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(many_till(
        anychar,
        peek(optval_end(input.get_bracket_depth())),
    ))(input)
}

const fn optval_end(
    starting_bracket_depth: BracketDepth,
) -> impl for<'s> Fn(OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    move |input: OrgSource<'_>| _optval_end(input, starting_bracket_depth)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn _optval_end<'s>(
    input: OrgSource<'s>,
    starting_bracket_depth: BracketDepth,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    let current_depth = input.get_bracket_depth() - starting_bracket_depth;
    if current_depth < 0 {
        // This shouldn't be possible because if depth is 0 then a closing bracket should end the opval.
        unreachable!("Exceeded optval bracket depth.")
    }
    if current_depth == 0 {
        let close_bracket = tag::<_, _, CustomError>("]")(input);
        if close_bracket.is_ok() {
            return close_bracket;
        }
    }
    tag("\n")(input)
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn export_keyword<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    recognize(tuple((
        tag_no_case("attr_"),
        take_while1(|c: char| c.is_alphanumeric() || "-_".contains(c)),
    )))(input)
}

#[cfg(test)]
mod tests {
    use test::Bencher;

    use super::*;

    #[bench]
    fn bench_affiliated_keyword(b: &mut Bencher) {
        let input = OrgSource::new("#+CAPTION[*foo*]: bar *baz*");

        b.iter(|| assert!(affiliated_keyword(input).is_ok()));
    }
}
