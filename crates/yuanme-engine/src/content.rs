//! The content stage: Site config, and the Site resolved from its Entries
//! and Data files, read through the leaf caches (ADR 0011); then each
//! Entry's body, its code blocks highlighted, as HTML.

pub(crate) mod bodies;
pub(crate) mod entries;
pub(crate) mod highlight;

use yuanme_content::config::{self, Config};
use yuanme_content::site::{self, Project, Site};
use yuanme_diag::{Diagnostic, Files, codes};

use crate::inputs::utf8;
use crate::{Cache, Inputs};

/// Site config, read from `yuanme.toml`.
pub(crate) fn site_config(
    inputs: &Inputs,
    engine: &str,
    sources: &mut Files,
) -> (Option<Config>, Vec<Diagnostic>) {
    const PATH: &str = "yuanme.toml";
    let input = inputs.files.get(PATH).expect("a project has yuanme.toml");
    let bytes = input
        .bytes
        .as_deref()
        .expect("yuanme.toml is read, not streamed");
    match utf8(PATH, bytes, &codes::CONFIG_SYNTAX, sources) {
        Ok(file) => config::parse(sources, file, engine),
        Err(d) => (None, vec![d]),
    }
}

/// Reads every Entry and Data file and resolves the Site.
pub(crate) fn resolve(
    inputs: &Inputs,
    config: &Config,
    drafts: bool,
    sources: &mut Files,
    cache: &mut Cache,
) -> (Option<Site>, Vec<Diagnostic>) {
    let (entries, data) = site::sources(config, inputs.files.keys().map(String::as_str));
    let mut diagnostics = Vec::new();
    let mut add = |path: &str| {
        let bytes = inputs.files[path]
            .bytes
            .as_deref()
            .expect("text is read, not streamed");
        utf8(path, bytes, &codes::ENTRY_SYNTAX, sources)
            .map_err(|d| diagnostics.push(d))
            .ok()
    };
    let entries: Vec<_> = entries
        .into_iter()
        .filter_map(|(c, p)| Some((c, add(p)?)))
        .collect();
    let data: Vec<_> = data.into_iter().filter_map(&mut add).collect();
    if !diagnostics.is_empty() {
        return (None, diagnostics);
    }
    let is_file = |p: &str| inputs.files.contains_key(p);
    let is_dir = |d: &str| inputs.is_dir(d);
    let entries = entries::read(inputs, config, &entries, sources, cache);
    let project = Project {
        files: sources,
        entries,
        data,
        is_file: &is_file,
        is_dir: &is_dir,
    };
    site::resolve(config, project, drafts)
}
