use lightningcss::css_modules::{Config, Pattern};
use lightningcss::printer::PrinterOptions;
use lightningcss::selector::{Component, PseudoClass, Selector};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, StyleSheet};
use lightningcss::targets::{Browsers, Targets};
use lightningcss::visit_types;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};
use lol_html::{comments, element, rewrite_str, RewriteStrSettings};
use minijinja::{context, Environment};
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;
use std::time::{Duration, Instant};

const HASH: &str = "data-y-k3f9q2";

fn br(s: &str) -> usize {
  let mut out = Vec::new();
  let mut p = brotli::enc::BrotliEncoderParams::default();
  p.quality = 11;
  brotli::BrotliCompress(&mut s.as_bytes(), &mut out, &p).unwrap();
  out.len()
}
fn median(mut v: Vec<Duration>) -> Duration { v.sort(); v[v.len() / 2] }
fn time<F: FnMut()>(n: usize, mut f: F) -> Duration {
  median((0..n).map(|_| { let t = Instant::now(); f(); t.elapsed() }).collect())
}
fn v(maj: u32, min: u32) -> Option<u32> { Some(maj << 16 | min << 8) }

fn css(src: &str, targets: Targets, minify: bool, modules: Option<Config>, scope: bool) -> Result<String, String> {
  let mut warnings_opts = ParserOptions::default();
  warnings_opts.css_modules = modules;
  let mut ss = StyleSheet::parse(src, warnings_opts).map_err(|e| e.to_string())?;
  if scope { ss.visit(&mut Scoper).unwrap(); }
  ss.minify(MinifyOptions { targets, ..Default::default() }).map_err(|e| e.to_string())?;
  let r = ss.to_css(PrinterOptions { minify, targets, ..Default::default() }).map_err(|e| e.to_string())?;
  Ok(r.code)
}

/// B: append [data-y-HASH] to every compound that is not :global(...)
struct Scoper;
impl<'i> Visitor<'i> for Scoper {
  type Error = Infallible;
  fn visit_types(&self) -> VisitTypes { visit_types!(SELECTORS) }
  fn visit_selector(&mut self, sel: &mut Selector<'i>) -> Result<(), Infallible> {
    let comps: Vec<Component<'i>> = sel.iter_raw_parse_order_from(0).cloned().collect();
    let attr = || Component::AttributeInNoNamespaceExists { local_name: HASH.into(), local_name_lower: HASH.into() };
    let mut out = Vec::with_capacity(comps.len() + 4);
    let mut compound: Vec<Component<'i>> = Vec::new();
    let flush = |compound: &mut Vec<Component<'i>>, out: &mut Vec<Component<'i>>| {
      if compound.is_empty() { return; }
      let global = compound.iter().any(|c| matches!(c, Component::NonTSPseudoClass(PseudoClass::Global { .. })));
      let pos = compound.iter().position(|c| matches!(c, Component::PseudoElement(_))).unwrap_or(compound.len());
      if !global { compound.insert(pos, attr()); }
      out.append(compound);
    };
    for c in comps {
      if c.as_combinator().is_some() { flush(&mut compound, &mut out); out.push(c); } else { compound.push(c); }
    }
    flush(&mut compound, &mut out);
    *sel = Selector::from(out);
    Ok(())
  }
}

/// B-static: inject the attribute into literal start tags inside {% macro %} bodies at template-load time.
fn inject_static(src: &str, attr: &str) -> String {
  let b = src.as_bytes();
  let mut out = String::with_capacity(src.len() + 256);
  let (mut i, mut in_macro) = (0, false);
  while i < b.len() {
    if b[i] == b'{' && i + 1 < b.len() && matches!(b[i + 1], b'{' | b'%' | b'#') {
      let close = match b[i + 1] { b'{' => "}}", b'%' => "%}", _ => "#}" };
      let end = src[i..].find(close).map(|e| i + e + 2).unwrap_or(b.len());
      let tag = &src[i..end];
      if b[i + 1] == b'%' {
        let w = tag.trim_start_matches("{%").trim_start_matches('-').trim_start();
        if w.starts_with("macro ") { in_macro = true } else if w.starts_with("endmacro") { in_macro = false }
      }
      out.push_str(tag); i = end; continue;
    }
    if in_macro && b[i] == b'<' && i + 1 < b.len() && b[i + 1].is_ascii_alphabetic() {
      let mut j = i + 1;
      while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'-') { j += 1 }
      out.push_str(&src[i..j]); out.push(' '); out.push_str(attr); i = j; continue;
    }
    let ch = src[i..].chars().next().unwrap();
    out.push(ch); i += ch.len_utf8();
  }
  out
}

