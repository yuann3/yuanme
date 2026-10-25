//! Scoped Sidecar styles (ADR 0006): a Template with a Sidecar stylesheet
//! has its scope attribute added to every start tag written in its source,
//! before minijinja reads it, following the rules of the spike on
//! `spike/scoped-styles`. Tags that Jinja prints, from strings, Values or
//! other Templates, are never touched.

use std::ops::Range;

/// Elements that never take the attribute, as in Astro: what they hold is
/// never styled, or not in the document.
const NEVER: [&str; 9] = [
    "head", "meta", "link", "title", "base", "script", "style", "noscript", "template",
];

/// Elements whose content is text, not markup.
const RAW_TEXT: [&str; 4] = ["script", "style", "textarea", "title"];

/// The Sidecar stylesheet of the Template `name`, by its path from the
/// project root: `templates/components/row.css` for `components/row.html`.
pub fn sidecar(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    format!("templates/{stem}.css")
}

/// The attribute that scopes the Template `name` and its Sidecar:
/// `data-y-` and six base-36 digits of the FNV-1a hash of the Sidecar's
/// path. It is frozen once v1 ships, since Sites' HTML holds it. It comes
/// from the path, not the CSS, so editing a Sidecar never changes a Page,
/// and two Templates that share a Sidecar, `row.html` and `row.xml`, share
/// it.
pub fn attribute(name: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in sidecar(name).bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let mut out = String::from("data-y-");
    for _ in 0..6 {
        out.push(char::from_digit((hash % 36) as u32, 36).expect("below 36"));
        hash /= 36;
    }
    out
}

/// A Template's source with its scope attribute added.
pub struct Scoped {
    pub text: String,
    /// Where each insertion is in the original source, and its length, in
    /// order. Inserted text has no newline, so lines never move.
    inserted: Vec<(usize, usize)>,
}

impl Scoped {
    /// The byte of the original source that byte `at` of [`Scoped::text`]
    /// comes from; inserted text maps to where it was inserted.
    pub fn original(&self, at: usize) -> usize {
        let mut shift = 0;
        for &(start, len) in &self.inserted {
            if start + shift >= at {
                break;
            }
            if at < start + shift + len {
                return start;
            }
            shift += len;
        }
        at - shift
    }
}

/// Why a start tag can't be scoped as written.
#[derive(Debug, PartialEq, Eq)]
pub enum Unscoped {
    /// `<{{ tag }}>` or `<h{{ n }}>`: scoped, but no type selector can be
    /// checked against it.
    DynamicName,
    /// `<{% if x %}h1{% else %}h2{% endif %}>`: not scoped.
    BranchedName,
    /// A tag whose end the scan never found, such as one whose quotes are
    /// unbalanced in each branch of an `{% if %}`: the tags after it are
    /// not scoped.
    Unterminated,
}

/// A start tag the attribute is added to, for `check` to match a Sidecar's
/// selectors against.
#[derive(Debug, Default)]
pub struct Element {
    /// Lowercase. `None` when an expression builds it.
    pub name: Option<String>,
    pub classes: Vec<String>,
    /// When Jinja prints part of the `class`, or prints attributes, so any
    /// class may be there.
    pub any_class: bool,
    pub id: Option<String>,
    pub any_id: bool,
}

impl Element {
    /// Whether a compound selector of the type `name`, the `classes` and
    /// the `ids` can match it. Attributes and pseudo-classes are left out:
    /// Scripts set them.
    pub fn matches(&self, name: Option<&str>, classes: &[String], ids: &[String]) -> bool {
        let name_ok = match (name, &self.name) {
            (None, _) | (_, None) => true,
            (Some(want), Some(have)) => want.eq_ignore_ascii_case(have),
        };
        name_ok
            && classes
                .iter()
                .all(|c| self.any_class || self.classes.contains(c))
            && ids
                .iter()
                .all(|i| self.any_id || self.id.as_deref() == Some(i))
    }
}

/// `source` with `attribute` added to each of its start tags, and the tags
/// that couldn't be scoped as written.
pub fn rewrite(source: &str, attribute: &str) -> (Scoped, Vec<(Range<usize>, Unscoped)>) {
    let scan = scan(source);
    let mut text = String::with_capacity(source.len() + scan.tags.len() * (attribute.len() + 2));
    let mut inserted = Vec::new();
    let mut done = 0;
    for tag in scan.tags.iter().filter(|t| t.scoped) {
        text.push_str(&source[done..tag.name_end]);
        let mut add = format!(" {attribute}");
        // `<a{% if x %} class="a"{% endif %}>` needs no space after; a
        // `{{ }}` glued to the name does.
        if source[tag.name_end..]
            .starts_with(|c: char| !(c.is_ascii_whitespace() || c == '>' || c == '/'))
        {
            add.push(' ');
        }
        text.push_str(&add);
        inserted.push((tag.name_end, add.len()));
        done = tag.name_end;
    }
    text.push_str(&source[done..]);
    (Scoped { text, inserted }, scan.unscoped)
}

