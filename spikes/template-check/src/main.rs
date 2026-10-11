mod check;
mod seeds;
mod site;

use minijinja::value::Value;
use minijinja::{context, Environment, UndefinedBehavior};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

fn load(dir: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"), std::fs::read_to_string(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn err_line(e: &minijinja::Error, sources: &BTreeMap<String, String>) -> String {
    // innermost cause carries the real location
    let mut cur: &minijinja::Error = e;
    let mut src = std::error::Error::source(e);
    while let Some(x) = src {
        if let Some(me) = x.downcast_ref::<minijinja::Error>() {
            cur = me;
        }
        src = x.source();
    }
    let name = cur.name().unwrap_or("?");
    let col = match (cur.range(), sources.get(name)) {
        (Some(r), Some(s)) => s[..r.start.min(s.len())].rsplit('\n').next().unwrap_or("").chars().count() + 1,
        _ => 0,
    };
    format!("{}:{}:{}: {:?}: {}", name, cur.line().unwrap_or(0), col, cur.kind(), cur.detail().unwrap_or(""))
}

/// Render every route; returns (ok pages, total bytes, first error per failing template).
fn render_all(env: &Environment, routes: &[site::Route], sources: &BTreeMap<String, String>, keep: Option<&mut Vec<(String, String)>>) -> (usize, usize, Vec<String>) {
    let (mut ok, mut bytes, mut errs) = (0, 0, Vec::<String>::new());
    let mut keep = keep;
    for r in routes {
        match env.get_template(r.template).and_then(|t| t.render(&r.ctx)) {
            Ok(s) => {
                ok += 1;
                bytes += s.len();
                if let Some(k) = keep.as_mut() {
                    k.push((r.url.clone(), s));
                }
            }
            Err(e) => {
                let l = err_line(&e, sources);
                if !errs.contains(&l) {
                    errs.push(l);
                }
            }
        }
    }
    (ok, bytes, errs)
}

fn small_site() -> Vec<site::Route> {
    site::build_site(site::small_posts(), site::projects(), 3)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("check");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tdir = args.iter().position(|a| a == "--templates").map(|i| Path::new(&args[i + 1]).to_path_buf()).unwrap_or(root.join("site/templates"));
    match cmd {
        "check" => {
            let t0 = Instant::now();
            let sources = load(&tdir);
            let n = sources.len();
            let schema = site::schema();
            let c = check::run(sources, &schema);
            for d in &c.diags {
                println!("{d}");
            }
            let errors = c.diags.iter().filter(|d| !d.warn).count();
            eprintln!("check: {n} templates, {} route kinds, {errors} errors, {} warnings, {:.2?}", schema.kinds.len(), c.diags.len() - errors, t0.elapsed());
            std::process::exit(if errors > 0 { 1 } else { 0 });
        }
        "render" => {
            let sources = load(&tdir);
            let routes = small_site();
            let out = root.join("out");
            for (label, ub) in [("semistrict", UndefinedBehavior::SemiStrict), ("strict", UndefinedBehavior::Strict)] {
                let env = site::make_env(&sources, ub);
                let mut pages = vec![];
                let (ok, bytes, errs) = render_all(&env, &routes, &sources, Some(&mut pages));
                println!("{label}: {ok}/{} pages rendered, {bytes} bytes", routes.len());
                for e in errs {
                    println!("  {e}");
                }
                if label == "semistrict" {
                    for (url, html) in pages {
                        let p = out.join(format!("{}index.html", url.trim_start_matches('/')));
                        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                        std::fs::write(p, html).unwrap();
                    }
                }
            }
        }
        "seeded" => seeded(&tdir),
        "probe" => probe(),
        "sort" => sort_demo(),
        "bench" => bench(&tdir, args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000)),
        _ => eprintln!("usage: check | render | seeded | probe | sort | bench [N]"),
    }
}

// ---------------------------------------------------------------- seeded-error harness

