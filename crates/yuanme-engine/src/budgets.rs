//! The Site budgets (#15, ADR 0004): each HTML Page of a build measured in
//! bytes of Brotli at quality 11, by type wherever they sit, and in
//! requests on its render path, against `[budgets]`. A breach is a warning;
//! strict mode, which the CLI applies, makes it an error. A number under
//! half its limit is estimated at quality 5 instead (#91).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use rayon::prelude::*;
use yuanme_assets::html::{self, Kind};
use yuanme_content::config::{self, Budget, Budgets, Config};
use yuanme_content::site::Site;
use yuanme_diag::{Diagnostic, Span, codes};

use crate::links::internal;
use crate::{Body, Cache, Measured, Output, host};

const FONTS: &[&str] = &[".woff2", ".woff", ".ttf", ".otf"];

/// Quality 11 takes about a second per MB, which was 61% of a cold build
/// of the Scale Site, and most numbers are far under their limits (#91).
/// Quality 5 takes 2–4% of that time. For one input of the corpus, the
/// Reference site or the Scale Site, quality 11 was 0.76–1.13 times it,
/// so only a number under half its limit is estimated: an estimate can't
/// hide a breach.
const ESTIMATE: i32 = 5;

/// What one Page's numbers are made of.
struct Plan<'a> {
    url: &'a str,
    /// Files of the build, each counted once per Budget.
    files: BTreeSet<(Budget, &'a str)>,
    /// The first-flight HTML, and the CSS and JS the Page holds inline.
    texts: Vec<(Budget, Cow<'a, str>)>,
    body: Option<&'a str>,
    /// Paths on the Site's origin, and URLs elsewhere.
    requests: BTreeSet<String>,
}

/// `bodies` is each Entry's Page's body, by output path; `math_font` the
/// math font's path, if a Page shows math.
pub(crate) fn measure(
    config: &Config,
    budgets: &Budgets,
    site: &Site,
    files: &BTreeMap<String, Output>,
    bodies: &BTreeMap<String, String>,
    math_font: Option<&str>,
    cache: &mut Cache,
) -> (Vec<Measured>, Vec<Diagnostic>) {
    let origin = config.site.url.trim_end_matches('/');
    let plans: Vec<Plan<'_>> = site
        .routes
        .par_iter()
        .filter(|r| r.output.ends_with(".html"))
        .filter_map(|r| {
            let text = text(files.get(&r.output)?)?;
            let body = bodies.get(&r.output).map(String::as_str);
            Some(plan(
                &r.url, &r.output, text, body, files, math_font, origin,
            ))
        })
        .collect();

    // Each distinct text once: a stylesheet is on most Pages. A part with
    // no Budget is an Entry's body.
    let mut blobs: Vec<&[u8]> = Vec::new();
    let mut index: HashMap<&[u8], usize> = HashMap::new();
    let mut id = |bytes| {
        *index.entry(bytes).or_insert_with(|| {
            blobs.push(bytes);
            blobs.len() - 1
        })
    };
    let parts: Vec<Vec<(Option<Budget>, usize)>> = plans
        .iter()
        .map(|p| {
            let files = p
                .files
                .iter()
                .filter_map(|(b, path)| match &files[*path].body {
                    Body::Bytes(bytes) => Some((Some(*b), &bytes[..])),
                    _ => None,
                });
            let texts = p.texts.iter().map(|(b, t)| (Some(*b), t.as_bytes()));
            let body = p.body.map(|b| (None, b.as_bytes()));
            files
                .chain(texts)
                .chain(body)
                .map(|(b, bytes)| (b, id(bytes)))
                .collect()
        })
        .collect();
    let sizes = |texts: &[&[u8]], quality, cache: &mut Cache| -> Vec<u64> {
        host::brotli(texts, quality, cache)
            .iter()
            .map(|br| br.len() as u64)
            .collect()
    };
    let estimates = sizes(&blobs, ESTIMATE, cache);
    // The body goes with its Page's HTML.
    let budget_of = |b: Option<Budget>| b.unwrap_or(Budget::Html);
    let estimated: Vec<BTreeSet<Budget>> = parts
        .iter()
        .map(|parts| {
            let mut sums: BTreeMap<Budget, u64> = BTreeMap::new();
            for (b, id) in parts {
                if let Some(b) = b {
                    *sums.entry(*b).or_default() += estimates[*id];
                }
            }
            // Not its limit and allowance: a number over its limit is
            // listed even when an Acknowledged overage allows it.
            sums.into_iter()
                .filter(|(b, sum)| 2 * sum <= budgets.limit(*b))
                .map(|(b, _)| b)
                .collect()
        })
        .collect();
    let mut wanted = vec![false; blobs.len()];
    for (parts, estimated) in parts.iter().zip(&estimated) {
        for (b, id) in parts {
            wanted[*id] |= !estimated.contains(&budget_of(*b));
        }
    }
    let ids: Vec<usize> = (0..blobs.len()).filter(|i| wanted[*i]).collect();
    let texts: Vec<&[u8]> = ids.iter().map(|i| blobs[*i]).collect();
    let mut exact = vec![0; blobs.len()];
    for (i, size) in ids.into_iter().zip(sizes(&texts, 11, cache)) {
        exact[i] = size;
    }

    let mut measured = Vec::with_capacity(plans.len());
    let mut diagnostics = Vec::new();
    for ((plan, parts), estimated) in plans.iter().zip(parts).zip(estimated) {
        let mut values: BTreeMap<Budget, u64> = Budget::ALL.map(|b| (b, 0)).into();
        let mut body = None;
        for (budget, id) in parts {
            let size = match estimated.contains(&budget_of(budget)) {
                true => estimates[id],
                false => exact[id],
            };
            match budget {
                Some(b) => *values.get_mut(&b).expect("every Budget") += size,
                None => body = Some(size),
            }
        }
        // The Page itself is a request too.
        values.insert(Budget::Requests, 1 + plan.requests.len() as u64);
        for (&budget, &value) in &values {
            if value > budgets.limit(budget) + budgets.allowance(plan.url, budget) {
                diagnostics.push(breach(config, budgets, plan.url, budget, value));
            }
        }
        measured.push(Measured {
            url: plan.url.to_string(),
            values,
            body,
            estimated,
        });
    }
    let urls: Vec<&str> = measured.iter().map(|m| m.url.as_str()).collect();
    let mut named = BTreeSet::new();
    for o in &budgets.overages {
        if urls.contains(&o.page.as_str()) || !named.insert(&o.page) {
            continue;
        }
        let span = Span {
            file: config.file,
            range: o.page_at.clone(),
        };
        let d = Diagnostic::warning(
            &codes::BUDGET_UNKNOWN_PAGE,
            format!("`{}` is no HTML Page of the Site", o.page),
            span,
        );
        diagnostics.push(match config::similar(&o.page, urls.iter().copied()) {
            Some(near) => d.help(format!("did you mean `{near}`?")),
            None => d.help("use the Page's URL, as `yuanme routes` lists it"),
        });
    }
    (measured, diagnostics)
}

fn text(output: &Output) -> Option<&str> {
    match &output.body {
        Body::Bytes(bytes) => std::str::from_utf8(bytes).ok(),
        _ => None,
    }
}

fn plan<'a>(
    url: &'a str,
    output: &str,
    text: &'a str,
    body: Option<&'a str>,
    files: &'a BTreeMap<String, Output>,
    math_font: Option<&str>,
    origin: &str,
) -> Plan<'a> {
    let base = format!("/{output}");
    let mut plan = Plan {
        url,
        files: BTreeSet::new(),
        texts: Vec::new(),
        body: None,
        requests: BTreeSet::new(),
    };
    // The body of an Entry's Page isn't in its first flight (#15).
    let first_flight = match body.and_then(|b| Some((b, text.find(b)?))) {
        Some((b, at)) if !b.is_empty() => {
            plan.body = Some(b);
            Cow::Owned([&text[..at], &text[at + b.len()..]].concat())
        }
        _ => Cow::Borrowed(text),
    };
    plan.texts.push((Budget::Html, first_flight));
    // `path` from the root.
    let font = |path: &str| match Some(&path[1..]) == math_font {
        true => Budget::MathFont,
        false => Budget::Fonts,
    };
    let mut stylesheets = Vec::new();
    let scan = html::scan(text);
    for r in &scan.references {
        let link = r.url.trim();
        let budget = match r.kind {
            Kind::Stylesheet => Some(Budget::Css),
            Kind::Module | Kind::Classic => Some(Budget::Js),
            Kind::Link if r.preload => None,
            Kind::Link => continue,
        };
        let Some((path, _)) = internal(link, &base, origin) else {
            if remote(link) {
                plan.requests.insert(link.to_string());
            }
            continue;
        };
        let file = &path[1..];
        let budget = budget.or_else(|| {
            let ext = |e: &str| file.ends_with(e);
            if ext(".css") {
                Some(Budget::Css)
            } else if ext(".js") || ext(".mjs") {
                Some(Budget::Js)
            } else {
                FONTS.iter().any(|e| ext(e)).then(|| font(&path))
            }
        });
        if let (Some(budget), Some((file, _))) = (budget, files.get_key_value(file)) {
            plan.files.insert((budget, file.as_str()));
            if budget == Budget::Css {
                stylesheets.push(file.as_str());
            }
        }
        plan.requests.insert(path);
    }
    let mut css = Vec::new();
    for inline in &scan.inline {
        let code = &text[inline.at.clone()];
        let budget = match inline.script {
            true => Budget::Js,
            false => {
                css.push((code, base.clone()));
                Budget::Css
            }
        };
        plan.texts.push((budget, Cow::Borrowed(code)));
    }
    for path in stylesheets {
        if let Some(code) = files.get(path).and_then(self::text) {
            css.push((code, format!("/{path}")));
        }
    }
    let is_font = |url: &str| {
        let path = &url[..url.len() - crate::assets::suffix(url).len()];
        FONTS.iter().any(|e| path.ends_with(e))
    };
    for (code, base) in css {
        for link in css_urls(code).into_iter().filter(|u| is_font(u)) {
            let Some((path, _)) = internal(link, &base, origin) else {
                if remote(link) {
                    plan.requests.insert(link.to_string());
                }
                continue;
            };
            if let Some((file, _)) = files.get_key_value(&path[1..]) {
                plan.files.insert((font(&path), file.as_str()));
            }
            plan.requests.insert(path);
        }
    }
    plan
}

