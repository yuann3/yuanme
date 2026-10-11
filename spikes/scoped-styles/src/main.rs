mod check;
mod scope;
mod site;

use minijinja::{context, Environment, UndefinedBehavior};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

type Map = BTreeMap<String, String>;

/// (templates, sidecars keyed by the template path they belong to)
fn load(dir: &Path) -> (Map, Map) {
    fn walk(root: &Path, dir: &Path, out: &mut Map) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"), std::fs::read_to_string(&p).unwrap());
            }
        }
    }
    let mut all = Map::new();
    walk(dir, dir, &mut all);
    let (mut t, mut c) = (Map::new(), Map::new());
    for (k, v) in all {
        if let Some(stem) = k.strip_suffix(".css") {
            c.insert(format!("{stem}.html"), v);
        } else {
            t.insert(k, v);
        }
    }
    (t, c)
}

#[derive(Clone, Copy, PartialEq)]
enum Stamp {
    /// only templates that have a sidecar (the design)
    Sidecar,
    /// every template (stress + equivalence test)
    All,
    /// every template, plus data-src="<path>" provenance for the browser check
    Prov,
}

struct Built {
    rewritten: Map,
    maps: BTreeMap<String, Vec<(usize, usize)>>,
    attrs: Map,
    warns: Vec<String>,
}

fn build(orig: &Map, sidecars: &Map, mode: Stamp) -> Built {
    let mut b = Built { rewritten: Map::new(), maps: BTreeMap::new(), attrs: Map::new(), warns: vec![] };
    for (name, src) in orig {
        let has = sidecars.contains_key(name);
        let attr = scope::scope_attr(name);
        if has { b.attrs.insert(name.clone(), attr.clone()); }
        let ins = match mode {
            Stamp::Sidecar if has => Some(attr.clone()),
            Stamp::Sidecar => None,
            Stamp::All => Some(attr.clone()),
            Stamp::Prov if has => Some(format!("{attr} data-src=\"{name}\"")),
            Stamp::Prov => Some(format!("data-src=\"{name}\"")),
        };
        let r = scope::rewrite(src, ins.as_deref());
        if has || mode == Stamp::All {
            for (off, msg) in &r.warns {
                let (l, c) = scope::line_col(src, *off);
                b.warns.push(format!("{name}:{l}:{c}: warning: {msg}"));
            }
        }
        b.maps.insert(name.clone(), r.map);
        b.rewritten.insert(name.clone(), r.out);
    }
    b
}

/// Innermost minijinja error -> (template, line, byte offset of the error range start)
fn err_at(e: &minijinja::Error) -> (String, usize, Option<usize>, String) {
    let mut cur: &minijinja::Error = e;
    let mut src = std::error::Error::source(e);
    while let Some(x) = src {
        if let Some(me) = x.downcast_ref::<minijinja::Error>() { cur = me; }
        src = x.source();
    }
    (cur.name().unwrap_or("?").to_string(), cur.line().unwrap_or(0), cur.range().map(|r| r.start), format!("{:?}: {}", cur.kind(), cur.detail().unwrap_or("")))
}

fn render_all(env: &Environment, routes: &[site::Route]) -> Vec<(String, String)> {
    routes.iter().map(|r| (r.url.clone(), env.get_template(r.template).and_then(|t| t.render(&r.ctx)).unwrap_or_else(|e| panic!("{}: {e:#}", r.url)))).collect()
}

fn br(s: &str) -> usize {
    let mut out = Vec::new();
    let mut p = brotli::enc::BrotliEncoderParams::default();
    p.quality = 11;
    brotli::BrotliCompress(&mut s.as_bytes(), &mut out, &p).unwrap();
    out.len()
}

