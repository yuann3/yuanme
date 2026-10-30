//! The parity test's first two checks (#23): URL equality and the
//! normalized DOM, of a Site against the Parity baseline. The pixel and
//! behaviour checks are in `browser/`; `docs/parity-exceptions.md` sets the
//! rules of all four.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use regex::Regex;
use url::Url;

/// The Parity exception of each rule that tolerates a difference. Every
/// rule normalizes both sides alike, so it lets a difference through and
/// never requires one: the baseline compared with itself passes under all
/// of them.
const RULES: [&str; 12] = [
    "apex-origin",
    "trailing-slash-hrefs",
    "assets-dir",
    "robots-txt",
    "unreferenced-files",
    "head-metas",
    "scoped-styles",
    "single-initial-script",
    "image-formats",
    "self-hosted-fonts",
    "immutable-assets",
    "class-highlighting",
];

/// The Reference site's origin. URLs on it are internal.
const ORIGIN: &str = "https://eyuan.me";

/// HTML's boolean attributes: for these, and only these, `="true"` and a
/// bare attribute are equal. `aria-hidden="true"` is not `aria-hidden`.
const BOOLEAN: [&str; 25] = [
    "allowfullscreen",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "defer",
    "disabled",
    "formnovalidate",
    "hidden",
    "inert",
    "ismap",
    "itemscope",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "selected",
];

/// `/_astro/about.DhiEpsgV.css`, and an image's transforms,
/// `/_astro/beach-market.ChlaUixY_Z1CycSd.webp`.
static ASTRO_ASSET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^/_astro/(.+)\.[A-Za-z0-9_-]{8}(?:_[A-Za-z0-9_-]+)?\.([A-Za-z0-9]+)$").unwrap()
});
/// The Engine's `/assets/site-0123456789abcdef.css`.
static YUANME_ASSET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/assets/(.+)-[0-9a-f]{16}\.([A-Za-z0-9]+)$").unwrap());
/// A `url()`, quoted or not. A quoted one is read whole, so a `url()`
/// inside a `data:` URL isn't taken for a link.
static CSS_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"url\(\s*(?:"([^"]*)"|'([^']*)'|([^'")\s]+))\s*\)"#).unwrap());
/// The Engine's image names keep the width: `park-480`, from `park.jpg`.
static WIDTH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.+)-[0-9]+$").unwrap());

#[derive(Debug, PartialEq)]
pub enum Difference {
    /// An output path of the baseline the candidate lacks: a file's path,
    /// or `asset:` and the source file of a hashed one.
    Missing(String),
    Added(String),
    /// A link target, by path, that only the baseline's Pages and CSS name.
    MissingRef(String),
    AddedRef(String),
    /// A link target that leads to a file in the baseline and doesn't in the
    /// candidate.
    Broken(String),
    Dom {
        page: String,
        diff: String,
    },
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Difference::Missing(p) => write!(f, "{p}: in the baseline, not the candidate"),
            Difference::Added(p) => write!(f, "{p}: in the candidate, not the baseline"),
            Difference::MissingRef(t) => {
                write!(f, "a link to {t}: in the baseline, not the candidate")
            }
            Difference::AddedRef(t) => {
                write!(f, "a link to {t}: in the candidate, not the baseline")
            }
            Difference::Broken(t) => {
                write!(
                    f,
                    "{t}: leads to a file in the baseline, not in the candidate"
                )
            }
            Difference::Dom { page, diff } => {
                write!(f, "{page}: the normalized DOM differs\n{diff}")
            }
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub differences: Vec<Difference>,
    /// How many distinct URLs, paths or elements each Parity exception let
    /// through, on either side.
    pub tolerated: BTreeMap<&'static str, usize>,
}

/// The ids of `docs/parity-exceptions.md`'s tables: the first column of
/// each row.
pub fn exception_ids(path: &Path) -> io::Result<BTreeSet<String>> {
    let row = Regex::new(r"^\|\s*`([a-z0-9-]+)`\s*\|").unwrap();
    Ok(fs::read_to_string(path)?
        .lines()
        .filter_map(|line| Some(row.captures(line)?[1].to_string()))
        .collect())
}

