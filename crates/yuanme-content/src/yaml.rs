//! YAML 1.2 read into a tree that keeps the byte span of every key and
//! value, which Diagnostics and their fixes need (#18). JSON is YAML 1.2, so
//! JSON Data files come through here too.
//!
//! saphyr-parser does the syntax; this applies the 1.2 core schema, so `no`,
//! `on` and `yes` stay strings, and rejects duplicate keys.

use std::collections::HashMap;
use std::ops::Range;

use saphyr_parser::{Event, Parser, ScalarStyle};

#[derive(Clone, Debug, PartialEq)]
pub enum Yaml {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Seq(Vec<Node>),
    Map(Vec<(Key, Node)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub value: Yaml,
    /// Bytes in the file, quotes included.
    pub span: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub name: String,
    pub span: Range<usize>,
}

#[derive(Debug, PartialEq)]
pub enum Error {
    Syntax {
        message: String,
        at: usize,
    },
    DuplicateKey {
        name: String,
        first: Range<usize>,
        again: Range<usize>,
    },
    KeyNotString {
        at: Range<usize>,
    },
    Tag {
        at: Range<usize>,
    },
}

impl Node {
    pub fn kind(&self) -> &'static str {
        match self.value {
            Yaml::Null => "empty",
            Yaml::Bool(_) => "a boolean",
            Yaml::Int(_) => "an integer",
            Yaml::Float(_) => "a number",
            Yaml::Str(_) => "a string",
            Yaml::Seq(_) => "a list",
            Yaml::Map(_) => "a mapping",
        }
    }
}

/// Reads one YAML document. `text` starts at byte `base` of its file, so
/// every span is a span in the file.
pub fn parse(text: &str, base: usize) -> Result<Node, Error> {
    // saphyr counts characters, not bytes.
    let bytes: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .map(|i| base + i)
        .collect();
    let at = |chars: usize| bytes[chars.min(bytes.len() - 1)];
    let mut anchors: HashMap<usize, Node> = HashMap::new();
    // Each open collection: its start, its items, and for a mapping the key
    // waiting for its value.
    enum Open {
        Seq(usize, usize, Vec<Node>),
        Map(usize, usize, Vec<(Key, Node)>, Option<Node>),
    }
    let mut stack: Vec<Open> = Vec::new();
    let mut root = None;
    for event in Parser::new_from_str(text) {
        let (event, span) = event.map_err(|e| Error::Syntax {
            message: e.info().to_string(),
            at: at(e.marker().index()),
        })?;
        let range = at(span.start.index())..at(span.end.index());
        // An anchored node is recorded once it is complete.
        let (node, anchor) = match event {
            Event::Scalar(value, style, anchor, tag) => {
                if let Some(tag) = &tag
                    && !(tag.handle == "tag:yaml.org,2002:" && tag.suffix == "str")
                {
                    return Err(Error::Tag { at: range });
                }
                let value = if style == ScalarStyle::Plain && tag.is_none() {
                    resolve(&value)
                } else {
                    Yaml::Str(value.into_owned())
                };
                (Node { value, span: range }, anchor)
            }
            Event::Alias(id) => {
                let target = anchors.get(&id).ok_or_else(|| Error::Syntax {
                    message: "unknown anchor".into(),
                    at: range.start,
                })?;
                let node = Node {
                    value: target.value.clone(),
                    span: range,
                };
                (node, 0)
            }
            Event::SequenceStart(_, Some(_)) | Event::MappingStart(_, Some(_)) => {
                return Err(Error::Tag { at: range });
            }
            Event::SequenceStart(anchor, _) => {
                stack.push(Open::Seq(range.start, anchor, Vec::new()));
                continue;
            }
            Event::MappingStart(anchor, _) => {
                stack.push(Open::Map(range.start, anchor, Vec::new(), None));
                continue;
            }
            Event::SequenceEnd => {
                let Some(Open::Seq(start, anchor, items)) = stack.pop() else {
                    unreachable!("balanced")
                };
                let node = Node {
                    value: Yaml::Seq(items),
                    span: start..range.end,
                };
                (node, anchor)
            }
            Event::MappingEnd => {
                let Some(Open::Map(start, anchor, entries, _)) = stack.pop() else {
                    unreachable!("balanced")
                };
                let end = entries
                    .last()
                    .map_or(range.end, |(_, v)| v.span.end.max(range.end));
                let node = Node {
                    value: Yaml::Map(entries),
                    span: start..end,
                };
                (node, anchor)
            }
            _ => continue,
        };
        if anchor > 0 {
            anchors.insert(anchor, node.clone());
        }
        match stack.last_mut() {
            None => root = Some(node),
            Some(Open::Seq(_, _, items)) => items.push(node),
            Some(Open::Map(_, _, entries, pending)) => match pending.take() {
                None => *pending = Some(node),
                Some(key) => {
                    let Yaml::Str(name) = key.value else {
                        return Err(Error::KeyNotString { at: key.span });
                    };
                    if let Some((first, _)) = entries.iter().find(|(k, _)| k.name == name) {
                        return Err(Error::DuplicateKey {
                            name,
                            first: first.span.clone(),
                            again: key.span,
                        });
                    }
                    entries.push((
                        Key {
                            name,
                            span: key.span,
                        },
                        node,
                    ));
                }
            },
        }
    }
    Ok(root.unwrap_or(Node {
        value: Yaml::Null,
        span: base..base,
    }))
}