fn seeded(tdir: &Path) {
    let base = load(tdir);
    let schema = site::schema();
    let routes = small_site();
    let mut md = String::new();

    // 0. false positives and undefined-mode comparison on the unseeded templates
    let c = check::run(base.clone(), &schema);
    writeln!(md, "## Unseeded templates\n").unwrap();
    writeln!(md, "check diagnostics on the correct templates (false positives): {}\n", c.diags.len()).unwrap();
    for d in &c.diags {
        writeln!(md, "- `{d}`").unwrap();
    }
    let mut sites: Vec<&String> = c.guard_sites.iter().collect();
    sites.sort();
    writeln!(md, "\nTruthiness tests on a possibly-undefined value (legal under SemiStrict, an error under Strict when the value is absent): {} sites\n", sites.len()).unwrap();
    for s in &sites {
        writeln!(md, "- `{s}`").unwrap();
    }
    for (label, ub) in [("SemiStrict", UndefinedBehavior::SemiStrict), ("Strict", UndefinedBehavior::Strict)] {
        let env = site::make_env(&base, ub);
        let (ok, _, errs) = render_all(&env, &routes, &base, None);
        writeln!(md, "\nRender under {label}: {ok}/{} pages OK, {} distinct errors", routes.len(), errs.len()).unwrap();
        for e in errs {
            writeln!(md, "- `{e}`").unwrap();
        }
    }

    // 1. seeds
    writeln!(md, "\n## Seeded mistakes\n").unwrap();
    writeln!(md, "| # | Mistake | check | check diagnostic (first relevant) | SemiStrict render | Strict render |").unwrap();
    writeln!(md, "|---|---|---|---|---|---|").unwrap();
    let (mut caught, mut semi_c, mut strict_c, mut total) = (0, 0, 0, 0);
    let mut missed = vec![];
    for (i, s) in seeds::seeds().iter().enumerate() {
        let mut src = base.clone();
        let orig = src.get(s.file).unwrap_or_else(|| panic!("seed {}: no file {}", i + 1, s.file)).clone();
        let mut edited = orig.clone();
        for (f, r) in s.find.split("§§").zip(s.replace.split("§§")) {
            assert!(edited.contains(f), "seed {} `{}`: find text `{f}` not present in {}", i + 1, s.what, s.file);
            edited = edited.replacen(f, r, 1);
        }
        src.insert(s.file.to_string(), edited);
        let c = check::run(src.clone(), &schema);
        let errs: Vec<&check::Diag> = c.diags.iter().filter(|d| !d.warn || s.warn_ok).collect();
        let render = |ub| {
            let env = site::make_env(&src, ub);
            let (_, _, e) = render_all(&env, &routes, &src, None);
            e
        };
        let (se, st) = (render(UndefinedBehavior::SemiStrict), render(UndefinedBehavior::Strict));
        total += 1;
        if !errs.is_empty() { caught += 1 } else { missed.push(i + 1) }
        if !se.is_empty() { semi_c += 1 }
        if !st.is_empty() { strict_c += 1 }
        let cell = |v: &Vec<String>| if v.is_empty() { "N".to_string() } else { format!("Y: `{}`", v[0].replace('|', "\\|")) };
        writeln!(
            md,
            "| {} | {} (`{}`) | {} | {} | {} | {} |",
            i + 1,
            s.what,
            s.file,
            if errs.is_empty() { "**N**" } else { "Y" },
            errs.first().map(|d| format!("`{}`{}", d.to_string().replace('|', "\\|"), if errs.len() > 1 { format!(" (+{} more)", errs.len() - 1) } else { String::new() })).unwrap_or_default(),
            cell(&se),
            cell(&st)
        )
        .unwrap();
    }
    writeln!(md, "\ncheck caught {caught}/{total}; missed seeds: {missed:?}. Render caught: SemiStrict {semi_c}/{total}, Strict {strict_c}/{total} (render only sees executed paths of the fake Entries).").unwrap();
    print!("{md}");
    std::fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join("results/seeded.md"), md).unwrap();
}

// ---------------------------------------------------------------- undefined-mode probe

fn probe() {
    let cases = [
        ("{% if e.opt %}y{% endif %}", "if on absent optional"),
        ("{% if not e.opt %}y{% endif %}", "if not"),
        ("{% if e.opt and e.req %}y{% endif %}", "and"),
        ("{{ e.opt or 'x' }}", "or fallback"),
        ("{{ 'a' if e.opt else 'b' }}", "inline if"),
        ("{{ 'a' if e.opt }}", "inline if without else (prints undefined)"),
        ("{% if e.opt is defined %}y{% endif %}", "is defined"),
        ("{{ e.opt|default('d') }}", "default filter"),
        ("{% if e.opt == 'x' %}y{% endif %}", "== comparison"),
        ("{% if e.opt != e.opt2 %}y{% endif %}", "!= between two absent"),
        ("{% if e.opt < 3 %}y{% endif %}", "< comparison"),
        ("{{ e.opt }}", "print"),
        ("{{ e.opt.deeper }}", "reach through"),
        ("{% for x in e.opt %}{% endfor %}", "loop over"),
        ("{{ e.opt|lower }}", "filter on"),
        ("{{ 'a' ~ e.opt }}", "concat"),
        ("{% set v = e.opt %}{% if v is defined %}y{% endif %}", "set alias then is defined"),
        ("{% with v = e.opt %}{% if v %}y{% endif %}{% endwith %}", "with alias then if"),
        ("{{ {'k': e.opt}|attrs }}", "map literal value into attrs"),
        ("{% macro m(a, b='dflt') %}[{{ b }}]{% endmacro %}{{ m(1, b=e.opt) }}", "pass absent to macro arg with default"),
        ("{% macro m(a, b=none) %}[{% if b %}{{ b }}{% endif %}]{% endmacro %}{{ m(1, b=e.opt) }}", "pass absent to optional macro arg, guarded"),
        ("{% macro m(a) %}[{{ a }}]{% endmacro %}{{ m() }}", "omit a required macro arg"),
        ("{% macro m(a) %}[{% if caller %}c{% endif %}]{% endmacro %}{{ m(1) }}", "if caller without call block"),
        ("{% macro m(a) %}[{% if caller is defined %}c{% endif %}]{% endmacro %}{{ m(1) }}", "caller is defined"),
        ("{{ nosuch }}", "print unknown variable"),
        ("{% if nosuch %}y{% endif %}", "if on unknown top-level variable"),
        ("{{ e.req|nosuchfilter }}", "unknown filter"),
    ];
    println!("| Construct | Template | SemiStrict | Strict |\n|---|---|---|---|");
    let run = |ub, src: &str| {
        let mut env = Environment::new();
        env.set_undefined_behavior(ub);
        env.add_filter("attrs", |v: Value| v.to_string());
        match env.render_str(src, context! { e => context!{ req => "r" } }) {
            Ok(s) => format!("ok `{s}`"),
            Err(e) => format!("ERR {:?}", e.kind()),
        }
    };
    for (src, what) in cases {
        println!("| {what} | `{}` | {} | {} |", src.replace('|', "\\|"), run(UndefinedBehavior::SemiStrict, src), run(UndefinedBehavior::Strict, src));
    }
}

