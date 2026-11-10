//! Math at build time (#17, #50), converted to MathML Core by math-core.
//!
//! An Entry's math is converted in one `convert_all`, in the order it is
//! written, so equations are numbered as amsmath numbers them and an
//! `\eqref` finds an equation written after it. The Engine checks the
//! references itself: math-core renders an unknown Label as `(??)` with
//! only a warning that says nothing of where it is.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;

use math_core::{LatexError, LatexToMathML, MathCoreConfig, MathDisplay};
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Span, codes};

use crate::config::similar;
use crate::document::{Document, Kind, NodeId};
use crate::label;

/// Macros an Org file's `#+LATEX_HEADER` defines, as LaTeX that defines
/// them, converted before the Entry's math.
pub(crate) struct Header {
    pub tex: String,
    /// The keyword's value in the file.
    pub at: Range<usize>,
    /// `tex` is the value as written, so an error in it can be pointed at
    /// exactly; otherwise it was rewritten, as `\DeclareMathOperator` is.
    pub verbatim: bool,
}

/// A converter for one Entry, with `macros` defined, each a name without
/// its backslash and a definition.
pub(crate) fn converter(macros: &[(String, String)]) -> Result<LatexToMathML, BadMacro> {
    let config = MathCoreConfig {
        // A `\newcommand` stays defined for the rest of the Entry (#17).
        global_group: true,
        ..Default::default()
    };
    LatexToMathML::new(config, macros.to_vec())
        .map_err(|(error, index, _)| BadMacro { index, error })
}

/// A macro math-core can't read.
#[derive(Debug)]
pub(crate) struct BadMacro {
    /// Its index in the macros.
    pub index: usize,
    pub error: Box<LatexError>,
}

impl fmt::Display for BadMacro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&message(&self.error))
    }
}

impl std::error::Error for BadMacro {}

/// math-core's message as a Diagnostic's: `Unknown command "\x".` reads
/// `unknown command "\x"`.
pub(crate) fn message(e: &LatexError) -> String {
    let m = e.error_message();
    let m = m.strip_suffix('.').unwrap_or(&m);
    let mut chars = m.chars();
    chars
        .next()
        .map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
}

/// What to use instead of an environment yuanme doesn't render.
pub(crate) fn environment_help(name: &str) -> String {
    let name = name.trim_end_matches('*');
    match name {
        "eqnarray" => "use `align`, with `&=` where `eqnarray` has `&=&`".into(),
        "split" => "use `aligned` inside `equation`, which numbers it as one equation".into(),
        "alignat" | "flalign" => "use `align`".into(),
        "center" => "in Org, use `#+begin_center`".into(),
        "theorem" | "lemma" | "proposition" | "corollary" | "conjecture" | "definition"
        | "example" | "exercise" | "remark" | "note" | "proof" => format!(
            "write a Theorem block: `:::{name}` in Markdown, `#+begin_{name}` in Org"
        ),
        _ => "yuanme renders `equation`, `align`, `gather` and `multline`, starred or not, and inside them `aligned`, `gathered`, `cases`, the `matrix` family and `array`".into(),
    }
}

/// One Entry's math: the Entry's file, as `text` and `file`, which node
/// spans are in, the Site's `macros`, and an Org file's `headers`.
pub(crate) struct EntryMath<'a> {
    pub(crate) text: &'a str,
    pub(crate) file: FileId,
    pub(crate) macros: &'a [(String, String)],
    pub(crate) headers: &'a [Header],
}

/// What converting an Entry's math found.
struct Converted {
    /// Each `\label`, where it is, and its equation's number, if it has one.
    equations: Vec<(String, Range<usize>, Option<String>)>,
    /// Each reference inside math, such as `\eqref`.
    references: Vec<(String, Range<usize>)>,
    /// The Labels written in math that didn't convert.
    unconverted: BTreeSet<String>,
}

