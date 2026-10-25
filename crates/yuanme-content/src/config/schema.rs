//! What Site config may hold, in one table that both the checker and the
//! JSON Schema read, so the two can't disagree. Each key's `doc` is the
//! text an editor shows on hover and the docs show for it (#21).

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::{Collection, Config, DataSchema, Field, FieldType};

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
    /// One of these strings.
    Choice(&'static [&'static str]),
    /// An absolute `http` or `https` URL.
    Url,
    /// A URL path from the Site root. It must hold this placeholder, and no other.
    Route(Option<&'static str>),
    /// The oldest Engine version a project accepts: `"X.Y"`.
    Version,
    /// A browser's version: `"111"`, `"16.4"` or `"16.4.1"`.
    BrowserVersion,
    Int {
        min: i64,
        max: i64,
    },
    /// An integer or a float.
    Number,
    Bool,
    /// A Theorem kind's counter: its name, or `false` for none.
    Counter,
    Strings,
    /// An array of one shape, such as `[images] widths`.
    Array(&'static Shape),
    Table(&'static [Key]),
    /// An array of tables, as `[[host.headers]]` writes one.
    Tables(&'static [Key]),
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
    Key::optional(
        "build",
        Shape::Table(BUILD),
        "How yuanme builds the Site's stylesheets and Scripts.",
    ),
    Key::optional(
        "router",
        Shape::Table(ROUTER),
        "How a visitor moves between Pages.",
    ),
    Key::optional(
        "host",
        Shape::Table(HOST),
        "The static host the Site is deployed to, and the headers and redirects it sends.",
    ),
    Key::optional("math", Shape::Table(MATH), "How math renders."),
    Key::optional(
        "images",
        Shape::Table(IMAGES),
        "How yuanme resizes and encodes the images that bodies, `image` fields and `image()` name.",
    ),
];

const IMAGES: &[Key] = &[
    // WebP's limit, and far wider than any screen.
    Key::optional(
        "widths",
        Shape::Array(&Shape::Int { min: 1, max: 16383 }),
        "The widths each image is resized to, in pixels. A source narrower than one uses its own width instead: nothing is upscaled. Defaults to [480, 800, 1200, 1600, 2400].",
    ),
    Key::optional(
        "sizes",
        Shape::String,
        "The `sizes` attribute: how wide an image is shown, which picks the width a browser fetches. Defaults to \"100vw\"; set it to the prose width, such as \"(min-width: 768px) 720px, 100vw\".",
    ),
    Key::optional(
        "formats",
        Shape::Array(&Shape::Choice(&["avif", "webp"])),
        "The formats offered before the JPEG fallback, or PNG for a source with transparency, in order. Defaults to [\"avif\"], which every default target browser decodes; add \"webp\" for older ones.",
    ),
    Key::optional(
        "avif_speed",
        Shape::Int { min: 1, max: 10 },
        "How hard the AVIF encoder works, from 1, the smallest files and slowest, to 10. Defaults to 6.",
    ),
    Key::optional(
        "quality",
        Shape::Table(QUALITY),
        "Each encoder's quality, on its own scale from 1 to 100.",
    ),
    Key::optional(
        "allow_unoptimized",
        Shape::Array(&Shape::String),
        "Rasters over 50 KB that a Page may load as they are, by URL from the root, such as [\"/favicon.png\"]: `image/unoptimized` doesn't warn of them.",
    ),
];

const QUALITY: &[Key] = &[
    Key::optional("avif", Shape::Int { min: 1, max: 100 }, "Defaults to 72."),
    Key::optional("jpeg", Shape::Int { min: 1, max: 100 }, "Defaults to 80."),
    Key::optional("webp", Shape::Int { min: 1, max: 100 }, "Defaults to 80."),
];

const MATH: &[Key] = &[
    Key::optional(
        "macros",
        Shape::Map(&Shape::String),
        "LaTeX macros for every Entry's math, by name without the backslash: `R = '\\mathbb{R}'`, or with arguments, `norm = '\\left\\lVert #1 \\right\\rVert'`.",
    ),
    Key::optional(
        "theorems",
        Shape::Map(&Shape::Table(THEOREM)),
        "Theorem block kinds, by the name a block is written with: `[math.theorems.claim]` makes `:::claim` in Markdown and `#+begin_claim` in Org Theorem blocks. The built-in kinds are theorem, lemma, proposition, corollary, conjecture, definition, example and exercise, numbered on one shared counter, and remark, note and proof, unnumbered; a table here changes one of them or adds a kind.",
    ),
];

const THEOREM: &[Key] = &[
    Key::optional(
        "label",
        Shape::String,
        "What the kind's blocks are called on the Page, as in \"Claim 2\". Defaults to the built-in kind's, or to the name with its first letter capitalized.",
    ),
    Key::optional(
        "counter",
        Shape::Counter,
        "The counter the kind's blocks are numbered on, per Entry: \"theorem\", which the built-in numbered kinds share, a name of its own, or false for unnumbered blocks. Defaults to the built-in kind's, or to \"theorem\".",
    ),
];

const ROUTER: &[Key] = &[Key::optional(
    "mode",
    Shape::Choice(&["off", "native", "swap"]),
    "\"off\" (the default): every move between Pages is a full page load. \"native\": full page loads that the browser animates with a cross-document View Transition, with no JS. \"swap\": the Router on every Page, which swaps the next Page into the current document, so elements marked `data-yuanme-persist` survive; browsers without the Navigation API get full page loads.",
)];

const HOST: &[Key] = &[
    Key::optional(
        "adapter",
        Shape::Choice(&[
            "cloudflare",
            "netlify",
            "vercel",
            "github-pages",
            "cloudflare-pages",
            "static",
        ]),
        "The host whose configuration the build writes: \"cloudflare\" (Workers static assets, the default), \"netlify\", \"vercel\", \"github-pages\", \"cloudflare-pages\" or \"static\".",
    ),
    Key::optional(
        "precompress",
        Shape::Bool,
        "Write a brotli `.br` and a gzip `.gz` beside each text file, for Caddy or nginx. Only with the `static` adapter: no other host serves them.",
    ),
    Key::optional(
        "headers",
        Shape::Tables(HEADER_RULE),
        "Response headers for the URLs a path matches, after yuanme's own cache rules. Where two rules set one header, the later one wins.",
    ),
    Key::optional(
        "redirects",
        Shape::Tables(REDIRECT),
        "URLs the host redirects, such as a Page's old address.",
    ),
];

const HEADER_RULE: &[Key] = &[
    Key::required(
        "path",
        Shape::Route(None),
        "The URL path the headers are for, such as \"/resume.pdf\". One `*` matches any characters: \"/fonts/*\".",
    ),
    Key::required(
        "headers",
        Shape::Map(&Shape::String),
        "The headers, by name: { \"Cache-Control\" = \"public, max-age=3600\" }.",
    ),
];

const REDIRECT: &[Key] = &[
    Key::required(
        "from",
        Shape::Route(None),
        "The URL path that redirects, such as \"/old-post/\".",
    ),
    Key::required(
        "to",
        Shape::String,
        "Where it goes: a URL path such as \"/blog/new-post/\", or an absolute URL.",
    ),
    Key::optional(
        "status",
        Shape::Int {
            min: 301,
            max: i64::MAX,
        },
        "301, 302, 307 or 308. Defaults to 308, a permanent redirect that keeps the method.",
    ),
];

const BUILD: &[Key] = &[Key::optional(
    "targets",
    Shape::Table(TARGETS),
    "The oldest browsers the Site supports, as explicit versions such as { chrome = \"111\", safari = \"16.4\" }. yuanme adds the prefixes and fallbacks they need and drops the rest; a browser left out gets none. Defaults to Tailwind v4's baseline: Chrome 111, Firefox 128 and Safari 16.4.",
)];

/// The browsers `targets` names, which both the CSS and the JS pipeline know.
const TARGETS: &[Key] = &[
    Key::optional("chrome", Shape::BrowserVersion, "Chrome."),
    Key::optional("edge", Shape::BrowserVersion, "Edge."),
    Key::optional("firefox", Shape::BrowserVersion, "Firefox."),
    Key::optional(
        "safari",
        Shape::BrowserVersion,
        "Safari, on macOS and on iOS.",
    ),
    Key::optional("opera", Shape::BrowserVersion, "Opera."),
    Key::optional("samsung", Shape::BrowserVersion, "Samsung Internet."),
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
        Shape::Int {
            min: 1,
            max: i64::MAX,
        },
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
    Key::optional(
        "paginate",
        Shape::Int {
            min: 1,
            max: i64::MAX,
        },
        "Entries per Term page.",
    ),
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
            Key::optional(
                "min",
                Shape::Int {
                    min: 0,
                    max: i64::MAX,
                },
                "The fewest characters.",
            ),
            Key::optional(
                "max",
                Shape::Int {
                    min: 0,
                    max: i64::MAX,
                },
                "The most characters.",
            ),
        ],
    },
    FieldTypeDoc {
        name: "int",
        doc: "A whole number.",
        keys: &[
            Key::optional(
                "min",
                Shape::Int {
                    min: i64::MIN,
                    max: i64::MAX,
                },
                "The smallest value.",
            ),
            Key::optional(
                "max",
                Shape::Int {
                    min: i64::MIN,
                    max: i64::MAX,
                },
                "The largest value.",
            ),
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
    // Sorted, as without serde_json's `preserve_order`, which a dependency
    // (oxc_resolver) may turn on for the whole build.
    let mut schema = Value::Object(head);
    schema.sort_all_objects();
    schema
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
        Shape::Choice(choices) => json!({ "enum": choices }),
        Shape::Url => json!({ "type": "string", "format": "uri", "pattern": "^https?://[^/\\s]+" }),
        Shape::Route(Some(placeholder)) => {
            let placeholder = placeholder.replace('{', "\\{").replace('}', "\\}");
            json!({ "type": "string", "pattern": format!("^/[^{{}}]*{placeholder}[^{{}}]*$") })
        }
        Shape::Route(None) => json!({ "type": "string", "pattern": "^/[^{}]*$" }),
        Shape::Version => json!({ "type": "string", "pattern": "^[0-9]+\\.[0-9]+$" }),
        Shape::BrowserVersion => {
            json!({ "type": "string", "pattern": "^[0-9]+(\\.[0-9]+){0,2}$" })
        }
        Shape::Int { min, max } => {
            let mut schema = json!({ "type": "integer" });
            if *min != i64::MIN {
                schema["minimum"] = json!(min);
            }
            if *max != i64::MAX {
                schema["maximum"] = json!(max);
            }
            schema
        }
        Shape::Array(item) => json!({ "type": "array", "items": self::shape(item) }),
        Shape::Number => json!({ "type": "number" }),
        Shape::Bool => json!({ "type": "boolean" }),
        Shape::Counter => json!({ "anyOf": [{ "type": "string" }, { "const": false }] }),
        Shape::Strings => json!({ "type": "array", "items": { "type": "string" } }),
        Shape::Table(keys) => table(keys.iter()),
        Shape::Tables(keys) => json!({ "type": "array", "items": table(keys.iter()) }),
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

/// The draft 2020-12 JSON Schema of the front matter of `collection`'s
/// Entries, named `name` (#21): the fields every Entry shares, its declared
/// fields and its Taxonomies. Org keywords set the same fields.
pub fn entry_schema(config: &Config, name: &str, collection: &Collection) -> Value {
    let string = FieldType::String {
        min: None,
        max: None,
    };
    let built_in = |kind: FieldType, optional: bool, doc: &str| Field {
        kind,
        optional,
        default: None,
        description: Some(doc.into()),
    };
    let mut fields = BTreeMap::from([
        (
            "title".to_string(),
            built_in(string.clone(), false, "The Entry's title."),
        ),
        (
            "date".into(),
            built_in(FieldType::Date, true, "When the Entry was published."),
        ),
        (
            "updated".into(),
            built_in(FieldType::Date, true, "When the Entry was last changed."),
        ),
        (
            "slug".into(),
            built_in(
                string,
                true,
                "Its URL-safe name, which defaults to one from the file's path.",
            ),
        ),
        (
            "draft".into(),
            built_in(
                FieldType::Bool,
                true,
                "Shown while previewing, absent from the published Site.",
            ),
        ),
        (
            "unlisted".into(),
            built_in(
                FieldType::Bool,
                true,
                "Published, but left out of every list, Taxonomy, feed and sitemap.",
            ),
        ),
    ]);
    for (field, f) in &collection.fields {
        // A declared `date` only makes the built-in required.
        match fields.get_mut(field) {
            Some(date) => date.optional = f.optional,
            None => {
                fields.insert(field.clone(), f.clone());
            }
        }
    }
    let mut schema = object(&fields);
    for (t, taxonomy) in &config.taxonomies {
        if !taxonomy.collections.iter().any(|c| c == name) {
            continue;
        }
        let terms = match taxonomy.single {
            true => json!({ "type": "string", "description": format!("Its Term of `{t}`.") }),
            false => json!({
                "type": "array",
                "items": { "type": "string" },
                "description": format!("Its Terms of `{t}`."),
            }),
        };
        schema["properties"][t] = terms;
    }
    head(schema, &format!("Front matter of `{name}` Entries."))
}

/// The draft 2020-12 JSON Schema of the Data file `[data.<name>]` declares.
pub fn data_schema(name: &str, data: &DataSchema) -> Value {
    head(field_type(&data.kind), &format!("The Data file `{name}`."))
}

fn head(mut schema: Value, description: &str) -> Value {
    schema["$schema"] = "https://json-schema.org/draft/2020-12/schema".into();
    schema["description"] = description.into();
    schema.sort_all_objects();
    schema
}

/// A closed object: unknown keys are errors (#18).
fn object(fields: &BTreeMap<String, Field>) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for (name, f) in fields {
        let mut property = field_type(&f.kind);
        if let Some(doc) = &f.description {
            property["description"] = doc.as_str().into();
        }
        if let Some(default) = &f.default {
            property["default"] = toml_json(default);
        }
        if !f.optional {
            required.push(name.as_str());
        }
        properties.insert(name.clone(), property);
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn field_type(kind: &FieldType) -> Value {
    let bounds =
        |schema: &mut Value, lo: &str, hi: &str, min: Option<Value>, max: Option<Value>| {
            if let Some(min) = min {
                schema[lo] = min;
            }
            if let Some(max) = max {
                schema[hi] = max;
            }
        };
    match kind {
        FieldType::String { min, max } => {
            let mut s = json!({ "type": "string" });
            bounds(
                &mut s,
                "minLength",
                "maxLength",
                min.map(Value::from),
                max.map(Value::from),
            );
            s
        }
        FieldType::Int { min, max } => {
            let mut s = json!({ "type": "integer" });
            bounds(
                &mut s,
                "minimum",
                "maximum",
                min.map(Value::from),
                max.map(Value::from),
            );
            s
        }
        FieldType::Float { min, max } => {
            let mut s = json!({ "type": "number" });
            bounds(
                &mut s,
                "minimum",
                "maximum",
                min.map(Value::from),
                max.map(Value::from),
            );
            s
        }
        FieldType::Bool => json!({ "type": "boolean" }),
        FieldType::Date => json!({ "type": "string", "format": "date" }),
        // Without an offset it is in `site.timezone`, which `date-time` forbids.
        FieldType::Datetime => json!({ "type": "string" }),
        FieldType::Url => shape(&Shape::Url),
        FieldType::Image => json!({ "type": "string" }),
        FieldType::Enum(values) => json!({ "enum": values }),
        FieldType::List { items, unique } => {
            json!({ "type": "array", "items": field_type(items), "uniqueItems": unique })
        }
        FieldType::Object(fields) => object(fields),
    }
}

fn toml_json(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(s) => s.as_str().into(),
        toml::Value::Integer(i) => (*i).into(),
        toml::Value::Float(f) => (*f).into(),
        toml::Value::Boolean(b) => (*b).into(),
        toml::Value::Datetime(d) => d.to_string().into(),
        toml::Value::Array(items) => items.iter().map(toml_json).collect(),
        toml::Value::Table(t) => t.iter().map(|(k, v)| (k.clone(), toml_json(v))).collect(),
    }
}
