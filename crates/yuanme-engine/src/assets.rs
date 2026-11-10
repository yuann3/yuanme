//! The assets stage (#20): the stylesheets, Scripts and files that Pages
//! name, found in their rendered HTML, the Site-wide Sidecar stylesheet and
//! each Page's Sidecar scripts. Each becomes an [`Asset`] that the link pass
//! names by its hash.

mod diagnostics;
mod project;
mod scripts;
pub(crate) mod styles;
pub(crate) mod tailwind;
pub(crate) mod typescript;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use rayon::prelude::*;
use yuanme_assets::css::{self, Packages};
use yuanme_assets::html::{self, Kind};
use yuanme_assets::{js, tailwind as tw};
use yuanme_content::config::{Config, RouterMode};
use yuanme_content::fields::Value;
use yuanme_diag::{Applicability, Diagnostic, FileId, Files, Span, codes};
use yuanme_render::template::{Page, scope};

use diagnostics::{Diagnostics, SourceFiles, module_type, reference_span};
use project::{Project, Target, copy};

use crate::hashing::{Asset, placeholder};
use crate::url::suffix;
use crate::{Body, Bundle, Cache, Inputs};

/// The stylesheet holding every used Template's Sidecar CSS.
const SIDECARS: &str = "sidecars.css";
/// Where that stylesheet's `@import`s are, as lightningcss sees it. NUL
/// can't be in a project file's name.
const SIDECARS_ENTRY: &str = "\0sidecars.css";

