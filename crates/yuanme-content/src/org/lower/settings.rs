//! In-buffer settings: keywords, `#+OPTIONS`, tags and `#+LATEX_HEADER`.

use yuanme_diag::{Diagnostic, codes};
use yuanme_org::types::{AstNode, Document as OrgDocument};

use super::{Lower, Scripts, offset, preamble_end};
use crate::math;
use crate::org::Keyword;

impl<'s> Lower<'s> {
    /// Every keyword, for the Entry's fields, and the in-buffer settings
    /// that change how the body reads. Org applies a setting wherever it
    /// is written.
    pub(super) fn keywords(&mut self, parsed: &OrgDocument<'s>) -> Vec<Keyword> {
        let end = preamble_end(self.text, parsed);
        let mut out = Vec::new();
        let mut exclude = Vec::new();
        let mut select = Vec::new();
        for node in AstNode::from(parsed).iter_all_ast_nodes() {
            let AstNode::Keyword(k) = node else { continue };
            let key = k.key.to_ascii_uppercase();
            let start = offset(self.text, k.source);
            match key.as_str() {
                "SETUPFILE" => {
                    let d = Diagnostic::error(
                        &codes::ORG_SETUPFILE,
                        "`#+SETUPFILE` isn't read: an Entry is one file".to_string(),
                        self.span(self.at(k.source)),
                    )
                    .help("put shared settings where yuanme reads them: `#+OPTIONS` in each file, LaTeX macros in `[math]` in yuanme.toml, and styles, scripts and analytics in Templates");
                    self.diagnostics.push(d);
                    continue;
                }
                "INCLUDE" => {
                    self.unsupported(
                        "`#+INCLUDE` keywords",
                        k.source,
                        "copy the text in, or link to the other file",
                    );
                    continue;
                }
                "MACRO" => {
                    self.unsupported(
                        "macros",
                        k.source,
                        "write the text the macro would expand to",
                    );
                    continue;
                }
                // Raw HTML is written where it stands; the other back
                // ends' are dropped, as ox-html drops them (ADR 0009).
                "HTML" | "LATEX" | "BEAMER" | "ASCII" | "MAN" | "MD" | "ODT" | "TEXINFO" => {
                    continue;
                }
                "OPTIONS" => {
                    for token in k.value.split_whitespace() {
                        match token {
                            "^:t" => self.scripts = Scripts::All,
                            "^:nil" => self.scripts = Scripts::None,
                            "^:{}" => self.scripts = Scripts::Braces,
                            _ => {}
                        }
                    }
                }
                "EXCLUDE_TAGS" => exclude.extend(k.value.split_whitespace().map(String::from)),
                "LATEX_HEADER" => self.latex_header(k.value),
                "SELECT_TAGS" => select.extend(k.value.split_whitespace().map(String::from)),
                _ => {}
            }
            let key_start = offset(self.text, k.key);
            // An empty value may be no slice of the file.
            let value_start = if k.value.is_empty() {
                key_start + k.key.len()
            } else {
                offset(self.text, k.value)
            };
            out.push(Keyword {
                key: k.key.to_string(),
                key_at: key_start..key_start + k.key.len(),
                value: k.value.to_string(),
                value_at: value_start..value_start + k.value.len(),
                preamble: start < end,
            });
        }
        if !exclude.is_empty() {
            self.exclude = exclude;
        }
        if !select.is_empty() {
            self.select = select;
        }
        out
    }

    /// A `#+LATEX_HEADER` line (#17): one that defines macros defines them
    /// for the Entry's math, a `\usepackage` of what yuanme's math has
    /// anyway is accepted, and any other has no effect.
    fn latex_header(&mut self, value: &'s str) {
        let tex = value.trim();
        if tex.is_empty() {
            return;
        }
        let start = offset(self.text, value) + value.find(tex).unwrap_or(0);
        let at = start..start + tex.len();
        if let Some(rest) = tex.strip_prefix("\\usepackage") {
            let packages = rest
                .split_once('{')
                .and_then(|(_, p)| p.split_once('}'))
                .map_or("", |(p, _)| p);
            if !packages.is_empty() && packages.split(',').all(|p| PACKAGES.contains(&p.trim())) {
                return;
            }
        } else if let Some(defined) = declare_math_operator(tex) {
            self.headers.push(math::Header {
                tex: defined,
                at,
                verbatim: false,
            });
            return;
        } else if [
            "\\newcommand",
            "\\renewcommand",
            "\\providecommand",
            "\\def",
        ]
        .iter()
        .any(|c| tex.starts_with(c))
        {
            self.headers.push(math::Header {
                tex: tex.to_string(),
                at,
                verbatim: true,
            });
            return;
        }
        self.no_effect(
            "this `#+LATEX_HEADER` has no effect in yuanme".into(),
            at,
            "yuanme reads `\\newcommand`, `\\renewcommand`, `\\def` and `\\DeclareMathOperator` lines, which define macros for the Entry's math, and accepts `\\usepackage` of amsmath, amssymb, amsfonts, amsthm, mathtools and bm",
        );
    }
}

/// Packages whose `\usepackage` a `#+LATEX_HEADER` may have: yuanme's math
/// has what they give, or they give nothing math needs (#17).
const PACKAGES: &[&str] = &[
    "amsmath",
    "amssymb",
    "amsfonts",
    "amsthm",
    "mathtools",
    "bm",
];

/// `\DeclareMathOperator{\X}{Y}` as the macro it defines,
/// `\newcommand{\X}{\operatorname{Y}}`, or `\operatorname*` for the
/// starred form.
fn declare_math_operator(tex: &str) -> Option<String> {
    let rest = tex.strip_prefix("\\DeclareMathOperator")?;
    let (star, rest) = match rest.strip_prefix('*') {
        Some(rest) => ("*", rest),
        None => ("", rest),
    };
    let rest = rest.trim_start();
    let (name, rest) = match rest.strip_prefix('{') {
        Some(r) => r.split_once('}')?,
        None => {
            let end = 1 + rest
                .get(1..)?
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(rest.len() - 1);
            rest.split_at(end)
        }
    };
    let name = name.trim();
    if !name.starts_with('\\') {
        return None;
    }
    let body = rest.trim().strip_prefix('{')?.strip_suffix('}')?;
    Some(format!(
        "\\newcommand{{{name}}}{{\\operatorname{star}{{{body}}}}}"
    ))
}