fn all_css(sidecars: &Map, attrs: Option<&Map>, minify: bool) -> Vec<(String, String)> {
    sidecars
        .iter()
        .map(|(name, css)| {
            let attr = attrs.map(|a| a[name].as_str());
            (name.clone(), scope::scope_css(css, attr, None, minify, scope::targets()).unwrap_or_else(|e| panic!("{name}: {e:?}")).css)
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("check");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tdir = args.iter().position(|a| a == "--templates").map(|i| Path::new(&args[i + 1]).to_path_buf()).unwrap_or(root.join("site/templates"));
    match cmd {
        "check" => {
            let (t, c) = load(&tdir);
            let n = sidecar_check(&t, &c, true);
            std::process::exit(if n > 0 { 1 } else { 0 });
        }
        "edges" => edges(root),
        "css" => css_edges(root),
        "offsets" => offsets(&tdir),
        "render" => render(&tdir, root),
        "bench" => bench(&tdir),
        _ => eprintln!("usage: check | edges | css | offsets | render | bench"),
    }
}

// ---------------------------------------------------------------- check

/// Template check on the ORIGINAL sources, then the sidecar validations. Returns the error count.
fn sidecar_check(t: &Map, c: &Map, print: bool) -> usize {
    let schema = site::schema();
    let ck = check::run(t.clone(), &schema);
    let mut lines: Vec<String> = ck.diags.iter().map(|d| d.to_string()).collect();
    let mut errors = ck.diags.iter().filter(|d| !d.warn).count();
    let mut seen = BTreeMap::new();
    for (name, css) in c {
        let Some(src) = t.get(name) else {
            lines.push(format!("{}: sidecar without a template", name.replace(".html", ".css")));
            errors += 1;
            continue;
        };
        let attr = scope::scope_attr(name);
        if let Some(prev) = seen.insert(attr.clone(), name.clone()) {
            lines.push(format!("{name}: scope id {attr} collides with {prev}"));
            errors += 1;
        }
        let r = scope::rewrite(src, None);
        for (off, msg) in &r.warns {
            let (l, col) = scope::line_col(src, *off);
            lines.push(format!("{name}:{l}:{col}: warning: {msg}"));
        }
        let elems = scope::elems(src, &r.tags);
        let cssname = name.replace(".html", ".css");
        match scope::scope_css(css, Some(&attr), Some(&elems), false, scope::targets()) {
            Ok(s) => {
                for d in s.diags {
                    lines.push(format!("{cssname}:{}:{}: {}", d.line, d.col, d.msg));
                    errors += 1;
                }
            }
            Err(d) => {
                lines.push(format!("{cssname}:{}:{}: {}", d.line, d.col, d.msg));
                errors += 1;
            }
        }
    }
    if print {
        for l in &lines { println!("{l}"); }
        eprintln!("check: {} templates, {} sidecars, {errors} errors, {} lines", t.len(), c.len(), lines.len());
    }
    errors
}

// ---------------------------------------------------------------- edge cases

struct Case {
    file: &'static str,
    what: &'static str,
    rule: &'static str,
    /// substrings the rendered page must contain / must not contain (H = this file's attr, H2 = the other file's)
    must: &'static [&'static str],
    must_not: &'static [&'static str],
    other: &'static str,
    warn: Option<&'static str>,
}

