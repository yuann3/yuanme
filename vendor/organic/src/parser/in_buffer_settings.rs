use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag_no_case;
use nom::bytes::complete::take_until;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::eof;
use nom::combinator::map;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::multi::many_till;
use nom::multi::separated_list0;
use nom::sequence::tuple;

use super::keyword::filtered_keyword;
use super::keyword_todo::todo_keywords;
use super::OrgSource;
use crate::context::HeadlineLevelFilter;
use crate::error::CustomError;
use crate::error::Res;
use crate::settings::GlobalSettings;
use crate::types::AstNode;
use crate::types::Document;
use crate::types::Keyword;

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn scan_for_in_buffer_settings<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Keyword<'s>>> {
    // TODO: Write some tests to make sure this is functioning properly.

    let mut keywords = Vec::new();
    let mut remaining = input;
    loop {
        // Skip text until possible in_buffer_setting
        let start_of_pound = take_until::<_, _, CustomError>("#+")(remaining);
        let start_of_pound = if let Ok((start_of_pound, _)) = start_of_pound {
            start_of_pound
        } else {
            break;
        };
        // Go backwards to the start of the line and run the filtered_keyword parser
        let start_of_line = start_of_pound.get_start_of_line();

        let (remain, maybe_kw) = match filtered_keyword(in_buffer_settings_key)(start_of_line) {
            Ok((remain, kw)) => (remain, Some(kw)),
            Err(_) => {
                let end_of_line = take_until::<_, _, CustomError>("\n")(start_of_pound);
                if let Ok((end_of_line, _)) = end_of_line {
                    (end_of_line, None)
                } else {
                    break;
                }
            }
        };

        if let Some(kw) = maybe_kw {
            keywords.push(kw);
        }
        remaining = remain;
    }

    Ok((remaining, keywords))
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn in_buffer_settings_key<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alt((
        tag_no_case("archive"),
        tag_no_case("category"),
        tag_no_case("columns"),
        tag_no_case("filetags"),
        tag_no_case("link"),
        tag_no_case("priorities"),
        tag_no_case("property"),
        tag_no_case("seq_todo"),
        tag_no_case("setupfile"),
        tag_no_case("startup"),
        tag_no_case("tags"),
        tag_no_case("todo"),
        tag_no_case("typ_todo"),
    ))(input)
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(level = "debug", skip(original_settings))
)]
pub(crate) fn apply_in_buffer_settings<'g, 's, 'sf>(
    keywords: Vec<Keyword<'sf>>,
    original_settings: &'g GlobalSettings<'g, 's>,
) -> Result<GlobalSettings<'g, 's>, CustomError> {
    let mut new_settings = original_settings.clone();

    // Todo Keywords
    for kw in keywords.iter().filter(|kw| {
        kw.key.eq_ignore_ascii_case("todo")
            || kw.key.eq_ignore_ascii_case("seq_todo")
            || kw.key.eq_ignore_ascii_case("typ_todo")
    }) {
        let (_, (in_progress_words, complete_words)) =
            todo_keywords(kw.value).map_err(|err| match err {
                nom::Err::Incomplete(_) => panic!("This parser does not support streaming."),
                nom::Err::Error(e) => e,
                nom::Err::Failure(e) => e,
            })?;
        new_settings
            .in_progress_todo_keywords
            .extend(in_progress_words.into_iter().map(str::to_string));
        new_settings
            .complete_todo_keywords
            .extend(complete_words.into_iter().map(str::to_string));
    }

    // Startup settings
    for kw in keywords
        .iter()
        .filter(|kw| kw.key.eq_ignore_ascii_case("startup"))
    {
        let (_remaining, settings) = separated_list0(space1::<&str, CustomError>, is_not(" \t"))(
            kw.value,
        )
        .map_err(|err: nom::Err<_>| match err {
            nom::Err::Incomplete(_) => panic!("This parser does not support streaming."),
            nom::Err::Error(e) => e,
            nom::Err::Failure(e) => e,
        })?;
        if settings.contains(&"odd") {
            new_settings.odd_levels_only = HeadlineLevelFilter::Odd;
        }
        if settings.contains(&"oddeven") {
            new_settings.odd_levels_only = HeadlineLevelFilter::OddEven;
        }
    }

    // Link templates
    for kw in keywords
        .iter()
        .filter(|kw| kw.key.eq_ignore_ascii_case("link"))
    {
        let (_, (link_key, link_value)) = link_template(kw.value).map_err(|err| match err {
            nom::Err::Incomplete(_) => panic!("This parser does not support streaming."),
            nom::Err::Error(e) => e,
            nom::Err::Failure(e) => e,
        })?;
        new_settings
            .link_templates
            .insert(link_key.to_owned(), link_value.to_owned());
    }

    Ok(new_settings)
}

/// Apply in-buffer settings that do not impact parsing and therefore can be applied after parsing.
#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
pub(crate) fn apply_post_parse_in_buffer_settings<'g, 's, 'sf>(document: &mut Document<'s>) {
    document.category = Into::<AstNode>::into(&*document)
        .iter_all_ast_nodes()
        .filter_map(|ast_node| {
            if let AstNode::Keyword(ast_node) = ast_node {
                if ast_node.key.eq_ignore_ascii_case("category") {
                    return Some(ast_node);
                }
            }
            None
        })
        .last()
        .map(|kw| kw.value.to_owned());
}

#[cfg_attr(feature = "tracing", tracing::instrument(ret, level = "debug"))]
fn link_template<'s>(input: &'s str) -> Res<&'s str, (&'s str, &'s str)> {
    let (remaining, key) = map(
        tuple((
            space0,
            recognize(many_till(anychar, peek(alt((space1, line_ending, eof))))),
        )),
        |(_, key)| key,
    )(input)?;

    let (remaining, replacement) = map(
        tuple((
            space1,
            recognize(many_till(
                anychar,
                peek(tuple((space0, alt((line_ending, eof))))),
            )),
        )),
        |(_, replacement)| replacement,
    )(remaining)?;

    Ok((remaining, (key, replacement)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_test() -> Result<(), Box<dyn std::error::Error>> {
        let input = OrgSource::new(
            r#"
foo
   #+archive: bar

baz #+category: lorem

#+label: ipsum

#+todo: dolar
cat
"#,
        );
        let (remaining, settings) = scan_for_in_buffer_settings(input)?;
        assert_eq!(Into::<&str>::into(remaining), "cat\n");
        let keys: Vec<_> = settings.iter().map(|kw| kw.key).collect();
        // category is skipped because it is not the first non-whitespace on the line.
        //
        // label is skipped because it is not an in-buffer setting.
        assert_eq!(keys, vec!["archive", "todo"]);
        Ok(())
    }
}
