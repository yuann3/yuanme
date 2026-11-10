//! `yuanme new COLLECTION "Title"` (#21): an Entry whose required fields
//! are filled in and valid, so it builds before the Author writes a word.

use std::collections::BTreeMap;

use jiff::civil::Date;
use yuanme_content::config::{Collection, Config, Field, FieldType};
use yuanme_content::slug;
use yuanme_content::yaml::{self, Yaml};

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Md,
    Org,
}

pub struct Created {
    /// From the project root.
    pub path: String,
    pub url: String,
    pub text: String,
    /// Required fields with no valid value to give, such as an image.
    pub unfilled: Vec<String>,
}

/// The Entry `new` writes. `today` is the date in the Site's time zone.
pub fn entry(
    collection: &Collection,
    title: &str,
    format: Format,
    today: Date,
) -> Result<Created, String> {
    if title.contains(['\n', '\r']) {
        return Err("a title is one line".into());
    }
    let slug = slug::slug(title.trim());
    if slug.is_empty() {
        return Err(format!(
            "`{title}` has no letters or digits to make a Slug from"
        ));
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
        url: slug::url_path(&collection.route.replace("{slug}", &slug)),
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
        yaml::parse(s, 0).map(|n| n.value),
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
pub fn collection(config: &Config, name: &str) -> Result<Collection, String> {
    let collections = yuanme_content::site::collections(config);
    collections.get(name).cloned().ok_or_else(|| {
        let mut message = format!("there is no Collection `{name}`");
        if let Some(near) =
            yuanme_content::config::similar(name, collections.keys().map(String::as_str))
        {
            message.push_str(&format!("; did you mean `{near}`?"));
        }
        message
    })
}
