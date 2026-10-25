//! The JSON Schemas generated per Site (#21), written to `.yuanme/schemas/`
//! whenever `check` or `build` runs, and printed by `yuanme schema`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;
use yuanme_content::config::{self, Config};
use yuanme_content::site::{self, Site};

/// By name, `entry/<collection>` and `data/<name>`. Site config's is
/// `config`, and each kind of Route's context `context/<kind>`.
pub fn entry_and_data(config: &Config) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for (name, collection) in site::collections(config) {
        let schema = config::entry_schema(config, &name, &collection);
        out.insert(format!("entry/{name}"), schema);
    }
    for (name, data) in &config.data {
        out.insert(format!("data/{name}"), config::data_schema(name, data));
    }
    out
}

pub fn context(config: &Config, site: &Site) -> BTreeMap<String, Value> {
    yuanme_render::template::context_schemas(config, site)
        .into_iter()
        .map(|(kind, schema)| (format!("context/{kind}"), schema))
        .collect()
}

/// The file a schema is written to, from `.yuanme/schemas/`. Site config's
/// keeps the name `init`'s `#:schema` line points at.
fn file(name: &str) -> String {
    match name {
        "config" => "yuanme.toml.json".into(),
        _ => format!("{name}.json"),
    }
}

/// Writes every schema the build could make into `dir`. A schema whose
/// part of the project failed keeps its old file, so an editor still has
/// one while the Author fixes it; a Collection or kind that is gone loses
/// its file. Files are rewritten only when they changed, so editors
/// watching them aren't woken.
pub fn write(dir: &Path, config: Option<&Config>, site: Option<&Site>) -> io::Result<()> {
    let mut schemas = BTreeMap::from([("config".to_string(), config::json_schema())]);
    let mut owned = Vec::new();
    if let Some(config) = config {
        schemas.extend(entry_and_data(config));
        owned.extend(["entry", "data"]);
        if let Some(site) = site {
            schemas.extend(context(config, site));
            owned.push("context");
        }
    }
    for (name, schema) in &schemas {
        let path = dir.join(file(name));
        let text = serde_json::to_string_pretty(schema)? + "\n";
        if fs::read_to_string(&path).is_ok_and(|old| old == text) {
            continue;
        }
        fs::create_dir_all(path.parent().expect("in the schemas folder"))?;
        fs::write(&path, text)?;
    }
    let wanted: Vec<String> = schemas.keys().map(|n| file(n)).collect();
    for sub in owned {
        let Ok(entries) = fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for entry in entries {
            let name = format!("{sub}/{}", entry?.file_name().to_string_lossy());
            if !wanted.contains(&name) {
                fs::remove_file(dir.join(&name))?;
            }
        }
    }
    Ok(())
}
