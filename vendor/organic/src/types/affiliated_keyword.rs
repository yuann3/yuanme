use std::collections::BTreeMap;

use super::Object;

#[derive(Debug)]
pub enum AffiliatedKeywordValue<'s> {
    SingleString(&'s str),
    ListOfStrings(Vec<&'s str>),
    OptionalPair {
        optval: Option<&'s str>,
        val: &'s str,
    },
    ObjectTree(Vec<(Option<Vec<Object<'s>>>, Vec<Object<'s>>)>),
}

#[derive(Debug)]
pub struct AffiliatedKeyword<'s> {
    pub name: &'s str,
    pub value: AffiliatedKeywordValue<'s>,
}

#[derive(Debug, Default)]
pub struct AffiliatedKeywords<'s> {
    pub keywords: BTreeMap<String, AffiliatedKeywordValue<'s>>,
}

pub trait GetAffiliatedKeywords<'s> {
    fn get_affiliated_keywords<'a>(&'a self) -> &'a AffiliatedKeywords<'s>;
}