fn edges(root: &Path) {
    let (t, c) = load(&root.join("edge/templates"));
    let cases = [
        Case { file: "split.html", what: "tag split across Jinja control flow", rule: "insert right after the tag name, never look for the tag end; add a space when the next byte is not whitespace, `>` or `/`",
            must: &["<a H class=\"a\" href=\"/\">", "<a H  class=\"b\">", "<a H  class=\"c\">"], must_not: &[], other: "", warn: None },
        Case { file: "attrgt.html", what: "`>` , `<` and Jinja expressions inside attribute values; `<` in text", rule: "quote-aware tag scan; Jinja delimiters are skipped whole (string-aware) in every state; `<` starts a tag only before a letter",
            must: &["<a H title=\"a > b\"", "<img H alt=\"<b>not a tag</b>\"", "<input H value=", "<p H>1 < 2 and a <3</p>"], must_not: &["<b H"], other: "", warn: None },
        Case { file: "rawtext.html", what: "`<script>`, `<style>`, `<textarea>`, HTML comments, `{# #}`, `{% raw %}` bodies", rule: "bodies copied verbatim; `<script>`/`<style>` themselves are never stamped; a tag inside `{% raw %}` is not stamped",
            must: &["<div H>", "<script>", "<style>", "<textarea H>", "<ins H>", "<aside>", "<!-- <section>commented</section> -->", "document.write(\"<p>x</p>\")", "<em>text</em>"], must_not: &["<p H", "<b H", "<section H", "<aside H", "<em H", "<footer"], other: "", warn: None },
        Case { file: "void.html", what: "void and self-closing tags, custom elements", rule: "same rule; the attribute lands before `/`",
            must: &["<br H><br H/>", "<img H src=\"a.png\"/>", "<hr H />", "<input H disabled>", "<my-el H></my-el>", "<x-foo H/>"], must_not: &[], other: "", warn: None },
        Case { file: "svg.html", what: "inline SVG (camelCase names, `xlink:href`, self-closing, foreignObject)", rule: "same rule; names keep their case; lower-case `data-*` works in SVG",
            must: &["<svg H viewBox", "<linearGradient H id=\"g\">", "<stop H offset=\"0\"/>", "<path H d=", "<use H xlink:href", "<foreignObject H><div H>", "<text H>"], must_not: &[], other: "", warn: None },
        Case { file: "layout_child.html", what: "`<html>`, `<head>`, `<body>` in a layout; child block content", rule: "`html` and `body` are stamped with the layout's id; `head`, `meta`, `title`, `link`, `base`, `script`, `style`, `noscript`, `template` never; block content takes the child template's id; `:root` is never scoped",
            must: &["<!doctype html>", "<html H2 lang=\"en\">", "<head>", "<meta charset", "<title>T <b></title>", "<link rel", "<base href", "<body H2 class=\"b\">", "<main H>hi</main>", "<noscript>"], must_not: &["<head H", "<title H", "<b H", "<main H2"], other: "layout.html", warn: None },
        Case { file: "inc_parent.html", what: "`{% include %}`", rule: "each file is rewritten when it is loaded, so the included file's tags carry the included file's id",
            must: &["<section H><p H2 class=\"inc\">included</p>"], must_not: &["<p H "], other: "inc_child.html", warn: None },
        Case { file: "macro_user.html", what: "macro imported from another file, with `caller()` slot content", rule: "macro body tags carry the macro file's id; slot content written at the call site carries the caller's id",
            must: &["<div H2 class=\"card\"><h2 H2>T</h2><div H2 class=\"slot\"><em H>slot</em></div></div>"], must_not: &[], other: "macro_lib.html", warn: None },
        Case { file: "safe.html", what: "elements from `| safe` strings and safe Values", rule: "never stamped; `| safe` in text position is a `check` warning; safe Values such as `entry.content` cannot be seen statically and are silent by design",
            must: &["<div H><b class=\"k\">bold</b>&lt;b class=&quot;k&quot;&gt;bold&lt;/b&gt;<b class=\"k\">bold</b></div>"], must_not: &["<b H"], other: "", warn: Some("| safe") },
        Case { file: "dyn.html", what: "dynamic tag names", rule: "`<{{ tag }}` and `<h{{ n }}`: stamped after the name expression, with a warning (type selectors cannot be checked); name built by `{% if %}`: not stamped, warning",
            must: &["<section H class=\"x\">a</section>", "<h2 H id=\"t\">b</h2>", "<h1 class=\"cf\">c</h1>", "<i H>after</i>"], must_not: &["<h1 H"], other: "", warn: Some("dynamic tag name") },
        Case { file: "strings.html", what: "tags inside Jinja string literals and `{% set %}`", rule: "Jinja delimiters are skipped whole, so strings are never stamped (and print escaped)",
            must: &["<div H title=\"%}\">"], must_not: &["<p H", "<u H"], other: "", warn: None },
        Case { file: "quotes.html", what: "quotes unbalanced inside each Jinja branch", rule: "KNOWN LIMIT: the scan loses the tag end; the following tags are not stamped; `check` reports `unterminated tag`",
            must: &["<a H title=\"a\">t</a><span>after</span>"], must_not: &["<span H"], other: "", warn: Some("unterminated tag") },
    ];
    let b = build(&t, &c, Stamp::Sidecar);
    let env = site::make_env(&b.rewritten, UndefinedBehavior::SemiStrict);
    let ctx = context! { x => true, level => 2, tag => "section", url => "/u", t => "T",
        html => "<b class=\"k\">bold</b>", content => minijinja::Value::from_safe_string("<b class=\"k\">bold</b>".into()) };
    let mut md = String::from("| Case | Rule | Result | Warning |\n|---|---|---|---|\n");
    let mut detail = String::new();
    let mut fails = 0;
    for case in &cases {
        let h = &b.attrs[case.file];
        let h2 = b.attrs.get(case.other).cloned().unwrap_or_default();
        let sub = |s: &str| s.replace("H2", &h2).replace('H', h);
        let out = env.get_template(case.file).and_then(|t| t.render(&ctx)).unwrap_or_else(|e| format!("RENDER ERROR {e:#}"));
        let mut bad: Vec<String> = case.must.iter().filter(|m| !out.contains(&sub(m))).map(|m| format!("missing `{m}`")).collect();
        bad.extend(case.must_not.iter().filter(|m| out.contains(&sub(m))).map(|m| format!("unexpected `{m}`")));
        let warns: Vec<&String> = b.warns.iter().filter(|w| w.starts_with(case.file) || (!case.other.is_empty() && w.starts_with(case.other))).collect();
        match case.warn {
            Some(w) if !warns.iter().any(|x| x.contains(w)) => bad.push(format!("expected a warning containing `{w}`")),
            None if !warns.is_empty() => bad.push("unexpected warning".into()),
            _ => {}
        }
        if !bad.is_empty() { fails += 1 }
        writeln!(md, "| {} (`{}`) | {} | {} | {} |", case.what, case.file, case.rule, if bad.is_empty() { "pass".to_string() } else { format!("**FAIL** {}", bad.join("; ")) }, if warns.is_empty() { "" } else { "yes" }).unwrap();
        writeln!(detail, "### {}\n\nsource:\n```html\n{}```\nloaded as:\n```html\n{}```\nrendered (H = `{h}`{}):\n```html\n{}\n```", case.file, t[case.file], b.rewritten[case.file],
            if h2.is_empty() { String::new() } else { format!(", H2 = `{h2}` from `{}`", case.other) }, { let o = out.trim_end().to_string(); let o = if h2.is_empty() { o } else { o.replace(&*h2, "H2") }; o.replace(h.as_str(), "H") }).unwrap();
        for w in warns { writeln!(detail, "- `{w}`").unwrap(); }
        detail.push('\n');
    }
    let out = format!("{md}\n{} cases, {fails} failed.\n\n## Detail\n\n{detail}", cases.len());
    print!("{out}");
    std::fs::write(root.join("results/edges.md"), out).unwrap();
}