/// Compares two Site directories. Fails, before comparing, if a rule names
/// an id that `ids` lacks.
pub fn compare(
    baseline: &Path,
    candidate: &Path,
    ids: &BTreeSet<String>,
) -> Result<Report, String> {
    let unknown: Vec<&str> = RULES.into_iter().filter(|id| !ids.contains(*id)).collect();
    if !unknown.is_empty() {
        return Err(format!(
            "the harness tolerates differences under ids that docs/parity-exceptions.md lacks: {}",
            unknown.join(", ")
        ));
    }
    let b = Side::load(baseline, "baseline")?;
    let c = Side::load(candidate, "candidate")?;
    let mut differences = Vec::new();

    // Output paths.
    let paths = |s: &Side| -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in &s.files {
            out.entry(s.asset(file).unwrap_or_else(|| file.clone()))
                .or_default()
                .push(file.clone());
        }
        out
    };
    let (bp, cp) = (paths(&b), paths(&c));
    for (path, files) in &bp {
        if cp.contains_key(path) {
            continue;
        }
        let unreferenced = !path.ends_with(".html")
            && path != "/ica.pdf"
            && files.iter().all(|f| !b.referenced().contains(f));
        if let Some(id) = bundled(path) {
            b.tolerate(id, path);
        } else if unreferenced {
            b.tolerate("unreferenced-files", path);
        } else {
            differences.push(Difference::Missing(path.clone()));
        }
    }
    for path in cp.keys().filter(|p| !bp.contains_key(*p)) {
        if path == "/robots.txt" {
            c.tolerate("robots-txt", path);
        } else if path == "/_headers" {
            c.tolerate("immutable-assets", path);
        } else if let Some(id) = bundled(path) {
            c.tolerate(id, path);
        } else {
            differences.push(Difference::Added(path.clone()));
        }
    }

    // Link targets, and whether each leads somewhere.
    let (br, cr) = (b.targets(), c.targets());
    for t in br.keys().filter(|t| !cr.contains_key(*t)) {
        match bundled(t) {
            Some(id) => b.tolerate(id, t),
            None => differences.push(Difference::MissingRef(t.clone())),
        }
    }
    for t in cr.keys().filter(|t| !br.contains_key(*t)) {
        match bundled(t) {
            Some(id) => c.tolerate(id, t),
            None => differences.push(Difference::AddedRef(t.clone())),
        }
    }
    for (t, resolves) in &br {
        if *resolves && cr.get(t) == Some(&false) {
            differences.push(Difference::Broken(t.clone()));
        }
    }

    // The normalized DOM of every Page both have.
    for (page, ours) in &b.pages {
        let Some(theirs) = c.pages.get(page) else {
            continue;
        };
        if ours != theirs {
            differences.push(Difference::Dom {
                page: page.clone(),
                diff: diff(ours, theirs),
            });
        }
    }

    let mut tolerated = BTreeMap::new();
    for (id, _) in b.tally.into_inner().into_iter().chain(c.tally.into_inner()) {
        *tolerated.entry(id).or_default() += 1;
    }
    Ok(Report {
        differences,
        tolerated,
    })
}

/// The Parity exception under which a hashed file of one side needn't have
/// a counterpart of the same name on the other: the Engine bundles CSS and
/// JS its own way, writes no WebP, and self-hosts the fonts.
fn bundled(path: &str) -> Option<&'static str> {
    let name = path.strip_prefix("asset:")?;
    let ext = name.rsplit_once('.')?.1;
    match ext {
        "css" => Some("scoped-styles"),
        "js" => Some("single-initial-script"),
        "webp" => Some("image-formats"),
        "woff2" => Some("self-hosted-fonts"),
        _ => None,
    }
}

/// A unified diff, cut short: the first lines show what changed.
fn diff(ours: &str, theirs: &str) -> String {
    const LINES: usize = 60;
    const WIDTH: usize = 200;
    let text = similar::TextDiff::from_lines(ours, theirs)
        .unified_diff()
        .context_radius(3)
        .header("baseline", "candidate")
        .to_string();
    let n = text.lines().count();
    let mut out = String::new();
    for line in text.lines().take(LINES) {
        match line.char_indices().nth(WIDTH) {
            Some((cut, _)) => out.push_str(&format!("{}…\n", &line[..cut])),
            None => out.push_str(&format!("{line}\n")),
        }
    }
    if n > LINES {
        out.push_str(&format!("… {} more lines\n", n - LINES));
    }
    out
}

/// A link target: its path, or `asset:` and a hashed file's source name,
/// with any query and fragment.
struct Ref {
    target: String,
    /// The file it leads to, if there is one.
    file: Option<String>,
    /// Written with the origin, as canonical links are.
    absolute: bool,
}

