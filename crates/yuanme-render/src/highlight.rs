//! Code blocks coloured as the Reference site's Shiki 3.19 colours them:
//! ferriki runs the same grammars with the Catppuccin themes, light and
//! dark (#5, #53, ADR 0014). Each run of characters with one style becomes
//! one span, whose class names that style's colours, and [`css`] writes
//! the rules for the classes a Site used.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use ferriki::{FontStyle, Highlighter, StandardAssetCatalogs, TokenizeOptions};
use yuanme_content::document::{Document, Kind, NodeId};

use crate::html::text;

include!(concat!(env!("OUT_DIR"), "/highlight.rs"));

struct Theme {
    id: &'static str,
    foreground: &'static str,
    background: &'static str,
    /// Every colour a token can have, sorted.
    palette: &'static [&'static str],
}

const LIGHT: Theme = CATPPUCCIN_LATTE;
const DARK: Theme = CATPPUCCIN_MOCHA;

/// The class of every highlighted `<pre>`.
pub const PRE_CLASS: &str = "y-code";

/// Names that are plain text with no warning: Org's (#16) and Shiki's.
const PLAIN: [&str; 7] = [
    "example",
    "fundamental",
    "none",
    "plain",
    "plaintext",
    "text",
    "txt",
];

/// Org's `org-src-lang-modes` (Org 9.8.7), with each mode as the grammar
/// that highlights it: a source block's language names its Emacs mode.
/// `asymptote` and `dot` stay unknown, as no grammar has their modes.
const ORG: &[(&str, &str)] = &[
    ("C", "c"),
    ("C++", "cpp"),
    ("ash", "shellscript"),
    ("bash2", "shellscript"),
    ("beamer", "latex"),
    ("calc", "text"),
    ("csh", "shellscript"),
    ("dash", "shellscript"),
    ("desktop", "ini"),
    // artist-mode, for drawing in plain text.
    ("ditaa", "text"),
    ("dtksh", "shellscript"),
    ("es", "shellscript"),
    ("itcsh", "shellscript"),
    ("jcsh", "shellscript"),
    ("jsh", "shellscript"),
    ("ksh", "shellscript"),
    ("ksh88", "shellscript"),
    ("mksh", "shellscript"),
    ("oash", "shellscript"),
    ("pdksh", "shellscript"),
    ("posix", "shellscript"),
    ("rc", "shellscript"),
    ("rpm", "shellscript"),
    ("screen", "shellscript"),
    ("sqlite", "sql"),
    ("tcsh", "shellscript"),
    ("wksh", "shellscript"),
    ("wsh", "shellscript"),
];

/// Each highlighted code block's lines, by its node.
pub type Code = BTreeMap<NodeId, String>;

/// What a code block's language name resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Plain,
    /// A grammar, by its Shiki id.
    Grammar(&'static str),
}

/// A Shiki name or alias, then an Org name (#16), then `jupyter-X` as `X`,
/// then the name in lower case, as `R` names the mode for `r`. `None` is a
/// name no grammar answers to.
pub fn language(name: &str) -> Option<Language> {
    let shiki = |name: &str| {
        if PLAIN.contains(&name) {
            return Some(Language::Plain);
        }
        let i = NAMES.binary_search_by_key(&name, |(n, _)| n).ok()?;
        Some(Language::Grammar(NAMES[i].1))
    };
    let org = |name: &str| {
        let (_, mode) = ORG.iter().find(|(n, _)| *n == name)?;
        shiki(mode)
    };
    shiki(name)
        .or_else(|| org(name))
        .or_else(|| shiki(name.strip_prefix("jupyter-")?))
        .or_else(|| shiki(&name.to_lowercase()))
}

/// Every name [`language`] resolves directly, for suggestions.
pub fn names() -> impl Iterator<Item = &'static str> {
    NAMES.iter().map(|(n, _)| *n).chain(PLAIN)
}

/// One theme's style of a token: its colour's place in the palette, and
/// its font style bits.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Look(usize, i32);

const DECORATION: i32 = FontStyle::UNDERLINE.bits() | FontStyle::STRIKETHROUGH.bits();

/// ferriki, like Shiki, stops tokenizing a line after 500 ms and leaves
/// the rest of it in the wrong colours, so a slow or busy machine wrote
/// different bytes. Without the limit, output depends only on the input;
/// ferroni's retry limit on each match still bounds the time.
fn untimed() -> TokenizeOptions {
    TokenizeOptions::default().with_time_limit_millis(0)
}

thread_local! {
    // ferriki's Highlighter is !Send, so each rayon thread keeps its own
    // (ADR 0014).
    static HIGHLIGHTER: RefCell<Highlighter> = RefCell::new(highlighter());
}