fn css_edges(root: &Path) {
    let dir = root.join("edge/css");
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    names.sort();
    let mut md = String::new();
    for n in names {
        let src = std::fs::read_to_string(dir.join(&n)).unwrap();
        writeln!(md, "### {n}\n\n```css\n{}```", src).unwrap();
        for (label, targets) in [("no targets (nesting kept)", lightningcss::targets::Targets::default()), ("Tailwind v4 default targets (nesting lowered)", scope::targets())] {
            match scope::scope_css(&src, Some("data-y-H"), None, false, targets) {
                Ok(s) => {
                    writeln!(md, "{label}:\n```css\n{}```", s.css).unwrap();
                    for d in &s.diags { writeln!(md, "- diagnostic `{n}:{}:{}: {}`", d.line, d.col, d.msg).unwrap(); }
                    if !s.keyframes.is_empty() { writeln!(md, "- keyframes left global: {:?}", s.keyframes).unwrap(); }
                }
                Err(d) => writeln!(md, "{label}: **error** `{n}:{}:{}: {}`", d.line, d.col, d.msg).unwrap(),
            }
            if !n.contains("nesting") { break }
        }
        md.push('\n');
    }
    print!("{md}");
    std::fs::write(root.join("results/css-edges.md"), md).unwrap();
}

