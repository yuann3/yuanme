//! Compiles the pinned grammars and themes in `src/highlight/shiki` into
//! ferriki's catalogs (#53), and writes `highlight.rs`, which embeds them
//! with the tables `src/highlight.rs` reads.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

use ferriki_asset_gen::AssetSourceRef;
use serde_json::Value;

fn main() {
    let shiki = Path::new("src/highlight/shiki");
    println!("cargo::rerun-if-changed={}", shiki.display());
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let catalogs = out.join("shiki");
    let _ = fs::remove_dir_all(&catalogs);
    let source = AssetSourceRef::new("shiki", Some("3.19.0".into()), None);
    ferriki_asset_gen::generate_catalogs_from_upstream(shiki, &catalogs, source)
        .expect("compile the grammars and themes");

    let json = |path: &Path| -> Value {
        serde_json::from_str(&fs::read_to_string(path).expect("read the JSON")).expect("valid JSON")
    };
    let mut rs = String::new();
    let embed = |rs: &mut String, name: &str, dir: &str, ext: &str, ids: &[String]| {
        let manifest = catalogs.join(dir).join("manifest.fkindex");
        writeln!(
            rs,
            "const {name}_MANIFEST: &[u8] = include_bytes!({manifest:?});"
        )
        .unwrap();
        writeln!(rs, "const {name}: &[(&str, &[u8])] = &[").unwrap();
        for id in ids {
            // As ferriki-asset-gen names an asset file.
            let file: String = id
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
                + ext;
            let path = catalogs.join(dir).join(&file);
            writeln!(rs, "    ({file:?}, include_bytes!({path:?})),").unwrap();
        }
        writeln!(rs, "];").unwrap();
    };

    // Every name a grammar answers to, sorted for a binary search.
    let languages = json(&shiki.join("languages.json"));
    let languages = languages["languages"].as_array().expect("a list");
    let mut names: Vec<(String, String)> = Vec::new();
    let mut ids = Vec::new();
    for l in languages {
        let id = l["id"].as_str().expect("an id").to_string();
        names.push((id.clone(), id.clone()));
        for alias in l["aliases"].as_array().expect("a list") {
            names.push((alias.as_str().expect("a name").to_string(), id.clone()));
        }
        ids.push(id);
    }
    names.sort();
    names.dedup();
    embed(&mut rs, "GRAMMARS", "languages", ".fkgram", &ids);
    writeln!(rs, "const NAMES: &[(&str, &str)] = &{names:?};").unwrap();

    // Each theme's colours: its foreground, background, and every colour a
    // token can have, sorted. A class names colours by their place here.
    let themes = json(&shiki.join("themes.json"));
    let mut ids = Vec::new();
    for t in themes["themes"].as_array().expect("a list") {
        let id = t["id"].as_str().expect("an id");
        let theme = json(
            &shiki
                .join("themes")
                .join(t["theme_file"].as_str().expect("a file")),
        );
        let colour = |key: &str| {
            theme["colors"][key]
                .as_str()
                .expect(key)
                .to_ascii_lowercase()
        };
        let mut palette: Vec<String> = theme["tokenColors"]
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(|r| r["settings"]["foreground"].as_str())
            .map(str::to_ascii_lowercase)
            .chain([colour("editor.foreground")])
            .collect();
        palette.sort();
        palette.dedup();
        let name = id.to_ascii_uppercase().replace('-', "_");
        writeln!(
            rs,
            "const {name}: Theme = Theme {{ id: {id:?}, foreground: {:?}, background: {:?}, palette: &{palette:?} }};",
            colour("editor.foreground"),
            colour("editor.background"),
        )
        .unwrap();
        ids.push(id.to_string());
    }
    embed(&mut rs, "THEMES", "themes", ".fktheme", &ids);
    fs::write(out.join("highlight.rs"), rs).expect("write highlight.rs");
}
