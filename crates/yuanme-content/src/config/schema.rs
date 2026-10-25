//! What Site config may hold, in one table that both the checker and the
//! JSON Schema read, so the two can't disagree. Each key's `doc` is the
//! text an editor shows on hover and the docs show for it (#21).

use serde_json::{Map, Value, json};

pub(crate) struct Key {
    pub name: &'static str,
    pub shape: Shape,
    pub required: bool,
    pub doc: &'static str,
    /// For a deprecated key, the key that replaced it, whose shape it takes.
    pub renamed_to: Option<&'static str>,
}

impl Key {
    pub const fn required(name: &'static str, shape: Shape, doc: &'static str) -> Key {
        Key {
            name,
            shape,
            required: true,
            doc,
            renamed_to: None,
        }
    }

    pub const fn optional(name: &'static str, shape: Shape, doc: &'static str) -> Key {
        Key {
            name,
            shape,
            required: false,
            doc,
            renamed_to: None,
        }
    }

    /// A key that still works under its old name until the next major
    /// release, with a `config/deprecated` warning (ADR 0015).
    #[cfg_attr(not(test), expect(dead_code, reason = "no key has been renamed yet"))]
    pub const fn renamed(name: &'static str, to: &'static str) -> Key {
        Key {
            name,
            shape: Shape::Any,
            required: false,
            doc: "",
            renamed_to: Some(to),
        }
    }
}

