//! B-static scoped styles: stamp literal start tags at template load, scope sidecar selectors with lightningcss.
//! Throwaway.
use lightningcss::css_modules::{Config, Pattern};
use lightningcss::printer::PrinterOptions;
use lightningcss::rules::{CssRule, CssRuleList};
use lightningcss::selector::{Combinator, Component, PseudoClass, Selector};
use lightningcss::stylesheet::{MinifyOptions, ParserOptions, StyleSheet};
use lightningcss::targets::{Browsers, Targets};
use lightningcss::traits::ToCss;

// ---------------------------------------------------------------- scope id

/// `data-y-` + 6 base36 chars of FNV-1a 64 over the template path. Path-derived: a CSS edit never changes HTML.
pub fn scope_attr(path: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in path.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let mut s = String::new();
    for _ in 0..6 {
        s.push(char::from_digit((h % 36) as u32, 36).unwrap());
        h /= 36;
    }
    format!("data-y-{s}")
}

// ---------------------------------------------------------------- template rewriter

/// Elements that never take the attribute (Astro's never-scoped list, minus the obsolete ones).
const NEVER: [&str; 9] = ["head", "meta", "link", "title", "base", "script", "style", "noscript", "template"];
/// Elements whose body is not markup.
const RAW_TEXT: [&str; 4] = ["script", "style", "textarea", "title"];

#[derive(Debug, Clone)]
pub struct Tag {
    /// byte range of the whole start tag in the ORIGINAL source
    pub start: usize,
    pub end: usize,
    pub name: String,
    pub dynamic_name: bool,
    pub stamped: bool,
}

#[derive(Debug, Default)]
pub struct Rewritten {
    pub out: String,
    /// (offset in the original source, inserted byte length), ascending. This is the offset map.
    pub map: Vec<(usize, usize)>,
    pub tags: Vec<Tag>,
    /// (offset in the original source, message)
    pub warns: Vec<(usize, String)>,
}

