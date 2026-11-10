//! `yuanme new COLLECTION "Title"` (#21): an Entry whose required fields
//! are filled in and valid, so it builds before the Author writes a word.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use jiff::civil::Date;
use yuanme_content::{Collection, Config, Field, FieldType, Yaml, parse_yaml, slug, url_path};
use yuanme_engine::Mode;

use crate::SITE_ERRORS;
use crate::events::Event;
use crate::out::{Out, stdout};
use crate::project::{self, config_or_report};

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Format {
    Md,
    Org,
}

pub(crate) struct Created {
    /// From the project root.
    pub(crate) path: String,
    pub(crate) url: String,
    pub(crate) text: String,
    /// Required fields with no valid value to give, such as an image.
    pub(crate) unfilled: Vec<String>,
}

/// Why `new` writes no Entry. Each is the command's mistake, not the
/// Site's.
#[derive(Debug)]
pub(crate) enum Error {
    MultilineTitle,
    /// The title, which has no letters or digits.
    NoSlug(String),
    NoCollection {
        name: String,
        /// The Collection with the closest name, if one is close.
        near: Option<String>,
    },
    /// The Entry's path from the project root, which holds a file.
    Exists(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MultilineTitle => f.write_str("a title is one line"),
            Error::NoSlug(title) => {
                write!(f, "`{title}` has no letters or digits to make a Slug from")
            }
            Error::NoCollection { name, near: None } => {
                write!(f, "there is no Collection `{name}`")
            }
            Error::NoCollection {
                name,
                near: Some(near),
            } => write!(f, "there is no Collection `{name}`; did you mean `{near}`?"),
            Error::Exists(path) => write!(f, "{path} already exists"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Error> for crate::Error {
    fn from(e: Error) -> crate::Error {
        crate::Error::BadCommand(e.to_string())
    }
}

/// The Entry `new` writes. `today` is the date in the Site's time zone.
pub(crate) fn entry(
    collection: &Collection,
    title: &str,
    format: Format,
    today: Date,
) -> Result<Created, Error> {
    if title.contains(['\n', '\r']) {
        return Err(Error::MultilineTitle);
    }
    let slug = slug(title.trim());
    if slug.is_empty() {
        return Err(Error::NoSlug(title.to_string()));
    }
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut unfilled = Vec::new();
    // `date` is the built-in, so every new Entry has one, declared or not.
    fields.push(("date".into(), today.to_string()));
    for (name, field) in &collection.fields {
        if name == "date" || field.optional || field.default.is_some() {
            continue;
        }
        match value(&field.kind, today) {
            Some(v) => fields.push((name.clone(), v)),
            None => unfilled.push(name.clone()),
        }
    }
    let (ext, text) = match format {
        Format::Md => {
            let mut text = format!("---\ntitle: {}\n", yaml_string(title.trim()));
            for (k, v) in &fields {
                text.push_str(&format!("{k}: {v}\n"));
            }
            ("md", text + "---\n\n")
        }
        Format::Org => {
            // A string keyword is its text as written; the rest are YAML
            // flow values, and a date may be an Org timestamp.
            let mut text = format!("#+TITLE: {}\n", title.trim());
            for (k, v) in &fields {
                let v = match k.as_str() {
                    "date" => today.strftime("[%Y-%m-%d %a]").to_string(),
                    _ => v.clone(),
                };
                text.push_str(&format!("#+{}: {v}\n", k.to_uppercase()));
            }
            ("org", text + "\n")
        }
    };
    Ok(Created {
        path: format!("{}/{slug}.{ext}", collection.dir),
        url: url_path(&collection.route.replace("{slug}", &slug)),
        text,
        unfilled,
    })
}

/// A valid YAML flow value for a required field, or `None` for one that
/// can't have one without the Author: an image needs a file.
fn value(kind: &FieldType, today: Date) -> Option<String> {
    Some(match kind {
        FieldType::String { min, max } => {
            let len = 4.clamp(min.unwrap_or(0), max.unwrap_or(u64::MAX)) as usize;
            yaml_string(&"TODO".chars().cycle().take(len).collect::<String>())
        }
        FieldType::Int { min, max } => 0
            .clamp(min.unwrap_or(i64::MIN), max.unwrap_or(i64::MAX))
            .to_string(),
        FieldType::Float { min, max } => {
            let v = 0f64.clamp(min.unwrap_or(f64::MIN), max.unwrap_or(f64::MAX));
            format!("{v:?}")
        }
        FieldType::Bool => "false".into(),
        FieldType::Date => today.to_string(),
        FieldType::Datetime => format!("{today}T00:00:00"),
        FieldType::Url => "https://example.com/".into(),
        FieldType::Enum(values) => yaml_string(&values[0]),
        FieldType::List { .. } => "[]".into(),
        FieldType::Object(fields) => object(fields, today)?,
        FieldType::Image => return None,
    })
}

fn object(fields: &BTreeMap<String, Field>, today: Date) -> Option<String> {
    let mut parts = Vec::new();
    for (name, field) in fields {
        if field.optional || field.default.is_some() {
            continue;
        }
        parts.push(format!("{name}: {}", value(&field.kind, today)?));
    }
    Some(format!("{{{}}}", parts.join(", ")))
}

/// `s` as plain YAML if it reads back as that string, or else quoted.
fn yaml_string(s: &str) -> String {
    let plain = matches!(
        parse_yaml(s, 0).map(|n| n.value),
        Ok(Yaml::Str(read)) if read == s
    );
    // Plain text with `, ` or `}` would break a flow mapping it sits in.
    if plain && !s.contains([',', '{', '}', '[', ']']) {
        s.to_string()
    } else {
        serde_json::to_string(s).expect("a string serializes")
    }
}

/// The Collection `name`, the built-in `pages` included.
pub(crate) fn collection(config: &Config, name: &str) -> Result<Collection, Error> {
    let collections = yuanme_content::collections(config);
    collections
        .get(name)
        .cloned()
        .ok_or_else(|| Error::NoCollection {
            name: name.to_string(),
            near: yuanme_content::similar(name, collections.keys().map(String::as_str))
                .map(str::to_string),
        })
}

pub(crate) fn run(
    root: &Path,
    collection: &str,
    title: &str,
    format: Format,
    out: &Out,
) -> Result<u8, crate::Error> {
    let run = project::build(root, None, Mode::Check { drafts: false }, false, false)?;
    let Some(config) = config_or_report(&run, out) else {
        return Ok(SITE_ERRORS);
    };
    let found = self::collection(config, collection)?;
    let zone =
        jiff::tz::TimeZone::get(&config.site.timezone).expect("Site config checked the zone");
    let today = jiff::Zoned::now().with_time_zone(zone).date();
    let created = entry(&found, title, format, today)?;
    let path = root.join(&created.path);
    if path.exists() {
        return Err(Error::Exists(created.path).into());
    }
    fs::create_dir_all(path.parent().expect("in a Collection's folder"))?;
    fs::write(&path, &created.text)?;
    for field in &created.unfilled {
        out.say(&format!(
            "warning: fill in `{field}` in {}: it needs an image",
            created.path
        ));
    }
    if out.json {
        out.event(Event::EntryCreated {
            path: created.path,
            url: created.url,
            collection: collection.to_string(),
            unfilled: created.unfilled,
        });
    } else {
        stdout(&format!("{}\n{}", created.path, created.url));
    }
    Ok(0)
}