pub(crate) enum Shape {
    String,
    /// An absolute `http` or `https` URL.
    Url,
    /// A URL path from the Site root. It must hold this placeholder, and no other.
    Route(Option<&'static str>),
    /// The oldest Engine version a project accepts: `"X.Y"`.
    Version,
    Int {
        min: i64,
    },
    /// An integer or a float.
    Number,
    Bool,
    Strings,
    Table(&'static [Key]),
    /// A table whose keys the Author names, such as Collections.
    Map(&'static Shape),
    /// A table that takes any keys: `[site.extra]`.
    Free,
    /// Any TOML value: a field's `default`, checked against the field's type.
    Any,
    /// A field's schema. Only a field of an Entry or object can be optional
    /// or have a default; list items and Data files can't.
    Field {
        in_table: bool,
    },
}

pub(crate) const ROOT: &[Key] = &[
    Key::optional(
        "yuanme",
        Shape::Version,
        "The oldest yuanme release this project works with, as \"X.Y\". A newer minor release in the same major builds it; before 1.0, only the same 0.Y does.",
    ),
    Key::required("site", Shape::Table(SITE), "The Site's metadata."),
    Key::optional(
        "collections",
        Shape::Map(&Shape::Table(COLLECTION)),
        "Named groups of Entries sharing one schema, such as posts or projects.",
    ),
    Key::optional(
        "taxonomies",
        Shape::Map(&Shape::Table(TAXONOMY)),
        "Named ways of grouping Entries by a shared value, such as tags or series.",
    ),
    Key::optional(
        "data",
        Shape::Map(&Shape::Field { in_table: false }),
        "Schemas for Data files: `[data.nav]` checks `data/nav.toml`, `.yaml` or `.json`. A Data file without one is not checked.",
    ),
    Key::optional(
        "routes",
        Shape::Map(&Shape::Table(ROUTE)),
        "Pages made by a Template alone, such as the home page or the 404 page.",
    ),
    Key::optional(
        "feeds",
        Shape::Map(&Shape::Table(FEED)),
        "Feeds of published, listed Entries, newest first.",
    ),
];

const ROUTE: &[Key] = &[
    Key::required(
        "path",
        Shape::Route(None),
        "The URL of the Page, such as \"/\" or \"/404.html\".",
    ),
    Key::required(
        "template",
        Shape::String,
        "The Template that makes the Page, by its path in `templates/`, such as \"home.html\".",
    ),
];

const FEED: &[Key] = &[
    Key::required(
        "collections",
        Shape::Strings,
        "The Collections whose Entries the feed holds.",
    ),
    Key::required(
        "path",
        Shape::Route(None),
        "The URL of the feed, such as \"/rss.xml\".",
    ),
    Key::optional(
        "template",
        Shape::String,
        "The Template that writes the feed. Defaults to \"rss.xml\": the built-in RSS 2.0 Template, unless `templates/rss.xml` replaces it.",
    ),
];

const SITE: &[Key] = &[
    Key::required(
        "url",
        Shape::Url,
        "The address the Site is served from, such as \"https://example.com\". Feeds, sitemaps and canonical links are built from it.",
    ),
    Key::optional("title", Shape::String, "The Site's name."),
    Key::optional(
        "description",
        Shape::String,
        "One or two sentences about the Site, for feeds and search engines.",
    ),
    Key::optional(
        "language",
        Shape::String,
        "The language of the Site's content, as a BCP 47 tag such as \"en\" or \"zh-Hans\".",
    ),
    Key::optional("author", Shape::String, "The Author's name."),
    Key::optional(
        "timezone",
        Shape::String,
        "The IANA time zone, such as \"Asia/Singapore\", for datetimes written without an offset. Defaults to \"UTC\", never the machine's zone.",
    ),
    Key::optional(
        "extra",
        Shape::Free,
        "Anything else Templates need, under keys of your own.",
    ),
];

const COLLECTION: &[Key] = &[
    Key::optional(
        "dir",
        Shape::String,
        "The folder of the Collection's Entries. Defaults to \"content/<name>\".",
    ),
    Key::required(
        "route",
        Shape::Route(Some("{slug}")),
        "The URL of each Entry, with `{slug}` where its Slug goes, such as \"/blog/{slug}/\".",
    ),
    Key::optional(
        "list_route",
        Shape::Route(None),
        "The URL of the page listing the Collection, such as \"/blog/\". Page N ≥ 2 is at \"<list_route>N/\".",
    ),
    Key::optional(
        "paginate",
        Shape::Int { min: 1 },
        "Entries per list page. Needs `list_route`.",
    ),
    Key::optional(
        "sort",
        Shape::Strings,
        "Field names to sort Entries by, each prefixed with `-` for descending. Entries missing a key sort last; ties break by Slug. Defaults to [\"-date\"].",
    ),
    Key::optional(
        "fields",
        Shape::Map(&Shape::Field { in_table: true }),
        "Fields Entries have beyond the built-in `title`, `date`, `updated`, `slug`, `draft` and `unlisted`. Declaring `date` makes it required.",
    ),
    Key::optional(
        "template",
        Shape::String,
        "The Template of each Entry's Page. Defaults to \"<name>/entry.html\".",
    ),
];

const TAXONOMY: &[Key] = &[
    Key::required(
        "collections",
        Shape::Strings,
        "The Collections whose Entries it groups. Each gets a field with the Taxonomy's name.",
    ),
    Key::required(
        "route",
        Shape::Route(Some("{term}")),
        "The URL of each Term's page, with `{term}` where its Slug goes, such as \"/tags/{term}/\".",
    ),
    Key::optional("paginate", Shape::Int { min: 1 }, "Entries per Term page."),
    Key::optional(
        "single",
        Shape::Bool,
        "Each Entry has at most one Term, written as a string rather than a list.",
    ),
    Key::optional(
        "sort",
        Shape::Strings,
        "How Entries sort on a Term's page, as for a Collection.",
    ),
    Key::optional(
        "lowercase",
        Shape::Bool,
        "Show every Term in lowercase. Otherwise a Term shows as first spelled.",
    ),
    Key::optional(
        "slugs",
        Shape::Map(&Shape::String),
        "Slugs that override the ones yuanme derives, by Term: { \"C++\" = \"cpp\" }.",
    ),
    Key::optional(
        "template",
        Shape::String,
        "The Template of each Term's Page. Defaults to \"<name>/term.html\".",
    ),
];

/// A field type and the keys only it takes.
pub(crate) struct FieldTypeDoc {
    pub name: &'static str,
    pub doc: &'static str,
    pub keys: &'static [Key],
}

pub(crate) const FIELD_TYPES: &[FieldTypeDoc] = &[
    FieldTypeDoc {
        name: "string",
        doc: "Text.",
        keys: &[
            Key::optional("min", Shape::Int { min: 0 }, "The fewest characters."),
            Key::optional("max", Shape::Int { min: 0 }, "The most characters."),
        ],
    },
    FieldTypeDoc {
        name: "int",
        doc: "A whole number.",
        keys: &[
            Key::optional("min", Shape::Int { min: i64::MIN }, "The smallest value."),
            Key::optional("max", Shape::Int { min: i64::MIN }, "The largest value."),
        ],
    },
    FieldTypeDoc {
        name: "float",
        doc: "A number.",
        keys: &[
            Key::optional("min", Shape::Number, "The smallest value."),
            Key::optional("max", Shape::Number, "The largest value."),
        ],
    },
    FieldTypeDoc {
        name: "bool",
        doc: "true or false.",
        keys: &[],
    },
    FieldTypeDoc {
        name: "date",
        doc: "A calendar date, such as 2025-12-12.",
        keys: &[],
    },
    FieldTypeDoc {
        name: "datetime",
        doc: "A date and time. Without an offset it is in `site.timezone`.",
        keys: &[],
    },
    FieldTypeDoc {
        name: "url",
        doc: "An absolute URL.",
        keys: &[],
    },
    FieldTypeDoc {
        name: "enum",
        doc: "One of a fixed set of strings.",
        keys: &[Key::required(
            "values",
            Shape::Strings,
            "The strings allowed.",
        )],
    },
    FieldTypeDoc {
        name: "list",
        doc: "A list of values of one type.",
        keys: &[
            Key::required(
                "items",
                Shape::Field { in_table: false },
                "The type of each item.",
            ),
            Key::optional("unique", Shape::Bool, "No item may appear twice."),
        ],
    },
    FieldTypeDoc {
        name: "object",
        doc: "A table with named fields.",
        keys: &[Key::required(
            "fields",
            Shape::Map(&Shape::Field { in_table: true }),
            "The object's fields.",
        )],
    },
    FieldTypeDoc {
        name: "image",
        doc: "A path to an image beside the Entry or in the project, or a remote URL.",
        keys: &[],
    },
];

const TYPE: Key = Key::required("type", Shape::String, "The field's type.");
const DESCRIPTION: Key = Key::optional("description", Shape::String, "What the field is for.");
const TABLE_FIELD_KEYS: &[Key] = &[
    TYPE,
    DESCRIPTION,
    Key::optional("optional", Shape::Bool, "Entries may leave the field out."),
    Key::optional(
        "default",
        Shape::Any,
        "The value an Entry that leaves the field out gets. A field with a default is optional.",
    ),
];
const ITEM_KEYS: &[Key] = &[TYPE, DESCRIPTION];

/// The keys every field takes, whatever its type.
pub(crate) fn common_keys(in_table: bool) -> &'static [Key] {
    if in_table {
        TABLE_FIELD_KEYS
    } else {
        ITEM_KEYS
    }
}

/// The draft 2020-12 JSON Schema for `yuanme.toml`, written to
/// `.yuanme/schemas/yuanme.toml.json` for editors and agents (#21).
pub fn json_schema() -> Value {
    let mut schema = table(ROOT.iter());
    let schema_map = schema.as_object_mut().expect("a table is an object");
    let mut head = Map::new();
    head.insert(
        "$schema".into(),
        "https://json-schema.org/draft/2020-12/schema".into(),
    );
    head.insert("title".into(), "yuanme.toml".into());
    head.insert("description".into(), "Site config for yuanme.".into());
    head.append(schema_map);
    head.insert(
        "$defs".into(),
        json!({ "field": field(true), "item": field(false) }),
    );
    Value::Object(head)
}

fn table<'a>(keys: impl IntoIterator<Item = &'a Key>) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for key in keys {
        let property = match key.renamed_to {
            Some(to) => json!({ "deprecated": true, "description": format!("Renamed to `{to}`.") }),
            None => {
                let mut property = shape(&key.shape);
                property["description"] = key.doc.into();
                property
            }
        };
        properties.insert(key.name.into(), property);
        if key.required {
            required.push(key.name);
        }
    }
    let mut table =
        json!({ "type": "object", "properties": properties, "additionalProperties": false });
    if !required.is_empty() {
        table["required"] = required.into();
    }
    table
}