fn highlighter() -> Highlighter {
    let assets = |files: &'static [(&str, &[u8])]| {
        files
            .iter()
            .map(|(name, bytes)| (name.to_string(), Cow::Borrowed(*bytes)))
    };
    let catalogs = StandardAssetCatalogs::from_embedded(
        GRAMMARS_MANIFEST,
        assets(GRAMMARS),
        THEMES_MANIFEST,
        assets(THEMES),
    )
    .expect("the embedded catalogs decode");
    Highlighter::builder()
        .with_assets(catalogs)
        .load_themes([LIGHT.id, DARK.id])
        .build()
        .expect("the embedded themes load")
}

/// The lines of `source` in `grammar`, or as plain text, as HTML. Text in
/// the themes' own colours is left bare; every other run of one style is a
/// span. Shiki's `<span class="line">` around each line is left out: it
/// would be a sixth of the code markup on the Reference site's largest
/// Post, which has to stay under 40 KB (#53).
pub fn block(grammar: Option<&str>, source: &str) -> String {
    let plain = (Look(fg(&LIGHT), 0), Look(fg(&DARK), 0));
    let lines: Vec<Vec<((Look, Look), String)>> = match grammar {
        None => lines(source)
            .map(|line| vec![(plain, line.to_string())])
            .collect(),
        Some(grammar) => HIGHLIGHTER.with_borrow_mut(|h| {
            let result = h
                .highlight_with_themes_and_options(
                    source,
                    grammar,
                    &[("light", LIGHT.id), ("dark", DARK.id)],
                    &untimed(),
                )
                .expect("an embedded grammar highlights any text");
            result
                .tokens
                .into_iter()
                .map(|line| {
                    line.into_iter()
                        .map(|t| {
                            let look = |name: &str, theme: &Theme| {
                                let v = &t.variants[name];
                                let colour = v
                                    .color
                                    .as_deref()
                                    .map_or(theme.foreground.into(), str::to_ascii_lowercase);
                                let i = theme
                                    .palette
                                    .binary_search(&colour.as_str())
                                    .unwrap_or_else(|_| {
                                        panic!("{colour} is no colour of {}", theme.id)
                                    });
                                Look(i, v.font_style.unwrap_or(FontStyle::NONE).bits().max(0))
                            };
                            ((look("light", &LIGHT), look("dark", &DARK)), t.content)
                        })
                        .collect()
                })
                .collect()
        }),
    };
    let mut html = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            html.push('\n');
        }
        // Blank text shows no colour, italics or weight, so it joins the
        // run before it unless a line would be drawn through or under it.
        let mut previous = plain;
        let looks = line.iter().map(|(look, content)| {
            let decorated = |(l, d): (Look, Look)| (l.1 | d.1) & DECORATION != 0;
            if content.trim().is_empty() && !decorated(*look) && !decorated(previous) {
                (previous, content)
            } else {
                previous = *look;
                (*look, content)
            }
        });
        let mut tokens = looks.peekable();
        while let Some((look, content)) = tokens.next() {
            let mut run = text(content);
            while let Some((_, next)) = tokens.next_if(|(l, _)| *l == look) {
                run.push_str(&text(next));
            }
            if look == plain {
                html.push_str(&run);
            } else {
                html.push_str(&format!("<span class=\"{}\">{run}</span>", class(look)));
            }
        }
    }
    html
}

/// Lines as Shiki splits them.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l))
}

fn fg(theme: &Theme) -> usize {
    theme
        .palette
        .binary_search(&theme.foreground)
        .expect("the foreground is in the palette")
}

/// Letters for each theme's colour, then, if either has a font style, a
/// hex digit for each theme's: `dk`, or `dk11` for italic in both.
fn class((light, dark): (Look, Look)) -> String {
    let mut class = letters(light.0, LIGHT.palette.len()) + &letters(dark.0, DARK.palette.len());
    if light.1 != 0 || dark.1 != 0 {
        class.push_str(&format!("{:x}{:x}", light.1, dark.1));
    }
    class
}

/// `i` in base 26 as lower-case letters, wide enough for any of `n`, so a
/// class starts with a letter and its parts don't run together.
fn letters(mut i: usize, n: usize) -> String {
    let mut width = 1;
    while 26usize.pow(width) < n {
        width += 1;
    }
    let mut out = vec![b'a'; width as usize];
    for slot in out.iter_mut().rev() {
        *slot = b'a' + (i % 26) as u8;
        i /= 26;
    }
    String::from_utf8(out).expect("ASCII")
}

