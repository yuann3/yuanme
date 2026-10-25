//! `--json`'s NDJSON events (#21): one object per line on stdout, each
//! with a `type`. Their schema is public API (ADR 0015): a field may be
//! added, and nothing renamed or removed before 2.0, so the schema leaves
//! objects open to the fields a later release adds.

use serde_json::{Value, json};
use yuanme_diag::{Diagnostic, Files};

pub enum Event<'a> {
    Diagnostic(&'a Diagnostic, &'a Files),
    BuildStarted {
        command: &'static str,
    },
    BuildFinished {
        ok: bool,
        duration_ms: u128,
        /// Routes built, or that would be.
        pages: usize,
        changed_urls: Vec<String>,
        errors: usize,
        warnings: usize,
    },
    Route(Route),
    EntryCreated {
        path: String,
        url: String,
        collection: String,
        /// Required fields `new` couldn't give a valid value, such as an
        /// image, which needs a file.
        unfilled: Vec<String>,
    },
    Schema {
        name: String,
        schema: Value,
    },
}

/// One Route and what produces it, for `yuanme routes`.
pub struct Route {
    pub url: String,
    pub output: String,
    /// As `page.kind` names it: entry, list, term, index, template or feed.
    pub kind: &'static str,
    pub template: String,
    /// The Entry file, for an Entry's Route.
    pub entry: Option<String>,
    /// The Collection of an Entry or list.
    pub collection: Option<String>,
    pub taxonomy: Option<String>,
    pub term: Option<String>,
    /// The page of a list or Term, from 1.
    pub page: Option<usize>,
    /// The name Site config gives a Template's Route or a feed.
    pub name: Option<String>,
}

impl Event<'_> {
    pub fn to_json(&self) -> Value {
        let mut json = match self {
            Event::Diagnostic(d, files) => return d.to_json(files),
            Event::BuildStarted { command } => {
                json!({ "type": "build-started", "command": command })
            }
            Event::BuildFinished {
                ok,
                duration_ms,
                pages,
                changed_urls,
                errors,
                warnings,
            } => json!({
                "type": "build-finished",
                "ok": ok,
                "duration_ms": duration_ms,
                "pages": pages,
                "changed_urls": changed_urls,
                "errors": errors,
                "warnings": warnings,
            }),
            Event::Route(r) => {
                let mut json = json!({
                    "type": "route",
                    "url": r.url,
                    "output": r.output,
                    "kind": r.kind,
                    "template": r.template,
                });
                for (key, value) in [
                    ("entry", r.entry.clone().map(Value::from)),
                    ("collection", r.collection.clone().map(Value::from)),
                    ("taxonomy", r.taxonomy.clone().map(Value::from)),
                    ("term", r.term.clone().map(Value::from)),
                    ("page", r.page.map(Value::from)),
                    ("name", r.name.clone().map(Value::from)),
                ] {
                    if let Some(value) = value {
                        json[key] = value;
                    }
                }
                json
            }
            Event::EntryCreated {
                path,
                url,
                collection,
                unfilled,
            } => json!({
                "type": "entry-created",
                "path": path,
                "url": url,
                "collection": collection,
                "unfilled": unfilled,
            }),
            Event::Schema { name, schema } => {
                json!({ "type": "schema", "name": name, "schema": schema })
            }
        };
        json.sort_all_objects();
        json
    }
}

