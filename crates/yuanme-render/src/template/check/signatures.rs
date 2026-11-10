//! Pass 1: every macro's signature and props comment, and every Layout's
//! vars comment.

use std::collections::BTreeMap;
use std::rc::Rc;

use minijinja::machinery::ast::{self, Expr, Spanned, Stmt};
use minijinja::machinery::{Token, tokenize};
use minijinja::syntax::SyntaxConfig;
use yuanme_diag::codes;

use super::Checker;
use super::headers::{Prop, headers};
use super::infer::range;

/// A macro, as its signature and props comment declare it.
pub(super) struct Sig {
    pub(super) name: String,
    /// Its Template, such as `components/row.html`.
    pub(super) file: String,
    pub(super) params: Vec<Param>,
    /// From the props comment. A macro outside `components/` may have
    /// none, and then only its arity is checked.
    pub(super) props: Option<Vec<Prop>>,
    pub(super) slot: Slot,
}

pub(super) struct Param {
    pub(super) name: String,
    pub(super) has_default: bool,
    /// `=none`, which leaves the prop absent to the macro body.
    pub(super) none_default: bool,
}

/// Whether a Component takes a `{% call %}` body, from its use of `caller`.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Slot {
    No,
    Optional,
    Required,
}

/// Whether a macro takes a `{% call %}` body: it reads `caller`, and
/// optionally if it asks `caller is defined` or tests `if caller` first.
fn slot(source: &str) -> Slot {
    let tokens: Vec<Token<'_>> = tokenize(source, false, SyntaxConfig::default())
        .map_while(Result::ok)
        .map(|(t, _)| t)
        .collect();
    let caller = |t: &Token<'_>| matches!(t, Token::Ident("caller"));
    if !tokens.iter().any(caller) {
        return Slot::No;
    }
    let guarded = tokens.windows(3).any(|w| {
        (caller(&w[1]) && matches!(w[0], Token::Ident("if")))
            || (caller(&w[0]) && matches!(w[1], Token::Ident("is")))
    });
    match guarded {
        true => Slot::Optional,
        false => Slot::Required,
    }
}

impl<'a> Checker<'a> {
    pub(super) fn signatures(&mut self) {
        for (&name, ast) in self.asts {
            let Stmt::Template(t) = ast else { continue };
            self.stack = vec![name];
            let source = self.source();
            let headers = headers(source);
            let mut paired = vec![false; headers.len()];
            for (i, h) in headers.iter().enumerate() {
                if !h.props {
                    paired[i] = true;
                    let vars = self.header(h);
                    self.vars.insert(name, vars);
                }
            }
            let mut sigs = BTreeMap::new();
            for s in &t.children {
                let Stmt::Macro(m) = s else { continue };
                let start = m.span().start_offset as usize;
                // Only the tag's opening may come between the comment and
                // `macro`.
                let above = headers.iter().position(|h| {
                    h.props
                        && h.at.end <= start
                        && source[h.at.end..start]
                            .chars()
                            .all(|c| c.is_whitespace() || "{%-+".contains(c))
                });
                let props = above.map(|i| {
                    paired[i] = true;
                    self.header(&headers[i])
                });
                let sig = self.signature(name, m, props);
                sigs.insert(m.name, Rc::new(sig));
            }
            for (h, paired) in headers.iter().zip(paired) {
                if !paired {
                    let d = self
                        .error(
                            &codes::TEMPLATE_BAD_HEADER,
                            "this props comment isn't directly above a `{% macro %}`".into(),
                            h.at.clone(),
                        )
                        .help("write it on the line before the macro it declares, with nothing between them");
                    self.push(d);
                }
            }
            self.macros.insert(name, sigs);
        }
        self.stack.clear();
    }

    fn signature(
        &mut self,
        file: &'a str,
        m: &'a Spanned<ast::Macro<'a>>,
        props: Option<Vec<Prop>>,
    ) -> Sig {
        let first_default = m.args.len() - m.defaults.len();
        let params: Vec<Param> = m
            .args
            .iter()
            .enumerate()
            .map(|(i, a)| Param {
                name: match a {
                    Expr::Var(v) => v.id.to_string(),
                    _ => String::new(),
                },
                has_default: i >= first_default,
                none_default: i >= first_default
                    && m.defaults[i - first_default]
                        .as_const()
                        .is_some_and(|v| v.is_none()),
            })
            .collect();
        let start = m.span().start_offset as usize;
        let name_at = self.find(start + "macro".len(), m.name);
        match &props {
            None if file.starts_with("components/") => {
                let d = self
                    .error(
                        &codes::TEMPLATE_BAD_HEADER,
                        format!("the Component `{}` has no props comment", m.name),
                        name_at,
                    )
                    .help("declare its props directly above it, such as `{# props: title: string, href?: url #}`");
                self.push(d);
            }
            None => {}
            Some(props) => {
                for (p, arg) in params.iter().zip(&m.args) {
                    let at = range(arg);
                    match props.iter().find(|x| x.name == p.name) {
                        // A misspelt entry gets its fix on the comment's side.
                        None => {
                            let d = self.error(
                                &codes::TEMPLATE_BAD_HEADER,
                                format!(
                                    "the props comment doesn't declare `{}`, a parameter of `{}`",
                                    p.name, m.name
                                ),
                                at,
                            );
                            self.push(d);
                        }
                        Some(x) if !x.optional && p.has_default => {
                            let d = self
                                .error(
                                    &codes::TEMPLATE_BAD_HEADER,
                                    format!(
                                        "`{}` gives the required prop `{}` a default",
                                        m.name, p.name
                                    ),
                                    at,
                                )
                                .help(format!(
                                    "mark it optional in the props comment: `{}?: {}`",
                                    p.name, x.ty
                                ));
                            self.push(d);
                        }
                        Some(_) => {}
                    }
                }
                for x in props {
                    if !params.iter().any(|p| p.name == x.name) {
                        let at = x.at.start..x.at.start + x.name.len();
                        let d = self.error(
                            &codes::TEMPLATE_BAD_HEADER,
                            format!(
                                "the props comment declares `{}`, which `{}` has no parameter for",
                                x.name, m.name
                            ),
                            at.clone(),
                        );
                        let d = Self::near(
                            d,
                            self.span(at),
                            &x.name,
                            params.iter().map(|p| p.name.as_str()),
                        );
                        self.push(d);
                    }
                }
            }
        }
        let body = &self.source()[start..m.span().end_offset as usize];
        Sig {
            name: m.name.to_string(),
            file: file.to_string(),
            params,
            props,
            slot: slot(body),
        }
    }
}
