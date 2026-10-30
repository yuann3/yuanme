//! The build report (#21), `.yuanme/report.md` and `.json`: every Page's
//! size, what went into each Script bundle (#20), how long each stage took,
//! what was written, and each Page's numbers against the Site budgets with
//! every breach and Acknowledged overage (#15).

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use yuanme_content::config::{Budget, Budgets};
use yuanme_diag::Level;
use yuanme_engine::{Body, Build, Measured};

use crate::sync;

/// A Page over one of its Site budgets.
pub struct Over {
    pub page: String,
    pub budget: Budget,
    pub value: u64,
    pub limit: u64,
    /// Its Acknowledged overage, or 0.
    pub allowance: u64,
    /// The overage allows it.
    pub acknowledged: bool,
}

/// Every breach, in the order of the Pages and then of the Budgets.
pub fn over(measured: &[Measured], budgets: &Budgets) -> Vec<Over> {
    let mut out = Vec::new();
    for page in measured {
        for (&budget, &value) in &page.values {
            let limit = budgets.limit(budget);
            let allowance = budgets.allowance(&page.url, budget);
            if value > limit {
                out.push(Over {
                    page: page.url.clone(),
                    budget,
                    value,
                    limit,
                    allowance,
                    acknowledged: value <= limit + allowance,
                });
            }
        }
    }
    out
}

pub struct Stages {
    pub load: Duration,
    pub build: Duration,
    pub write: Duration,
}