fn at(b: &[u8], i: usize, s: &str) -> bool {
    b.len() >= i + s.len() && &b[i..i + s.len()] == s.as_bytes()
}
fn is_jinja(b: &[u8], i: usize) -> bool {
    at(b, i, "{{") || at(b, i, "{%") || at(b, i, "{#")
}
/// `b[i..]` starts a Jinja delimiter; returns the offset just past its close. String literals are skipped.
fn jinja_end(b: &[u8], i: usize) -> usize {
    let close: &[u8] = match b[i + 1] {
        b'{' => b"}}",
        b'%' => b"%}",
        _ => b"#}",
    };
    let comment = b[i + 1] == b'#';
    let mut j = i + 2;
    let mut q = 0u8;
    while j < b.len() {
        let c = b[j];
        if q != 0 {
            if c == b'\\' { j += 2; continue; }
            if c == q { q = 0 }
        } else if !comment && (c == b'"' || c == b'\'') {
            q = c;
        } else if b[j..].starts_with(close) {
            return j + 2;
        }
        j += 1;
    }
    b.len()
}
fn block_keyword(src: &str, i: usize, e: usize) -> &str {
    src[i + 2..e].trim_start_matches(['-', '+']).trim_start().split(|c: char| !c.is_ascii_alphanumeric() && c != '_').next().unwrap_or("")
}
fn find_ci(b: &[u8], from: usize, needle: &str) -> Option<usize> {
    let n = needle.as_bytes();
    (from..b.len().saturating_sub(n.len() - 1)).find(|&i| b[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// Scan a template. With `insert = Some(attr)` every literal start tag gets ` attr` right after its tag name.
/// With `None` nothing is inserted (the scan still returns tags and warnings, for `check`).
pub fn rewrite(src: &str, insert: Option<&str>) -> Rewritten {
    let b = src.as_bytes();
    let n = b.len();
    let mut r = Rewritten::default();
    let mut ins: Vec<(usize, bool)> = vec![]; // (offset, needs trailing space)
    let mut i = 0;
    // tag state: Some((start, name, dynamic, stamped, quote))
    let mut tag: Option<(usize, String, bool, bool, u8)> = None;
    while i < n {
        if is_jinja(b, i) {
            let mut e = jinja_end(b, i);
            if b[i + 1] == b'%' {
                match block_keyword(src, i, e) {
                    "raw" => {
                        // verbatim until {% endraw %}: nothing in a raw body is stamped
                        let mut j = e;
                        e = n;
                        while let Some(p) = src[j..].find("{%").map(|p| p + j) {
                            let pe = jinja_end(b, p);
                            if block_keyword(src, p, pe) == "endraw" { e = pe; break; }
                            j = p + 2;
                        }
                    }
                    "autoescape" if tag.is_none() && src[i..e].contains("false") => r.warns.push((i, "autoescape is off: elements in printed strings are not stamped".into())),
                    _ => {}
                }
            } else if b[i + 1] == b'{' && tag.is_none() {
                let body = &src[i + 2..e.saturating_sub(2)];
                if body.split('|').skip(1).any(|f| f.trim_start().starts_with("safe")) {
                    r.warns.push((i, "`| safe` output is not stamped: sidecar rules reach its elements only through :global()".into()));
                }
            }
            i = e;
            continue;
        }
        if let Some((start, name, dynamic, stamped, quote)) = tag.as_mut() {
            let c = b[i];
            if *quote != 0 {
                if c == *quote { *quote = 0 }
            } else if c == b'"' || c == b'\'' {
                *quote = c;
            } else if c == b'>' {
                let lname = name.to_ascii_lowercase();
                if !name.is_empty() || *dynamic {
                    r.tags.push(Tag { start: *start, end: i + 1, name: name.clone(), dynamic_name: *dynamic, stamped: *stamped });
                }
                let raw = !*dynamic && RAW_TEXT.contains(&lname.as_str());
                tag = None;
                i += 1;
                if raw {
                    i = find_ci(b, i, &format!("</{lname}")).unwrap_or(n);
                }
                continue;
            }
            i += 1;
            continue;
        }
        if at(b, i, "<!--") {
            i = src[i..].find("-->").map(|p| i + p + 3).unwrap_or(n);
            continue;
        }
        if b[i] == b'<' && i + 1 < n {
            let c = b[i + 1];
            if c == b'/' || c == b'!' || c == b'?' {
                tag = Some((i, String::new(), false, false, 0)); // closing tag, doctype, PI: skip to '>'
                i += 2;
                continue;
            }
            if at(b, i + 1, "{%") {
                r.warns.push((i, "tag name built by control flow: not stamped (write the tag once per branch)".into()));
                tag = Some((i, String::new(), true, false, 0));
                i += 1;
                continue;
            }
            if c.is_ascii_alphabetic() || at(b, i + 1, "{{") {
                let mut j = i + 1;
                let mut name = String::new();
                let mut dynamic = false;
                loop {
                    if j < n && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'-' | b':' | b'_' | b'.')) {
                        name.push(b[j] as char);
                        j += 1;
                    } else if at(b, j, "{{") {
                        // `<h{{ level }}` and `<a{{ attrs|attrs }}` look alike; stamping after the expression is right for both
                        dynamic = true;
                        j = jinja_end(b, j);
                    } else {
                        break;
                    }
                }
                if dynamic {
                    r.warns.push((i, "expression glued to a tag name is read as a dynamic tag name: stamped, but `check` cannot match type selectors against it".into()));
                }
                let stamped = dynamic || !NEVER.contains(&name.to_ascii_lowercase().as_str());
                if stamped {
                    ins.push((j, j < n && !(b[j].is_ascii_whitespace() || b[j] == b'>' || b[j] == b'/')));
                }
                tag = Some((i, name, dynamic, stamped, 0));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    if let Some((start, ..)) = tag {
        r.warns.push((start, "unterminated tag (unbalanced quotes across Jinja branches?)".into()));
    }
    match insert {
        None => r.out = src.to_string(),
        Some(attr) => {
            let mut out = String::with_capacity(n + ins.len() * (attr.len() + 2));
            let mut last = 0;
            for (o, space) in ins {
                out.push_str(&src[last..o]);
                out.push(' ');
                out.push_str(attr);
                if space { out.push(' ') }
                r.map.push((o, attr.len() + 1 + space as usize));
                last = o;
            }
            out.push_str(&src[last..]);
            r.out = out;
        }
    }
    r
}

/// Map a byte offset in the rewritten source back to the original source.
pub fn to_orig(map: &[(usize, usize)], rewritten: usize) -> usize {
    let mut shift = 0;
    for &(o, len) in map {
        if o + shift >= rewritten { break }
        if rewritten < o + shift + len { return o }
        shift += len;
    }
    rewritten - shift
}

pub fn line_col(src: &str, off: usize) -> (usize, usize) {
    let off = off.min(src.len());
    let line = src[..off].matches('\n').count() + 1;
    let col = src[..off].rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (line, col)
}

// ---------------------------------------------------------------- literal elements, for `check`

#[derive(Debug, Default)]
pub struct Elem {
    pub name: String,
    pub dynamic_name: bool,
    pub classes: Vec<String>,
    /// class value contains Jinja, or Jinja sits between attributes: any class may be present
    pub open_class: bool,
    pub id: Option<String>,
    pub open_id: bool,
    pub offset: usize,
}

fn strip_jinja(s: &str) -> (String, bool) {
    let b = s.as_bytes();
    let (mut out, mut i, mut had) = (String::new(), 0, false);
    while i < b.len() {
        if is_jinja(b, i) {
            let e = jinja_end(b, i);
            had = true;
            out.push(' ');
            i = e;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    (out, had)
}

pub fn elems(src: &str, tags: &[Tag]) -> Vec<Elem> {
    tags.iter()
        .filter(|t| t.stamped)
        .map(|t| {
            let text = &src[t.start..t.end];
            let mut e = Elem { name: t.name.to_ascii_lowercase(), dynamic_name: t.dynamic_name, offset: t.start, ..Default::default() };
            let b = text.as_bytes();
            let mut i = 1 + t.name.len().min(b.len() - 1);
            while i < b.len() {
                if is_jinja(b, i) {
                    // Jinja between attributes may print any attribute
                    let je = jinja_end(b, i);
                    if b[i + 1] != b'#' { e.open_class = true; e.open_id = true; }
                    i = je;
                    continue;
                }
                if b[i].is_ascii_whitespace() || b[i] == b'/' || b[i] == b'>' { i += 1; continue; }
                let s = i;
                while i < b.len() && !(b[i].is_ascii_whitespace() || matches!(b[i], b'=' | b'>' | b'/')) && !is_jinja(b, i) { i += 1 }
                let an = text[s..i].to_ascii_lowercase();
                let mut val = "";
                if i < b.len() && b[i] == b'=' {
                    i += 1;
                    if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                        let q = b[i];
                        let vs = i + 1;
                        i = vs;
                        while i < b.len() && b[i] != q {
                            if is_jinja(b, i) { i = jinja_end(b, i) } else { i += 1 }
                        }
                        val = &text[vs..i.min(b.len())];
                        i += 1;
                    } else {
                        let vs = i;
                        while i < b.len() && !(b[i].is_ascii_whitespace() || b[i] == b'>') {
                            if is_jinja(b, i) { i = jinja_end(b, i) } else { i += 1 }
                        }
                        val = &text[vs..i.min(b.len())];
                    }
                }
                let (plain, had) = strip_jinja(val);
                match an.as_str() {
                    "class" => {
                        e.classes.extend(plain.split_whitespace().map(String::from));
                        e.open_class |= had;
                    }
                    "id" => {
                        e.id = Some(plain.trim().to_string());
                        e.open_id |= had;
                    }
                    _ => {}
                }
            }
            e
        })
        .collect()
}

// ---------------------------------------------------------------- CSS

fn v(maj: u32, min: u32) -> Option<u32> { Some(maj << 16 | min << 8) }
/// Tailwind v4's default targets, as in the #19 probe.
pub fn targets() -> Targets {
    Targets::from(Browsers { chrome: v(111, 0), safari: v(16, 4), ios_saf: v(16, 4), firefox: v(128, 0), ..Default::default() })
}
fn modules() -> Config {
    // :global() only parses with css_modules on. `[local]` and every rename switch off keep all names as written.
    Config { pattern: Pattern::parse("[local]").unwrap(), dashed_idents: false, animation: false, grid: false, custom_idents: false, container: false, pure: false }
}

#[derive(Debug, Clone)]
pub struct CssDiag {
    pub line: u32,
    pub col: u32,
    pub msg: String,
}

pub struct Scoped {
    pub css: String,
    pub diags: Vec<CssDiag>,
    pub keyframes: Vec<String>,
}

struct Walk<'a> {
    attr: Option<&'a str>,
    elems: Option<&'a [Elem]>,
    diags: Vec<CssDiag>,
    keyframes: Vec<String>,
}

fn sel_text(s: &Selector) -> String {
    s.to_css_string(PrinterOptions::default()).unwrap_or_default()
}

impl<'a> Walk<'a> {
    fn rules<'i>(&mut self, list: &mut CssRuleList<'i>) {
        for rule in list.0.iter_mut() {
            match rule {
                CssRule::Style(s) => {
                    let loc = (s.loc.line + 1, s.loc.column);
                    for sel in s.selectors.0.iter_mut() {
                        self.selector(sel, loc);
                    }
                    self.rules(&mut s.rules);
                }
                CssRule::Media(r) => self.rules(&mut r.rules),
                CssRule::Supports(r) => self.rules(&mut r.rules),
                CssRule::Container(r) => self.rules(&mut r.rules),
                CssRule::LayerBlock(r) => self.rules(&mut r.rules),
                CssRule::Scope(r) => self.rules(&mut r.rules),
                CssRule::StartingStyle(r) => self.rules(&mut r.rules),
                CssRule::MozDocument(r) => self.rules(&mut r.rules),
                CssRule::Nesting(r) => {
                    for sel in r.style.selectors.0.iter_mut() {
                        self.selector(sel, (r.loc.line + 1, r.loc.column));
                    }
                    self.rules(&mut r.style.rules);
                }
                CssRule::Keyframes(k) => self.keyframes.push(k.name.to_css_string(PrinterOptions::default()).unwrap_or_default()),
                _ => {}
            }
        }
    }

    /// Append the scope attribute to every compound of a selector, except compounds that are only
    /// `:global(...)` and compounds that contain `&` (the parent rule already carries the attribute).
    /// Arguments of :is(), :where(), :not() and :has() are left alone.
    fn selector<'i>(&mut self, sel: &mut Selector<'i>, loc: (u32, u32)) {
        let text = sel_text(sel);
        // iter_raw_parse_order_from yields compounds left to right but the simple selectors of each compound
        // reversed; Selector::from wants parse order. (The #19 probe skipped this and printed `:hover.list-row`;
        // with a type selector that is the invalid `:focus-visiblea`.)
        let mut segs: Vec<(Vec<Component<'i>>, Option<Component<'i>>)> = vec![]; // (compound, combinator after it)
        let mut cur: Vec<Component<'i>> = Vec::new();
        for c in sel.iter_raw_parse_order_from(0).cloned() {
            if c.as_combinator().is_some() {
                cur.reverse();
                segs.push((std::mem::take(&mut cur), Some(c)));
            } else {
                cur.push(c);
            }
        }
        cur.reverse();
        segs.push((cur, None));
        let pseudo_comb = |c: &Option<Component<'i>>| matches!(c.as_ref().and_then(|c| c.as_combinator()), Some(Combinator::PseudoElement | Combinator::Part | Combinator::SlotAssignment));
        let mut out: Vec<Component<'i>> = Vec::new();
        let mut after_pseudo = false;
        for (mut compound, comb) in segs {
            let is_global = |c: &Component| matches!(c, Component::NonTSPseudoClass(PseudoClass::Global { .. }));
            if compound.iter().any(|c| matches!(c, Component::NonTSPseudoClass(PseudoClass::Local { .. }))) {
                self.diags.push(CssDiag { line: loc.0, col: loc.1, msg: format!("`{text}`: :local() is not supported; sidecar selectors are scoped by default") });
            }
            let has_global = compound.iter().any(is_global);
            let only_global = has_global && compound.iter().all(is_global);
            let nesting = compound.iter().any(|c| matches!(c, Component::Nesting));
            let root = compound.iter().any(|c| matches!(c, Component::Root));
            if has_global && !only_global {
                self.diags.push(CssDiag { line: loc.0, col: loc.1, msg: format!("`{text}`: :global() inside a scoped compound does nothing (scoping is by attribute, not by class); drop it or make it its own compound") });
            }
            // `::before` after a pseudo-element combinator belongs to the compound before it
            let bare_pseudo = compound.is_empty() && pseudo_comb(&comb);
            if !after_pseudo && (!compound.is_empty() || bare_pseudo) && !only_global && !nesting && !root {
                if let Some(elems) = self.elems {
                    self.check_compound(&compound, elems, &text, loc);
                }
                if let Some(attr) = self.attr {
                    compound.push(Component::AttributeInNoNamespaceExists { local_name: attr.to_string().into(), local_name_lower: attr.to_string().into() });
                }
            }
            after_pseudo = pseudo_comb(&comb);
            out.append(&mut compound);
            out.extend(comb);
        }
        if self.attr.is_some() {
            *sel = Selector::from(out);
        }
    }

    /// `check`: a scoped compound must be able to match some literal element of the template.
    fn check_compound(&mut self, compound: &[Component], elems: &[Elem], text: &str, loc: (u32, u32)) {
        let mut ty: Option<String> = None;
        let (mut classes, mut ids) = (vec![], vec![]);
        for c in compound {
            match c {
                Component::LocalName(n) => ty = Some(n.lower_name.to_string()),
                Component::Class(c) => classes.push(c.to_string()),
                Component::ID(c) => ids.push(c.to_string()),
                _ => {}
            }
        }
        let hit = elems.iter().any(|e| {
            (ty.is_none() || e.dynamic_name || ty.as_deref() == Some(e.name.as_str()))
                && classes.iter().all(|c| e.open_class || e.classes.contains(c))
                && ids.iter().all(|i| e.open_id || e.id.as_deref() == Some(i.as_str()))
        });
        if !hit {
            let what = format!("{}{}{}", ty.unwrap_or_default(), ids.iter().map(|c| format!("#{c}")).collect::<String>(), classes.iter().map(|c| format!(".{c}")).collect::<String>());
            let what = if what.is_empty() { "*".to_string() } else { what };
            self.diags.push(CssDiag {
                line: loc.0,
                col: loc.1,
                msg: format!("`{text}`: `{what}` matches no literal element of this template (elements of other templates, slots and rendered content need :global())"),
            });
        }
    }
}

/// Scope a sidecar. `attr = None` leaves selectors alone (the unscoped baseline, same parse/minify/print).
/// `elems = Some(..)` adds the "matches no literal element" validation.
pub fn scope_css(src: &str, attr: Option<&str>, elems: Option<&[Elem]>, minify: bool, targets: Targets) -> Result<Scoped, CssDiag> {
    let mut po = ParserOptions::default();
    po.css_modules = Some(modules());
    let mut ss = StyleSheet::parse(src, po).map_err(|e| {
        let (line, col) = e.loc.as_ref().map(|l| (l.line + 1, l.column)).unwrap_or((0, 0));
        CssDiag { line, col, msg: format!("{}", e.kind) }
    })?;
    let mut w = Walk { attr, elems, diags: vec![], keyframes: vec![] };
    w.rules(&mut ss.rules);
    if minify {
        ss.minify(MinifyOptions { targets, ..Default::default() }).map_err(|e| CssDiag { line: 0, col: 0, msg: e.to_string() })?;
    }
    let r = ss.to_css(PrinterOptions { minify, targets, ..Default::default() }).map_err(|e| CssDiag { line: 0, col: 0, msg: e.to_string() })?;
    Ok(Scoped { css: r.code, diags: w.diags, keyframes: w.keyframes })
}