/// Every start tag of `source` that [`rewrite`] adds the attribute to.
pub fn elements(source: &str) -> Vec<Element> {
    scan(source)
        .tags
        .iter()
        .filter(|t| t.scoped)
        .map(|t| element(source, t))
        .collect()
}

struct Tag {
    /// From its `<` to just past its `>`.
    at: Range<usize>,
    /// Where its name ends, which is where the attribute goes.
    name_end: usize,
    /// As written; `None` when an expression builds it.
    name: Option<String>,
    scoped: bool,
}

struct Scan {
    tags: Vec<Tag>,
    unscoped: Vec<(Range<usize>, Unscoped)>,
}

/// The start tags of a Template's source. Jinja's delimiters are skipped
/// whole, string literals in them included, wherever they are, so a tag
/// split by `{% if %}` or with `{{ }}` in its values is still one tag.
fn scan(source: &str) -> Scan {
    let b = source.as_bytes();
    let mut tags: Vec<Tag> = Vec::new();
    let mut unscoped = Vec::new();
    // The tag being read: where it starts, its index in `tags` if it is a
    // start tag, and the quote it is in.
    let mut open: Option<(usize, Option<usize>, u8)> = None;
    let mut i = 0;
    while i < b.len() {
        if is_jinja(b, i) {
            let end = jinja_end(b, i);
            i = match keyword(source, i, end) {
                // Nothing in a raw block is markup.
                Some("raw") => raw_end(source, end),
                _ => end,
            };
            continue;
        }
        if let Some((start, index, quote)) = open.as_mut() {
            let c = b[i];
            i += 1;
            if *quote != 0 {
                if c == *quote {
                    *quote = 0;
                }
            } else if c == b'"' || c == b'\'' {
                *quote = c;
            } else if c == b'>' {
                let raw = index.and_then(|n| {
                    let tag = &mut tags[n];
                    tag.at = *start..i;
                    let name = tag.name.as_deref()?.to_ascii_lowercase();
                    RAW_TEXT.contains(&name.as_str()).then_some(name)
                });
                open = None;
                if let Some(name) = raw {
                    i = find_ignore_case(b, i, &format!("</{name}")).unwrap_or(b.len());
                }
            }
            continue;
        }
        if b[i..].starts_with(b"<!--") {
            i = source[i..].find("-->").map_or(b.len(), |e| i + e + 3);
            continue;
        }
        if b[i] != b'<' || i + 1 == b.len() {
            i += 1;
            continue;
        }
        let next = b[i + 1];
        if matches!(next, b'/' | b'!' | b'?') {
            // An end tag, the doctype: read to their `>`.
            open = Some((i, None, 0));
            i += 2;
        } else if b[i + 1..].starts_with(b"{%") {
            unscoped.push((i..i + 1, Unscoped::BranchedName));
            open = Some((i, None, 0));
            i += 1;
        } else if next.is_ascii_alphabetic() || b[i + 1..].starts_with(b"{{") {
            let mut j = i + 1;
            let mut dynamic = false;
            loop {
                if j < b.len() && (b[j].is_ascii_alphanumeric() || b"-:_.".contains(&b[j])) {
                    j += 1;
                } else if b[j..].starts_with(b"{{") {
                    dynamic = true;
                    j = jinja_end(b, j);
                } else {
                    break;
                }
            }
            let name = (!dynamic).then(|| source[i + 1..j].to_string());
            if dynamic {
                unscoped.push((i..j, Unscoped::DynamicName));
            }
            let scoped = name
                .as_deref()
                .is_none_or(|n| !NEVER.contains(&n.to_ascii_lowercase().as_str()));
            open = Some((i, Some(tags.len()), 0));
            tags.push(Tag {
                at: i..j,
                name_end: j,
                name,
                scoped,
            });
            i = j;
        } else {
            i += 1;
        }
    }
    if let Some((start, ..)) = open {
        unscoped.push((start..start + 1, Unscoped::Unterminated));
    }
    Scan { tags, unscoped }
}