/// [`class`] read back, if it is one.
fn look(class: &str) -> Option<(Look, Look)> {
    let colour = |s: &str, theme: &Theme| {
        let i = s.bytes().try_fold(0, |i, b| {
            b.is_ascii_lowercase()
                .then(|| i * 26 + usize::from(b - b'a'))
        })?;
        (i < theme.palette.len()).then_some(i)
    };
    let (l, rest) = class.split_at_checked(letters(0, LIGHT.palette.len()).len())?;
    let (d, style) = rest.split_at_checked(letters(0, DARK.palette.len()).len())?;
    let (lb, db) = match style.as_bytes() {
        [] => (0, 0),
        [l, d] => (hex(*l)?, hex(*d)?),
        _ => return None,
    };
    let looks = (Look(colour(l, &LIGHT)?, lb), Look(colour(d, &DARK)?, db));
    (class == self::class(looks)).then_some(looks)
}

fn hex(b: u8) -> Option<i32> {
    (b as char).to_digit(16).map(|d| d as i32)
}

/// The classes of the spans in `page`'s highlighted code, or `None` if it
/// has none.
pub fn classes(page: &str) -> Option<BTreeSet<&str>> {
    // How html::body opens a highlighted block, after any attributes.
    let open = format!(" class=\"{PRE_CLASS}\" tabindex=\"0\" data-language=");
    let mut found = None;
    let mut rest = page;
    while let Some(at) = rest.find(&open) {
        let classes = found.get_or_insert_with(BTreeSet::new);
        rest = &rest[at..];
        let end = rest.find("</pre>").unwrap_or(rest.len());
        for span in rest[..end].split("<span class=\"").skip(1) {
            if let Some(class) = span.split('"').next()
                && look(class).is_some()
            {
                classes.insert(class);
            }
        }
        rest = &rest[end..];
    }
    found
}

/// The stylesheet for `classes`: the light theme, then the dark one when
/// the visitor prefers it.
pub fn css<'a>(classes: impl IntoIterator<Item = &'a str>) -> String {
    let classes: Vec<(&str, (Look, Look))> = classes
        .into_iter()
        .filter_map(|c| Some((c, look(c)?)))
        .collect();
    let rules = |theme: &Theme, pick: fn((Look, Look)) -> Look| {
        let mut out = format!(
            ".{PRE_CLASS}{{background-color:{};color:{}}}",
            theme.background, theme.foreground
        );
        for (class, looks) in &classes {
            let Look(colour, bits) = pick(*looks);
            out.push_str(&format!(
                ".{PRE_CLASS} .{class}{{color:{}",
                theme.palette[colour]
            ));
            if looks.0.1 != 0 || looks.1.1 != 0 {
                let on = |bit: FontStyle, yes: &'static str, no: &'static str| {
                    if bits & bit.bits() != 0 { yes } else { no }
                };
                let mut decoration = Vec::new();
                if bits & FontStyle::UNDERLINE.bits() != 0 {
                    decoration.push("underline");
                }
                if bits & FontStyle::STRIKETHROUGH.bits() != 0 {
                    decoration.push("line-through");
                }
                let decoration = if decoration.is_empty() {
                    "none".into()
                } else {
                    decoration.join(" ")
                };
                out.push_str(&format!(
                    ";font-style:{};font-weight:{};text-decoration:{decoration}",
                    on(FontStyle::ITALIC, "italic", "normal"),
                    on(FontStyle::BOLD, "bold", "normal"),
                ));
            }
            out.push('}');
        }
        out
    };
    format!(
        "{}\n@media (prefers-color-scheme:dark){{{}}}\n",
        rules(&LIGHT, |(l, _)| l),
        rules(&DARK, |(_, d)| d)
    )
}

/// Every code block of `doc` highlighted, a block in a language no grammar
/// has as plain text. The Engine does this through its cache instead.
pub fn document(doc: &Document) -> Code {
    let mut blocks: Vec<_> = doc
        .descendants(Document::ROOT)
        .into_iter()
        .filter_map(|id| match &doc.node(id).kind {
            Kind::CodeBlock { lang, text } => Some((grammar(lang.as_deref()), id, text)),
            _ => None,
        })
        .collect();
    // By grammar: a thread's highlighter compiles a grammar again after
    // it highlights another.
    blocks.sort_by_key(|(grammar, _, _)| *grammar);
    blocks
        .into_iter()
        .map(|(grammar, id, text)| (id, block(grammar, text)))
        .collect()
}

/// The grammar a code block's language names, if any.
pub fn grammar(lang: Option<&str>) -> Option<&'static str> {
    match language(lang?)? {
        Language::Grammar(g) => Some(g),
        Language::Plain => None,
    }
}