pub(crate) struct Stage {
    /// Each Page's bytes, with placeholders for the assets it names.
    pub pages: Vec<(String, Vec<u8>)>,
    /// What each HTML Page shows, in the order of `pages`.
    pub seen: Vec<Option<Arc<Seen>>>,
    pub assets: Vec<Asset>,
    /// What went into each Script bundle.
    pub scripts: Vec<Bundle>,
    /// The dev client's chunk, by its path in the output directory.
    pub client: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Stage {
    fn failed(diagnostics: Diagnostics) -> Stage {
        Stage {
            pages: Vec::new(),
            seen: Vec::new(),
            assets: Vec::new(),
            scripts: Vec::new(),
            client: None,
            diagnostics: diagnostics.0,
        }
    }
}

/// What later passes ask of a Page's text: what it names, its highlighted
/// code's classes, and whether it has math. It reads nothing but the text,
/// so `yuanme dev` keeps it with the text for its next build.
pub(crate) struct Seen {
    pub scan: html::Scan,
    pub code: Option<BTreeSet<String>>,
    pub math: bool,
}

impl Seen {
    fn of(text: &str) -> Seen {
        let code = yuanme_render::highlight::classes(text);
        let mut scan = html::scan(text);
        // Only Tailwind's tokens read them, as a set; a Page of
        // highlighted code holds thousands of the same few.
        scan.classes.sort_unstable();
        scan.classes.dedup();
        Seen {
            scan,
            code: code.map(|c| c.into_iter().map(str::to_string).collect()),
            math: styles::has_math(text.as_bytes()),
        }
    }
}

/// `dev` is `yuanme dev`'s: it keeps a Tailwind CLI running for each
/// stylesheet, and bundles the dev client.
#[allow(clippy::too_many_arguments)] // Each is one thing the stage reads.
pub(crate) fn assets(
    inputs: &Inputs,
    config: &Config,
    data: &BTreeMap<String, Value>,
    pages: Vec<Page>,
    templates: &BTreeMap<String, FileId>,
    sources: &mut Files,
    cache: &mut Cache,
    dev: bool,
) -> Stage {
    let mut site = BTreeSet::new();
    for page in &pages {
        site.insert(format!("/{}", page.output));
        if let Some(dir) = page.output.strip_suffix("index.html") {
            site.insert(format!("/{dir}"));
        }
    }
    // Each used Sidecar stylesheet, and the attribute that scopes it.
    let scopes: BTreeMap<String, String> = pages
        .iter()
        .flat_map(|p| &p.templates)
        .map(|t| (scope::sidecar(t), scope::attribute(t)))
        .filter(|(path, _)| inputs.files.contains_key(path))
        .collect();
    let mut project = Project {
        inputs,
        site: &site,
        packages: inputs.root.as_deref().map(Packages::new),
        sidecars: scopes
            .keys()
            .map(|p| format!("@import {};\n", css_string(&format!("/{p}"))))
            .collect(),
        tailwind: BTreeMap::new(),
    };
    let mut diagnostics = Diagnostics::default();
    let mut files = SourceFiles::default();
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for (path, attribute) in &scopes {
        if let Some(other) = seen.insert(attribute, path) {
            let message =
                format!("`{other}` and `{path}` have the same scope attribute, `{attribute}`");
            let span = files.span(&project, sources, path, 1, 1, "");
            diagnostics.push(
                Diagnostic::error(&codes::ASSET_SCOPE_COLLISION, message, span)
                    .help("rename one of their Templates, and its Sidecar"),
            );
        }
    }

    // What each Page names.
    let kept = std::mem::take(&mut cache.seen);
    let scans: Vec<Option<Arc<Seen>>> = pages
        .par_iter()
        .map(|p| {
            if !p.output.ends_with(".html") {
                return None;
            }
            Some(match kept.get(&p.output) {
                Some((text, seen)) if *text == p.text => seen.clone(),
                _ => Arc::new(Seen::of(&p.text)),
            })
        })
        .collect();
    drop(kept);
    let mut entries: Vec<String> = Vec::new();
    let mut script_entries: Vec<String> = Vec::new();
    let mut copies: BTreeSet<String> = BTreeSet::new();
    // Per Page: (where, what goes there, keeping the URL's `?` and `#`).
    let mut edits: Vec<Vec<(Range<usize>, Use)>> = vec![Vec::new(); pages.len()];
    for (i, (page, scan)) in pages.iter().zip(&scans).enumerate() {
        let Some(Seen { scan, .. }) = scan.as_deref() else {
            continue;
        };
        let base = format!("/{}", page.output);
        for r in &scan.references {
            let at_template = || reference_span(page, &page.text[r.at.clone()], templates, sources);
            let path = match project.resolve(r.url.trim(), &base) {
                Ok(Target::Source(path)) => path,
                Ok(_) => continue,
                Err(problem) => {
                    let (span, _) = at_template();
                    diagnostics.push(problem.diagnostic(span).note(note(page)));
                    continue;
                }
            };
            let use_ = match r.kind {
                Kind::Classic => {
                    let (span, found) = at_template();
                    let mut d = Diagnostic::error(
                        &codes::ASSET_CLASSIC_SCRIPT,
                        format!("`{}` is loaded as a classic script", r.url.trim()),
                        span.clone(),
                    )
                    .help("yuanme bundles Scripts as modules");
                    if found && let Some(edit) = module_type(&span, sources) {
                        d = d.suggest(
                            "add `type=\"module\"`",
                            Applicability::MaybeIncorrect,
                            vec![edit],
                        );
                    }
                    diagnostics.push(d.note(note(page)));
                    continue;
                }
                Kind::Module => {
                    let problem = if path.ends_with(".jsx") || path.ends_with(".tsx") {
                        Some((&codes::ASSET_JSX, "is JSX, which yuanme doesn't compile"))
                    } else if !scripts::EXTENSIONS.iter().any(|e| path.ends_with(e)) {
                        Some((
                            &codes::ASSET_INVALID_SCRIPT,
                            "is no Script: a Script is a `.ts`, `.js`, `.mts` or `.mjs` file",
                        ))
                    } else {
                        None
                    };
                    if let Some((code, why)) = problem {
                        let (span, _) = at_template();
                        let message = format!("`{}` {why}", r.url.trim());
                        diagnostics.push(Diagnostic::error(code, message, span).note(note(page)));
                        continue;
                    }
                    Use::Script(path)
                }
                Kind::Stylesheet => Use::Bundle(path),
                // A preload of a stylesheet must fetch the bundle that
                // `<link rel=stylesheet>` gets.
                Kind::Link if path.ends_with(".css") => Use::Bundle(path),
                Kind::Link => {
                    copies.insert(path.clone());
                    Use::Copy(path)
                }
            };
            if let Use::Bundle(path) = &use_
                && !entries.contains(path)
            {
                entries.push(path.clone());
            }
            if let Use::Script(path) = &use_
                && !script_entries.contains(path)
            {
                script_entries.push(path.clone());
            }
            edits[i].push((r.at.clone(), use_));
        }
    }
    // Each Page's Script (ADR 0013): the Sidecar scripts of its Templates,
    // in their order, and with the Router on, the Router. Pages with the
    // same Sidecar scripts share one.
    let router = config.router == RouterMode::Swap;
    let mut page_scripts: Vec<Vec<String>> = Vec::new();
    // By Template, since every Page renders a few of the same ones.
    let mut sidecar_scripts: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let page_script: Vec<Option<usize>> = pages
        .iter()
        .zip(&scans)
        .map(|(page, scan)| {
            scan.as_ref()?.scan.stylesheet_at?;
            let mut sidecars = Vec::new();
            for t in &page.templates {
                let found = sidecar_scripts.entry(t).or_insert_with(|| {
                    let stem = t.rsplit_once('.').map_or(&**t, |(s, _)| s);
                    scripts::EXTENSIONS
                        .iter()
                        .map(|e| format!("templates/{stem}{e}"))
                        .filter(|path| inputs.files.contains_key(path))
                        .collect()
                });
                sidecars.extend(found.iter().cloned());
            }
            if sidecars.is_empty() && !router {
                return None;
            }
            Some(match page_scripts.iter().position(|s| *s == sidecars) {
                Some(i) => i,
                None => {
                    page_scripts.push(sidecars);
                    page_scripts.len() - 1
                }
            })
        })
        .collect();

    let entries_js: Vec<js::Entry> = script_entries
        .iter()
        .map(|path| js::Entry::Script(path.clone()))
        .chain(page_scripts.into_iter().map(js::Entry::Page))
        .chain(dev.then_some(js::Entry::Client))
        .collect();
    let scripts = scripts::bundle(
        &project,
        config,
        data,
        &entries_js,
        sources,
        &mut files,
        &mut diagnostics,
    );

    // Each stylesheet that imports Tailwind, built by it from the tokens of
    // every Page, Script and Sidecar. With a Script that failed, the tokens
    // aren't all known, and the build fails anyway.
    let tailwind: Vec<String> = entries
        .iter()
        .filter(|e| project.text(e).is_some_and(|css| tw::imported(&css)))
        .cloned()
        .collect();
    if let (Some(scripts), false) = (&scripts, tailwind.is_empty()) {
        let classes = scans.iter().flatten().flat_map(|s| &s.scan.classes);
        let mut strings = Vec::new();
        for asset in &scripts.assets {
            if let Asset::Copy {
                body: Body::Bytes(code),
                ..
            } = asset
            {
                strings.extend(js::strings(&String::from_utf8_lossy(code)));
            }
        }
        let sidecar_texts: Vec<String> = scopes.keys().filter_map(|p| project.text(p)).collect();
        let tokens = tw::tokens(
            classes
                .map(String::as_str)
                .chain(strings.iter().map(String::as_str))
                .chain(sidecar_texts.iter().map(String::as_str)),
        );
        project.tailwind = tailwind::outputs(
            &project,
            config,
            cache,
            &tailwind,
            &tokens,
            dev,
            sources,
            &mut files,
            &mut diagnostics,
        );
    }

    // Every stylesheet, bundled.
    let mut bundles: Vec<(String, &str)> = entries
        .iter()
        .map(|e| (file_name(e).to_string(), e.as_str()))
        .collect();
    if !scopes.is_empty() {
        bundles.push((SIDECARS.to_string(), SIDECARS_ENTRY));
    }
    let unscoped = BTreeMap::new();
    let bundled: Vec<_> = bundles
        .par_iter()
        .map(|(_, entry)| {
            let scopes = if *entry == SIDECARS_ENTRY {
                &scopes
            } else {
                &unscoped
            };
            let is_tailwind = tailwind.iter().any(|t| t == entry);
            // Tailwind failed, and said so.
            if is_tailwind && !project.tailwind.contains_key(*entry) {
                return None;
            }
            Some(css::bundle(
                &project,
                entry,
                &config.build.targets,
                scopes,
                is_tailwind,
            ))
        })
        .collect();
    // Each bundle's code, and where its `url()`s go.
    let mut codes: Vec<(String, Vec<(String, Use)>)> = Vec::new();
    for ((_, entry), result) in bundles.iter().zip(bundled) {
        let bundle = match result {
            None => continue,
            Some(Ok(bundle)) => bundle,
            Some(Err(e)) => {
                diagnostics.push(files.error(*e, entry, &project, sources));
                continue;
            }
        };
        for compound in &bundle.compounds {
            if let Some(d) = unmatched(compound, templates, &project, sources, &mut files) {
                diagnostics.push(d);
            }
        }
        let mut urls = Vec::new();
        for url in &bundle.urls {
            let use_ = match project.resolve(&url.url, &format!("/{}", url.file)) {
                Ok(Target::External) => Use::Url(url.url.clone()),
                // The bundle is served from `/assets/`, so a relative URL
                // must become one from the root.
                Ok(Target::Site(path)) => Use::Url(path + suffix(&url.url)),
                Ok(Target::Source(path)) => {
                    copies.insert(path.clone());
                    Use::Copy(path)
                }
                Err(problem) => {
                    let span =
                        files.span(&project, sources, &url.file, url.line, url.column, &url.url);
                    diagnostics.push(problem.diagnostic(span));
                    continue;
                }
            };
            urls.push((url.placeholder.clone(), use_));
        }
        codes.push((bundle.code, urls));
    }
    let Some(scripts) = scripts.filter(|_| !diagnostics.failed()) else {
        return Stage::failed(diagnostics);
    };

    // The link pass needs the copies first: bundles name them. Script
    // chunks name only each other, so they come last.
    let copies: Vec<String> = copies.into_iter().collect();
    let chunks_at = copies.len() + bundles.len();
    let script = |path: &str| {
        let entry = script_entries
            .iter()
            .position(|e| e == path)
            .expect("listed");
        scripts.entries[entry]
    };
    let index = |use_: &Use| -> usize {
        match use_ {
            Use::Copy(path) => copies.binary_search(path).expect("listed"),
            Use::Bundle(path) => {
                copies.len() + entries.iter().position(|e| e == path).expect("listed")
            }
            Use::Script(path) => chunks_at + script(path),
            Use::Url(_) => unreachable!("a URL is no asset"),
        }
    };
    let mut assets = Vec::new();
    for path in &copies {
        match copy(&project, path) {
            Ok(asset) => assets.push(asset),
            Err(problem) => diagnostics.push(problem.diagnostic(Span {
                file: config.file,
                range: 0..0,
            })),
        }
    }
    if diagnostics.failed() {
        return Stage::failed(diagnostics);
    }
    for ((name, _), (code, urls)) in bundles.iter().zip(codes) {
        let mut code = code;
        // lightningcss's placeholder for each `url()`, not the link pass's.
        for (written, use_) in urls {
            let url = match &use_ {
                Use::Url(url) => css_string(url),
                _ => format!("\"{}\"", placeholder(index(&use_))),
            };
            code = code.replacen(&format!("\"{written}\""), &url, 1);
        }
        assets.push(Asset::Linked {
            name: name.clone(),
            bytes: code.into_bytes(),
        });
    }
    assets.extend(scripts.assets);
    let sidecars_link = (!scopes.is_empty()).then(|| {
        format!(
            "<link rel=\"stylesheet\" href=\"{}\">",
            placeholder(copies.len() + entries.len())
        )
    });
    let page_chunk = |i: usize| scripts.entries[script_entries.len() + i];

    let (pages, seen): (Vec<(String, Vec<u8>)>, Vec<_>) = pages
        .into_par_iter()
        .zip(scans)
        .zip(edits)
        .zip(page_script)
        .map(|(((page, scan), edits), page_script)| {
            // Its Sidecar stylesheet and Page Script go after its own
            // stylesheets, and then a preload of every chunk its Scripts
            // import, so the browser fetches them all at once.
            let loaded: Vec<usize> = edits
                .iter()
                .filter_map(|(_, use_)| match use_ {
                    Use::Script(path) => Some(script(path)),
                    _ => None,
                })
                .chain(page_script.map(page_chunk))
                .collect();
            let mut preloads: Vec<usize> = Vec::new();
            for &chunk in &loaded {
                for &c in &scripts.preloads[chunk] {
                    if !loaded.contains(&c) && !preloads.contains(&c) {
                        preloads.push(c);
                    }
                }
            }
            let mut tags = sidecars_link.clone().unwrap_or_default();
            if config.router == RouterMode::Native {
                tags += "<style>@view-transition{navigation:auto}</style>";
            }
            if let Some(i) = page_script {
                let src = placeholder(chunks_at + page_chunk(i));
                // The Router swaps in only a Page that carries it.
                let marker = if router { " data-yuanme-router" } else { "" };
                tags += &format!("<script type=\"module\" src=\"{src}\"{marker}></script>");
            }
            for chunk in preloads {
                let href = placeholder(chunks_at + chunk);
                tags += &format!("<link rel=\"modulepreload\" href=\"{href}\">");
            }
            let mut edits: Vec<(Range<usize>, String)> = edits
                .into_iter()
                .map(|(at, use_)| {
                    let with = placeholder(index(&use_)) + suffix(page.text[at.clone()].trim());
                    (at, with)
                })
                .collect();
            let at = scan.as_ref().and_then(|s| s.scan.stylesheet_at);
            if let (false, Some(at)) = (tags.is_empty(), at) {
                edits.push((at..at, tags));
                edits.sort_by_key(|(at, _)| at.start);
            }
            let bytes = splice(&page.text, &edits);
            ((page.output, bytes), scan.map(|scan| (page.text, scan)))
        })
        .unzip();
    let seen = pages
        .iter()
        .zip(seen)
        .map(|((output, _), seen)| {
            let (text, seen) = seen?;
            if dev {
                cache.seen.insert(output.clone(), (text, seen.clone()));
            }
            Some(seen)
        })
        .collect();
    let client = dev.then(|| {
        let chunk = *scripts
            .entries
            .last()
            .expect("the client is the last entry");
        scripts.report[chunk].file.clone()
    });
    Stage {
        pages,
        seen,
        assets,
        client,
        scripts: scripts.report,
        diagnostics: diagnostics.0,
    }
}

/// A warning for a compound selector of a Sidecar that no tag its
/// Template writes can match.
fn unmatched(
    compound: &css::Compound,
    templates: &BTreeMap<String, FileId>,
    project: &Project<'_>,
    sources: &mut Files,
    files: &mut SourceFiles,
) -> Option<Diagnostic> {
    let mut own = templates
        .iter()
        .filter(|(name, _)| scope::sidecar(name) == compound.at.file)
        .peekable();
    let template = own.peek()?.0.clone();
    let matched = own.any(|(_, &file)| {
        scope::elements(sources.text(file))
            .iter()
            .any(|e| e.matches(compound.name.as_deref(), &compound.classes, &compound.ids))
    });
    if matched {
        return None;
    }
    let mut what = compound.name.clone().unwrap_or_default();
    for id in &compound.ids {
        what += &format!("#{id}");
    }
    for class in &compound.classes {
        what += &format!(".{class}");
    }
    if what.is_empty() {
        what = "*".into();
    }
    let at = &compound.at;
    let mut span = files.span(project, sources, &at.file, at.line, at.column, "");
    // The selectors, up to the rule's `{`.
    let text = sources.text(span.file);
    let end = text[span.range.start..]
        .find('{')
        .map_or(span.range.start, |i| {
            span.range.start + text[span.range.start..][..i].trim_end().len()
        });
    span.range.end = end;
    let message = format!("`{what}` matches no tag that `templates/{template}` writes");
    Some(
        Diagnostic::warning(&codes::ASSET_UNMATCHED_SELECTOR, message, span)
            .help("a Sidecar styles only the tags its own Template writes: style other elements with `:global()`"),
    )
}

/// `text` with each range replaced, in order.
fn splice(text: &str, edits: &[(Range<usize>, String)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut done = 0;
    for (at, with) in edits {
        out.extend_from_slice(&text.as_bytes()[done..at.start]);
        out.extend_from_slice(with.as_bytes());
        done = at.end;
    }
    out.extend_from_slice(&text.as_bytes()[done..]);
    out
}

#[derive(Clone)]
enum Use {
    Bundle(String),
    /// A Script, by the project file it starts from.
    Script(String),
    Copy(String),
    /// Written as is.
    Url(String),
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn note(page: &Page) -> String {
    format!("in the Page written to `{}`", page.output)
}

/// A CSS string, quoted.
fn css_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\a "),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
