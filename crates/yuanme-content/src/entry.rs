//! One content file read into an Entry: its fields checked against its
//! Collection, its Slug and its Terms (#18).

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use yuanme_diag::{Applicability, Diagnostic, Edit, codes};

use crate::config::{Collection, FieldType, Taxonomy};
use crate::document::{self, Document};
use crate::fields::{self, Cx, Value};
use crate::slug;
use crate::yaml::{self, Key, Node, Yaml};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceFormat {
    Markdown,
    Org,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub document: Arc<Document>,
    pub word_count: u64,
    /// Where the Slug came from, for a duplicate-Slug Diagnostic.
    pub slug_at: Range<usize>,
}

impl Entry {
    /// `updated` if set, otherwise `date` (#18).
    pub fn modified(&self) -> Option<Date> {
        self.updated.or(self.date)
    }

    /// Minutes to read, as the Reference site rounds them.
    pub fn reading_time(&self) -> u64 {
        document::reading_time(self.word_count)
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
            "word_count" => Some(Value::Int(self.word_count as i64)),
            "reading_time" => Some(Value::Int(self.reading_time() as i64)),
            _ => self.fields.get(field).cloned(),
        }
    }
}

/// An Entry's fields as its file writes them: front matter's keys and YAML
/// values, or an Org file's keywords.
pub(crate) struct Head {
    pub fields: Vec<(Key, Node)>,
    /// Where the fields are written, for a missing field.
    pub at: Range<usize>,
    /// The body's bytes in the file.
    pub body: Range<usize>,
}

/// Front matter between `---` lines, after a byte order mark if the file
/// starts with one.
pub(crate) fn front_matter(text: &str, cx: &mut Cx<'_>) -> Option<Head> {
    let bom = if text.starts_with('\u{FEFF}') { 3 } else { 0 };
    let first_line = |prefix: &str| {
        text[bom..]
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('\n') || rest.starts_with("\r\n"))
    };
    if first_line("+++") {
        cx.push(
            cx.error(
                &codes::ENTRY_TOML_FRONT_MATTER,
                "front matter is YAML, between `---` lines, not TOML".into(),
                bom..bom + 3,
            )
            .help("rewrite it as YAML: `title = \"Pew\"` becomes `title: Pew`"),
        );
        return None;
    }
    if !first_line("---") {
        // No front matter: every field is left out.
        return Some(Head {
            fields: Vec::new(),
            at: bom..bom,
            body: bom..text.len(),
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
            bom..bom + 3,
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
        at: bom..yaml_end + 3,
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
/// one from its path. `document` is `None` when the body failed, which
/// fails the Entry once its fields are checked too.
pub(crate) fn read(
    collection: (&str, &Collection),
    taxonomies: &[(&str, &Taxonomy)],
    head: Head,
    document: Option<Document>,
    path: &str,
    cx: &mut Cx<'_>,
) -> Option<Entry> {
    let (collection_name, collection) = collection;
    let mut given = Given {
        fields: &head.fields,
        at: head.at.clone(),
        path,
        ok: true,
    };

    let taxonomy_names: Vec<&str> = taxonomies.iter().map(|(n, _)| *n).collect();
    let extra: Vec<&str> = BUILT_IN
        .iter()
        .copied()
        .chain(taxonomy_names.iter().copied())
        .collect();
    // A declared `date` only makes the built-in required.
    let mut schema = collection.fields.clone();
    let date_required = schema.remove("date").is_some();
    let declared = fields::fields(&schema, &head.fields, &extra, head.at.clone(), "", cx);
    given.ok &= declared.is_some();

    let title = given.built_in("title", STRING, true, cx);
    let date = given.built_in("date", FieldType::Date, date_required, cx);
    let updated = given.built_in("updated", FieldType::Date, false, cx);
    let draft = given.built_in("draft", FieldType::Bool, false, cx);
    let unlisted = given.built_in("unlisted", FieldType::Bool, false, cx);
    let slug_field = given.built_in("slug", STRING, false, cx);
    let (slug, slug_at) = given.slug(slug_field, &collection.dir, cx);
    let terms = given.terms(taxonomies, cx);

    let document = document?;
    if !given.ok {
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
            _ => unreachable!("`title` is required, and checked as a string"),
        },
        date: date_of(date),
        updated: date_of(updated),
        draft: flag(draft),
        unlisted: flag(unlisted),
        fields: declared.expect("`ok` is false when the declared fields failed"),
        terms,
        word_count: document.word_count(),
        document: Arc::new(document),
        slug_at,
    })
}

/// The type of `title`, `slug` and each Term.
const STRING: FieldType = FieldType::String {
    min: None,
    max: None,
};

/// The fields an Entry's file gives, checked one at a time. A field that
/// fails reports why and clears `ok`, and the rest are still checked, so
/// every mistake in the file is reported at once.
struct Given<'a> {
    fields: &'a [(Key, Node)],
    /// Where the fields are written, for a missing field.
    at: Range<usize>,
    path: &'a str,
    ok: bool,
}

