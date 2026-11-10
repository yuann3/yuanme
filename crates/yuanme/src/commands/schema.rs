//! `yuanme schema`: the JSON Schemas of Site config, Entries, Data
//! files, Template contexts and `--json`'s events.

use std::path::Path;

use serde_json::Value;
use yuanme_engine::Mode;

use crate::cli::Which;
use crate::events::Event;
use crate::out::{Out, stdout};
use crate::project::{self, config_or_report, site_or_report};
use crate::{Error, SITE_ERRORS, schemas, unknown};

pub(crate) fn print(out: &Out, name: &str, schema: Value) -> u8 {
    if out.json {
        out.event(Event::Schema {
            name: name.to_string(),
            schema,
        });
    } else {
        stdout(&serde_json::to_string_pretty(&schema).expect("JSON serializes"));
    }
    0
}

pub(crate) fn run(root: &Path, which: Which, out: &Out) -> Result<u8, Error> {
    let run = project::build(root, None, Mode::Check { drafts: false }, false, false)?;
    let (name, mut schemas) = match which {
        Which::Entry { collection } => {
            let Some(config) = config_or_report(&run, out) else {
                return Ok(SITE_ERRORS);
            };
            (
                format!("entry/{collection}"),
                schemas::entry_and_data(config),
            )
        }
        Which::Data { name } => {
            let Some(config) = config_or_report(&run, out) else {
                return Ok(SITE_ERRORS);
            };
            (format!("data/{name}"), schemas::entry_and_data(config))
        }
        Which::Context { kind } => {
            let Some((config, site)) = site_or_report(&run, out) else {
                return Ok(SITE_ERRORS);
            };
            (format!("context/{kind}"), schemas::context(config, site))
        }
        Which::Config | Which::Events => unreachable!("answered without a project"),
    };
    let Some(schema) = schemas.remove(&name) else {
        let (group, item) = name.split_once('/').expect("group/name");
        let what = match group {
            "entry" => "Collection",
            "data" => "Data file with a schema",
            _ => "kind of Route",
        };
        let choices: Vec<&str> = schemas
            .keys()
            .filter_map(|k| k.strip_prefix(&format!("{group}/")))
            .collect();
        let otherwise = match choices.is_empty() {
            true => "the Site has none".to_string(),
            false => format!("the Site has {}", choices.join(", ")),
        };
        return Err(unknown(what, item, &choices, otherwise));
    };
    Ok(print(out, &name, schema))
}