/// A URL on another origin, which a Page fetches but whose bytes the
/// Engine doesn't have.
fn remote(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("//")
}

/// Every `url()` in a stylesheet, as written. A bundle has no comments.
fn css_urls(css: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = css;
    while let Some(i) = rest.find("url(") {
        rest = rest[i + 4..].trim_start();
        let (url, after) = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => match rest[1..].find(q) {
                Some(end) => (&rest[1..1 + end], &rest[2 + end..]),
                None => break,
            },
            _ => match rest.find(')') {
                Some(end) => (rest[..end].trim_end(), &rest[end..]),
                None => break,
            },
        };
        out.push(url);
        rest = after;
    }
    out
}

fn kb(bytes: u64) -> String {
    format!("{} kB", bytes as f64 / 1000.0)
}

/// `amount` as a number for `[budgets]`: kB, rounded up to a tenth, or
/// requests.
fn toml_amount(budget: Budget, amount: u64) -> String {
    match budget {
        Budget::Requests => amount.to_string(),
        _ => (amount.div_ceil(100) as f64 / 10.0).to_string(),
    }
}

fn breach(config: &Config, budgets: &Budgets, url: &str, budget: Budget, value: u64) -> Diagnostic {
    let limit = budgets.limit(budget);
    let (what, amount) = match budget {
        Budget::Html => ("has", format!("{} of first-flight HTML", kb(value))),
        Budget::Css => ("loads", format!("{} of CSS", kb(value))),
        Budget::Js => ("loads", format!("{} of JS", kb(value))),
        Budget::Fonts => ("loads", format!("{} of text fonts", kb(value))),
        Budget::MathFont => ("loads", format!("{} of math font", kb(value))),
        Budget::Requests => ("makes", format!("{value} requests on its render path")),
    };
    let limit_text = match budget {
        Budget::Requests => limit.to_string(),
        _ => kb(limit),
    };
    let message = format!("`{url}` {what} {amount}, over its budget of {limit_text}");
    let overage = budgets.overage(url, budget);
    let range = overage
        .map(|o| o.at.clone())
        .or_else(|| budgets.limits[&budget].1.clone())
        .unwrap_or(0..0);
    let span = Span {
        file: config.file,
        range,
    };
    let code = match budget {
        Budget::Html => &codes::BUDGET_HTML,
        Budget::Css => &codes::BUDGET_CSS,
        Budget::Js => &codes::BUDGET_JS,
        Budget::Fonts => &codes::BUDGET_FONTS,
        Budget::MathFont => &codes::BUDGET_MATH_FONT,
        Budget::Requests => &codes::BUDGET_REQUESTS,
    };
    let need = toml_amount(budget, value - limit);
    let key = budget.key();
    let smaller = match budget {
        Budget::Requests => "make fewer",
        _ => "make it smaller",
    };
    let d = Diagnostic::warning(code, message, span);
    match overage {
        Some(o) => d
            .note(format!(
                "its Acknowledged overage allows {} more",
                toml_amount(budget, o.allowance)
            ))
            .help(format!(
                "{smaller}, or raise the overage to `{key} = {need}`"
            )),
        None => d.help(format!(
            "{smaller}, raise `budgets.{key}`, or acknowledge it: `[budgets.overages.\"{url}\"] {key} = {need}`"
        )),
    }
}
