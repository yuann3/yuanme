//! One content file read into an Entry: its fields checked against its
//! Collection, its Slug and its Terms (#18).

use std::collections::BTreeMap;
use std::ops::Range;

use jiff::civil::Date;
use yuanme_diag::{Applicability, Diagnostic, Edit, codes};

use crate::config::{Collection, FieldType, Taxonomy};
use crate::fields::{self, Cx, Value};
use crate::slug;
use crate::yaml::{self, Key, Node, Yaml};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFormat {
    Markdown,
    Org,
}

#[derive(Debug)]
pub struct Entry {
    pub file: yuanme_diag::FileId,
    /// From the project root.
    pub path: String,
    pub collection: String,
    pub format: SourceFormat,
    pub slug: String,
    pub title: String,
    pub date: Option<Date>,
    pub updated: Option<Date>,
    pub draft: bool,
    pub unlisted: bool,
    /// The Collection's declared fields. One left out with no default is
    /// absent, not null.
    pub fields: BTreeMap<String, Value>,
    /// Terms by Taxonomy, as the Entry spells them, and where.
    pub terms: BTreeMap<String, Vec<(String, Range<usize>)>>,
    /// The body's bytes in the file.
    pub body: Range<usize>,
    /// Where the Slug came from, for a duplicate-Slug Diagnostic.
    pub slug_at: Range<usize>,
}

impl Entry {
    /// `updated` if set, otherwise `date` (#18).
    pub fn modified(&self) -> Option<Date> {
        self.updated.or(self.date)
    }

    /// A field by name, built-in, derived or declared, for sorting.
    pub fn get(&self, field: &str) -> Option<Value> {
        match field {
            "title" => Some(Value::String(self.title.clone())),
            "slug" => Some(Value::String(self.slug.clone())),
            "date" => self.date.map(Value::Date),
            "updated" => self.updated.map(Value::Date),
            "modified" => self.modified().map(Value::Date),
            "draft" => Some(Value::Bool(self.draft)),
            "unlisted" => Some(Value::Bool(self.unlisted)),
            // word_count and reading_time come with the body, in #47.
            _ => self.fields.get(field).cloned(),
        }
    }
}

/// An Entry's fields as its file writes them: front matter's keys and YAML
/// values, or, from #49, an Org file's keywords.
pub(crate) struct Head {
    pub fields: Vec<(Key, Node)>,
    /// Where the fields are written, for a missing field.
    pub at: Range<usize>,
    /// The body's bytes in the file.
    pub body: Range<usize>,
}

/// Front matter between `---` lines.
pub(crate) fn front_matter(text: &str, cx: &mut Cx<'_>) -> Option<Head> {
    let first_line = |prefix: &str| {
        text.strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('\n') || rest.starts_with("\r\n"))
    };
    if first_line("+++") {
        cx.push(
            cx.error(
                &codes::ENTRY_TOML_FRONT_MATTER,
                "front matter is YAML, between `---` lines, not TOML".into(),
                0..3,
            )
            .help("rewrite it as YAML: `title = \"Pew\"` becomes `title: Pew`"),
        );
        return None;
    }
    if !first_line("---") {
        // No front matter: every field is left out.
        return Some(Head {
            fields: Vec::new(),
            at: 0..0,
            body: 0..text.len(),
        });
    }
    let start = text.find('\n').expect("checked") + 1;
    let mut at = start;
    let close = loop {
        let end = text[at..].find('\n').map_or(text.len(), |i| at + i);
        let line = text[at..end].trim_end_matches('\r');
        if line == "---" || line == "..." {
            break Some((at, (end + 1).min(text.len())));
        }
        if end >= text.len() {
            break None;
        }
        at = end + 1;
    };
    let Some((yaml_end, body_start)) = close else {
        cx.push(cx.error(
            &codes::ENTRY_SYNTAX,
            "front matter starts here but has no closing `---`".into(),
            0..3,
        ));
        return None;
    };
    let root = match yaml::parse(&text[start..yaml_end], start) {
        Ok(root) => root,
        Err(e) => {
            cx.push(yaml_error(e, cx));
            return None;
        }
    };
    let fields = match root.value {
        Yaml::Null => Vec::new(),
        Yaml::Map(entries) => entries,
        _ => {
            let message = format!(
                "front matter must be a mapping of fields, not {}",
                root.kind()
            );
            cx.push(cx.error(&codes::ENTRY_WRONG_TYPE, message, root.span));
            return None;
        }
    };
    Some(Head {
        fields,
        at: 0..yaml_end + 3,
        body: body_start..text.len(),
    })
}