// ---------------------------------------------------------------- offsets

fn offsets(tdir: &Path) {
    let (t, c) = load(tdir);
    let mut md = String::new();
    // 1. the checker on rewritten sources: does props pairing survive? (it walks byte offsets)
    let b = build(&t, &c, Stamp::All);
    let schema = site::schema();
    let on_orig = check::run(t.clone(), &schema);
    let on_rw = check::run(b.rewritten.clone(), &schema);
    writeln!(md, "check on original sources: {} diagnostics; check on rewritten sources (every template stamped): {} diagnostics\n", on_orig.diags.len(), on_rw.diags.len()).unwrap();

    // 2. seeded error after stamped tags on the same line
    let file = "layouts/post.html";
    let find = "<span>{{ entry.reading_time }} min read</span>";
    let repl = "<span>ok</span> <span>{{ entry.reading_tme.x }} min read</span>";
    assert!(t[file].contains(find));
    let mut seeded = t.clone();
    seeded.insert(file.into(), t[file].replace(find, repl));
    let line = seeded[file].lines().find(|l| l.contains("reading_tme")).unwrap();
    writeln!(md, "seed in `{file}` (two stamped tags before the error, same line):\n```html\n{}\n```", line.trim_end()).unwrap();
    let true_col = line.find("entry.reading_tme").unwrap() + 1;
    writeln!(md, "true column of `entry.reading_tme` in the Author's file: {true_col}\n").unwrap();

    let ck = check::run(seeded.clone(), &schema);
    writeln!(md, "| Path | Diagnostic |\n|---|---|").unwrap();
    for d in ck.diags.iter().filter(|d| d.file == file) {
        writeln!(md, "| `check` on the original source | `{d}` |").unwrap();
    }
    let ck2 = check::run(build(&seeded, &c, Stamp::Sidecar).rewritten, &schema);
    for d in ck2.diags.iter().filter(|d| d.file == file) {
        writeln!(md, "| `check` on the rewritten source, unmapped (wrong) | `{d}` |").unwrap();
    }
    let b = build(&seeded, &c, Stamp::Sidecar);
    let env = site::make_env(&b.rewritten, UndefinedBehavior::SemiStrict);
    let routes = site::build_site(site::small_posts(), site::projects(), 3);
    let r = routes.iter().find(|r| r.template == "posts/entry.html").unwrap();
    let e = env.get_template(r.template).and_then(|t| t.render(&r.ctx)).unwrap_err();
    let (name, l, off, msg) = err_at(&e);
    let off = off.unwrap();
    let (_, raw_col) = scope::line_col(&b.rewritten[&name], off);
    let mapped = scope::to_orig(&b.maps[&name], off);
    let (ml, mc) = scope::line_col(&seeded[&name], mapped);
    writeln!(md, "| render of the rewritten source, unmapped (wrong) | `{name}:{l}:{raw_col}: {msg}` |").unwrap();
    writeln!(md, "| render of the rewritten source, through the offset map | `{name}:{ml}:{mc}: {msg}` |").unwrap();
    let env0 = site::make_env(&seeded, UndefinedBehavior::SemiStrict);
    let e0 = env0.get_template(r.template).and_then(|t| t.render(&r.ctx)).unwrap_err();
    let (n0, l0, o0, m0) = err_at(&e0);
    let (_, c0) = scope::line_col(&seeded[&n0], o0.unwrap());
    writeln!(md, "| render of the original source (reference) | `{n0}:{l0}:{c0}: {m0}` |").unwrap();
    writeln!(md, "\noffset map for `{file}`: {} insertions of {} bytes; inserted text never contains a newline, so line numbers are unchanged and only same-line columns move.", b.maps[file].len(), b.maps[file].first().map(|x| x.1).unwrap_or(0)).unwrap();
    writeln!(md, "mapped column == reference column: {}", mc == c0 && ml == l0).unwrap();
    // 3. exhaustive: every original offset of every template survives the round trip
    let ball = build(&t, &c, Stamp::All);
    let mut n = 0;
    for (name, src) in &t {
        let (mut shift, mut k) = (0, 0);
        let map = &ball.maps[name];
        for o in 0..=src.len() {
            while k < map.len() && map[k].0 < o { shift += map[k].1; k += 1; }
            // bytes at the insertion point sit after the inserted text
            let rw = o + shift + if k < map.len() && map[k].0 == o { map[k].1 } else { 0 };
            assert_eq!(scope::to_orig(map, rw), o, "{name} @{o}");
            n += 1;
        }
    }
    writeln!(md, "round trip original -> rewritten -> original holds for all {n} byte offsets of the {} templates.", t.len()).unwrap();
    print!("{md}");
    std::fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join("results/offsets.md"), md).unwrap();
}

