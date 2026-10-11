//! Hand-written schema (after #18's renames), fake Entries, context Values, the Environment
//! and the Engine filters (`sort_by`, `attrs`, `pluralize`). Throwaway.
use crate::check::{obj, Field, Obj, RouteKind, Ty};
use minijinja::syntax::SyntaxConfig;
use minijinja::value::{Kwargs, Value, ValueKind};
use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior};
use serde_json::{json, Value as J};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::rc::Rc;

// ---------------------------------------------------------------- schema

pub struct Schema {
    pub globals: Vec<(String, Ty)>,
    pub kinds: Vec<RouteKind>,
    pub named: BTreeMap<String, Ty>,
}

fn builtin(fields: &mut Vec<(&'static str, Ty, bool)>) {
    // built-in on every Entry (#18) + derived
    for (n, t, o) in [
        ("title", Ty::Str, false),
        ("slug", Ty::Str, false),
        ("draft", Ty::Bool, false),
        ("unlisted", Ty::Bool, false),
        ("updated", Ty::Date, true),
        ("url", Ty::Url, false),
        ("content", Ty::Slot, false),
        ("modified", Ty::Date, false),
        ("word_count", Ty::Int, false),
        ("reading_time", Ty::Int, false),
    ] {
        fields.push((n, t, o));
    }
    fields.push((
        "headings",
        Ty::List(Box::new(obj("Heading", &[("id", Ty::Str, false), ("text", Ty::Str, false), ("level", Ty::Int, false)]))),
        false,
    ));
}

pub fn schema() -> Schema {
    let mut post = vec![
        ("date", Ty::Date, false), // posts make `date` required
        ("description", Ty::Str, false),
        ("hero", obj("Post.hero", &[("src", Ty::Image, false), ("alt", Ty::Str, true)]), true),
        ("og_image", Ty::Str, true),
        ("tags", Ty::List(Box::new(Ty::Str)), false), // taxonomy field, defaults to []
        ("series", Ty::Str, true),                    // single taxonomy
        ("order", Ty::Int, true),
    ];
    builtin(&mut post);
    let post = obj("Post", &post);
    let mut project = vec![
        ("date", Ty::Date, false),
        ("description", Ty::Str, false),
        ("summary", Ty::Str, true),
        ("summary_short", Ty::Str, true),
        ("hero", obj("Project.hero", &[("url", Ty::Image, false), ("alt", Ty::Str, true)]), true),
        ("stack", Ty::List(Box::new(Ty::Str)), false),
        ("website", Ty::Url, true),
        ("github", Ty::Url, true),
        ("order", Ty::Int, true),
    ];
    builtin(&mut project);
    let project = obj("Project", &project);

    let term = obj("Term", &[("name", Ty::Str, false), ("slug", Ty::Str, false), ("url", Ty::Url, false), ("count", Ty::Int, false)]);
    let taxonomy = obj("Taxonomy", &[("name", Ty::Str, false), ("url", Ty::Url, false), ("terms", Ty::List(Box::new(term.clone())), false)]);
    let paginator = obj("Paginator", &[("number", Ty::Int, false), ("total", Ty::Int, false), ("prev", Ty::Url, true), ("next", Ty::Url, true)]);
    let collection = obj("Collection", &[("name", Ty::Str, false), ("url", Ty::Url, false)]);
    let page = obj("page", &[("url", Ty::Url, false), ("path", Ty::Str, false), ("canonical", Ty::Url, false), ("kind", Ty::Str, false)]);

    // site.extra and Data files have no declared schema: infer a structural one from the values.
    let site = obj(
        "site",
        &[
            ("url", Ty::Url, false),
            ("title", Ty::Str, false),
            ("description", Ty::Str, false),
            ("language", Ty::Str, false),
            ("author", Ty::Str, false),
            ("timezone", Ty::Str, false),
            ("extra", infer("site.extra", &site_json()["extra"]), false),
        ],
    );
    let data = infer("data", &data_json());
    let collections = obj(
        "collections",
        &[("posts", Ty::List(Box::new(post.clone())), false), ("projects", Ty::List(Box::new(project.clone())), false)],
    );
    let taxonomies = obj("taxonomies", &[("tags", taxonomy.clone(), false), ("series", taxonomy.clone(), false)]);

    let globals = vec![
        ("site".to_string(), site),
        ("data".to_string(), data),
        ("collections".to_string(), collections),
        ("taxonomies".to_string(), taxonomies),
        ("page".to_string(), page),
    ];
    let posts_l = Ty::List(Box::new(post.clone()));
    let kinds = vec![
        RouteKind { name: "entry(posts)".into(), template: "posts/entry.html".into(), vars: vec![("entry".into(), post.clone(), false)] },
        RouteKind {
            name: "list(posts)".into(),
            template: "posts/list.html".into(),
            vars: vec![("collection".into(), collection.clone(), false), ("entries".into(), posts_l.clone(), false), ("paginator".into(), paginator.clone(), false)],
        },
        RouteKind {
            name: "taxonomy-index(tags)".into(),
            template: "tags/index.html".into(),
            vars: vec![("taxonomy".into(), taxonomy.clone(), false), ("terms".into(), Ty::List(Box::new(term.clone())), false)],
        },
        RouteKind {
            name: "term(series)".into(),
            template: "series/term.html".into(),
            vars: vec![
                ("taxonomy".into(), taxonomy.clone(), false),
                ("term".into(), term.clone(), false),
                ("entries".into(), posts_l.clone(), false),
                ("paginator".into(), paginator.clone(), false),
            ],
        },
        RouteKind { name: "entry(projects)".into(), template: "projects/entry.html".into(), vars: vec![("entry".into(), project.clone(), false)] },
    ];
    let mut named = BTreeMap::new();
    named.insert("entry<posts>".to_string(), post);
    named.insert("entry<projects>".to_string(), project);
    named.insert("term".to_string(), term);
    named.insert("paginator".to_string(), paginator);
    Schema { globals, kinds, named }
}

fn infer(name: &str, v: &J) -> Ty {
    match v {
        J::String(_) => Ty::Str,
        J::Bool(_) => Ty::Bool,
        J::Number(n) => if n.is_i64() { Ty::Int } else { Ty::Float },
        J::Null => Ty::Any,
        J::Array(a) => Ty::List(Box::new(a.first().map(|x| infer(&format!("{name}[]"), x)).unwrap_or(Ty::Any))),
        J::Object(m) => {
            let mut fields = BTreeMap::new();
            for (k, x) in m {
                fields.insert(k.clone(), Field { ty: infer(&format!("{name}.{k}"), x), optional: false });
            }
            Ty::Obj(Rc::new(Obj { name: name.to_string(), fields, open: false }))
        }
    }
}

// ---------------------------------------------------------------- data

fn site_json() -> J {
    json!({
        "url": "https://www.eyuan.me", "title": "Eyuan", "description": "YY's Cave", "language": "en-US",
        "author": "Yiyuan Li", "timezone": "Asia/Singapore",
        "extra": { "greeting": "Yiyuan's here, welcome!", "sub": "i'm a software engineer based in singapore" }
    })
}
fn data_json() -> J {
    json!({
        "nav": [
            {"id": "about", "title": "About", "path": "/about"},
            {"id": "projects", "title": "Projects", "path": "/projects"},
            {"id": "writing", "title": "Writing", "path": "/blog"},
            {"id": "resume", "title": "Resume", "path": "/resume"}],
        "footer_links": [
            {"title": "EMAIL", "link": "mailto:yy@eyuan.me"},
            {"title": "GITHUB", "link": "https://github.com/yuann3"},
            {"title": "X", "link": "https://twitter.com/eyuann3"},
            {"title": "RSS", "link": "/rss.xml"}],
        "photos": [
            {"src": "/_assets/sentosa", "w": 1200, "h": 1600, "caption": "sentosa", "alt": "Palm trees", "fx": 0.02, "fy": 0.01, "r": -4, "t": -2},
            {"src": "/_assets/market", "w": 1600, "h": 1200, "caption": "beach market", "alt": "Shade sails", "fx": 0.9, "fy": 0.25, "r": 5, "t": 1.5},
            {"src": "/_assets/hornbills", "w": 900, "h": 1600, "caption": "hornbills", "alt": "Two hornbills", "fx": 0.08, "fy": 0.84, "r": 3, "t": -1.5},
            {"src": "/_assets/phuket", "w": 1200, "h": 1600, "caption": "phuket old town", "alt": "Old town", "fx": 0.92, "fy": 0.97, "r": -6, "t": 2.5}],
        "film": { "rules": [
            {"selector": "html:not([data-section]) .film-fallback", "gradient": "linear-gradient(#fff,#eee)"},
            {"selector": "html[data-section=\"writing\"] .film-fallback", "gradient": "linear-gradient(#eef,#fee)"}] }
    })
}

/// JSON -> minijinja Value. `content` becomes a safe string. Absent optional fields are simply absent
/// (undefined, not null), per #18.
pub fn j2v(v: &J) -> Value {
    match v {
        J::Null => Value::from(()),
        J::Bool(b) => Value::from(*b),
        J::Number(n) => n.as_i64().map(Value::from).unwrap_or_else(|| Value::from(n.as_f64().unwrap())),
        J::String(s) => Value::from(s.as_str()),
        J::Array(a) => Value::from(a.iter().map(j2v).collect::<Vec<_>>()),
        J::Object(m) => Value::from(
            m.iter()
                .map(|(k, x)| (k.clone(), if k == "content" { Value::from_safe_string(x.as_str().unwrap().to_string()) } else { j2v(x) }))
                .collect::<BTreeMap<String, Value>>(),
        ),
    }
}

fn ymd(days: i64) -> String {
    // civil from days since 1970-01-01 (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

/// Stands in for rendered Markdown. Every element carries data-md (or sits under one) and reuses the class
/// names the sidecars style, so any scoped rule that reaches it without :global() is a leak.
pub const ADV_CONTENT: &str = r##"<h1 data-md>Title again</h1>
<p data-md>Body with <em>emphasis</em>, <i>italics</i> and <a class="meta-link text-ink-2" href="#">a link <span class="meta-arrow">↗</span></a>.</p>
<h1 data-md>Second h1</h1>
<article data-md class="project-body"><h1>nested h1</h1><em>nested em</em></article>
<nav data-md><a class="page-link" href="#">Prev</a></nav>
<ul data-md><li><a class="row row-link project list-row" href="#"><span class="project-name">n</span><span class="project-summary">s</span><span class="project-year">2020</span><span class="row-arrow project-arrow list-row-arrow">→</span></a></li></ul>
<div data-md class="about-body"><div class="pile"><div class="pile-stage"><figure class="photo photo-tall"><picture><img src="/x.jpg" alt=""></picture><figcaption>md caption</figcaption></figure></div></div></div>
"##;

fn finish_entry(mut e: J, base: &str) -> J {
    let o = e.as_object_mut().unwrap();
    let slug = o["slug"].as_str().unwrap().to_string();
    let modified = o.get("updated").cloned().unwrap_or_else(|| o["date"].clone());
    o.insert("modified".into(), modified);
    o.insert("url".into(), json!(format!("{base}{slug}/")));
    o.entry("draft").or_insert(json!(false));
    o.entry("unlisted").or_insert(json!(false));
    let content = o.entry("content").or_insert(json!(ADV_CONTENT)).as_str().unwrap().to_string();
    let wc = content.split_whitespace().count() as i64;
    o.insert("word_count".into(), json!(wc));
    o.insert("reading_time".into(), json!((wc / 200).max(1)));
    o.insert("headings".into(), json!([{"id": "intro", "text": "Intro", "level": 2}]));
    e
}

/// The small site: the two Reference posts plus fakes that exercise pins, ties, updated, series, hero.
pub fn small_posts() -> Vec<J> {
    let raw = vec![
        json!({"slug": "extending_c_stdlib", "title": "Stealing Ideas from Rust and C++ for Your C Standard Library", "description": "d", "date": "2025-12-12", "order": 2, "tags": ["c", "programming"]}),
        json!({"slug": "so_here_we_go_again", "title": "So, Here we go again", "description": "d", "date": "2024-07-01", "tags": ["life", "draft"]}),
        json!({"slug": "pinned-first", "title": "Pinned first", "description": "d", "date": "2023-01-01", "order": 1, "tags": ["life"], "series": "c-notes"}),
        json!({"slug": "updated-late", "title": "Updated late", "description": "d", "date": "2022-05-05", "updated": "2026-02-02", "tags": ["c"], "series": "c-notes",
               "hero": {"src": "/img/hero.png", "alt": "A hero"}, "og_image": "/og/custom.png"}),
        json!({"slug": "tie-b", "title": "Tie B", "description": "d", "date": "2025-03-03", "tags": ["rust"], "hero": {"src": "/img/b.png"}}),
        json!({"slug": "tie-a", "title": "Tie A <script>", "description": "d & d", "date": "2025-03-03", "tags": ["rust", "c"], "series": "c-notes"}),
        json!({"slug": "hidden", "title": "Hidden", "description": "d", "date": "2026-01-01", "unlisted": true, "tags": []}),
        json!({"slug": "a-draft", "title": "A draft", "description": "d", "date": "2025-06-06", "draft": true, "tags": ["life"]}),
    ];
    raw.into_iter().map(|e| finish_entry(e, "/blog/")).collect()
}

pub const PROJECT_ORDER: [&str; 13] = ["emaclaude", "mra", "shaderview", "linear-app", "sequel", "rego", "rayrust", "lrle", "shell", "ruskey", "pew", "hrust", "ylib"];

pub fn projects() -> Vec<J> {
    // slugs and dates from the Reference content (pubDate rewritten in ISO form)
    let raw = [
        ("lrle", "LRLE", "2025-12-22", vec!["rust", "wgsl", "wgpu"]),
        ("linear-app", "linear-app.el", "2026-01-10", vec!["elisp"]),
        ("hrust", "HRust", "2024-09-26", vec!["rust"]),
        ("emaclaude", "emaclaude", "2026-03-15", vec!["rust", "elisp"]),
        ("rayrust", "RayRust", "2025-02-28", vec!["rust", "wgsl", "wgpu"]),
        ("rego", "Rego", "2025-05-07", vec!["go"]),
        ("pew", "Pew", "2025-04-04", vec!["go"]),
        ("mra", "mra", "2026-02-20", vec!["rust", "tokio"]),
        ("shaderview", "shaderview", "2026-03-01", vec!["rust", "wgpu", "glsl"]),
        ("shell", "Shell", "2025-07-02", vec!["rust"]),
        ("ruskey", "Ruskey", "2025-03-06", vec!["rust"]),
        ("sequel", "Sequel", "2025-05-23", vec!["rust"]),
        ("ylib", "Ylib", "2024-06-30", vec!["c"]),
    ];
    raw.iter()
        .enumerate()
        .map(|(i, (slug, title, date, stack))| {
            let mut e = json!({"slug": slug, "title": title, "description": format!("{title} description"), "date": date, "stack": stack,
                "order": PROJECT_ORDER.iter().position(|s| s == slug).unwrap() + 1});
            if i % 2 == 0 { e["github"] = json!(format!("https://github.com/yuann3/{slug}")); }
            if i % 3 == 0 { e["website"] = json!(format!("https://{slug}.example")); }
            if i % 4 == 0 { e["hero"] = json!({"url": format!("/img/{slug}.png")}); }
            finish_entry(e, "/projects/")
        })
        .collect()
}

pub fn fake_posts(n: usize) -> Vec<J> {
    let tags = ["rust", "c", "life", "emacs", "wgpu", "go", "math", "org", "tools", "perf", "notes", "linux"];
    let para = "<p>Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p>\n";
    (0..n)
        .map(|i| {
            let mut e = json!({
                "slug": format!("post-{i:04}"), "title": format!("Fake post number {i}"),
                "description": format!("Description of fake post {i}, with an & and a <tag>"),
                "date": ymd(18262 + (i as i64 * 37) % 2300),
                "tags": [tags[i % 12], tags[(i * 7 + 3) % 12]],
                "content": para.repeat(20 + i % 10),
            });
            if i % 7 == 0 { e["updated"] = json!(ymd(18262 + 2300 + (i as i64 % 200))); }
            if i == 3 { e["order"] = json!(1); }
            if i == 10 || i == 11 { e["order"] = json!(2); }
            if i % 10 == 0 { e["series"] = json!(["rust-notes", "c-stdlib"][(i / 10) % 2]); }
            if i % 9 == 0 { e["hero"] = if i % 2 == 0 { json!({"src": "/img/h.png", "alt": "alt"}) } else { json!({"src": "/img/h.png"}) }; }
            if i % 50 == 49 { e["unlisted"] = json!(true); }
            if i % 40 == 39 { e["draft"] = json!(true); }
            finish_entry(e, "/blog/")
        })
        .collect()
}

// ---------------------------------------------------------------- sort_by (the Engine sort, #18 rules)

/// keys: `-` prefix = descending; items missing a key sort last (in either direction); ties by slug.
pub fn sort_values(items: &mut [Value], keys: &[String]) {
    let get = |v: &Value, k: &str| v.get_attr(k).ok().filter(|x| !x.is_undefined() && !x.is_none());
    items.sort_by(|a, b| {
        for k in keys {
            let (desc, k) = match k.strip_prefix('-') { Some(k) => (true, k), None => (false, k.as_str()) };
            match (get(a, k), get(b, k)) {
                (None, None) => continue,
                (None, Some(_)) => return Ordering::Greater,
                (Some(_), None) => return Ordering::Less,
                (Some(x), Some(y)) => {
                    let o = if desc { y.cmp(&x) } else { x.cmp(&y) };
                    if o != Ordering::Equal { return o; }
                }
            }
        }
        get(a, "slug").cmp(&get(b, "slug"))
    });
}

fn sort_by(v: Value, keys: Vec<String>) -> Result<Value, Error> {
    let mut items: Vec<Value> = v.try_iter()?.collect();
    sort_values(&mut items, &keys);
    Ok(Value::from(items))
}

fn attrs(v: Value) -> Result<Value, Error> {
    if v.is_undefined() || v.is_none() { return Ok(Value::from_safe_string(String::new())); }
    if v.kind() != ValueKind::Map { return Err(Error::new(ErrorKind::InvalidOperation, "attrs expects a map")); }
    let mut out = String::new();
    for k in v.try_iter()? {
        let val = v.get_item(&k)?;
        if val.is_undefined() || val.is_none() || (val.kind() == ValueKind::Bool && !val.is_true()) { continue; }
        out.push(' ');
        out.push_str(k.as_str().unwrap_or(""));
        if val.kind() == ValueKind::Bool { continue; }
        out.push_str("=\"");
        out.push_str(&val.to_string().replace('&', "&amp;").replace('"', "&quot;"));
        out.push('"');
    }
    Ok(Value::from_safe_string(out))
}

// ---------------------------------------------------------------- environment

pub fn make_env(sources: &BTreeMap<String, String>, ub: UndefinedBehavior) -> Environment<'static> {
    let mut env = Environment::new();
    minijinja_contrib::add_to_environment(&mut env);
    env.set_undefined_behavior(ub);
    env.set_debug(true);
    env.set_syntax(SyntaxConfig::builder().trim_blocks(true).lstrip_blocks(true).build().unwrap());
    let src = sources.clone();
    env.set_loader(move |name: &str| Ok(src.get(name).cloned()));
    // `none` prints as empty; html-escaper-style escaping (no `&#x2f;`, `&#39;` for the apostrophe)
    env.set_formatter(|out, state, value| {
        if value.is_none() {
            return Ok(());
        }
        if value.is_safe() || !matches!(state.auto_escape(), minijinja::AutoEscape::Html) {
            return minijinja::escape_formatter(out, state, value);
        }
        let s = value.to_string();
        let mut last = 0;
        for (i, c) in s.char_indices() {
            let rep = match c {
                '&' => "&amp;",
                '<' => "&lt;",
                '>' => "&gt;",
                '"' => "&quot;",
                '\'' => "&#39;",
                _ => continue,
            };
            out.write_str(&s[last..i]).and_then(|_| out.write_str(rep)).map_err(|_| Error::new(ErrorKind::WriteFailure, "write"))?;
            last = i + 1;
        }
        out.write_str(&s[last..]).map_err(|_| Error::new(ErrorKind::WriteFailure, "write"))
    });
    env.add_filter("sort_by", sort_by);
    env.add_filter("attrs", attrs);
    env.add_filter("pluralize", |n: i64, one: String, many: String| if n == 1 { one } else { many });
    env.add_function("asset", |name: String| format!("/_assets/{name}"));
    env.add_function("url", |path: String| path);
    env.add_function("absolute_url", |path: String, _kw: Kwargs| format!("https://www.eyuan.me{path}"));
    env.add_global("TIMEZONE", "Asia/Singapore");
    env
}

// ---------------------------------------------------------------- site (routes + contexts)

pub struct Route {
    pub url: String,
    pub template: &'static str,
    pub ctx: Value,
}

fn vmap(pairs: Vec<(&str, Value)>) -> Value {
    Value::from(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<BTreeMap<String, Value>>())
}

fn slugify(s: &str) -> String {
    s.to_lowercase().replace(' ', "-")
}

/// Builds every Value once (Collections shared between pages) and one small context map per route.
pub fn build_site(posts: Vec<J>, projects: Vec<J>, page_size: usize) -> Vec<Route> {
    let site = j2v(&site_json());
    let data = j2v(&data_json());
    let mut post_vals: Vec<Value> = posts.iter().map(j2v).collect();
    let mut project_vals: Vec<Value> = projects.iter().map(j2v).collect();
    let listed = |v: &Value| !v.get_attr("unlisted").map(|x| x.is_true()).unwrap_or(false);
    let mut listed_posts: Vec<Value> = post_vals.iter().filter(|v| listed(v)).cloned().collect();
    sort_values(&mut listed_posts, &["order".into(), "-modified".into()]);
    sort_values(&mut post_vals, &["slug".into()]);
    sort_values(&mut project_vals, &["order".into(), "-modified".into()]);

    let taxo = |field: &str, base: &str| -> (Value, Vec<(Value, Vec<Value>)>) {
        let mut terms: BTreeMap<String, (String, Vec<Value>)> = BTreeMap::new();
        for p in &listed_posts {
            let f = p.get_attr(field).unwrap();
            let names: Vec<String> = if f.is_undefined() { vec![] } else if f.kind() == ValueKind::Seq { f.try_iter().unwrap().map(|x| x.to_string()).collect() } else { vec![f.to_string()] };
            for n in names {
                terms.entry(slugify(&n)).or_insert_with(|| (n.clone(), vec![])).1.push(p.clone());
            }
        }
        let list: Vec<(Value, Vec<Value>)> = terms
            .into_iter()
            .map(|(slug, (name, es))| {
                (vmap(vec![("name", Value::from(name)), ("slug", Value::from(slug.clone())), ("url", Value::from(format!("{base}{slug}/"))), ("count", Value::from(es.len()))]), es)
            })
            .collect();
        let t = vmap(vec![("name", Value::from(field)), ("url", Value::from(base)), ("terms", Value::from(list.iter().map(|x| x.0.clone()).collect::<Vec<_>>()))]);
        (t, list)
    };
    let (tags, tag_terms) = taxo("tags", "/tags/");
    let (series, series_terms) = taxo("series", "/series/");
    let collections = vmap(vec![("posts", Value::from(listed_posts.clone())), ("projects", Value::from(project_vals.clone()))]);
    let taxonomies = vmap(vec![("tags", tags.clone()), ("series", series.clone())]);

    let mut routes = Vec::new();
    let mut add = |url: String, kind: &str, template: &'static str, extra: Vec<(&str, Value)>| {
        let page = vmap(vec![
            ("url", Value::from(url.clone())),
            ("path", Value::from(if url.len() > 1 { url.trim_end_matches('/').to_string() } else { url.clone() })),
            ("canonical", Value::from(format!("https://www.eyuan.me{url}"))),
            ("kind", Value::from(kind)),
        ]);
        let mut pairs = vec![("site", site.clone()), ("data", data.clone()), ("collections", collections.clone()), ("taxonomies", taxonomies.clone()), ("page", page)];
        pairs.extend(extra);
        routes.push(Route { url, template, ctx: vmap(pairs) });
    };
    for p in &post_vals {
        add(p.get_attr("url").unwrap().to_string(), "entry", "posts/entry.html", vec![("entry", p.clone())]);
    }
    for p in &project_vals {
        add(p.get_attr("url").unwrap().to_string(), "entry", "projects/entry.html", vec![("entry", p.clone())]);
    }
    let paginator = |i: usize, total: usize, base: &str| {
        let u = |n: usize| if n == 1 { base.to_string() } else { format!("{base}{n}/") };
        let mut m = vec![("number", Value::from(i)), ("total", Value::from(total))];
        if i > 1 { m.push(("prev", Value::from(u(i - 1)))); }
        if i < total { m.push(("next", Value::from(u(i + 1)))); }
        (u(i), vmap(m))
    };
    add("/projects/".into(), "list", "projects/index.html", vec![("entries", Value::from(project_vals.clone()))]);
    add("/about/".into(), "page", "about.html", vec![]);
    add("/adversarial/".into(), "page", "adversarial.html", vec![]);
    let coll = vmap(vec![("name", Value::from("posts")), ("url", Value::from("/blog/"))]);
    let chunks: Vec<&[Value]> = if listed_posts.is_empty() { vec![&[]] } else { listed_posts.chunks(page_size).collect() };
    for (i, c) in chunks.iter().enumerate() {
        let (url, pg) = paginator(i + 1, chunks.len(), "/blog/");
        add(url, "list", "posts/list.html", vec![("collection", coll.clone()), ("entries", Value::from(c.to_vec())), ("paginator", pg)]);
    }
    add("/tags/".into(), "taxonomy-index", "tags/index.html", vec![("taxonomy", tags.clone()), ("terms", tags.get_attr("terms").unwrap())]);
    let _ = tag_terms;
    for (term, es) in &series_terms {
        let base = term.get_attr("url").unwrap().to_string();
        let chunks: Vec<&[Value]> = es.chunks(page_size).collect();
        for (i, c) in chunks.iter().enumerate() {
            let (url, pg) = paginator(i + 1, chunks.len(), &base);
            add(url, "term", "series/term.html", vec![("taxonomy", series.clone()), ("term", term.clone()), ("entries", Value::from(c.to_vec())), ("paginator", pg)]);
        }
    }
    routes
}
