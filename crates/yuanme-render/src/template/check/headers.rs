//! Props and vars comments: `{# props: … #}` above a macro and
//! `{# vars: … #}` in a Layout, which declare what each one takes.

use std::ops::Range;
use std::rc::Rc;

use yuanme_diag::codes;

use super::Checker;
use super::types::Ty;

const TYPES: &[&str] = &[
    "string",
    "int",
    "float",
    "bool",
    "date",
    "datetime",
    "url",
    "image",
    "enum",
    "list",
    "object",
    "slot",
    "attrs",
    "term",
    "paginator",
];

#[derive(Clone)]
pub(super) struct Prop {
    pub(super) name: String,
    pub(super) ty: Ty,
    pub(super) optional: bool,
    /// The entry in its comment.
    pub(super) at: Range<usize>,
}

/// A `{# props: … #}` or `{# vars: … #}` comment.
pub(super) struct Header<'a> {
    /// The whole comment.
    pub(super) at: Range<usize>,
    pub(super) props: bool,
    /// Its entries, and where each starts in the file.
    pub(super) body: &'a str,
    pub(super) body_at: usize,
}

/// minijinja's lexer drops comments, so headers are found in the source.
pub(super) fn headers(source: &str) -> Vec<Header<'_>> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(start) = source[at..].find("{#").map(|i| at + i) {
        let Some(end) = source[start..].find("#}").map(|i| start + i + 2) else {
            break;
        };
        let inner = &source[start + 2..end - 2];
        let trimmed = inner.trim_start_matches(['-', '+']).trim_start();
        for (props, keyword) in [(true, "props:"), (false, "vars:")] {
            if let Some(body) = trimmed.strip_prefix(keyword) {
                let body = body.trim_end_matches(['-', '+']);
                let body_at = start + 2 + (inner.len() - trimmed.len()) + keyword.len();
                out.push(Header {
                    at: start..end,
                    props,
                    body,
                    body_at,
                });
            }
        }
        at = end;
    }
    out
}

/// Splits a header at the commas outside `<>` and `()`, with the range of
/// each entry relative to the header, trimmed.
fn entries(body: &str) -> Vec<(Range<usize>, &str)> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in body.char_indices().chain([(body.len(), ',')]) {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth <= 0 => {
                let part = &body[start..i];
                let lead = part.len() - part.trim_start().len();
                let text = part.trim();
                if !text.is_empty() {
                    out.push((start + lead..start + lead + text.len(), text));
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    out
}

impl<'a> Checker<'a> {
    pub(super) fn header(&mut self, h: &Header<'_>) -> Vec<Prop> {
        let kind = if h.props { "props" } else { "vars" };
        let mut out = Vec::new();
        for (at, entry) in entries(h.body) {
            let at = h.body_at + at.start..h.body_at + at.end;
            let Some((name, ty)) = entry.split_once(':') else {
                let d = self.error(
                    &codes::TEMPLATE_BAD_HEADER,
                    format!("`{entry}` in a {kind} comment isn't `name: type`"),
                    at,
                );
                self.push(d.help("write `name: type`, or `name?: type` for an optional one"));
                continue;
            };
            let (name, optional) = match name.trim().strip_suffix('?') {
                Some(name) => (name.trim(), true),
                None => (name.trim(), false),
            };
            let ty_at = at.end - ty.len()..at.end;
            let ty = match self.parse_ty(ty.trim()) {
                Ok(ty) => ty,
                Err(unknown) => {
                    let ty_at = self.find(ty_at.start, &unknown);
                    let d = self.error(
                        &codes::TEMPLATE_BAD_HEADER,
                        format!("there is no type `{unknown}`, in the {kind} comment's `{name}`"),
                        ty_at.clone(),
                    );
                    let candidates: Vec<&str> = TYPES
                        .iter()
                        .copied()
                        .chain(self.named.keys().map(String::as_str))
                        .collect();
                    let d = Self::near(d, self.span(ty_at), &unknown, candidates);
                    self.push(d);
                    Ty::Any
                }
            };
            out.push(Prop {
                name: name.to_string(),
                ty,
                optional,
                at,
            });
        }
        out
    }

    /// A type as a header writes it; the error is the unknown part.
    fn parse_ty(&self, s: &str) -> Result<Ty, String> {
        Ok(match s {
            "string" => Ty::Str,
            "int" => Ty::Int,
            "float" => Ty::Float,
            "bool" => Ty::Bool,
            "date" => Ty::Date,
            "datetime" => Ty::Datetime,
            "url" => Ty::Url,
            "image" => Ty::Image,
            "slot" => Ty::Slot,
            "attrs" => Ty::Attrs,
            "object" => Ty::Any,
            _ => {
                if let Some(item) = s.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
                    Ty::List(Rc::new(self.parse_ty(item.trim())?))
                } else if let Some(values) =
                    s.strip_prefix("enum(").and_then(|r| r.strip_suffix(')'))
                {
                    Ty::Enum(Rc::new(
                        values.split('|').map(|v| v.trim().to_string()).collect(),
                    ))
                } else if let Some(ty) = self.named.get(s) {
                    ty.clone()
                } else {
                    return Err(s.to_string());
                }
            }
        })
    }
}