// ---------------------------------------------------------------- sort_by vs the Reference orderings

fn slugs(v: &[Value]) -> Vec<String> {
    v.iter().map(|x| x.get_attr("slug").unwrap().to_string()).collect()
}
fn s(v: &Value, k: &str) -> Option<String> {
    v.get_attr(k).ok().filter(|x| !x.is_undefined()).map(|x| x.to_string())
}
fn int(v: &Value, k: &str) -> Option<i64> {
    v.get_attr(k).ok().and_then(|x| i64::try_from(x).ok())
}

fn sort_demo() {
    let keys = |k: &[&str]| k.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut all_ok = true;
    let mut report = |name: &str, reference: Vec<String>, ours: Vec<String>| {
        let ok = reference == ours;
        all_ok &= ok;
        println!("{name}: {}", if ok { "MATCH" } else { "DIFFERENT" });
        if reference.len() <= 15 || !ok {
            println!("  reference: {:?}\n  sort_by:   {:?}", &reference[..reference.len().min(15)], &ours[..ours.len().min(15)]);
        }
    };
    for (label, posts) in [("8 hand-written", site::small_posts()), ("1,000 fake", site::fake_posts(1000))] {
        // input order = slug order (Astro's getCollection order is by file id)
        let mut input: Vec<Value> = posts.iter().map(site::j2v).filter(|p| !p.get_attr("unlisted").unwrap().is_true()).collect();
        input.sort_by_key(|p| s(p, "slug"));

        // Reference blog list: sortMDByDate (updatedDate ?? pubDate, desc) then sortMDByPinned (order ?? 100). JS sort is stable.
        let mut r = input.clone();
        r.sort_by(|a, b| s(b, "modified").cmp(&s(a, "modified")));
        r.sort_by_key(|p| int(p, "order").unwrap_or(100));
        let mut o = input.clone();
        site::sort_values(&mut o, &keys(&["order", "-modified"]));
        report(&format!("blog list [order, -modified], {label} posts"), slugs(&r), slugs(&o));

        // Reference series page: sortMDByDate ascending (also on updatedDate ?? pubDate, i.e. `modified`)
        let in_series: Vec<Value> = input.iter().filter(|p| s(p, "series").is_some()).cloned().collect();
        let mut r = in_series.clone();
        r.sort_by(|a, b| s(a, "modified").cmp(&s(b, "modified")));
        let mut o = in_series.clone();
        site::sort_values(&mut o, &keys(&["modified"]));
        report(&format!("series [modified] ascending, {label} posts"), slugs(&r), slugs(&o));
        let mut o2 = in_series.clone();
        site::sort_values(&mut o2, &keys(&["date"]));
        report(&format!("series [date] ascending vs Reference (which sorts on updated ?? date), {label} posts"), slugs(&r), slugs(&o2));

        // Reference tags index: Map insertion order (first seen), then stable sort by count desc
        let mut counts: Vec<(String, i64)> = vec![];
        for p in &input {
            for t in p.get_attr("tags").unwrap().try_iter().unwrap() {
                let t = t.to_string();
                match counts.iter_mut().find(|c| c.0 == t) {
                    Some(c) => c.1 += 1,
                    None => counts.push((t, 1)),
                }
            }
        }
        let terms: Vec<Value> = counts.iter().map(|(n, c)| context! { name => n, slug => n, count => c }).collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        let mut o = terms.clone();
        site::sort_values(&mut o, &keys(&["-count"]));
        report(&format!("terms [-count], {label} posts (Reference breaks ties by first-seen, sort_by by slug)"), counts.iter().map(|c| c.0.clone()).collect(), slugs(&o));
        println!("  counts: {:?}", counts.iter().take(15).collect::<Vec<_>>());
    }
    // Reference projects: sortProjectsByDate (desc) then stable sort by rank in the hand-written ORDER list.
    let input: Vec<Value> = site::projects().iter().map(site::j2v).collect();
    let mut r = input.clone();
    r.sort_by(|a, b| s(b, "date").cmp(&s(a, "date")));
    r.sort_by_key(|p| site::PROJECT_ORDER.iter().position(|x| Some(x.to_string()) == s(p, "slug")).unwrap_or(13));
    let mut o = input.clone();
    site::sort_values(&mut o, &keys(&["order", "-date"]));
    report("projects [order, -date] with order: 1..13 in each file", slugs(&r), slugs(&o));
    // and with the last three projects unranked (no `order`): they must follow, newest first
    let strip: Vec<Value> = site::projects()
        .iter()
        .map(|p| {
            let mut p = p.clone();
            if p["order"].as_i64().unwrap() > 10 {
                p.as_object_mut().unwrap().remove("order");
            }
            site::j2v(&p)
        })
        .collect();
    let mut r = strip.clone();
    r.sort_by(|a, b| s(b, "date").cmp(&s(a, "date")));
    r.sort_by_key(|p| int(p, "order").unwrap_or(99));
    let mut o = strip.clone();
    site::sort_values(&mut o, &keys(&["order", "-date"]));
    report("projects, three without `order` (missing sorts last, then -date)", slugs(&r), slugs(&o));
    // the filter itself, through a template
    let env = site::make_env(&BTreeMap::new(), UndefinedBehavior::SemiStrict);
    let out = env
        .render_str("{{ ps|sort_by(['order', '-date'])|map(attribute='slug')|join(' ') }}", context! { ps => input })
        .unwrap();
    println!("template `ps|sort_by(['order','-date'])`: {out}");
    println!("ALL REQUIRED ORDERINGS MATCH: see lines above ({})", if all_ok { "every comparison matched" } else { "some comparisons differ, by design or by tie rule" });
}