pub(crate) fn yaml_error(e: yaml::Error, cx: &Cx<'_>) -> Diagnostic {
    let span = |r| cx.span(r);
    match e {
        yaml::Error::Syntax { message, at } => Diagnostic::error(
            &codes::ENTRY_SYNTAX,
            format!("invalid YAML: {message}"),
            span(at..at),
        ),
        yaml::Error::DuplicateKey { name, first, again } => Diagnostic::error(
            &codes::ENTRY_DUPLICATE_KEY,
            format!("`{name}` is given twice"),
            span(again.clone()),
        )
        .label("given again here")
        .secondary(span(first), "first given here"),
        yaml::Error::KeyNotString { at } => {
            Diagnostic::error(&codes::ENTRY_SYNTAX, "a key must be a plain name", span(at))
        }
        yaml::Error::Tag { at } => Diagnostic::error(
            &codes::ENTRY_SYNTAX,
            "YAML tags other than `!!str` aren't supported",
            span(at),
        ),
    }
}

const BUILT_IN: &[&str] = &["title", "date", "updated", "slug", "draft", "unlisted"];

/// Checks an Entry's fields and works out its Slug, which defaults to the
/// one from its path.
pub(crate) fn read(
    collection: (&str, &Collection),
    taxonomies: &[(&str, &Taxonomy)],
    head: Head,
    path: &str,
    cx: &mut Cx<'_>,
) -> Option<Entry> {
    let (collection_name, collection) = collection;
    let Head {
        fields: given,
        at: head,
        body,
    } = head;
    let find = |key: &str| {
        given
            .iter()
            .find(|(k, n)| k.name == key && n.value != Yaml::Null)
            .map(|(_, n)| n)
    };
    let mut ok = true;

    let taxonomy_names: Vec<&str> = taxonomies.iter().map(|(n, _)| *n).collect();
    let extra: Vec<&str> = BUILT_IN
        .iter()
        .copied()
        .chain(taxonomy_names.iter().copied())
        .collect();
    // A declared `date` only makes the built-in required.
    let mut schema = collection.fields.clone();
    let date_required = schema.remove("date").is_some();
    let declared = fields::fields(&schema, &given, &extra, head.clone(), "", cx);
    ok &= declared.is_some();

    let mut built_in = |name: &str, ty: FieldType, required: bool| -> Option<Value> {
        match find(name) {
            Some(f) => {
                let v = fields::value(&ty, f, name, cx);
                ok &= v.is_some();
                v
            }
            None if required => {
                let mut d = cx.error(
                    &codes::ENTRY_MISSING_FIELD,
                    format!("missing field `{name}`"),
                    head.clone(),
                );
                if name == "title" && head.is_empty() {
                    let title = path
                        .rsplit('/')
                        .next()
                        .unwrap_or(path)
                        .split('.')
                        .next()
                        .unwrap_or("");
                    let insert = format!("---\ntitle: {title}\n---\n\n");
                    let edit = Edit {
                        span: cx.span(0..0),
                        replacement: insert,
                    };
                    d = d.suggest(
                        "add front matter with a title",
                        Applicability::HasPlaceholders,
                        vec![edit],
                    );
                }
                cx.push(d);
                ok = false;
                None
            }
            None => None,
        }
    };
    let string = FieldType::String {
        min: None,
        max: None,
    };
    let title = built_in("title", string.clone(), true);
    let date = built_in("date", FieldType::Date, date_required);
    let updated = built_in("updated", FieldType::Date, false);
    let draft = built_in("draft", FieldType::Bool, false);
    let unlisted = built_in("unlisted", FieldType::Bool, false);
    let slug_field = built_in("slug", string.clone(), false);

    let (slug, slug_at) = match (slug_field, find("slug")) {
        (Some(Value::String(s)), Some(f)) => {
            if !slug::is_valid(&s) {
                let mut d = cx.error(
                    &codes::ENTRY_INVALID_SLUG,
                    format!("`{s}` is not a valid Slug"),
                    f.span.clone(),
                );
                let fixed = slug::corrected(&s);
                if !fixed.is_empty() {
                    d = cx.fix(d, format!("use `{fixed}`"), f.span.clone(), fixed);
                }
                d = d.note("a Slug is lowercase letters, digits, `-` and `_`, in segments separated by `/`");
                cx.push(d);
                ok = false;
            }
            (s, f.span.clone())
        }
        _ => (path_slug(path, &collection.dir), 0..0),
    };

    let mut terms = BTreeMap::new();
    for (name, taxonomy) in taxonomies {
        let Some(f) = find(name) else { continue };
        let list = FieldType::List {
            items: Box::new(string.clone()),
            unique: false,
        };
        let got = match (&f.value, taxonomy.single) {
            (Yaml::Str(_), true) => fields::value(&string, f, name, cx).map(|v| vec![v]),
            (Yaml::Str(s), false) => {
                let message = format!("`{name}` must be a list of Terms, not a string");
                let d = cx.error(&codes::ENTRY_WRONG_TYPE, message, f.span.clone());
                cx.push(cx.fix(d, "make it a list".into(), f.span.clone(), format!("[{s}]")));
                None
            }
            (_, true) => {
                let message = format!("`{name}` holds one Term, as a string, not {}", f.kind());
                cx.push(
                    cx.error(&codes::ENTRY_WRONG_TYPE, message, f.span.clone())
                        .note(format!("`[taxonomies.{name}]` has `single = true`")),
                );
                None
            }
            (_, false) => match fields::value(&list, f, name, cx) {
                Some(Value::List(items)) => Some(items),
                _ => None,
            },
        };
        let Some(got) = got else {
            ok = false;
            continue;
        };
        let spans: Vec<Range<usize>> = match &f.value {
            Yaml::Seq(items) => items.iter().map(|n| n.span.clone()).collect(),
            _ => vec![f.span.clone()],
        };
        let mut spelled = Vec::new();
        for (v, at) in got.into_iter().zip(spans) {
            let Value::String(term) = v else {
                unreachable!("checked as strings")
            };
            if slug::term_slug(&term).is_empty() {
                let message =
                    format!("the Term `{term}` has no letters or digits to make a URL from");
                cx.push(cx.error(&codes::ENTRY_INVALID_VALUE, message, at));
                ok = false;
            } else if !spelled.iter().any(|(t, _)| *t == term) {
                spelled.push((term, at));
            }
        }
        terms.insert(name.to_string(), spelled);
    }

    if !ok {
        return None;
    }
    let date_of = |v: Option<Value>| match v {
        Some(Value::Date(d)) => Some(d),
        _ => None,
    };
    let flag = |v: Option<Value>| matches!(v, Some(Value::Bool(true)));
    Some(Entry {
        file: cx.file,
        path: path.to_string(),
        collection: collection_name.to_string(),
        format: if path.ends_with(".org") {
            SourceFormat::Org
        } else {
            SourceFormat::Markdown
        },
        slug,
        title: match title {
            Some(Value::String(t)) => t,
            _ => unreachable!("required and checked"),
        },
        date: date_of(date),
        updated: date_of(updated),
        draft: flag(draft),
        unlisted: flag(unlisted),
        fields: declared.expect("checked"),
        terms,
        body,
        slug_at,
    })
}

/// The Slug from a file's path in its Collection: each folder and the file
/// name slugged, and `foo/index.md` is `foo`.
pub(crate) fn path_slug(path: &str, dir: &str) -> String {
    let relative = path
        .strip_prefix(dir)
        .map_or(path, |p| p.trim_start_matches('/'));
    let stem = relative.rsplit_once('.').map_or(relative, |(stem, _)| stem);
    let stem = match stem.rsplit_once('/') {
        Some((parent, "index")) => parent,
        _ => stem,
    };
    stem.split('/')
        .map(slug::slug)
        .collect::<Vec<_>>()
        .join("/")
}