/// The YAML 1.2 core schema for a plain scalar.
fn resolve(s: &str) -> Yaml {
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Yaml::Null,
        "true" | "True" | "TRUE" => return Yaml::Bool(true),
        "false" | "False" | "FALSE" => return Yaml::Bool(false),
        ".nan" | ".NaN" | ".NAN" => return Yaml::Float(f64::NAN),
        _ => {}
    }
    let (sign, digits) = match s.as_bytes().first() {
        Some(b'-') => (-1.0, &s[1..]),
        Some(b'+') => (1.0, &s[1..]),
        _ => (1.0, s),
    };
    if matches!(digits, ".inf" | ".Inf" | ".INF") {
        return Yaml::Float(sign * f64::INFINITY);
    }
    let all = |s: &str, ok: fn(&u8) -> bool| !s.is_empty() && s.as_bytes().iter().all(ok);
    // An integer too large for 64 bits stays text rather than becoming a
    // rounded float.
    if all(digits, u8::is_ascii_digit) {
        return s.parse().map_or_else(|_| Yaml::Str(s.into()), Yaml::Int);
    }
    if let Some(oct) = s
        .strip_prefix("0o")
        .filter(|o| all(o, |b| (b'0'..=b'7').contains(b)))
    {
        return i64::from_str_radix(oct, 8).map_or_else(|_| Yaml::Str(s.into()), Yaml::Int);
    }
    if let Some(hex) = s
        .strip_prefix("0x")
        .filter(|h| all(h, u8::is_ascii_hexdigit))
    {
        return i64::from_str_radix(hex, 16).map_or_else(|_| Yaml::Str(s.into()), Yaml::Int);
    }
    if is_float(digits) {
        return Yaml::Float(s.parse().expect("a 1.2 float parses"));
    }
    Yaml::Str(s.into())
}

/// `(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?`
fn is_float(s: &str) -> bool {
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some(("", frac)) => !frac.is_empty() && digits(frac),
        Some((int, frac)) => !int.is_empty() && digits(int) && digits(frac),
        None => !mantissa.is_empty() && digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['-', '+']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exponent_ok && (mantissa.contains('.') || exponent.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(s: &str) -> Yaml {
        let Yaml::Map(entries) = parse(&format!("k: {s}\n"), 0).unwrap().value else {
            panic!()
        };
        entries.into_iter().next().unwrap().1.value
    }

    #[test]
    fn plain_scalars_follow_the_1_2_core_schema() {
        // YAML 1.1 read these as booleans; 1.2 reads them as text.
        for s in ["no", "yes", "on", "off", "y", "N"] {
            assert_eq!(scalar(s), Yaml::Str(s.into()), "{s}");
        }
        assert_eq!(scalar("True"), Yaml::Bool(true));
        assert_eq!(scalar("~"), Yaml::Null);
        assert_eq!(scalar(""), Yaml::Null);
        assert_eq!(scalar("0x1F"), Yaml::Int(31));
        assert_eq!(scalar("0o17"), Yaml::Int(15));
        assert_eq!(scalar("-12"), Yaml::Int(-12));
        assert_eq!(scalar("1e3"), Yaml::Float(1000.0));
        assert_eq!(scalar(".5"), Yaml::Float(0.5));
        assert_eq!(scalar("-.inf"), Yaml::Float(f64::NEG_INFINITY));
        // 1.1-only forms stay text: underscores, sexagesimals, dates.
        assert_eq!(scalar("1_000"), Yaml::Str("1_000".into()));
        assert_eq!(scalar("1:30"), Yaml::Str("1:30".into()));
        assert_eq!(scalar("2025-12-12"), Yaml::Str("2025-12-12".into()));
        // Too large for 64 bits: text, not a rounded float.
        assert_eq!(
            scalar("99999999999999999999"),
            Yaml::Str("99999999999999999999".into())
        );
        // Quoted and tagged scalars are always text.
        assert_eq!(scalar("\"12\""), Yaml::Str("12".into()));
        assert_eq!(scalar("!!str 12"), Yaml::Str("12".into()));
    }

    #[test]
    fn spans_are_bytes_in_the_file_even_after_non_ascii_text() {
        // saphyr counts characters; `é` and `😀` are 2 and 4 bytes.
        let text = "a: é😀\nb: \"x\"\n";
        let base = 4; // as if after `---\n`
        let Yaml::Map(entries) = parse(text, base).unwrap().value else {
            panic!()
        };
        let (key, value) = &entries[1];
        assert_eq!(&format!("----{text}")[key.span.clone()], "b");
        assert_eq!(&format!("----{text}")[value.span.clone()], "\"x\"");
    }

    #[test]
    fn a_duplicate_key_is_an_error_naming_both() {
        let err = parse("a: 1\nb: 2\na: 3\n", 0).unwrap_err();
        assert_eq!(
            err,
            Error::DuplicateKey {
                name: "a".into(),
                first: 0..1,
                again: 10..11
            }
        );
    }

    #[test]
    fn json_is_read_as_yaml() {
        let node = parse(r#"{"links": [{"href": "/a", "n": 1}], "ok": true}"#, 0).unwrap();
        let Yaml::Map(entries) = node.value else {
            panic!()
        };
        assert_eq!(entries[1].1.value, Yaml::Bool(true));
        let Yaml::Seq(links) = &entries[0].1.value else {
            panic!()
        };
        let Yaml::Map(link) = &links[0].value else {
            panic!()
        };
        assert_eq!(link[1].1.value, Yaml::Int(1));
    }
}
