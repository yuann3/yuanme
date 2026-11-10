//! What Templates see, as types (#19), and as the JSON Schema
//! `yuanme schema` writes (#21).

use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{Value, json};
use yuanme_content::config::{Config, Field as Declared, FieldType};
use yuanme_content::fields;
use yuanme_content::site::{self, RouteKind, Site};

use super::types::{Field, Obj, Ty, closed, list_of, merge, obj};

/// The types of what Templates see, from Site config and the Site (#19).
pub(super) struct Schema {
    entries: BTreeMap<String, Ty>,
    term: Ty,
    taxonomy: Ty,
    paginator: Ty,
    collection: Ty,
    feed: Ty,
    pub(super) globals: Vec<(&'static str, Ty)>,
}

impl Schema {
    pub(super) fn new(config: &Config, site: &Site) -> Schema {
        let collections = site::collections(config);
        let mut entries = BTreeMap::new();
        for (name, c) in &collections {
            let mut fields: BTreeMap<String, Field> = c
                .fields
                .iter()
                .map(|(k, f)| (k.clone(), declared(&format!("entry<{name}>.{k}"), f)))
                .collect();
            let dated = fields.get("date").is_some_and(|f| !f.optional);
            let heading = obj(
                "heading",
                [
                    ("id", Ty::Str, false),
                    ("text", Ty::Str, false),
                    ("level", Ty::Int, false),
                ],
            );
            for (k, ty, optional) in [
                ("title", Ty::Str, false),
                ("slug", Ty::Str, false),
                ("draft", Ty::Bool, false),
                ("unlisted", Ty::Bool, false),
                ("date", Ty::Date, !dated),
                ("updated", Ty::Date, true),
                ("modified", Ty::Date, !dated),
                ("url", Ty::Url, false),
                ("content", Ty::Slot, false),
                ("headings", Ty::List(Rc::new(heading)), false),
                ("word_count", Ty::Int, false),
                ("reading_time", Ty::Int, false),
            ] {
                fields.insert(k.into(), Field { ty, optional });
            }
            for (t, taxonomy) in &config.taxonomies {
                if taxonomy.collections.contains(name) {
                    let field = match taxonomy.single {
                        true => Field {
                            ty: Ty::Str,
                            optional: true,
                        },
                        false => Field {
                            ty: Ty::List(Rc::new(Ty::Str)),
                            optional: false,
                        },
                    };
                    fields.insert(t.clone(), field);
                }
            }
            let entry = Ty::Obj(Rc::new(Obj {
                name: format!("entry<{name}>"),
                fields,
                open: false,
            }));
            entries.insert(name.clone(), entry);
        }
        let term = obj(
            "term",
            [
                ("name", Ty::Str, false),
                ("slug", Ty::Str, false),
                ("url", Ty::Url, false),
                ("count", Ty::Int, false),
            ],
        );
        let taxonomy = obj(
            "taxonomy",
            [
                ("name", Ty::Str, false),
                ("url", Ty::Url, false),
                ("terms", Ty::List(Rc::new(term.clone())), false),
            ],
        );
        let paginator = obj(
            "paginator",
            [
                ("number", Ty::Int, false),
                ("total", Ty::Int, false),
                ("prev", Ty::Url, true),
                ("next", Ty::Url, true),
            ],
        );
        let named =
            |name: &'static str| obj(name, [("name", Ty::Str, false), ("url", Ty::Url, false)]);
        let s = &config.site;
        let mut site_fields = vec![
            ("url", Ty::Url, false),
            ("timezone", Ty::Str, false),
            (
                "extra",
                toml("site.extra", &toml::Value::Table(s.extra.clone())),
                false,
            ),
        ];
        for (k, v) in [
            ("title", &s.title),
            ("description", &s.description),
            ("language", &s.language),
            ("author", &s.author),
        ] {
            site_fields.push((k, Ty::Str, v.is_none()));
        }
        let data = closed(
            "data",
            site.data.iter().map(|(name, v)| {
                let path = format!("data.{name}");
                let ty = match config.data.get(name) {
                    Some(schema) => field_ty(&path, &schema.kind),
                    None => infer(&path, v),
                };
                (name, ty)
            }),
        );
        let collections_ty = closed(
            "collections",
            collections
                .keys()
                .map(|n| (n, Ty::List(Rc::new(entries[n].clone())))),
        );
        let taxonomies_ty = closed(
            "taxonomies",
            config.taxonomies.keys().map(|n| (n, taxonomy.clone())),
        );
        let kinds = ["entry", "list", "term", "index", "template", "feed"];
        let page = obj(
            "page",
            [
                ("url", Ty::Url, false),
                ("canonical", Ty::Url, false),
                (
                    "kind",
                    Ty::Enum(Rc::new(kinds.map(String::from).to_vec())),
                    false,
                ),
            ],
        );
        let globals = vec![
            ("site", obj("site", site_fields)),
            ("data", data),
            ("collections", collections_ty),
            ("taxonomies", taxonomies_ty),
            ("page", page),
            ("yuanme", obj("yuanme", [("version", Ty::Str, false)])),
        ];
        Schema {
            entries,
            term,
            taxonomy,
            paginator,
            collection: named("collection"),
            feed: named("feed"),
            globals,
        }
    }