// ---------------------------------------------------------------- render (input for verify/verify.ts)

fn strip(html: &str, attrs: &[String]) -> String {
    let mut s = html.to_string();
    for a in attrs { s = s.replace(&format!(" {a}"), ""); }
    s
}

fn render(tdir: &Path, root: &Path) {
    let (t, c) = load(tdir);
    let routes = site::build_site(site::small_posts(), site::projects(), 3);
    let plain = render_all(&site::make_env(&t, UndefinedBehavior::SemiStrict), &routes);
    let mut md = String::new();
    // equivalence: stamping changes nothing but the attributes
    for (label, mode) in [("sidecar templates only", Stamp::Sidecar), ("every template", Stamp::All)] {
        let b = build(&t, &c, mode);
        let attrs: Vec<String> = t.keys().map(|n| scope::scope_attr(n)).collect();
        let out = render_all(&site::make_env(&b.rewritten, UndefinedBehavior::SemiStrict), &routes);
        let same = out.iter().zip(&plain).filter(|(a, p)| strip(&a.1, &attrs) == p.1).count();
        let stamped: usize = out.iter().map(|(_, h)| h.matches(" data-y-").count()).sum();
        let tags: usize = plain.iter().map(|(_, h)| h.match_indices('<').filter(|(i, _)| h.as_bytes().get(i + 1).is_some_and(|c| c.is_ascii_alphabetic())).count()).sum();
        writeln!(md, "stamp {label}: {same}/{} pages byte-identical to the unstamped render once the attributes are removed; {stamped} attributes on {tags} start tags; {} rewriter warnings", plain.len(), b.warns.len()).unwrap();
        for w in &b.warns { writeln!(md, "  {w}").unwrap(); }
    }
    let b = build(&t, &c, Stamp::Prov);
    let out = render_all(&site::make_env(&b.rewritten, UndefinedBehavior::SemiStrict), &routes);
    let css: String = all_css(&c, Some(&b.attrs), true).into_iter().map(|(n, s)| format!("/* {n} */\n{s}\n")).collect();
    let dir = root.join("out/site");
    let _ = std::fs::remove_dir_all(&dir);
    for (url, html) in &out {
        let p = dir.join(format!("{}index.html", url.trim_start_matches('/')));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let html = html.replace("</head>", "<link rel=\"stylesheet\" href=\"/_ref/global.css\" data-global><link rel=\"stylesheet\" href=\"/scoped.css\" data-scoped></head>");
        std::fs::write(p, html).unwrap();
    }
    std::fs::write(dir.join("scoped.css"), &css).unwrap();
    std::fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&serde_json::json!({ "attrs": b.attrs, "pages": out.iter().map(|x| &x.0).collect::<Vec<_>>() })).unwrap()).unwrap();
    writeln!(md, "wrote {} pages with provenance attributes, scoped.css ({} B) and manifest.json to out/site", out.len(), css.len()).unwrap();
    print!("{md}");
    std::fs::write(root.join("results/render.txt"), md).unwrap();
}

// ---------------------------------------------------------------- bench