/// Writes both files into `dir`. `written` is `None` for a build that
/// failed, which writes nothing.
pub fn write(
    dir: &Path,
    root: &Path,
    build: &Build,
    stages: &Stages,
    written: Option<&sync::Report>,
) -> io::Result<()> {
    let size = |path: &str| -> io::Result<u64> {
        Ok(match &build.files[path].body {
            Body::Bytes(bytes) => bytes.len() as u64,
            Body::Input(input) => fs::metadata(root.join(input))?.len(),
            Body::Cached(file) => fs::metadata(file)?.len(),
        })
    };
    let mut pages = Vec::new();
    if let (Some(site), Some(config)) = (&build.site, &build.config)
        && !build.failed()
    {
        for route in &site.routes {
            let file = yuanme_engine::file_of(&config.host, &route.output);
            pages.push((route.url.as_str(), size(&file)?));
        }
    }
    // Largest first: what a reader of the report looks for.
    pages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let mut bytes = 0;
    for path in build.files.keys() {
        bytes += size(path)?;
    }
    let count = |level| {
        build
            .diagnostics
            .iter()
            .filter(|d| d.level == level)
            .count()
    };
    let ms = |d: Duration| d.as_millis();
    let stages_json = json!({
        "load_ms": ms(stages.load),
        "build_ms": ms(stages.build),
        "write_ms": ms(stages.write),
    });
    let mut report = json!({
        "ok": !build.failed(),
        "errors": count(Level::Error),
        "warnings": count(Level::Warning),
        "stages": stages_json,
        "pages": pages.iter().map(|(url, bytes)| json!({ "url": url, "bytes": bytes })).collect::<Vec<_>>(),
        "scripts": build.scripts.iter().map(|b| json!({
            "file": b.file,
            "bytes": b.size,
            "modules": b.modules.iter().map(|(m, bytes)| json!({ "module": m, "bytes": bytes })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "files": build.files.len(),
        "bytes": bytes,
        "written": written.map(|w| w.written.len()),
        "deleted": written.map(|w| w.deleted.len()),
        "unknown": written.map(|w| w.unknown.clone()),
        "budgets": budgets_json(build),
    });
    report.sort_all_objects();

    let mut md = String::from("# Build report\n\n");
    if build.failed() {
        md.push_str(&format!(
            "The build failed with {} errors and {} warnings, and wrote nothing.\n",
            count(Level::Error),
            count(Level::Warning)
        ));
    } else {
        md.push_str(&format!(
            "{} Pages and {} files, {} in all. {} errors, {} warnings.\n",
            pages.len(),
            build.files.len(),
            human(bytes),
            count(Level::Error),
            count(Level::Warning)
        ));
    }
    md.push_str("\n| Stage | Time |\n|---|---:|\n");
    for (stage, d) in [
        ("load", stages.load),
        ("build", stages.build),
        ("write", stages.write),
    ] {
        md.push_str(&format!("| {stage} | {} ms |\n", ms(d)));
    }
    if let Some(w) = written {
        md.push_str(&format!(
            "\n{} files written, {} deleted.\n",
            w.written.len(),
            w.deleted.len()
        ));
        if !w.unknown.is_empty() {
            md.push_str(
                "\nFiles in the output directory that yuanme didn't write, left as they are:\n\n",
            );
            for path in &w.unknown {
                md.push_str(&format!("- `{path}`\n"));
            }
        }
    }
    if !pages.is_empty() {
        md.push_str("\n## Pages\n\n| Page | Size |\n|---|---:|\n");
        for (url, bytes) in &pages {
            md.push_str(&format!("| `{url}` | {} |\n", human(*bytes)));
        }
    }

    if !build.scripts.is_empty() {
        md.push_str("\n## Scripts\n");
        for bundle in &build.scripts {
            md.push_str(&format!(
                "\n`{}`, {}\n\n| Module | Size |\n|---|---:|\n",
                bundle.file,
                human(bundle.size as u64)
            ));
            for (module, bytes) in &bundle.modules {
                md.push_str(&format!("| `{module}` | {} |\n", human(*bytes as u64)));
            }
        }
    }

    budgets_md(build, &mut md);

    fs::create_dir_all(dir)?;
    fs::write(dir.join("report.md"), md)?;
    fs::write(
        dir.join("report.json"),
        serde_json::to_string_pretty(&report)? + "\n",
    )
}

/// `null` unless the build measured the Site budgets.
fn budgets_json(build: &Build) -> Value {
    let Some(budgets) = build.config.as_ref().and_then(|c| c.budgets.as_ref()) else {
        return Value::Null;
    };
    if build.budgets.is_empty() {
        return Value::Null;
    }
    let limits: serde_json::Map<String, Value> = Budget::ALL
        .iter()
        .map(|b| (b.key().to_string(), budgets.limit(*b).into()))
        .collect();
    let pages: Vec<Value> = build
        .budgets
        .iter()
        .map(|m| {
            let mut page = json!({ "url": m.url, "body": m.body });
            for (b, value) in &m.values {
                page[b.key()] = (*value).into();
            }
            let mut estimated: Vec<&str> = m.estimated.iter().map(|b| b.key()).collect();
            if m.body.is_some() && m.estimated.contains(&Budget::Html) {
                estimated.insert(0, "body");
            }
            page["estimated"] = estimated.into();
            page
        })
        .collect();
    let over: Vec<Value> = over(&build.budgets, budgets)
        .iter()
        .map(|o| {
            json!({
                "page": o.page,
                "budget": o.budget.key(),
                "value": o.value,
                "limit": o.limit,
                "allowance": o.allowance,
                "acknowledged": o.acknowledged,
            })
        })
        .collect();
    json!({ "limits": limits, "pages": pages, "over": over })
}

/// `n` of what `budget` counts, in words.
fn phrase(budget: Budget, n: u64) -> String {
    let of = match budget {
        Budget::Html => "HTML",
        Budget::Css => "CSS",
        Budget::Js => "JS",
        Budget::Fonts => "text fonts",
        Budget::MathFont => "math font",
        Budget::Requests => return format!("{n} requests"),
    };
    format!("{} of {of}", human(n))
}

/// A number as its Budget counts it.
fn amount(budget: Budget, n: u64) -> String {
    match budget {
        Budget::Requests => n.to_string(),
        _ => human(n),
    }
}

fn budgets_md(build: &Build, md: &mut String) {
    let Some(budgets) = build.config.as_ref().and_then(|c| c.budgets.as_ref()) else {
        return;
    };
    if build.budgets.is_empty() {
        return;
    }
    let limits: Vec<String> = Budget::ALL
        .iter()
        .map(|b| phrase(*b, budgets.limit(*b)))
        .collect();
    md.push_str(&format!(
        "\n## Budgets\n\nBytes are Brotli at quality 11, but a number with `~` is under half its limit, and estimated at quality 5, which is about 15% more. The limits: {}. An Entry's body isn't part of its Page's HTML.\n\n",
        limits.join(", ")
    ));
    md.push_str("| Page | HTML | Body | CSS | JS | Text fonts | Math font | Requests |\n");
    md.push_str("|---|---:|---:|---:|---:|---:|---:|---:|\n");
    let over = over(&build.budgets, budgets);
    for page in &build.budgets {
        let estimated = |b: Budget| match page.estimated.contains(&b) {
            true => "~",
            false => "",
        };
        let cell = |b: Budget| {
            let n = page.values[&b];
            match over.iter().find(|o| o.page == page.url && o.budget == b) {
                Some(o) if o.acknowledged => format!("{}, acknowledged", amount(b, n)),
                Some(_) => format!("{}, over", amount(b, n)),
                None => format!("{}{}", estimated(b), amount(b, n)),
            }
        };
        let body = page
            .body
            .map(|n| format!("{}{}", estimated(Budget::Html), human(n)))
            .unwrap_or_default();
        md.push_str(&format!(
            "| `{}` | {} | {body} | {} | {} | {} | {} | {} |\n",
            page.url,
            cell(Budget::Html),
            cell(Budget::Css),
            cell(Budget::Js),
            cell(Budget::Fonts),
            cell(Budget::MathFont),
            cell(Budget::Requests),
        ));
    }
    if over.is_empty() {
        return;
    }
    md.push_str("\nOver budget:\n\n");
    for o in &over {
        let allowance = amount(o.budget, o.allowance);
        let ack = match (o.acknowledged, o.allowance) {
            (true, _) => format!(", within its Acknowledged overage of {allowance}"),
            (false, 0) => String::new(),
            (false, _) => format!(", beyond its Acknowledged overage of {allowance}"),
        };
        md.push_str(&format!(
            "- `{}`: {}, over its {}{ack}\n",
            o.page,
            phrase(o.budget, o.value),
            amount(o.budget, o.limit),
        ));
    }
}

fn human(bytes: u64) -> String {
    match bytes {
        b if b < 1_000 => format!("{b} B"),
        b if b < 1_000_000 => format!("{:.1} kB", b as f64 / 1e3),
        b => format!("{:.1} MB", b as f64 / 1e6),
    }
}