    /// The types `props` and `vars` comments can name besides #18's.
    pub(super) fn named(&self) -> BTreeMap<String, Ty> {
        let mut named: BTreeMap<String, Ty> = self
            .entries
            .iter()
            .map(|(c, ty)| (format!("entry<{c}>"), ty.clone()))
            .collect();
        named.insert("term".into(), self.term.clone());
        named.insert("paginator".into(), self.paginator.clone());
        named
    }

    /// What a Route's Page sees besides the globals: its kind, how a
    /// Diagnostic names it, and its values.
    pub(super) fn route(
        &self,
        config: &Config,
        site: &Site,
        kind: &RouteKind,
    ) -> (&'static str, String, Vec<(&'static str, Ty)>) {
        let list = |ty: &Ty| Ty::List(Rc::new(ty.clone()));
        // A Taxonomy or feed over several Collections lists Entries of each.
        let of = |collections: &[String]| {
            let mut tys = collections.iter().map(|c| self.entries[c].clone());
            let first = tys.next().unwrap_or(Ty::Any);
            list(&tys.fold(first, |a, b| merge(&a, &b)))
        };
        match kind {
            RouteKind::Entry(i) => {
                let c = &site.entries[*i].collection;
                (
                    "entry",
                    format!("an Entry of `{c}`"),
                    vec![("entry", self.entries[c].clone())],
                )
            }
            RouteKind::List { collection, .. } => (
                "list",
                format!("the `{collection}` list"),
                vec![
                    ("collection", self.collection.clone()),
                    ("entries", list(&self.entries[collection])),
                    ("paginator", self.paginator.clone()),
                ],
            ),
            RouteKind::Term { taxonomy, .. } => (
                "term",
                format!("a Term of `{taxonomy}`"),
                vec![
                    ("taxonomy", self.taxonomy.clone()),
                    ("term", self.term.clone()),
                    ("entries", of(&config.taxonomies[taxonomy].collections)),
                    ("paginator", self.paginator.clone()),
                ],
            ),
            RouteKind::Terms { taxonomy } => (
                "index",
                format!("the `{taxonomy}` index"),
                vec![
                    ("taxonomy", self.taxonomy.clone()),
                    ("terms", list(&self.term)),
                ],
            ),
            RouteKind::Template(name) => ("template", format!("the Route `{name}`"), Vec::new()),
            RouteKind::Feed { name, .. } => (
                "feed",
                format!("the feed `{name}`"),
                vec![
                    ("feed", self.feed.clone()),
                    ("entries", of(&config.feeds[name].collections)),
                ],
            ),
        }
    }
}

/// The draft 2020-12 JSON Schema of what each kind of Route's Template
/// sees, by the name `page.kind` gives the kind: the schema `check` checks
/// against (#21). A kind several Collections, Taxonomies or feeds make is
/// any of their contexts.
pub fn context_schemas(config: &Config, site: &Site) -> BTreeMap<&'static str, Value> {
    let schema = Schema::new(config, site);
    let collections = site::collections(config);
    let mut variants: BTreeMap<&'static str, Vec<Vec<(&'static str, Ty)>>> = BTreeMap::new();
    for ty in schema.entries.values() {
        variants
            .entry("entry")
            .or_default()
            .push(vec![("entry", ty.clone())]);
    }
    // Only Entry Routes name an Entry by its index; for the others, the
    // pages and Entries a RouteKind holds don't change what is seen.
    let mut kinds = vec![RouteKind::Template(String::new())];
    for (name, c) in &collections {
        if c.list_route.is_some() {
            kinds.push(RouteKind::List {
                collection: name.clone(),
                page: 1,
                entries: Vec::new(),
            });
        }
    }
    for taxonomy in config.taxonomies.keys() {
        kinds.push(RouteKind::Term {
            taxonomy: taxonomy.clone(),
            term: 0,
            page: 1,
            entries: Vec::new(),
        });
        kinds.push(RouteKind::Terms {
            taxonomy: taxonomy.clone(),
        });
    }
    for name in config.feeds.keys() {
        kinds.push(RouteKind::Feed {
            name: name.clone(),
            entries: Vec::new(),
        });
    }
    for kind in &kinds {
        let (name, _, values) = schema.route(config, site, kind);
        variants.entry(name).or_default().push(values);
    }
    variants
        .into_iter()
        .map(|(kind, variants)| {
            let mut contexts: Vec<Value> = Vec::new();
            for values in variants {
                let fields = schema
                    .globals
                    .iter()
                    .chain(&values)
                    .map(|(k, ty)| (k.to_string(), ty.clone(), false));
                let context = object_schema(&format!("context of {kind}"), fields, false);
                if !contexts.contains(&context) {
                    contexts.push(context);
                }
            }
            let mut schema = match <[Value; 1]>::try_from(contexts) {
                Ok([one]) => one,
                Err(many) => json!({ "anyOf": many }),
            };
            schema["$schema"] = "https://json-schema.org/draft/2020-12/schema".into();
            schema.sort_all_objects();
            (kind, schema)
        })
        .collect()
}

