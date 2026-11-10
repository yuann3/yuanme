//! `[collections]` and `[taxonomies]`, and the sort keys both take.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use toml::Spanned;

use super::check::{Checker, similar};
use super::fields::field;
use super::{
    Field, FieldType, Table, Value, as_table, boolean, entries, int, items, string, strings, table,
};

#[derive(Clone, Debug)]
pub struct Collection {
    pub dir: String,
    /// Where the folder is set, or the Collection named when it isn't.
    pub dir_at: Range<usize>,
    pub route: String,
    pub list_route: Option<String>,
    pub paginate: Option<u64>,
    pub sort: Vec<SortKey>,
    pub fields: BTreeMap<String, Field>,
    /// The Template of each Entry's Page, when not the default.
    pub template: Option<String>,
    /// Where `template` is set, or the Collection named when it isn't.
    pub template_at: Range<usize>,
}

#[derive(Debug)]
pub struct Taxonomy {
    pub collections: Vec<String>,
    pub route: String,
    pub paginate: Option<u64>,
    pub single: bool,
    pub sort: Vec<SortKey>,
    pub lowercase: bool,
    /// Slugs the Author chose, by Term.
    pub slugs: BTreeMap<String, String>,
    /// The Template of each Term's Page, when not the default.
    pub template: Option<String>,
    /// Where `template` is set, or the Taxonomy named when it isn't.
    pub template_at: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortKey {
    pub field: String,
    pub descending: bool,
}

/// Fields every Entry has. Only `date` can be declared, to make it required.
const BUILT_IN: &[&str] = &["title", "date", "updated", "slug", "draft", "unlisted"];
/// Fields yuanme derives from each Entry, so no Author field can take their
/// names. Templates see `url` and `content` on every Entry too (#19).
const DERIVED: &[&str] = &[
    "modified",
    "word_count",
    "reading_time",
    "headings",
    "url",
    "content",
];

/// `[collections]`, then `[taxonomies]`, which name Collections and sort by
/// their fields.
pub(super) fn read(
    collections: Option<&Table<'_>>,
    taxonomies: Option<&Table<'_>>,
    checker: &mut Checker,
) -> (BTreeMap<String, Collection>, BTreeMap<String, Taxonomy>) {
    let taxonomies = entries(taxonomies);
    let collections: BTreeMap<String, Collection> = entries(collections)
        .iter()
        .map(|(key, c)| {
            let name: &str = key.get_ref().as_ref();
            // Each Taxonomy over this Collection adds a field of its name.
            let taxonomy_fields: Vec<&str> = taxonomies
                .iter()
                .filter(|(_, t)| {
                    items(&as_table(t)["collections"])
                        .iter()
                        .any(|c| c.get_ref().as_str() == Some(name))
                })
                .map(|(t, _)| t.get_ref().as_ref())
                .collect();
            (
                name.to_string(),
                collection(key, as_table(c), &taxonomy_fields, checker),
            )
        })
        .collect();
    let names = names(&collections);
    let taxonomies = taxonomies
        .iter()
        .map(|(name, t)| {
            (
                name.get_ref().to_string(),
                taxonomy(name, as_table(t), &names, &collections, checker),
            )
        })
        .collect();
    (collections, taxonomies)
}

/// The Collections a Taxonomy or feed can name: the built-in `pages` too.
pub(super) fn names(collections: &BTreeMap<String, Collection>) -> BTreeSet<&str> {
    let mut names: BTreeSet<&str> = collections.keys().map(String::as_str).collect();
    names.insert("pages");
    names
}

fn collection(
    name: &Spanned<std::borrow::Cow<'_, str>>,
    c: &Table<'_>,
    taxonomy_fields: &[&str],
    checker: &mut Checker,
) -> Collection {
    let key = name;
    let name: &str = key.get_ref().as_ref();
    let mut fields = BTreeMap::new();
    for (field_name, spec) in entries(table(c, "fields")) {
        let at = field_name.span();
        let field_name = field_name.get_ref().as_ref();
        let spec = as_table(spec);
        if DERIVED.contains(&field_name) {
            checker.invalid(format!("`{field_name}` is a field yuanme derives from each Entry, so no field can take its name"), at);
        } else if taxonomy_fields.contains(&field_name) {
            checker.invalid(
                format!("`{field_name}` is the field the Taxonomy `{field_name}` adds to `{name}`"),
                at,
            );
        } else if field_name == "date" && spec["type"].get_ref().as_str() != Some("date") {
            let message =
                "`date` is a built-in field of type `date`: declaring it only makes it required"
                    .to_string();
            checker.invalid(message, spec["type"].span());
        } else if field_name != "date" && BUILT_IN.contains(&field_name) {
            checker.invalid(
                format!("`{field_name}` is a built-in field of every Entry"),
                at,
            );
        }
        fields.insert(field_name.to_string(), field(spec, true, checker));
    }
    let list_route = string(c, "list_route");
    let paginate = c.get("paginate");
    if let Some(paginate) = paginate
        && list_route.is_none()
    {
        checker.invalid(
            format!("`collections.{name}.paginate` needs a `list_route` to paginate"),
            paginate.span(),
        );
    }
    let (sortable, unsortable) = sortable(&fields);
    Collection {
        // `content/posts/` names the same folder as `content/posts`.
        dir: string(c, "dir").map_or_else(
            || format!("content/{name}"),
            |d| d.trim_end_matches('/').to_string(),
        ),
        dir_at: c.get("dir").map_or(key.span(), |d| d.span()),
        route: string(c, "route").expect("required"),
        list_route,
        paginate: paginate.map(|p| int(p) as u64),
        sort: sort(c.get("sort"), &sortable, &unsortable, checker),
        fields,
        template: string(c, "template"),
        template_at: c.get("template").map_or(key.span(), |t| t.span()),
    }
}

fn taxonomy(
    name: &Spanned<std::borrow::Cow<'_, str>>,
    t: &Table<'_>,
    names: &BTreeSet<&str>,
    collections: &BTreeMap<String, Collection>,
    checker: &mut Checker,
) -> Taxonomy {
    if BUILT_IN.iter().chain(DERIVED).any(|f| *f == name.get_ref()) {
        let message = format!(
            "`{}` is a built-in field of every Entry, so no Taxonomy can take its name",
            name.get_ref()
        );
        checker.invalid(message, name.span());
    }
    let no_fields = BTreeMap::new();
    let (mut sortable, mut unsortable) = sortable(&no_fields);
    for collection in items(&t["collections"]) {
        let s = collection.get_ref().as_str().expect("checked");
        if let Some(c) = collections.get(s) {
            let (yes, no) = self::sortable(&c.fields);
            sortable.extend(yes);
            unsortable.extend(no);
        } else {
            collection_name(collection, names, checker);
        }
    }
    Taxonomy {
        collections: strings(&t["collections"]),
        route: string(t, "route").expect("required"),
        paginate: t.get("paginate").map(|p| int(p) as u64),
        single: boolean(t, "single"),
        sort: sort(t.get("sort"), &sortable, &unsortable, checker),
        lowercase: boolean(t, "lowercase"),
        slugs: entries(table(t, "slugs"))
            .iter()
            .map(|(term, slug)| {
                let s = slug.get_ref().as_str().expect("checked");
                if !crate::slug::is_valid(s) {
                    checker.invalid(format!("`{s}` is not a valid Slug"), slug.span());
                    let fixed = crate::slug::corrected(s);
                    if !fixed.is_empty() {
                        checker.suggest(
                            format!("use `{fixed}`"),
                            slug.span(),
                            format!("\"{fixed}\""),
                        );
                    }
                }
                (term.get_ref().to_string(), s.to_string())
            })
            .collect(),
        template: string(t, "template"),
        template_at: t.get("template").map_or(name.span(), |t| t.span()),
    }
}

/// Checks that `name` is a Collection's.
pub(super) fn collection_name(name: &Value<'_>, names: &BTreeSet<&str>, checker: &mut Checker) {
    let s = name.get_ref().as_str().expect("checked");
    if names.contains(s) {
        return;
    }
    checker.invalid(format!("there is no Collection `{s}`"), name.span());
    if let Some(near) = similar(s, names.iter().copied()) {
        checker.suggest(
            format!("did you mean `{near}`?"),
            name.span(),
            format!("\"{near}\""),
        );
    }
}

/// The fields Entries can sort by, and the ones they can't: lists and objects
/// have no order.
fn sortable(fields: &BTreeMap<String, Field>) -> (Vec<&str>, Vec<&str>) {
    let mut yes: Vec<&str> = BUILT_IN
        .iter()
        .chain(DERIVED)
        .copied()
        .filter(|f| !["headings", "url", "content"].contains(f))
        .collect();
    let mut no = vec!["headings"];
    for (name, field) in fields {
        match field.kind {
            FieldType::List { .. } | FieldType::Object(_) => no.push(name),
            _ => yes.push(name),
        }
    }
    (yes, no)
}

fn sort(
    keys: Option<&Value<'_>>,
    sortable: &[&str],
    unsortable: &[&str],
    checker: &mut Checker,
) -> Vec<SortKey> {
    let Some(keys) = keys else {
        return vec![SortKey {
            field: "date".into(),
            descending: true,
        }];
    };
    items(keys)
        .iter()
        .map(|key| {
            let s = key.get_ref().as_str().expect("checked");
            let (field, descending) = match s.strip_prefix('-') {
                Some(field) => (field, true),
                None => (s, false),
            };
            if field.is_empty() {
                checker.invalid(
                    "a sort key needs a field name, such as \"-date\"".into(),
                    key.span(),
                );
            } else if unsortable.contains(&field) && !sortable.contains(&field) {
                checker.invalid(
                    format!("Entries can't sort by `{field}`: it is a list or an object"),
                    key.span(),
                );
            } else if !sortable.contains(&field) {
                checker.invalid(
                    format!("there is no field `{field}` to sort by"),
                    key.span(),
                );
                if let Some(near) = similar(field, sortable.iter().copied()) {
                    let sign = if descending { "-" } else { "" };
                    checker.suggest(
                        format!("did you mean `{near}`?"),
                        key.span(),
                        format!("\"{sign}{near}\""),
                    );
                }
            }
            SortKey {
                field: field.to_string(),
                descending,
            }
        })
        .collect()
}