/// B-marker: macro output wrapped in <!--y:HASH-->…<!--/y-->, slots in <!--s-->…<!--/s-->; lol_html post-pass.
fn rewrite_markers(html: &str) -> String {
  let stack: Rc<RefCell<Vec<Option<String>>>> = Rc::new(RefCell::new(Vec::new()));
  let (s1, s2) = (stack.clone(), stack.clone());
  let settings = RewriteStrSettings::new()
    .append_element_content_handler(element!("*", move |el| {
      if let Some(Some(a)) = s1.borrow().last() { el.set_attribute(a, "").unwrap(); }
      Ok(())
    }))
    .append_element_content_handler(comments!("*", move |c| {
      let t = c.text();
      let mut s = s2.borrow_mut();
      if let Some(h) = t.strip_prefix("y:") { s.push(Some(h.to_string())); c.remove(); }
      else if t == "s" { s.push(None); c.remove(); }
      else if t == "/y" || t == "/s" { s.pop(); c.remove(); }
      Ok(())
    }));
  rewrite_str(html, settings).unwrap()
}

fn render(row_src: &str, n: usize) -> String {
  let mut env = Environment::new();
  env.add_template_owned("row.html", row_src.to_string()).unwrap();
  env.add_template_owned("page.html", std::fs::read_to_string("fx/page.html").unwrap()).unwrap();
  let md = std::fs::read_to_string("fx/md.html").unwrap();
  env.get_template("page.html").unwrap().render(context! { n => n, content => md }).unwrap()
}

