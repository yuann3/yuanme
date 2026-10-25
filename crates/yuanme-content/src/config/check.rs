//! The first pass over Site config: every key against the schema table, so
//! the second pass reads a table whose shape is known.

use std::borrow::Cow;
use std::ops::Range;

use toml::Spanned;
use toml::de::{DeTable, DeValue};
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Level, Span, codes};

use super::schema::{FIELD_TYPES, Key, Shape, common_keys};

pub(crate) struct Checker {
    file: FileId,
    pub diagnostics: Vec<Diagnostic>,
}

impl Checker {
    pub fn new(file: FileId) -> Checker {
        Checker {
            file,
            diagnostics: Vec::new(),
        }
    }

    pub fn file(&self) -> FileId {
        self.file
    }

    pub fn failed(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == Level::Error)
    }

    pub fn span(&self, range: Range<usize>) -> Span {
        Span {
            file: self.file,
            range,
        }
    }

    pub fn error(
        &mut self,
        code: &'static yuanme_diag::Code,
        message: String,
        at: Range<usize>,
    ) -> &mut Self {
        let d = Diagnostic::error(code, message, self.span(at));
        self.diagnostics.push(d);
        self
    }

    /// Adds to the Diagnostic just reported.
    pub fn then(&mut self, add: impl FnOnce(Diagnostic) -> Diagnostic) {
        let d = self.diagnostics.pop().expect("a Diagnostic to add to");
        self.diagnostics.push(add(d));
    }

    /// An edit replacing `at` that a person or agent should confirm first.
    pub fn suggest(&mut self, message: String, at: Range<usize>, replacement: String) {
        let edit = Edit {
            span: self.span(at),
            replacement,
        };
        self.then(|d| d.suggest(message, Applicability::MaybeIncorrect, vec![edit]));
    }

    pub fn invalid(&mut self, message: String, at: Range<usize>) {
        self.error(&codes::CONFIG_INVALID_VALUE, message, at);
    }

    /// Checks `table` against `keys`. `at` is where the table is named, for a
    /// missing key; `path` is its dotted name.
    pub fn table(&mut self, keys: &[&Key], table: &mut DeTable<'_>, at: Range<usize>, path: &str) {
        let present: Vec<(String, Range<usize>)> = table
            .iter()
            .map(|(k, _)| (k.get_ref().to_string(), k.span()))
            .collect();
        // Required keys a typo already accounts for: `[sites]` is reported
        // once, as unknown, not again as `[site]` missing.
        let mut explained = Vec::new();
        for (name, key_span) in present {
            let child = join(path, &name);
            let Some(key) = keys.iter().find(|k| k.name == name) else {
                explained.extend(self.unknown_key(keys, &name, &child, path, key_span));
                continue;
            };
            let key = match key.renamed_to {
                None => key,
                Some(to) => {
                    if table.contains_key(to) {
                        self.invalid(
                            format!(
                                "`{child}` is the old name of `{}`, which is also set",
                                join(path, to)
                            ),
                            key_span,
                        );
                        continue;
                    }
                    let warning = Diagnostic::warning(
                        &codes::CONFIG_DEPRECATED,
                        format!("`{child}` has been renamed to `{}`", join(path, to)),
                        self.span(key_span.clone()),
                    )
                    .suggest(
                        format!("rename it to `{to}`"),
                        Applicability::MachineApplicable,
                        vec![Edit {
                            span: self.span(key_span.clone()),
                            replacement: to.to_string(),
                        }],
                    );
                    self.diagnostics.push(warning);
                    let (_, value) = table.remove_entry(name.as_str()).expect("listed above");
                    table.insert(
                        Spanned::new(key_span.clone(), Cow::Owned(to.to_string())),
                        value,
                    );
                    keys.iter()
                        .find(|k| k.name == to)
                        .expect("a key is renamed to a key")
                }
            };
            let name = key.name;
            let value = table.get_mut(name).expect("listed above");
            self.value(&key.shape, value, key_span, &join(path, name));
        }
        let missing = keys
            .iter()
            .filter(|k| k.required && !table.contains_key(k.name));
        for key in missing.filter(|k| !explained.contains(&k.name)) {
            let message = match path {
                "" => format!("Site config has no `[{}]`", key.name),
                _ => format!("`{path}` has no `{}`", key.name),
            };
            let help = format!("add `{}`: {}", key.name, key.doc);
            self.error(&codes::CONFIG_MISSING_KEY, message, at.clone())
                .then(|d| d.help(help));
        }
    }

    /// Returns the key it suggests instead, if any.
    fn unknown_key(
        &mut self,
        keys: &[&Key],
        name: &str,
        child: &str,
        path: &str,
        at: Range<usize>,
    ) -> Option<&'static str> {
        let message = match path {
            "" => format!("unknown key `{name}`"),
            _ => format!("unknown key `{name}` in `[{path}]`"),
        };
        self.error(&codes::CONFIG_UNKNOWN_KEY, message, at.clone())
            .then(|d| d.label("unknown key"));
        let valid = keys
            .iter()
            .filter(|k| k.renamed_to.is_none())
            .map(|k| k.name);
        let near = similar(name, valid.clone());
        match near {
            Some(near) => self.suggest(
                format!("a key with a similar name exists: `{near}`"),
                at,
                near.to_string(),
            ),
            None => {
                let list: Vec<_> = valid.map(|k| format!("`{k}`")).collect();
                let note = if list.is_empty() {
                    format!(
                        "`{}` takes no keys",
                        if path.is_empty() { child } else { path }
                    )
                } else {
                    format!("expected one of {}", list.join(", "))
                };
                self.then(|d| d.note(note));
            }
        }
        near
    }

    fn value(
        &mut self,
        shape: &Shape,
        value: &mut Spanned<DeValue<'_>>,
        key_span: Range<usize>,
        path: &str,
    ) {
        let at = value.span();
        let found = kind(value.get_ref());
        let wrong = |this: &mut Self, expected: &str| {
            let label = format!("expected {expected}");
            this.error(
                &codes::CONFIG_WRONG_TYPE,
                format!("`{path}` must be {expected}, not {found}"),
                at.clone(),
            )
            .then(|d| d.label(label));
        };
        match (shape, value.get_ref()) {
            (Shape::Any, _)
            | (Shape::Free, DeValue::Table(_))
            | (Shape::Number, DeValue::Integer(_)) => {
                self.fits_i64(value);
            }
            (Shape::String, DeValue::String(_))
            | (Shape::Bool, DeValue::Boolean(_))
            | (Shape::Number, DeValue::Float(_)) => {}
            (Shape::Url, DeValue::String(s)) => self.url(s, at, path),
            (Shape::Choice(choices), DeValue::String(s)) => {
                if !choices.contains(&&**s) {
                    let list: Vec<_> = choices.iter().map(|c| format!("\"{c}\"")).collect();
                    self.invalid(
                        format!("`{path}` must be one of {}", list.join(", ")),
                        at.clone(),
                    );
                    if let Some(near) = similar(s, choices.iter().copied()) {
                        self.suggest(format!("did you mean `{near}`?"), at, format!("\"{near}\""));
                    }
                }
            }
            (Shape::Route(placeholder), DeValue::String(s)) => {
                self.route(s, *placeholder, at, path)
            }
            (Shape::Version, DeValue::String(s)) => {
                if parse_version(s).is_none() {
                    self.invalid(
                        format!("`{path}` must be a version as \"X.Y\", such as \"1.2\""),
                        at,
                    );
                }
            }
            (Shape::BrowserVersion, DeValue::String(s)) => {
                if parse_browser_version(s).is_none() {
                    self.invalid(
                        format!("`{path}` must be a browser version, such as \"111\" or \"16.4\""),
                        at,
                    );
                }
            }
            (Shape::Int { min, max }, DeValue::Integer(i)) => {
                match i64::from_str_radix(i.as_str(), i.radix()) {
                    Ok(n) if n < *min || n > *max => {
                        let message = match *max {
                            i64::MAX => format!("`{path}` must be at least {min}, not {n}"),
                            max => format!("`{path}` must be from {min} to {max}, not {n}"),
                        };
                        self.error(&codes::CONFIG_OUT_OF_RANGE, message, at);
                    }
                    Ok(_) => {}
                    Err(_) => self.too_large(path, at),
                }
            }
            (Shape::Strings | Shape::Array(_) | Shape::Tables(_), DeValue::Array(_))
            | (Shape::Table(_) | Shape::Map(_) | Shape::Field { .. }, DeValue::Table(_)) => {
                self.nested(shape, value.get_mut(), key_span, path);
            }
            (
                Shape::String
                | Shape::Choice(_)
                | Shape::Url
                | Shape::Route(_)
                | Shape::Version
                | Shape::BrowserVersion,
                _,
            ) => wrong(self, "a string"),
            (Shape::Int { .. }, _) => wrong(self, "an integer"),
            (Shape::Number, _) => wrong(self, "a number"),
            (Shape::Bool, _) => wrong(self, "a boolean"),
            (Shape::Strings, _) => wrong(self, "an array of strings"),
            (Shape::Array(_), _) => wrong(self, "an array"),
            (Shape::Tables(_), _) => wrong(self, "an array of tables"),
            (Shape::Table(_) | Shape::Map(_) | Shape::Free | Shape::Field { .. }, _) => {
                wrong(self, "a table")
            }
        }
    }

    fn nested(
        &mut self,
        shape: &Shape,
        value: &mut DeValue<'_>,
        key_span: Range<usize>,
        path: &str,
    ) {
        match (shape, value) {
            (Shape::Strings, DeValue::Array(items)) => {
                for item in items.iter_mut() {
                    self.value(&Shape::String, item, key_span.clone(), &format!("{path}[]"));
                }
            }
            (Shape::Array(shape), DeValue::Array(items)) => {
                for item in items.iter_mut() {
                    self.value(shape, item, key_span.clone(), &format!("{path}[]"));
                }
            }
            (Shape::Table(keys), DeValue::Table(table)) => {
                let keys: Vec<&Key> = keys.iter().collect();
                self.table(&keys, table, key_span, path);
            }
            (Shape::Tables(keys), DeValue::Array(items)) => {
                for item in items.iter_mut() {
                    self.value(
                        &Shape::Table(keys),
                        item,
                        key_span.clone(),
                        &format!("{path}[]"),
                    );
                }
            }
            (Shape::Map(shape), DeValue::Table(table)) => {
                for (name, value) in table.iter_mut() {
                    let path = join(path, name.get_ref());
                    self.value(shape, value, name.span(), &path);
                }
            }
            (Shape::Field { in_table }, DeValue::Table(table)) => {
                self.field(*in_table, table, key_span, path)
            }
            _ => unreachable!("value() matched the shape"),
        }
    }

    /// A field's keys depend on its type, so the type is read first.
    fn field(&mut self, in_table: bool, table: &mut DeTable<'_>, at: Range<usize>, path: &str) {
        let Some(ty) = table.get("type") else {
            let help = format!("add `type`, one of {}", type_list());
            self.error(
                &codes::CONFIG_MISSING_KEY,
                format!("field `{path}` has no `type`"),
                at,
            )
            .then(|d| d.help(help));
            return;
        };
        let ty_span = ty.span();
        let DeValue::String(name) = ty.get_ref() else {
            let message = format!("`{path}.type` must be a string, not {}", kind(ty.get_ref()));
            self.error(&codes::CONFIG_WRONG_TYPE, message, ty_span);
            return;
        };
        let Some(ty) = FIELD_TYPES.iter().find(|t| t.name == name) else {
            let message = format!("`{name}` is not a field type");
            self.invalid(message, ty_span.clone());
            match similar(name, FIELD_TYPES.iter().map(|t| t.name)) {
                Some(near) => self.suggest(
                    format!("did you mean `{near}`?"),
                    ty_span,
                    format!("\"{near}\""),
                ),
                None => self.then(|d| d.note(format!("the field types are {}", type_list()))),
            }
            return;
        };
        let keys: Vec<&Key> = common_keys(in_table).iter().chain(ty.keys).collect();
        self.table(&keys, table, at, path);
    }

    fn url(&mut self, s: &str, at: Range<usize>, path: &str) {
        let rest = s
            .strip_prefix("https://")
            .or_else(|| s.strip_prefix("http://"));
        let host = rest.map(|r| r.split(['/', '?', '#']).next().unwrap_or(""));
        if host.is_some_and(|h| !h.is_empty()) && !s.contains(char::is_whitespace) {
            return;
        }
        self.invalid(
            format!("`{path}` must be an absolute URL starting with `https://`"),
            at.clone(),
        );
        if rest.is_none() && !s.is_empty() && !s.contains(char::is_whitespace) && !s.contains("://")
        {
            self.suggest("add the scheme".into(), at, format!("\"https://{s}\""));
        }
    }

    fn route(&mut self, s: &str, placeholder: Option<&str>, at: Range<usize>, path: &str) {
        if !s.starts_with('/') {
            self.invalid(format!("`{path}` must start with `/`"), at.clone());
            self.suggest(
                "start it from the Site root".into(),
                at,
                format!("\"/{s}\""),
            );
            return;
        }
        let found: Vec<&str> = s
            .match_indices('{')
            .map(|(i, _)| &s[i..s[i..].find('}').map_or(s.len(), |j| i + j + 1)])
            .collect();
        if let Some(other) = found.iter().find(|p| Some(**p) != placeholder) {
            let message = match placeholder {
                Some(p) => format!("`{other}` is not a placeholder in `{path}`; only `{p}` is"),
                None => format!("`{path}` takes no placeholders, but has `{other}`"),
            };
            self.invalid(message, at);
        } else if let Some(p) = placeholder
            && found.len() != 1
        {
            let message =
                format!("`{path}` must contain `{p}` once, where each Entry's or Term's Slug goes");
            self.invalid(message, at);
        }
    }

    fn too_large(&mut self, path: &str, at: Range<usize>) {
        let message = format!("`{path}` is too large: integers are 64-bit");
        self.error(&codes::CONFIG_OUT_OF_RANGE, message, at);
    }

    /// TOML integers are 64-bit, but the parser keeps their digits as text,
    /// so a value read later as a whole is checked here.
    fn fits_i64(&mut self, value: &Spanned<DeValue<'_>>) {
        match value.get_ref() {
            DeValue::Integer(i) if i64::from_str_radix(i.as_str(), i.radix()).is_err() => {
                self.too_large("this integer", value.span());
            }
            DeValue::Array(items) => items.iter().for_each(|v| self.fits_i64(v)),
            DeValue::Table(table) => table.iter().for_each(|(_, v)| self.fits_i64(v)),
            _ => {}
        }
    }
}