fn is_jinja(b: &[u8], i: usize) -> bool {
    b[i..].starts_with(b"{{") || b[i..].starts_with(b"{%") || b[i..].starts_with(b"{#")
}

/// Just past the end of the Jinja delimiter at `i`, skipping string
/// literals in expressions and tags.
fn jinja_end(b: &[u8], i: usize) -> usize {
    let (close, comment): (&[u8], bool) = match b[i + 1] {
        b'{' => (b"}}", false),
        b'%' => (b"%}", false),
        _ => (b"#}", true),
    };
    let mut j = i + 2;
    let mut quote = 0;
    while j < b.len() {
        let c = b[j];
        if quote != 0 {
            if c == b'\\' {
                j += 1;
            } else if c == quote {
                quote = 0;
            }
        } else if !comment && (c == b'"' || c == b'\'') {
            quote = c;
        } else if b[j..].starts_with(close) {
            return j + 2;
        }
        j += 1;
    }
    b.len()
}

/// The name of the block tag from `start` to `end`, such as `raw`.
fn keyword(source: &str, start: usize, end: usize) -> Option<&str> {
    let body = source.get(start + 2..end)?;
    if !source[start..].starts_with("{%") {
        return None;
    }
    body.trim_start_matches(['-', '+'])
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .find(|w| !w.is_empty())
}

/// Just past the `{% endraw %}` after `from`.
fn raw_end(source: &str, from: usize) -> usize {
    let b = source.as_bytes();
    let mut j = from;
    while let Some(at) = source[j..].find("{%").map(|p| p + j) {
        let end = jinja_end(b, at);
        if keyword(source, at, end) == Some("endraw") {
            return end;
        }
        j = at + 2;
    }
    b.len()
}

fn find_ignore_case(b: &[u8], from: usize, needle: &str) -> Option<usize> {
    let n = needle.as_bytes();
    (from..=b.len().checked_sub(n.len())?).find(|&i| b[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// The classes and id a start tag writes.
fn element(source: &str, tag: &Tag) -> Element {
    let text = &source[tag.at.clone()];
    let b = text.as_bytes();
    let mut e = Element {
        name: tag.name.as_ref().map(|n| n.to_ascii_lowercase()),
        ..Element::default()
    };
    let mut i = tag.name_end - tag.at.start;
    while i < b.len() {
        if is_jinja(b, i) {
            // Between attributes, an expression may print any of them.
            if b[i + 1] == b'{' {
                e.any_class = true;
                e.any_id = true;
            }
            i = jinja_end(b, i);
            continue;
        }
        if b[i].is_ascii_whitespace() || b[i] == b'/' || b[i] == b'>' {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len()
            && !(b[i].is_ascii_whitespace() || b"=>/".contains(&b[i]))
            && !is_jinja(b, i)
        {
            i += 1;
        }
        let name = text[start..i].to_ascii_lowercase();
        let mut value = "";
        if b.get(i) == Some(&b'=') {
            i += 1;
            let quote = b.get(i).copied().filter(|c| *c == b'"' || *c == b'\'');
            if quote.is_some() {
                i += 1;
            }
            let from = i;
            while i < b.len() {
                let end = match quote {
                    Some(q) => b[i] == q,
                    None => b[i].is_ascii_whitespace() || b[i] == b'>',
                };
                if end {
                    break;
                }
                i = if is_jinja(b, i) {
                    jinja_end(b, i)
                } else {
                    i + 1
                };
            }
            value = &text[from..i.min(b.len())];
            if quote.is_some() {
                i += 1;
            }
        }
        let (plain, printed) = without_jinja(value);
        match name.as_str() {
            "class" => {
                e.classes
                    .extend(plain.split_ascii_whitespace().map(String::from));
                e.any_class |= printed;
            }
            "id" => {
                e.id = Some(plain.trim().to_string());
                e.any_id |= printed;
            }
            _ => {}
        }
    }
    e
}

/// `value` with each Jinja delimiter replaced by a space, and whether it
/// has an expression.
fn without_jinja(value: &str) -> (String, bool) {
    let b = value.as_bytes();
    let mut out = String::new();
    let mut printed = false;
    let mut i = 0;
    let mut done = 0;
    while i < b.len() {
        if is_jinja(b, i) {
            out.push_str(&value[done..i]);
            out.push(' ');
            printed |= b[i + 1] == b'{';
            i = jinja_end(b, i);
            done = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&value[done.min(value.len())..]);
    (out, printed)
}