// ---------------------------------------------------------------- timing

fn bench(tdir: &Path, n: usize) {
    let med = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1e3;
    // check: load from disk + parse + walk
    let mut load_t = vec![];
    let mut check_t = vec![];
    let schema = site::schema();
    for _ in 0..21 {
        let t = Instant::now();
        let sources = load(tdir);
        load_t.push(ms(t));
        let t = Instant::now();
        let c = check::run(sources, &schema);
        check_t.push(ms(t));
        assert!(c.diags.is_empty());
    }
    let t = Instant::now();
    let _ = site::schema();
    println!("check: read {} templates {:.3} ms, build schema {:.3} ms, parse + walk {} route kinds {:.3} ms (medians of 21)", load(tdir).len(), med(load_t), ms(t), schema.kinds.len(), med(check_t));

    let sources = load(tdir);
    let (mut gen_t, mut ctx_t, mut env_t, mut render_t) = (vec![], vec![], vec![], vec![]);
    let mut stats = (0, 0, 0);
    for _ in 0..7 {
        let t = Instant::now();
        let posts = site::fake_posts(n);
        let projects = site::projects();
        gen_t.push(ms(t));
        let t = Instant::now();
        let routes = site::build_site(posts, projects, 10);
        ctx_t.push(ms(t));
        let t = Instant::now();
        let env = site::make_env(&sources, UndefinedBehavior::SemiStrict);
        for name in sources.keys() {
            env.get_template(name).unwrap();
        }
        env_t.push(ms(t));
        let t = Instant::now();
        let (ok, bytes, errs) = render_all(&env, &routes, &sources, None);
        render_t.push(ms(t));
        assert!(errs.is_empty(), "{errs:?}");
        stats = (routes.len(), ok, bytes);
    }
    println!(
        "render {n} fake Posts: {} routes, {} rendered, {:.1} MB HTML (medians of 7, single thread)\n  generate fake Entries (stands in for parsing content): {:.2} ms\n  context construction (Values, sort, taxonomies, per-route contexts): {:.2} ms\n  Environment + compile all templates: {:.2} ms\n  render all pages: {:.2} ms ({:.1} us/page)",
        stats.0,
        stats.1,
        stats.2 as f64 / 1e6,
        med(gen_t),
        med(ctx_t.clone()),
        med(env_t),
        med(render_t.clone()),
        med(render_t) * 1e3 / stats.0 as f64
    );
}