fn shape(shape: &Shape) -> Value {
    match shape {
        Shape::String => json!({ "type": "string" }),
        Shape::Url => json!({ "type": "string", "format": "uri", "pattern": "^https?://[^/\\s]+" }),
        Shape::Route(Some(placeholder)) => {
            let placeholder = placeholder.replace('{', "\\{").replace('}', "\\}");
            json!({ "type": "string", "pattern": format!("^/[^{{}}]*{placeholder}[^{{}}]*$") })
        }
        Shape::Route(None) => json!({ "type": "string", "pattern": "^/[^{}]*$" }),
        Shape::Version => json!({ "type": "string", "pattern": "^[0-9]+\\.[0-9]+$" }),
        Shape::Int { min } if *min == i64::MIN => json!({ "type": "integer" }),
        Shape::Int { min } => json!({ "type": "integer", "minimum": min }),
        Shape::Number => json!({ "type": "number" }),
        Shape::Bool => json!({ "type": "boolean" }),
        Shape::Strings => json!({ "type": "array", "items": { "type": "string" } }),
        Shape::Table(keys) => table(keys.iter()),
        Shape::Map(value) => {
            json!({ "type": "object", "additionalProperties": self::shape(value) })
        }
        Shape::Free => json!({ "type": "object" }),
        Shape::Any => json!({}),
        Shape::Field { in_table: true } => json!({ "$ref": "#/$defs/field" }),
        Shape::Field { in_table: false } => json!({ "$ref": "#/$defs/item" }),
    }
}

/// One schema per field type, so an editor offers only the keys that type takes.
fn field(in_table: bool) -> Value {
    let types: Vec<_> = FIELD_TYPES
        .iter()
        .map(|ty| {
            let mut schema = table(common_keys(in_table).iter().chain(ty.keys));
            schema["description"] = ty.doc.into();
            schema["properties"]["type"] =
                json!({ "const": ty.name, "description": "The field's type." });
            schema
        })
        .collect();
    json!({ "oneOf": types })
}