impl EntryMath<'_> {
    /// Converts every Math node of `doc` and fills in its MathML, numbers
    /// Org's `\ref`s, and checks the Entry's Labels. Returns every Label in
    /// the Entry, which a generated heading id must not take.
    pub(crate) fn render(&self, doc: &mut Document, out: &mut Vec<Diagnostic>) -> BTreeSet<String> {
        let mut nodes: Vec<NodeId> = doc
            .descendants(Document::ROOT)
            .into_iter()
            .filter(|&id| matches!(doc.node(id).kind, Kind::Math { .. } | Kind::Ref { .. }))
            .collect();
        // Source order, which is LaTeX's: Markdown keeps footnote definitions
        // at the end of the tree.
        nodes.sort_by_key(|&id| doc.node(id).span.start);
        let Converted {
            equations,
            references,
            unconverted,
        } = self.convert(doc, &nodes, out);
        let names: Vec<(String, Range<usize>)> = equations
            .iter()
            .map(|(n, at, _)| (n.clone(), at.clone()))
            .collect();
        let labels = label::check(doc, self.text, self.file, &names, out);
        let mut check = References {
            math: self,
            numbers: number(doc, &equations),
            unconverted: &unconverted,
            labels: &labels,
            near: BTreeMap::new(),
        };
        for (name, at) in references {
            check.reference(&name, at, out);
        }
        for id in nodes {
            let node = doc.node(id);
            let Kind::Ref { label, .. } = &node.kind else {
                continue;
            };
            let at = node.span.clone();
            let found = check.reference(&label.clone(), at, out);
            if let (Some(tag), Kind::Ref { number, .. }) = (found, doc.kind_mut(id)) {
                *number = tag;
            }
        }
        labels
    }

    fn span(&self, range: Range<usize>) -> Span {
        Span {
            file: self.file,
            range,
        }
    }

    /// Converts the headers, then each Math node in `nodes`, in one
    /// `convert_all`, filling in each node's MathML, with an error for each
    /// snippet that doesn't convert.
    fn convert(
        &self,
        doc: &mut Document,
        nodes: &[NodeId],
        out: &mut Vec<Diagnostic>,
    ) -> Converted {
        let maths: Vec<NodeId> = nodes
            .iter()
            .copied()
            .filter(|&id| matches!(doc.node(id).kind, Kind::Math { .. }))
            .collect();
        // Where each snippet's first byte is in the file, if it is there as
        // written, and the span to fall back on if not.
        let mut snippets: Vec<(String, MathDisplay)> = Vec::new();
        let mut places: Vec<(Option<usize>, Range<usize>)> = Vec::new();
        for h in self.headers {
            snippets.push((h.tex.clone(), MathDisplay::Inline));
            places.push((h.verbatim.then_some(h.at.start), h.at.clone()));
        }
        for &id in &maths {
            let node = doc.node(id);
            let Kind::Math { display, tex, .. } = &node.kind else {
                unreachable!("`maths` holds only Math nodes")
            };
            let display = if *display {
                MathDisplay::Block
            } else {
                MathDisplay::Inline
            };
            snippets.push((tex.clone(), display));
            let start = self
                .text
                .get(node.span.clone())
                .and_then(|s| s.find(tex.as_str()))
                .map(|i| node.span.start + i);
            places.push((start, node.span.clone()));
        }
        let results = if snippets.is_empty() {
            Vec::new()
        } else {
            let converter = converter(self.macros).expect("Site config checked its macros");
            converter.convert_all(&snippets)
        };

        let at = |i: usize, range: Range<usize>| match &places[i] {
            (Some(start), _) => start + range.start..start + range.end,
            (None, whole) => whole.clone(),
        };
        let mut converted = Converted {
            equations: Vec::new(),
            references: Vec::new(),
            unconverted: BTreeSet::new(),
        };
        for (i, result) in results.into_iter().enumerate() {
            let node = i.checked_sub(self.headers.len()).map(|n| maths[n]);
            match result {
                Ok(r) => {
                    for l in r.labels {
                        converted.equations.push((l.name, at(i, l.span), l.tag));
                    }
                    for e in r.references {
                        converted.references.push((e.name, at(i, e.span)));
                    }
                    if let Some(id) = node
                        && let Kind::Math { mathml, .. } = doc.kind_mut(id)
                    {
                        *mathml = r.mathml;
                    }
                }
                Err(e) => {
                    let snippet = &snippets[i].0;
                    // Its Labels are unknown, but an `\eqref` to one is not
                    // another mistake to report.
                    converted.unconverted.extend(written_labels(snippet));
                    let mut d = Diagnostic::error(
                        &codes::MATH_UNSUPPORTED,
                        message(&e),
                        self.span(at(i, e.0.clone())),
                    )
                    .label(e.label());
                    if let Some(help) = help(&e, snippet) {
                        d = d.help(help);
                    }
                    out.push(d);
                }
            }
        }
        converted
    }
}