fn ty_schema(ty: &Ty) -> Value {
    match ty {
        Ty::Any | Ty::Macro(_) | Ty::Module(_) => json!({}),
        Ty::None => json!({ "type": "null" }),
        Ty::Str | Ty::Image => json!({ "type": "string" }),
        Ty::Url => json!({ "type": "string", "format": "uri-reference" }),
        Ty::Int => json!({ "type": "integer" }),
        Ty::Float => json!({ "type": "number" }),
        Ty::Bool => json!({ "type": "boolean" }),
        Ty::Date => json!({ "type": "string", "format": "date" }),
        Ty::Datetime => json!({ "type": "string", "format": "date-time" }),
        Ty::Enum(values) => json!({ "enum": values.as_slice() }),
        Ty::List(item) => json!({ "type": "array", "items": ty_schema(item) }),
        Ty::Obj(o) => object_schema(
            &o.name,
            o.fields
                .iter()
                .map(|(k, f)| (k.clone(), f.ty.clone(), f.optional)),
            o.open,
        ),
        Ty::Slot => json!({ "type": "string", "description": "HTML" }),
        Ty::Attrs => json!({ "type": "object" }),
    }
}

/// `title` is the type's name as `check`'s messages and props comments
/// write it, such as `entry<posts>`.
fn object_schema(
    title: &str,
    fields: impl Iterator<Item = (String, Ty, bool)>,
    open: bool,
) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, ty, optional) in fields {
        if !optional {
            required.push(name.clone());
        }
        properties.insert(name, ty_schema(&ty));
    }
    json!({
        "title": title,
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": open,
    })
}

/// A declared field, which a Template finds absent only when it has no
/// default.
fn declared(path: &str, f: &Declared) -> Field {
    Field {
        ty: field_ty(path, &f.kind),
        optional: f.optional && f.default.is_none(),
    }
}

fn field_ty(path: &str, kind: &FieldType) -> Ty {
    match kind {
        FieldType::String { .. } => Ty::Str,
        FieldType::Int { .. } => Ty::Int,
        FieldType::Float { .. } => Ty::Float,
        FieldType::Bool => Ty::Bool,
        FieldType::Date => Ty::Date,
        FieldType::Datetime => Ty::Datetime,
        FieldType::Url => Ty::Url,
        FieldType::Image => Ty::Image,
        FieldType::Enum(values) => Ty::Enum(Rc::new(values.clone())),
        FieldType::List { items, .. } => Ty::List(Rc::new(field_ty(path, items))),
        FieldType::Object(fields) => Ty::Obj(Rc::new(Obj {
            name: path.into(),
            fields: fields
                .iter()
                .map(|(k, f)| (k.clone(), declared(&format!("{path}.{k}"), f)))
                .collect(),
            open: false,
        })),
    }
}

/// The type of a Data file with no schema, from the values in it (#19).
fn infer(path: &str, v: &fields::Value) -> Ty {
    use fields::Value as V;
    match v {
        V::String(_) => Ty::Str,
        V::Url(_) => Ty::Url,
        V::Int(_) => Ty::Int,
        V::Float(_) => Ty::Float,
        V::Bool(_) => Ty::Bool,
        V::Date(_) => Ty::Date,
        V::Datetime(_) => Ty::Datetime,
        V::Image(_) => Ty::Image,
        V::List(items) => list_of(items.iter().map(|i| infer(&format!("{path}[]"), i))),
        V::Object(o) => closed(
            path,
            o.iter().map(|(k, v)| (k, infer(&format!("{path}.{k}"), v))),
        ),
    }
}

/// `[site.extra]`, from the values in it.
fn toml(path: &str, v: &toml::Value) -> Ty {
    match v {
        toml::Value::String(_) | toml::Value::Datetime(_) => Ty::Str,
        toml::Value::Integer(_) => Ty::Int,
        toml::Value::Float(_) => Ty::Float,
        toml::Value::Boolean(_) => Ty::Bool,
        toml::Value::Array(items) => list_of(items.iter().map(|i| toml(&format!("{path}[]"), i))),
        toml::Value::Table(t) => closed(
            path,
            t.iter().map(|(k, v)| (k, toml(&format!("{path}.{k}"), v))),
        ),
    }
}