impl<'a> Given<'a> {
    /// A field the file gives a value, not null.
    fn find(&self, key: &str) -> Option<&'a Node> {
        self.fields
            .iter()
            .find(|(k, n)| k.name == key && n.value != Yaml::Null)
            .map(|(_, n)| n)
    }

    /// A field every Entry has, as `ty`.
    fn built_in(
        &mut self,
        name: &str,
        ty: FieldType,
        required: bool,
        cx: &mut Cx<'_>,
    ) -> Option<Value> {
        match self.find(name) {
            Some(f) => {
                let v = fields::value(&ty, f, name, cx);
                self.ok &= v.is_some();
                v
            }
            None if required => {
                let d = self.missing(name, cx);
                cx.push(d);
                self.ok = false;
                None
            }
            None => None,
        }
    }

    /// A required field left out. A file with no fields at all is offered
    /// a title from its name.
    fn missing(&self, name: &str, cx: &Cx<'_>) -> Diagnostic {
        let (head, path) = (&self.at, self.path);
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
            let (insert, what) = if path.ends_with(".org") {
                (format!("#+TITLE: {title}\n"), "add a `#+TITLE`")
            } else {
                (
                    format!("---\ntitle: {title}\n---\n\n"),
                    "add front matter with a title",
                )
            };
            let edit = Edit {
                span: cx.span(0..0),
                replacement: insert,
            };
            d = d.suggest(what, Applicability::HasPlaceholders, vec![edit]);
        }
        d
    }

    /// The Slug `slug:` gives, `value`, if it does; otherwise the one from
    /// the file's path in `dir`. And where it is given.
    fn slug(&mut self, value: Option<Value>, dir: &str, cx: &mut Cx<'_>) -> (String, Range<usize>) {
        let (Some(Value::String(s)), Some(f)) = (value, self.find("slug")) else {
            return (path_slug(self.path, dir), 0..0);
        };
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
            d = d.note(
                "a Slug is lowercase letters, digits, `-` and `_`, in segments separated by `/`",
            );
            cx.push(d);
            self.ok = false;
        }
        (s, f.span.clone())
    }

    /// The Terms the Entry has in each of `taxonomies`, as it spells them.
    fn terms(
        &mut self,
        taxonomies: &[(&str, &Taxonomy)],
        cx: &mut Cx<'_>,
    ) -> BTreeMap<String, Vec<(String, Range<usize>)>> {
        let mut terms = BTreeMap::new();
        for (name, taxonomy) in taxonomies {
            let Some(f) = self.find(name) else { continue };
            let Some(got) = term_values(name, taxonomy, f, cx) else {
                self.ok = false;
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
                    self.ok = false;
                } else if !spelled.iter().any(|(t, _)| *t == term) {
                    spelled.push((term, at));
                }
            }
            terms.insert(name.to_string(), spelled);
        }
        terms
    }
}

/// The field `f` of the Taxonomy `name` as Terms: a list of strings, or one
/// string when the Taxonomy is `single`.
fn term_values(name: &str, taxonomy: &Taxonomy, f: &Node, cx: &mut Cx<'_>) -> Option<Vec<Value>> {
    match (&f.value, taxonomy.single) {
        (Yaml::Str(_), true) => fields::value(&STRING, f, name, cx).map(|v| vec![v]),
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
        (_, false) => {
            let list = FieldType::List {
                items: Box::new(STRING),
                unique: false,
            };
            match fields::value(&list, f, name, cx) {
                Some(Value::List(items)) => Some(items),
                _ => None,
            }
        }
    }
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