/// Each equation Label's number, if it has one. The first equation with a
/// Label is the one it names; a second is already an error. A numbered
/// equation's Label becomes an id of `doc`, which links find as they find
/// a `#+NAME`.
fn number<'e>(
    doc: &mut Document,
    equations: &'e [(String, Range<usize>, Option<String>)],
) -> BTreeMap<&'e str, Option<&'e str>> {
    let mut numbers: BTreeMap<&str, Option<&str>> = BTreeMap::new();
    for (name, _, tag) in equations {
        numbers.entry(name).or_insert(tag.as_deref());
        if let Some(tag) = tag {
            doc.names
                .targets
                .entry(name.clone())
                .or_insert_with(|| name.clone());
            doc.names
                .numbers
                .entry(name.clone())
                .or_insert_with(|| format!("({tag})"));
        }
    }
    numbers
}

/// Checks each reference to an equation's Label in one Entry.
struct References<'a> {
    math: &'a EntryMath<'a>,
    numbers: BTreeMap<&'a str, Option<&'a str>>,
    unconverted: &'a BTreeSet<String>,
    /// Every Label in the Entry.
    labels: &'a BTreeSet<String>,
    /// Each unknown name is compared with every Label, so only the first
    /// few are, which keeps an Entry's parse linear in its length (#15).
    near: BTreeMap<String, Option<String>>,
}

impl References<'_> {
    /// The number of the equation `name` labels, or else an error at `at`,
    /// unless its Label was in math that didn't convert.
    fn reference(
        &mut self,
        name: &str,
        at: Range<usize>,
        out: &mut Vec<Diagnostic>,
    ) -> Option<String> {
        // The Label itself, so a fix replaces only it.
        let at = label_at(self.math.text, at, name);
        let error = |message: String| {
            Diagnostic::error(
                &codes::MATH_UNKNOWN_LABEL,
                message,
                self.math.span(at.clone()),
            )
            .label("referenced here")
        };
        match self.numbers.get(name) {
            Some(Some(tag)) => return Some(tag.to_string()),
            Some(None) => {
                let message = format!("the equation `{name}` labels has no number");
                out.push(
                    error(message)
                        .help("`equation`, `align`, `gather` and `multline` number their lines; their starred forms, `$$…$$`, `\\[…\\]` and lines with `\\notag` don't"),
                );
            }
            None if self.unconverted.contains(name) => {}
            None if self.labels.contains(name) => {
                let message = format!("`{name}` labels something other than an equation");
                out.push(error(message).help(format!(
                    "link to it instead: a link with no text shows a Theorem block's number, as `[](#{name})` in Markdown and `[[{name}]]` in Org"
                )));
            }
            None => {
                let mut d = error(format!("no equation in this Entry has the Label `{name}`"));
                if let Some(near) = self.near(name) {
                    let edit = Edit {
                        span: self.math.span(at.clone()),
                        replacement: near.to_string(),
                    };
                    d = d.suggest(
                        format!("a Label with a similar name exists: `{near}`"),
                        Applicability::MaybeIncorrect,
                        vec![edit],
                    );
                }
                out.push(d);
            }
        }
        None
    }

    /// A numbered Label with a name like `name`, for the first few names.
    fn near(&mut self, name: &str) -> Option<String> {
        if self.near.len() >= 32 && !self.near.contains_key(name) {
            return None;
        }
        let numbers = &self.numbers;
        self.near
            .entry(name.to_string())
            .or_insert_with(|| {
                let known = numbers.iter().filter(|(_, t)| t.is_some());
                similar(name, known.map(|(n, _)| *n)).map(str::to_string)
            })
            .clone()
    }
}

/// The Labels `tex` gives with `\label{…}`, read without converting it.
fn written_labels(tex: &str) -> impl Iterator<Item = String> + '_ {
    tex.split("\\label{")
        .skip(1)
        .filter_map(|rest| Some(rest[..rest.find('}')?].trim().to_string()))
}

/// Where `name` is written inside a reference at `at`, such as
/// `\eqref{name}`, or all of `at` if it isn't there as written.
fn label_at(text: &str, at: Range<usize>, name: &str) -> Range<usize> {
    match text.get(at.clone()).and_then(|s| s.rfind(name)) {
        Some(i) => at.start + i..at.start + i + name.len(),
        None => at,
    }
}