fn bench(tdir: &Path) {
    let med = |mut v: Vec<f64>| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() / 2] };
    let p95 = |mut v: Vec<f64>| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v[v.len() * 95 / 100] };
    let us = |t: Instant| t.elapsed().as_secs_f64() * 1e6;
    let (t, c) = load(tdir);
    let mut md = String::new();

    // ---- rewrite
    writeln!(md, "## Template rewrite at load (median of 2001 runs)\n\n| Template | Source B | Tags stamped | Rewrite | Added B |\n|---|---|---|---|---|").unwrap();
    let mut total = 0.0;
    for (name, src) in &t {
        let attr = scope::scope_attr(name);
        let r = scope::rewrite(src, Some(&attr));
        let m = med((0..2001).map(|_| { let s = Instant::now(); std::hint::black_box(scope::rewrite(std::hint::black_box(src), Some(&attr))); us(s) }).collect());
        total += m;
        writeln!(md, "| `{name}`{} | {} | {} | {:.2} µs | {} |", if c.contains_key(name) { " (sidecar)" } else { "" }, src.len(), r.map.len(), m, r.out.len() - src.len()).unwrap();
    }
    let all = |mode| (0..2001).map(|_| { let s = Instant::now(); std::hint::black_box(build(&t, &c, mode)); us(s) }).collect::<Vec<f64>>();
    writeln!(md, "\nsum of medians, all {} templates: {:.1} µs. One pass over all templates: stamp-all median {:.1} µs / p95 {:.1} µs; sidecar-only (the design; others are scanned for nothing here) median {:.1} µs.\n", t.len(), total, med(all(Stamp::All)), p95(all(Stamp::All)), med(all(Stamp::Sidecar))).unwrap();

    // ---- css
    writeln!(md, "## Sidecar CSS (parse + scope + minify + print, Tailwind v4 default targets; median of 501)\n\n| Sidecar | Source B | Unscoped min B / br | Scoped min B / br | Added B / br | Transform | Unscoped transform |\n|---|---|---|---|---|---|---|").unwrap();
    let (mut tot_t, mut su, mut ss) = (0.0, String::new(), String::new());
    for (name, css) in &c {
        let attr = scope::scope_attr(name);
        let run = |a: Option<&str>| scope::scope_css(css, a, None, true, scope::targets()).unwrap().css;
        let (u, s) = (run(None), run(Some(&attr)));
        let ts = med((0..501).map(|_| { let x = Instant::now(); std::hint::black_box(run(Some(&attr))); us(x) }).collect());
        let tu = med((0..501).map(|_| { let x = Instant::now(); std::hint::black_box(run(None)); us(x) }).collect());
        tot_t += ts;
        writeln!(md, "| `{}` | {} | {} / {} | {} / {} | +{} / +{} | {:.1} µs | {:.1} µs |", name.replace(".html", ".css"), css.len(), u.len(), br(&u), s.len(), br(&s), s.len() - u.len(), br(&s) as i64 - br(&u) as i64, ts, tu).unwrap();
        su.push_str(&u);
        ss.push_str(&s);
    }
    writeln!(md, "| **all 6, concatenated** | | {} / {} | {} / {} | +{} / +{} | {:.1} µs | |\n", su.len(), br(&su), ss.len(), br(&ss), ss.len() - su.len(), br(&ss) as i64 - br(&su) as i64, tot_t).unwrap();
    // with the validation on
    let tv = med((0..501).map(|_| { let x = Instant::now(); std::hint::black_box(sidecar_css_check(&t, &c)); us(x) }).collect());
    writeln!(md, "sidecar validations for all 6 (scan template, collect literal elements, parse CSS, match compounds): {:.1} µs.\n", tv).unwrap();

    // ---- html bytes
    let routes = site::build_site(site::fake_posts(1000), site::projects(), 10);
    let bs = build(&t, &c, Stamp::Sidecar);
    let ba = build(&t, &c, Stamp::All);
    let envs = [site::make_env(&t, UndefinedBehavior::SemiStrict), site::make_env(&bs.rewritten, UndefinedBehavior::SemiStrict), site::make_env(&ba.rewritten, UndefinedBehavior::SemiStrict)];
    writeln!(md, "## HTML bytes (brotli q11)\n\n| Page | Plain raw / br | Stamped raw / br | Added raw / br | Attributes | Stamp-every-template raw / br (added) |\n|---|---|---|---|---|---|").unwrap();
    for (label, pick) in [("Post page (`/blog/post-0001/`)", "/blog/post-0001/"), ("Post list, 10 rows (`/blog/`)", "/blog/"), ("Projects index, 13 rows", "/projects/"), ("About (PhotoPile)", "/about/"), ("Project page", "/projects/lrle/")] {
        let r = routes.iter().find(|r| r.url == pick).unwrap();
        let h: Vec<String> = envs.iter().map(|e| e.get_template(r.template).unwrap().render(&r.ctx).unwrap()).collect();
        writeln!(md, "| {label} | {} / {} | {} / {} | +{} / +{} | {} | {} / {} (+{} / +{}) |", h[0].len(), br(&h[0]), h[1].len(), br(&h[1]), h[1].len() - h[0].len(), br(&h[1]) as i64 - br(&h[0]) as i64, h[1].matches(" data-y-").count(),
            h[2].len(), br(&h[2]), h[2].len() - h[0].len(), br(&h[2]) as i64 - br(&h[0]) as i64).unwrap();
    }

    // ---- render cost
    writeln!(md, "\n## Render cost ({} pages, 1,000 fake Posts, one thread, median of 9)\n", routes.len()).unwrap();
    let mut res = vec![];
    for (label, env) in ["plain", "stamped (sidecar templates)", "stamped (every template)"].iter().zip(&envs) {
        for name in t.keys() { env.get_template(name).unwrap(); }
        let mut bytes = 0;
        let m = med((0..9).map(|_| { let s = Instant::now(); bytes = routes.iter().map(|r| env.get_template(r.template).unwrap().render(&r.ctx).unwrap().len()).sum(); us(s) / 1e3 }).collect());
        res.push(m);
        writeln!(md, "- {label}: {:.2} ms, {} bytes, {:.2} µs a page", m, bytes, m * 1e3 / routes.len() as f64).unwrap();
    }
    // template compile
    let compile = |src: &Map| med((0..201).map(|_| { let env = site::make_env(src, UndefinedBehavior::SemiStrict); let s = Instant::now(); for n in src.keys() { env.get_template(n).unwrap(); } us(s) }).collect());
    writeln!(md, "- compile all templates: plain {:.1} µs, stamped {:.1} µs", compile(&t), compile(&bs.rewritten)).unwrap();
    writeln!(md, "\nNo per-page pass exists: the attribute is template text. The render delta is the cost of writing the extra bytes.").unwrap();

    // ---- incremental
    writeln!(md, "\n## Incremental (p95 of 501)\n").unwrap();
    let css = &c["components/photo_pile.html"];
    let attr = scope::scope_attr("components/photo_pile.html");
    let e1 = p95((0..501).map(|_| { let s = Instant::now(); std::hint::black_box(scope::scope_css(css, Some(&attr), None, true, scope::targets()).unwrap()); us(s) }).collect());
    writeln!(md, "- edit the largest sidecar (`photo_pile.css`): re-scope it, {:.1} µs; no template is reloaded and no page re-renders (the id comes from the path).", e1).unwrap();
    let src = &t["layouts/shell.html"];
    let e2 = p95((0..501).map(|_| { let s = Instant::now(); let r = scope::rewrite(src, Some("data-y-abcdef")); let mut env = Environment::new(); env.set_syntax(check::syntax()); env.add_template_owned("x", r.out).unwrap(); us(s) }).collect());
    writeln!(md, "- edit the largest template (`layouts/shell.html`): rewrite + minijinja compile, {:.1} µs (pages that use it re-render as they would without scoping).", e2).unwrap();
    print!("{md}");
    std::fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join("results/bench.md"), md).unwrap();
}

fn sidecar_css_check(t: &Map, c: &Map) -> usize {
    let mut n = 0;
    for (name, css) in c {
        let src = &t[name];
        let r = scope::rewrite(src, None);
        let elems = scope::elems(src, &r.tags);
        n += scope::scope_css(css, Some("data-y-abcdef"), Some(&elems), false, scope::targets()).map(|s| s.diags.len()).unwrap_or(1);
    }
    n
}
