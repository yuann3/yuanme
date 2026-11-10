//! The cloudflare adapter's own check: that the Worker's `wrangler.jsonc`
//! serves the Site as the adapter writes it.

use std::ops::Range;

use yuanme_content::config::Adapter;
use yuanme_diag::{Diagnostic, Span, codes};

use crate::Build;

/// Compares `text`, the project's `wrangler.jsonc`, with what the
/// cloudflare adapter writes into `out`, the output directory from the
/// project root, such as `dist`. The file is the Worker's, not an input of
/// the build, so the CLI reads it. Other adapters, and a file that isn't
/// JSONC, which wrangler reports itself, are left alone.
pub fn wrangler(build: &mut Build, text: &str, out: &str) {
    let adapter = build.config.as_ref().map(|c| c.host.adapter);
    if adapter != Some(Adapter::Cloudflare) {
        return;
    }
    // Comments and trailing commas become spaces, so offsets hold.
    let mut json = text.to_owned();
    if json_strip_comments::strip(&mut json).is_err() {
        return;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return;
    };
    let assets = &value["assets"];
    let get = |key: &str| assets[key].as_str();

    let mut found: Vec<(&str, String, String)> = Vec::new();
    let directory = get("directory").map(|d| d.trim_start_matches("./").trim_end_matches('/'));
    if directory != Some(out) {
        let message = match directory {
            Some(_) => format!(
                "`assets.directory` is {}, but yuanme writes the Site into `{out}`",
                assets["directory"]
            ),
            None => format!("`assets.directory` isn't set, so the Worker serves none of `{out}`"),
        };
        found.push((
            "directory",
            message,
            format!("set `\"directory\": \"./{out}\"`"),
        ));
    }
    if let Some(v) = get("html_handling").filter(|v| *v != "auto-trailing-slash") {
        let message = format!(
            "`assets.html_handling` is \"{v}\", but the Site's Pages are at URLs that end in `/`"
        );
        found.push((
            "html_handling",
            message,
            "delete it: \"auto-trailing-slash\" is the default".into(),
        ));
    }
    if build.files.contains_key("404.html") && get("not_found_handling") != Some("404-page") {
        found.push((
            "not_found_handling",
            "the Site has a 404 Page, which Workers serves only with `assets.not_found_handling` set to \"404-page\"".into(),
            "set `\"not_found_handling\": \"404-page\"`".into(),
        ));
    }
    if found.is_empty() {
        return;
    }
    let file = build.sources.add("wrangler.jsonc", text);
    for (key, message, help) in found {
        let span = Span {
            file,
            range: value_at(&json, key),
        };
        build
            .diagnostics
            .push(Diagnostic::warning(&codes::HOST_WRANGLER_MISMATCH, message, span).help(help));
    }
}

/// Where `assets.<key>`'s value is in `json`; the `"assets"` key when
/// it is missing, or the start when that is too.
fn value_at(json: &str, key: &str) -> Range<usize> {
    let Some(assets) = json.find("\"assets\"") else {
        return 0..0;
    };
    let quoted = format!("\"{key}\"");
    let Some(at) = json[assets..]
        .find(&quoted)
        .map(|i| assets + i + quoted.len())
    else {
        return assets..assets + "\"assets\"".len();
    };
    let start = at
        + json[at..]
            .find(|c: char| c != ':' && !c.is_whitespace())
            .unwrap_or(0);
    let rest = &json[start..];
    let len = match rest.strip_prefix('"') {
        Some(s) => s.find('"').map_or(rest.len(), |i| i + 2),
        None => rest.find([',', '}', '\n']).unwrap_or(rest.len()),
    };
    start..start + rest[..len].trim_end().len()
}