/// What to do instead of what `snippet` has at the error.
fn help(e: &LatexError, snippet: &str) -> Option<String> {
    let at = snippet.get(e.0.clone()).unwrap_or("");
    let label = e.label();
    if label == "unknown environment" {
        let name = at.trim_matches(|c| c == '{' || c == '}');
        return Some(environment_help(name));
    }
    if label == "unknown command" {
        return Some(match at {
            "\\DeclareMathOperator" | "\\DeclareMathOperator*" => {
                "declare it in `[math.macros]` in yuanme.toml, as `name = '\\operatorname{name}'`, or in Org with `#+LATEX_HEADER: \\DeclareMathOperator…`".into()
            }
            "\\ref" => "inside math, use `\\eqref`, which gives `(n)`".into(),
            _ => format!(
                "define `{at}` in `[math.macros]` in yuanme.toml, or with `\\newcommand` in the Entry"
            ),
        });
    }
    if label == "already defined" {
        return Some(format!(
            "yuanme's math already has `{at}`; use `\\renewcommand` to change what it means"
        ));
    }
    if at.starts_with("\\label") {
        return Some("only `equation`, `align`, `gather` and `multline` are numbered, and so take a `\\label`; `$$…$$` and `\\[…\\]` aren't".into());
    }
    None
}

/// The characters a browser draws for MathML (#29): its text, with
/// math-core's escapes read back; for a one-character `<mi>` with no
/// `mathvariant`, the mathematical italic MathML Core's `math-auto` draws
/// instead; and U+221A for a root, which no text names. A port of
/// math-core's `extract_math_chars.py`.
pub(crate) fn drawn_chars(mathml: &str, out: &mut BTreeSet<char>) {
    let mut rest = mathml;
    // The text of the `<mi>` being read, when `math-auto` applies to it.
    let mut mi: Option<String> = None;
    while let Some(c) = rest.chars().next() {
        match c {
            '<' => {
                let end = rest.find('>').unwrap_or(rest.len() - 1);
                let tag = &rest[1..end];
                if tag == "mi" {
                    mi = Some(String::new());
                } else if tag == "/mi"
                    && let Some(text) = mi.take()
                    && let [c] = text.chars().collect::<Vec<_>>()[..]
                {
                    out.extend(italic(c));
                } else if ["msqrt", "mroot"].contains(&tag.split(' ').next().unwrap_or(tag)) {
                    out.insert('√');
                }
                rest = &rest[end + 1..];
            }
            '&' => {
                let end = rest.find(';').unwrap_or(rest.len() - 1);
                let decoded = match &rest[1..end] {
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "amp" => Some('&'),
                    "quot" => Some('"'),
                    e => e
                        .strip_prefix("#x")
                        .and_then(|h| u32::from_str_radix(h, 16).ok())
                        .or_else(|| e.strip_prefix('#').and_then(|d| d.parse().ok()))
                        .and_then(char::from_u32),
                };
                out.extend(decoded);
                mi.iter_mut().for_each(|t| t.extend(decoded));
                rest = &rest[end + 1..];
            }
            c => {
                out.insert(c);
                mi.iter_mut().for_each(|t| t.push(c));
                rest = &rest[c.len_utf8()..];
            }
        }
    }
}

/// The mathematical italic `math-auto` draws for `c`: MathML Core's
/// appendix C.1, 112 pairs.
fn italic(c: char) -> Option<char> {
    let offset = |from: char, to: u32| char::from_u32(to + (c as u32 - from as u32));
    match c {
        // Italic h was encoded first, as the Planck constant.
        'h' => Some('\u{210E}'),
        'A'..='Z' => offset('A', 0x1D434),
        'a'..='z' => offset('a', 0x1D44E),
        // U+03A2 is unassigned, and U+1D6F3 in its place is ϴ's italic.
        '\u{391}'..='\u{3A9}' if c != '\u{3A2}' => offset('\u{391}', 0x1D6E2),
        '\u{3B1}'..='\u{3C9}' => offset('\u{3B1}', 0x1D6FC),
        'ϴ' => Some('\u{1D6F3}'),
        '∇' => Some('\u{1D6FB}'),
        '∂' => Some('\u{1D715}'),
        'ϵ' => Some('\u{1D716}'),
        'ϑ' => Some('\u{1D717}'),
        'ϰ' => Some('\u{1D718}'),
        'ϕ' => Some('\u{1D719}'),
        'ϱ' => Some('\u{1D71A}'),
        'ϖ' => Some('\u{1D71B}'),
        'ı' => Some('\u{1D6A4}'),
        'ȷ' => Some('\u{1D6A5}'),
        _ => None,
    }
}