fn main() {
  let row = std::fs::read_to_string("fx/row.html").unwrap();
  let row_css = std::fs::read_to_string("fx/row.css").unwrap();
  let extra = std::fs::read_to_string("fx/row.b.extra.css").unwrap();
  let n = 22;
  let none = Targets::default();
  let tw = Targets::from(Browsers { chrome: v(111, 0), safari: v(16, 4), ios_saf: v(16, 4), firefox: v(128, 0), ..Default::default() });
  let old = Targets::from(Browsers { chrome: v(80, 0), safari: v(13, 0), firefox: v(78, 0), ..Default::default() });

  // ---------- D: global baseline
  let d_css = css(&row_css, tw, true, None, false).unwrap();
  let d_html = render(&row, n);
  std::fs::write("out/d.css", &d_css).unwrap(); std::fs::write("out/d.html", &d_html).unwrap();
  println!("D  css {} B / {} br | html {} B / {} br", d_css.len(), br(&d_css), d_html.len(), br(&d_html));

  // ---------- A: @scope donut, no HTML rewriting
  let a_src = format!("@scope (.c-row) to ([data-slot] > *, .prose > *) {{\n{row_css}}}\n.c-row [data-slot] > h1:first-of-type {{ display: none; }}\n");
  for (name, t) in [("none", none), ("tw-default c111/s16.4/f128", tw), ("old c80/s13/f78", old)] {
    match css(&a_src, t, true, None, false) { Ok(c) => println!("A  targets={name}: {} B\n   {c}", c.len()), Err(e) => println!("A  targets={name}: ERR {e}") }
  }
  println!("A  pretty (tw targets):\n{}", css(&a_src, tw, false, None, false).unwrap());
  // & and :scope and nesting inside @scope under lowering targets
  let amp = "@scope (.c-row) to (.prose) { & { color: red } :scope > a { color: blue } .x { .y & { color: green } &:hover { color: red } } }";
  println!("A  &/:scope/nesting, tw targets: {}", css(amp, tw, true, None, false).unwrap());
  println!("A  &/:scope/nesting, old targets: {}", css(amp, old, true, None, false).unwrap());
  let a_css = css(&a_src, tw, true, None, false).unwrap();
  let t_a = time(2000, || { css(&a_src, tw, true, None, false).unwrap(); });
  std::fs::write("out/a.css", &a_css).unwrap(); std::fs::write("out/a.html", &d_html).unwrap();
  println!("A  css {} B / {} br | html {} B / {} br (identical to D) | css pass {:?} | html pass 0", a_css.len(), br(&a_css), d_html.len(), br(&d_html), t_a);

  // ---------- B: attribute scoping
  let b_src = format!("{row_css}{extra}");
  println!("B  :global without css_modules: {:?}", css(&b_src, tw, true, None, true));
  let cfg = || Some(Config { pattern: Pattern::parse("[local]").unwrap(), dashed_idents: false, animation: false, grid: false, custom_idents: false, container: false, pure: false });
  let b_css = css(&b_src, tw, true, cfg(), true).unwrap();
  println!("B  css pretty:\n{}", css(&b_src, tw, false, cfg(), true).unwrap());
  let t_bcss = time(2000, || { css(&b_src, tw, true, cfg(), true).unwrap(); });
  // B-static
  let row_static = inject_static(&row, HASH);
  let t_inj = time(2000, || { inject_static(&row, HASH); });
  println!("B-static macro source after load-time injection:\n{row_static}");
  let b_html = render(&row_static, n);
  let t_render_plain = time(500, || { render(&row, n); });
  let t_render_static = time(500, || { render(&row_static, n); });
  std::fs::write("out/b.css", &b_css).unwrap(); std::fs::write("out/b.html", &b_html).unwrap();
  println!("B  css {} B / {} br | html {} B / {} br | css pass {:?} | inject {:?}/template | render(env+parse+render) plain {:?} vs static {:?}",
    b_css.len(), br(&b_css), b_html.len(), br(&b_html), t_bcss, t_inj, t_render_plain, t_render_static);
  // B-marker
  let row_marked = row.replace("{% macro row(href, title) %}", &format!("{{% macro row(href, title) %}}<!--y:{HASH}-->"))
    .replace("{% endmacro %}", "<!--/y-->{% endmacro %}")
    .replace("{{ caller() }}", "<!--s-->{{ caller() }}<!--/s-->");
  let marked = render(&row_marked, n);
  let bm_html = rewrite_markers(&marked);
  let t_rw = time(1000, || { rewrite_markers(&marked); });
  let t_noop = time(1000, || { rewrite_str(&d_html, RewriteStrSettings::new()).unwrap(); });
  let norm = |s: &str| s.replace(&format!(" {HASH}=\"\""), &format!(" {HASH}")).split_whitespace().collect::<String>();
  std::fs::write("out/b-marker.html", &bm_html).unwrap();
  println!("B-marker input {} B -> {} B | lol_html rewrite {:?} (no-handler pass over {} B: {:?}) | same DOM as static: {}",
    marked.len(), bm_html.len(), t_rw, d_html.len(), t_noop, norm(&bm_html) == norm(&b_html));
  let cnt = |s: &str, pat: &str| s.matches(pat).count();
  println!("B  elements tagged: static {} marker {} | <em> in page {} of which tagged {}", cnt(&b_html, HASH), cnt(&bm_html, HASH), cnt(&b_html, "<em"), cnt(&b_html, &format!("<em {HASH}")));
  println!("first row (B-static):\n{}", &b_html[b_html.find("<li").unwrap()..b_html.find("<h2").unwrap()]);

  // ---------- C: CSS modules
  let mut po = ParserOptions::default();
  po.css_modules = Some(Config::default());
  po.filename = "components/row.css".into();
  let mut ss = StyleSheet::parse(&row_css, po).unwrap();
  ss.minify(MinifyOptions::default()).unwrap();
  let r = ss.to_css(PrinterOptions { minify: true, targets: tw, ..Default::default() }).unwrap();
  let mut ex: Vec<_> = r.exports.unwrap().into_iter().map(|(k, e)| format!("{k}->{}", e.name)).collect(); ex.sort();
  println!("C  css {} B / {} br: {}\n   exports: {:?}", r.code.len(), br(&r.code), r.code, ex);
}
#[test]
fn amp() {
  let modern = Targets::from(Browsers { chrome: v(143, 0), safari: v(26, 4), firefox: v(146, 0), ..Default::default() });
  let amp = "@scope (.c-row) { & { color: red } & img { color: blue } }";
  println!("AMP none: {}", css(amp, Targets::default(), true, None, false).unwrap());
  println!("AMP modern: {}", css(amp, modern, true, None, false).unwrap());
}
#[test]
fn amp2() {
  use lightningcss::targets::Features;
  let tw = Targets { browsers: Some(Browsers { chrome: v(111, 0), safari: v(16, 4), firefox: v(128, 0), ..Default::default() }), include: Features::empty(), exclude: Features::Nesting };
  let amp = "@scope (.c-row) { & { color: red } & img { color: blue } }";
  println!("AMP tw exclude-nesting: {}", css(amp, tw, true, None, false).unwrap());
  let tw2 = Targets::from(Browsers { chrome: v(111, 0), safari: v(16, 4), firefox: v(128, 0), ..Default::default() });
  println!("AMP tw: {}", css(amp, tw2, true, None, false).unwrap());
  let c120 = Targets::from(Browsers { chrome: v(120, 0), safari: v(17, 4), firefox: v(128, 0), ..Default::default() });
  println!("AMP c120/s17.4/f128: {}", css(amp, c120, true, None, false).unwrap());
}