pub(crate) fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{path}.{name}")
    }
}

pub(crate) fn kind(value: &DeValue<'_>) -> &'static str {
    match value {
        DeValue::String(_) => "a string",
        DeValue::Integer(_) => "an integer",
        DeValue::Float(_) => "a float",
        DeValue::Boolean(_) => "a boolean",
        DeValue::Datetime(_) => "a date or time",
        DeValue::Array(_) => "an array",
        DeValue::Table(_) => "a table",
    }
}

fn type_list() -> String {
    let names: Vec<_> = FIELD_TYPES
        .iter()
        .map(|t| format!("`{}`", t.name))
        .collect();
    names.join(", ")
}

/// `"X.Y"`, as `yuanme = ` holds it.
pub(crate) fn parse_version(s: &str) -> Option<(u64, u64)> {
    let (major, minor) = s.split_once('.')?;
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if !(digits(major) && digits(minor)) {
        return None;
    }
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// `"111"`, `"16.4"` or `"16.4.1"`. A string, not a TOML number, because
/// `16.10` would read as `16.1`. The parts' limits are lightningcss's, which
/// packs a version into one `u32`.
pub(crate) fn parse_browser_version(s: &str) -> Option<[u32; 3]> {
    let mut version = [0; 3];
    for (i, part) in s.split('.').enumerate() {
        if i == 3 || part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let n: u32 = part.parse().ok()?;
        if n > if i == 0 { 0xffff } else { 0xff } {
            return None;
        }
        version[i] = n;
    }
    Some(version)
}

/// The candidate a typo most likely meant, as rustc picks one: the nearest
/// by edit distance, within a third of the name's length.
pub fn similar<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|c| (strsim::damerau_levenshtein(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}