/// The draft 2020-12 JSON Schema of every event, `yuanme schema events`.
/// `budget`, `server-ready`, `reload` and `browser-console` are #21's, for
/// the budgets (#66) and the dev server (#67) to emit.
pub fn schema() -> Value {
    let string = json!({ "type": "string" });
    let strings = json!({ "type": "array", "items": { "type": "string" } });
    let count = json!({ "type": "integer", "minimum": 0 });
    let position = json!({ "type": "integer", "minimum": 1 });
    let span = event(
        None,
        "A place in a file. Paths are from the project root; lines and columns are 1-based, and columns count Unicode scalar values.",
        &[
            ("file", string.clone()),
            ("byte_start", count.clone()),
            ("byte_end", count.clone()),
            ("line_start", position.clone()),
            ("column_start", position.clone()),
            ("line_end", position.clone()),
            ("column_end", position.clone()),
            ("is_primary", json!({ "type": "boolean" })),
            ("label", json!({ "type": ["string", "null"] })),
            (
                "text",
                json!({ "type": "string", "description": "The line the span starts on." }),
            ),
        ],
        &[],
    );
    let edit = event(
        None,
        "Replace a byte range of a file.",
        &[
            ("file", string.clone()),
            ("byte_start", count.clone()),
            ("byte_end", count.clone()),
            ("replacement", string.clone()),
        ],
        &[],
    );
    let suggestion = event(
        None,
        "One fix: its edits applied together. Only a machine-applicable fix may be applied without a person.",
        &[
            (
                "applicability",
                json!({ "enum": ["machine-applicable", "maybe-incorrect", "has-placeholders", "unspecified"] }),
            ),
            ("edits", json!({ "type": "array", "items": edit })),
        ],
        &[],
    );
    let child = event(
        None,
        "A note or help, which may carry fixes.",
        &[
            ("level", json!({ "enum": ["help", "note"] })),
            ("message", string.clone()),
            (
                "suggestions",
                json!({ "type": "array", "items": suggestion }),
            ),
        ],
        &[],
    );
    let events = [
        event(
            Some("diagnostic"),
            "An error or warning about the project, shaped like rustc's (ADR 0012).",
            &[
                ("level", json!({ "enum": ["error", "warning"] })),
                (
                    "code",
                    json!({ "type": "string", "pattern": "^[a-z]+/[a-z0-9-]+$", "description": "area/kebab-name, never reused." }),
                ),
                ("message", string.clone()),
                (
                    "spans",
                    json!({ "type": "array", "items": span, "minItems": 1, "description": "The first is the primary span." }),
                ),
                ("children", json!({ "type": "array", "items": child })),
                (
                    "docs",
                    json!({ "type": "string", "description": "The command that prints the code's docs page." }),
                ),
                (
                    "rendered",
                    json!({ "type": "string", "description": "What a person sees in a terminal." }),
                ),
            ],
            &[],
        ),
        event(
            Some("build-started"),
            "A build or check began.",
            &[("command", json!({ "enum": ["build", "check", "dev"] }))],
            &[],
        ),
        event(
            Some("build-finished"),
            "A build or check ended. `ok` is false if it reported an error.",
            &[
                ("ok", json!({ "type": "boolean" })),
                ("duration_ms", count.clone()),
                (
                    "pages",
                    json!({ "type": "integer", "minimum": 0, "description": "Routes built, or that a check would build." }),
                ),
                (
                    "changed_urls",
                    json!({ "type": "array", "items": string, "description": "URLs whose files this build wrote. A check writes none." }),
                ),
                ("errors", count.clone()),
                ("warnings", count.clone()),
            ],
            &[],
        ),
        event(
            Some("budget"),
            "One Budget measured against its limit (#15).",
            &[
                ("name", string.clone()),
                ("value", json!({ "type": "number" })),
                ("limit", json!({ "type": "number" })),
                (
                    "page",
                    json!({ "type": ["string", "null"], "description": "The Page's URL, or null for a Site-wide Budget." }),
                ),
                ("passed", json!({ "type": "boolean" })),
                (
                    "acknowledged",
                    json!({ "type": "boolean", "description": "Over the limit, but an Acknowledged overage." }),
                ),
            ],
            &[],
        ),
        event(
            Some("server-ready"),
            "The dev server is serving its first build.",
            &[("url", string.clone()), ("pid", count.clone())],
            &[],
        ),
        event(
            Some("reload"),
            "The dev server told browsers to reload these Pages.",
            &[("urls", strings.clone())],
            &[],
        ),
        event(
            Some("browser-console"),
            "Console output or an uncaught error from a browser on the dev server.",
            &[
                (
                    "level",
                    json!({ "enum": ["log", "info", "warn", "error", "debug"] }),
                ),
                ("message", string.clone()),
                ("url", string.clone()),
                ("stack", json!({ "type": ["string", "null"] })),
            ],
            &[],
        ),
        event(
            Some("route"),
            "One Route and what produces it, from `yuanme routes`.",
            &[
                ("url", string.clone()),
                (
                    "output",
                    json!({ "type": "string", "description": "The file it writes, from the output directory." }),
                ),
                (
                    "kind",
                    json!({ "enum": ["entry", "list", "term", "index", "template", "feed"] }),
                ),
                ("template", string.clone()),
            ],
            &[
                (
                    "entry",
                    json!({ "type": "string", "description": "The Entry file, for an Entry's Route." }),
                ),
                ("collection", string.clone()),
                ("taxonomy", string.clone()),
                ("term", string.clone()),
                (
                    "page",
                    json!({ "type": "integer", "minimum": 1, "description": "The page of a list or Term, from 1." }),
                ),
                (
                    "name",
                    json!({ "type": "string", "description": "The name Site config gives a Template's Route or a feed." }),
                ),
            ],
        ),
        event(
            Some("entry-created"),
            "`yuanme new` wrote an Entry.",
            &[
                ("path", string.clone()),
                ("url", string.clone()),
                ("collection", string.clone()),
                (
                    "unfilled",
                    json!({ "type": "array", "items": { "type": "string" }, "description": "Required fields left for the Author, such as an image, which needs a file." }),
                ),
            ],
            &[],
        ),
        event(
            Some("schema"),
            "A JSON Schema, from `yuanme schema`.",
            &[
                (
                    "name",
                    json!({ "type": "string", "description": "config, entry/<collection>, data/<name>, context/<kind> or events." }),
                ),
                ("schema", json!({ "type": "object" })),
            ],
            &[],
        ),
    ];
    let mut schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "yuanme --json events",
        "description": "Each line yuanme writes to stdout with --json is one of these. Fields may be added; none is renamed or removed before 2.0.",
        "oneOf": events,
    });
    schema.sort_all_objects();
    schema
}

/// An object with these required and optional properties. With a `type`,
/// it is an event, and `type` is required too.
fn event(
    ty: Option<&str>,
    description: &str,
    required: &[(&str, Value)],
    optional: &[(&str, Value)],
) -> Value {
    let mut properties = serde_json::Map::new();
    let mut names = Vec::new();
    if let Some(ty) = ty {
        properties.insert("type".into(), json!({ "const": ty }));
        names.push("type");
    }
    for (name, schema) in required {
        properties.insert(name.to_string(), schema.clone());
        names.push(name);
    }
    for (name, schema) in optional {
        properties.insert(name.to_string(), schema.clone());
    }
    json!({
        "type": "object",
        "description": description,
        "properties": properties,
        "required": names,
    })
}