/// One side of the comparison.
struct Side {
    name: &'static str,
    /// Every file, as `/about/index.html`.
    files: BTreeSet<String>,
    /// Each Page's normalized DOM.
    pages: BTreeMap<String, String>,
    refs: Vec<Ref>,
    /// What each rule let through, as (id, `side:what`), so a URL seen on
    /// many Pages counts once.
    tally: RefCell<BTreeSet<(&'static str, String)>>,
}

impl Side {
    fn load(root: &Path, name: &'static str) -> Result<Side, String> {
        let mut side = Side {
            name,
            files: BTreeSet::new(),
            pages: BTreeMap::new(),
            refs: Vec::new(),
            tally: RefCell::default(),
        };
        let mut sources = Vec::new();
        for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
            let entry = entry.map_err(|e| format!("{}: {e}", root.display()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry.path().strip_prefix(root).expect("under the root");
            let path = format!(
                "/{}",
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            );
            if path.ends_with(".html") || path.ends_with(".css") {
                sources.push((path.clone(), entry.path().to_path_buf()));
            }
            side.files.insert(path);
        }
        if side.files.is_empty() {
            return Err(format!("{}: no files", root.display()));
        }
        for (path, file) in sources {
            let bytes = fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            if path.ends_with(".css") {
                let css = String::from_utf8_lossy(&bytes);
                for m in CSS_URL.captures_iter(&css) {
                    let url = m.get(1).or(m.get(2)).or(m.get(3)).expect("one matches");
                    side.add_ref(url.as_str(), &path);
                }
            } else {
                let dom = html5ever::parse_document(RcDom::default(), Default::default())
                    .from_utf8()
                    .read_from(&mut &bytes[..])
                    .map_err(|e| format!("{}: {e}", file.display()))?;
                let mut out = String::new();
                for node in dom.document.children.borrow().iter() {
                    side.render(node, &path, 0, &mut out);
                }
                side.pages.insert(path, out);
            }
        }
        Ok(side)
    }

    fn tolerate(&self, id: &'static str, what: &str) {
        self.tally
            .borrow_mut()
            .insert((id, format!("{}:{what}", self.name)));
    }

    /// The file a URL path leads to.
    fn file(&self, path: &str) -> Option<String> {
        let file = match path.ends_with('/') {
            true => format!("{path}index.html"),
            false => path.to_string(),
        };
        self.files.contains(&file).then_some(file)
    }

    /// A hashed file's source name: hashed URLs are replaced by the source
    /// file they were built from.
    fn asset(&self, path: &str) -> Option<String> {
        if let Some(m) = ASTRO_ASSET.captures(path) {
            return Some(format!("asset:{}.{}", &m[1], &m[2]));
        }
        let m = YUANME_ASSET.captures(path)?;
        self.tolerate("assets-dir", path);
        let ext = &m[2];
        let image = matches!(ext, "jpg" | "jpeg" | "png" | "webp" | "avif" | "gif");
        match WIDTH.captures(&m[1]) {
            Some(w) if image => Some(format!("asset:{}.{ext}", &w[1])),
            _ => Some(format!("asset:{}.{ext}", &m[1])),
        }
    }

    /// `raw`, read on the Page or stylesheet at `base`, if it is internal.
    fn reference(&self, raw: &str, base: &str) -> Option<Ref> {
        let raw = raw.trim();
        if raw.is_empty() || raw.starts_with('#') {
            return None;
        }
        let mut url = Url::parse(ORIGIN).ok()?.join(base).ok()?.join(raw).ok()?;
        if url.host_str() == Some("www.eyuan.me") {
            self.tolerate("apex-origin", raw);
            url.set_host(Some("eyuan.me")).ok()?;
        }
        if !matches!(url.scheme(), "http" | "https") || url.host_str() != Some("eyuan.me") {
            return None;
        }
        let mut path = url.path().to_string();
        if !path.ends_with('/')
            && !self.files.contains(&path)
            && self.files.contains(&format!("{path}/index.html"))
        {
            self.tolerate("trailing-slash-hrefs", raw);
            path.push('/');
        }
        let file = self.file(&path);
        let mut target = self.asset(&path).unwrap_or(path);
        if let Some(q) = url.query() {
            target = format!("{target}?{q}");
        }
        if let Some(f) = url.fragment() {
            target = format!("{target}#{f}");
        }
        Some(Ref {
            target,
            file,
            absolute: raw.contains("//"),
        })
    }

    fn add_ref(&mut self, raw: &str, base: &str) {
        if let Some(r) = self.reference(raw, base) {
            self.refs.push(r);
        }
    }

    /// Every file a link leads to.
    fn referenced(&self) -> BTreeSet<&String> {
        self.refs.iter().filter_map(|r| r.file.as_ref()).collect()
    }

    /// Each link target, and whether every link to it leads to a file.
    fn targets(&self) -> BTreeMap<String, bool> {
        let mut out = BTreeMap::new();
        for r in &self.refs {
            *out.entry(r.target.clone()).or_insert(true) &= r.file.is_some();
        }
        out
    }

    /// A URL attribute as the DOM compares it.
    fn url_value(&self, raw: &str, base: &str) -> String {
        match self.reference(raw, base) {
            Some(r) if r.absolute => format!("{ORIGIN}{}", r.target),
            Some(r) => r.target,
            None => raw.to_string(),
        }
    }

    fn render(&mut self, node: &Handle, page: &str, depth: usize, out: &mut String) {
        let NodeData::Element { name, attrs, .. } = &node.data else {
            return;
        };
        let tag = name.local.to_string();
        // Highlighted code: a `<pre>` is compared by its text only, its look
        // is the pixel diff's (`class-highlighting`).
        if tag == "pre" {
            indent(out, depth, "pre");
            lines(out, depth + 1, &text(node));
            return;
        }
        let attr = |n: &str| {
            attrs
                .borrow()
                .iter()
                .find(|a| &*a.name.local == n)
                .map(|a| a.value.to_string())
                .unwrap_or_default()
        };
        let mut line = tag.clone();
        let mut list: Vec<(String, String)> = Vec::new();
        for a in attrs.borrow().iter() {
            let name = a.name.local.to_string();
            if name.starts_with("data-astro-cid-") || name.starts_with("data-y-") {
                continue;
            }
            let mut value = a.value.to_string();
            if BOOLEAN.contains(&name.as_str()) && (value == "true" || value == name) {
                value.clear();
            }
            match name.as_str() {
                "href" | "src" | "poster" => {
                    if tag != "meta" {
                        self.add_ref(&value, page);
                    }
                    value = self.url_value(&value, page);
                }
                "srcset" => {
                    let mut candidates = Vec::new();
                    for candidate in value.split(',') {
                        let mut words = candidate.split_whitespace();
                        let Some(url) = words.next() else {
                            continue;
                        };
                        self.add_ref(url, page);
                        let mut c = self.url_value(url, page);
                        for w in words {
                            c = format!("{c} {w}");
                        }
                        candidates.push(c);
                    }
                    value = candidates.join(", ");
                }
                "content" if tag == "meta" && value.starts_with("http") => {
                    value = self.url_value(&value, page);
                }
                "class" => {
                    let mut tokens: Vec<&str> = value.split_whitespace().collect();
                    tokens.sort();
                    value = tokens.join(" ");
                }
                _ => {}
            }
            list.push((name, value));
        }
        list.sort();
        for (name, value) in list {
            line = format!("{line} {name}={}", json(&value));
        }
        // Left out after its links are read: the file it loads is still
        // compared as an output path, under the same exception.
        if let Some(id) = bundled_element(&tag, &attr) {
            self.tolerate(id, &line);
            return;
        }
        indent(out, depth, &line);

        let children = node.children.borrow();
        match tag.as_str() {
            "head" => {
                let mut sheets = String::new();
                let mut rest = BTreeSet::new();
                for child in children.iter() {
                    let NodeData::Element { name, attrs, .. } = &child.data else {
                        continue;
                    };
                    let attr = |n: &str| {
                        attrs
                            .borrow()
                            .iter()
                            .find(|a| &*a.name.local == n)
                            .map(|a| a.value.to_string())
                    };
                    let mut one = String::new();
                    self.render(child, page, depth + 2, &mut one);
                    if one.is_empty() {
                        continue;
                    }
                    let local = &*name.local;
                    let meta_name = attr("name").unwrap_or_default();
                    if local == "meta"
                        && (meta_name == "generator"
                            || meta_name.starts_with("astro-view-transitions-"))
                    {
                        self.tolerate("head-metas", one.trim());
                    } else if let Some(id) = (local == "style" || local == "link")
                        .then(|| sheet_bundle(&attr("href").unwrap_or_default(), &text(child)))
                        .flatten()
                    {
                        // The Engine bundles stylesheets its own way
                        // (`scoped-styles`), so a run of bundles is one.
                        self.tolerate(id, one.trim());
                        if !sheets.ends_with(BUNDLES) {
                            indent(&mut sheets, depth + 2, BUNDLES.trim_end());
                        }
                    } else if local == "style"
                        || (local == "link"
                            && attr("rel")
                                .is_some_and(|r| r.split_whitespace().any(|t| t == "stylesheet")))
                    {
                        sheets.push_str(&one);
                    } else {
                        rest.insert(one);
                    }
                }
                indent(out, depth + 1, "stylesheets, in order:");
                out.push_str(&sheets);
                indent(out, depth + 1, "the rest, as a set:");
                out.extend(rest);
            }
            _ => {
                let raw = matches!(tag.as_str(), "script" | "style" | "textarea");
                let kept: Vec<&Handle> = children
                    .iter()
                    .filter(|c| match &c.data {
                        NodeData::Element { .. } => true,
                        NodeData::Text { contents } => {
                            raw || !contents.borrow().trim_ascii().is_empty()
                        }
                        _ => false,
                    })
                    .collect();
                let last = kept.len().saturating_sub(1);
                for (i, child) in kept.iter().enumerate() {
                    let NodeData::Text { contents } = &child.data else {
                        self.render(child, page, depth + 1, out);
                        continue;
                    };
                    let contents = contents.borrow();
                    if raw {
                        lines(out, depth + 1, &contents);
                        continue;
                    }
                    // Collapsed, and trimmed where it meets its element's
                    // edges: `<p>\n  text\n</p>` is `<p>text</p>`.
                    let mut t: String = collapse(&contents);
                    if i == 0 {
                        t = t.trim_start().to_string();
                    }
                    if i == last {
                        t = t.trim_end().to_string();
                    }
                    indent(out, depth + 1, &json(&t));
                }
            }
        }
    }
}

/// What a run of stylesheet bundles in the head compares as.
const BUNDLES: &str = "the Site's stylesheet bundles\n";

/// The Parity exception under which a stylesheet in the head is one of the
/// Site's bundles: a hashed file, or Astro's scoped styles inlined.
fn sheet_bundle(href: &str, text: &str) -> Option<&'static str> {
    if href.starts_with("/assets/code-") {
        Some("class-highlighting")
    } else if ASTRO_ASSET.is_match(href)
        || YUANME_ASSET.is_match(href)
        || text.contains("[data-astro-cid-")
    {
        Some("scoped-styles")
    } else {
        None
    }
}

/// The Parity exception under which an element that loads a Script, a font
/// or a WebP is left out of the DOM comparison,
/// once its links are read: the files are compared as output paths, and
/// what they do by the pixel and behaviour checks.
fn bundled_element(tag: &str, attr: &dyn Fn(&str) -> String) -> Option<&'static str> {
    let rel = attr("rel");
    let href = attr("href");
    let font_origin = href.starts_with("https://fonts.googleapis.com")
        || href.starts_with("https://fonts.gstatic.com");
    match tag {
        "script" if attr("type") == "module" => Some("single-initial-script"),
        "link" if rel == "modulepreload" => Some("single-initial-script"),
        "link" if font_origin || (rel == "preload" && attr("as") == "font") => {
            Some("self-hosted-fonts")
        }
        "source" if attr("type") == "image/webp" => Some("image-formats"),
        _ => None,
    }
}

fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for ch in s.chars() {
        if ch.is_ascii_whitespace() {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(ch);
            space = false;
        }
    }
    out
}

fn text(node: &Handle) -> String {
    let mut out = String::new();
    for child in node.children.borrow().iter() {
        match &child.data {
            NodeData::Text { contents } => out.push_str(&contents.borrow()),
            NodeData::Element { .. } => out.push_str(&text(child)),
            _ => {}
        }
    }
    out
}

fn json(s: &str) -> String {
    serde_json::to_string(s).expect("a string serializes")
}

/// Kept text, one line of it per line, so a diff shows the line that
/// changed in a stylesheet or a code block.
fn lines(out: &mut String, depth: usize, text: &str) {
    for line in text.split_inclusive('\n') {
        indent(out, depth, &json(line));
    }
}

fn indent(out: &mut String, depth: usize, line: &str) {
    out.push_str(&"  ".repeat(depth));
    out.push_str(line);
    out.push('\n');
}
