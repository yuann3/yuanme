use std::collections::BTreeMap;

use nom::bytes::complete::tag;
use nom::bytes::complete::take_until;
use nom::character::complete::anychar;
use nom::combinator::all_consuming;
use nom::combinator::eof;
use nom::combinator::map;
use nom::combinator::map_parser;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::multi::many0;
use nom::multi::many_till;
use nom::sequence::tuple;

use super::fuel;
use super::keyword::affiliated_keyword;
use super::object_parser::standard_set_object;
use super::util::confine_context;
use super::OrgSource;
use crate::context::bind_context;
use crate::context::constants::ORG_ELEMENT_DUAL_KEYWORDS;
use crate::context::constants::ORG_ELEMENT_KEYWORD_TRANSLATION_ALIST;
use crate::context::constants::ORG_ELEMENT_PARSED_KEYWORDS;
use crate::context::Context;
use crate::context::ContextElement;
use crate::context::GlobalSettings;
use crate::context::List;
use crate::error::Res;
use crate::types::AffiliatedKeywordValue;
use crate::types::AffiliatedKeywords;
use crate::types::Keyword;

pub(crate) fn affiliated_keywords<'s>(
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Vec<Keyword<'s>>> {
    let mut ret = Vec::new();
    let mut remaining = input;

    loop {
        let result = affiliated_keyword(remaining);
        match result {
            Ok((remain, kw)) => {
                remaining = remain;
                ret.push(kw);
            }
            Err(_) => {
                break;
            }
        }
    }

    Ok((remaining, ret))
}

pub(crate) fn parse_affiliated_keywords<'g, 's, AK>(
    global_settings: &'g GlobalSettings<'g, 's>,
    input: AK,
) -> AffiliatedKeywords<'s>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let mut ret = BTreeMap::new();
    for kw in input {
        let translated_name = translate_name(kw.key);
        let keyword_type = identify_keyword_type(translated_name.as_str());
        match keyword_type {
            AffiliatedKeywordType::SingleString => {
                ret.insert(
                    translated_name,
                    AffiliatedKeywordValue::SingleString(kw.value),
                );
            }
            AffiliatedKeywordType::ListOfStrings => {
                let list_of_strings = ret.entry(translated_name).or_insert_with(|| {
                    AffiliatedKeywordValue::ListOfStrings(Vec::with_capacity(1))
                });
                match list_of_strings {
                    AffiliatedKeywordValue::ListOfStrings(list_of_strings) => {
                        list_of_strings.push(kw.value);
                    }
                    _ => panic!("Invalid AffiliatedKeywordValue type."),
                }
            }
            AffiliatedKeywordType::OptionalPair => {
                let (_remaining, optional_string) = opt(all_consuming(map(
                    tuple((
                        take_until::<_, &str, nom::error::Error<_>>("["),
                        tag("["),
                        recognize(many_till(anychar, peek(tuple((tag("]"), eof))))),
                        tag("]"),
                        eof,
                    )),
                    |(_, _, objects, _, _)| objects,
                )))(kw.key)
                .expect("Parser should always succeed.");
                ret.insert(
                    translated_name,
                    AffiliatedKeywordValue::OptionalPair {
                        optval: optional_string,
                        val: kw.value,
                    },
                );
            }
            AffiliatedKeywordType::ObjectTree => {
                let initial_context = ContextElement::document_context();
                let initial_context = Context::new(global_settings, List::new(&initial_context));

                let (_remaining, optional_objects) = opt(all_consuming(map(
                    tuple((
                        take_until("["),
                        tag("["),
                        map_parser(
                            recognize(many_till(anychar, peek(tuple((tag("]"), eof))))),
                            confine_context(|i| {
                                all_consuming(many0(bind_context!(
                                    standard_set_object,
                                    &initial_context
                                )))(i)
                            }),
                        ),
                        tag("]"),
                        eof,
                    )),
                    |(_, _, objects, _, _)| objects,
                )))(kw.key.into())
                .expect("Object parser should always succeed.");

                // TODO: This should be omitting footnote references
                let objects = match all_consuming(many0(bind_context!(
                    standard_set_object,
                    &initial_context
                )))(kw.value.into())
                {
                    Ok((_remaining, objects)) => objects,
                    // Once the parse stops every exit matcher matches, so
                    // objects stop short, and the whole parse fails anyway.
                    Err(_) if fuel::stopped().is_some() => Vec::new(),
                    Err(error) => panic!("Object parser should always succeed: {error:?}"),
                };

                let entry_per_keyword_list = ret
                    .entry(translated_name)
                    .or_insert_with(|| AffiliatedKeywordValue::ObjectTree(Vec::with_capacity(1)));
                match entry_per_keyword_list {
                    AffiliatedKeywordValue::ObjectTree(entry_per_keyword_list) => {
                        entry_per_keyword_list.push((optional_objects, objects));
                    }
                    _ => panic!("Invalid AffiliatedKeywordValue type."),
                }
            }
        };
    }
    AffiliatedKeywords { keywords: ret }
}

fn translate_name(name: &str) -> String {
    let name_until_optval = name
        .split_once('[')
        .map(|(before, _after)| before)
        .unwrap_or(name);
    for (src, dst) in ORG_ELEMENT_KEYWORD_TRANSLATION_ALIST {
        if name_until_optval.eq_ignore_ascii_case(src) {
            return dst.to_lowercase();
        }
    }
    name_until_optval.to_lowercase()
}

enum AffiliatedKeywordType {
    SingleString,
    ListOfStrings,
    OptionalPair,
    ObjectTree,
}

fn identify_keyword_type(name: &str) -> AffiliatedKeywordType {
    let is_multiple = ["CAPTION", "HEADER"]
        .into_iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
        || name.to_lowercase().starts_with("attr_");
    let is_parsed = ORG_ELEMENT_PARSED_KEYWORDS
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate));
    let can_have_optval = ORG_ELEMENT_DUAL_KEYWORDS
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate));
    match (is_multiple, is_parsed, can_have_optval) {
        (true, true, true) => AffiliatedKeywordType::ObjectTree,
        (true, true, false) => unreachable!("Nothing like this exists in upstream org-mode."),
        (true, false, true) => unreachable!("Nothing like this exists in upstream org-mode."),
        (true, false, false) => AffiliatedKeywordType::ListOfStrings,
        (false, true, true) => unreachable!("Nothing like this exists in upstream org-mode."),
        (false, true, false) => unreachable!("Nothing like this exists in upstream org-mode."),
        (false, false, true) => AffiliatedKeywordType::OptionalPair,
        (false, false, false) => AffiliatedKeywordType::SingleString,
    }
}
