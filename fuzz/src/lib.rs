//! Fuzzing for the Markdown parse's guarantees (#15): no input panics, and
//! none parses slower than 1 ms/KB + 1 ms on the M4 Max the fuel was
//! calibrated on, or as many times longer as the machine is slower. The
//! parse is all of `markdown::document`: comrak, lowering, smartypants and
//! heading ids. The HTML writer runs too, for panics, but isn't timed.
//! From the repository root, on stable Rust:
//!
//! ```text
//! cargo fuzz run --sanitizer none adversarial fuzz/corpus/adversarial fuzz/seeds/adversarial
//! cargo run --release --manifest-path fuzz/Cargo.toml --example replay -- adversarial fuzz/corpus/adversarial fuzz/seeds/adversarial
//! ```
//!
//! The adversarial seeds are inputs that broke the bound once. The bytes
//! target starts from `corpus/md`.

use std::time::Duration;
use std::time::Instant;

use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use yuanme_diag::Files;

/// Pieces of Markdown that open, close or interrupt something. Random bytes
/// almost never nest unclosed openers, which is how every slow path found
/// so far starts (#15, #47). The seeds are encoded against this list, so
/// add to its end.
#[rustfmt::skip]
const PIECES: &[&str] = &[
    // Inlines that open and may never close.
    "[", "![", "[a](", "[a][", "[^a]", "[^", "^[", "*", "**", "_", "__", "~~", "~", "`", "``",
    "$", "$$", "$`", "\\(", "\\[", "<", "<a ", "<a href=\"", "<!--", "<?", "<![CDATA[",
    "<http://a", "<a@b.c", "www.a", "http://a.b", "a@b.c", "&", "&amp", "&#", "\\", "{#a",
    "{.a", "\"", "'",
    // Their closers.
    "]", "](", ")", "](b)", "`$", ">", "-->", "?>", "]]>", "}", "\\)", "\\]",
    // Text between them.
    "a", "word", " ", "  ", ".", ". ", ",", ":", ";", "|", "-", "--", "é", "\t",
    // Line breaks and what starts a line.
    "\n", "\n", "\n\n", "\n# ", "\n## a {#b}", "\na\n===", "\n> ", ">", "\n- ", "- ", "\n1. ",
    "1) ", "\n  - ", "\n- [ ] ", "\n- [x] ", "\n    ", "\n```\n", "\n```a\n", "\n~~~\n",
    "\n:::a\n", ":::a\n", "\n:::\n", ":::\n", "\n::::a\n", "\n::::\n", "\n:::theorem{#thm:a}\n",
    "\n:::proof\n", "\n:::note[T]\n", "\n$$\n", "\n| a | b |", "\n|-|-|", "\n---\n", "\n***\n",
    "\n[^a]: ", "\n[a]: b", "\n[a]: <", "\n<div>\n", "\n</div>\n", "\n<!--\n", "\n-->\n",
    // Math, which math-core converts (#50): groups, environments, Labels
    // and macros, whose expansions can multiply what is written.
    "{", "\\frac{", "\\sqrt", "\\left(", "\\right)", "^", "_", "&", "\\\\", "\\begin{align}",
    "\\end{align}", "\\begin{matrix}", "\\end{matrix}", "\\label{a}", "\\eqref{a}", "\\notag",
    "\\newcommand{\\a}{", "\\newcommand{\\b}{\\a\\a", "\\a", "\\b", "\\text{", "\\begin{a}",
];

#[derive(Arbitrary, Debug)]
struct Part {
    piece: u8,
    repeat: u8,
}

/// The document the `adversarial` target parses for `data`: a sequence of
/// pieces, each repeated, since a run of one opener is what makes a slow
/// path slow.
pub fn adversarial(data: &[u8]) -> String {
    let parts = Vec::<Part>::arbitrary_take_rest(Unstructured::new(data)).unwrap_or_default();
    let mut text = String::new();
    for part in parts {
        let piece = PIECES[usize::from(part.piece) % PIECES.len()];
        for _ in 0..=part.repeat % 64 {
            text.push_str(piece);
        }
    }
    text
}

/// Parses `text` as a Markdown body, as the Engine parses an Entry's, and
/// writes it as HTML. A panic is the failure libFuzzer looks for;
/// Diagnostics, such as running out of fuel, are how the parse reports bad
/// input.
pub fn check(text: &str) {
    if let Some(doc) = parse(text) {
        let _ = yuanme_render::html::body(
            &doc,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        );
    }
}

fn parse(text: &str) -> Option<yuanme_content::document::Document> {
    let mut files = Files::default();
    let file = files.add("a.md", text);
    yuanme_content::markdown::document(&files, file, 0..text.len()).0
}

/// How long #15 lets `text` take to parse on the machine the fuel was
/// calibrated on, an M4 Max (#47).
pub fn bound(text: &str) -> Duration {
    Duration::from_millis(1) + Duration::from_secs_f64(text.len() as f64 / 1024.0 / 1000.0)
}

/// The best time to parse `reference_document` on that M4 Max, in release.
const REFERENCE: Duration = Duration::from_micros(3_600);

/// How many times slower than the M4 Max this machine parses, at least 1.
///
/// The bound is there to catch superlinear paths (#15), and a runner
/// slower at everything would fail it on inputs that run out of fuel well
/// inside it here, so fuzz CI scales it by how much slower the machine
/// parses a corpus-like document, as the Org fuzz job does (#48). That also
/// scales it with the parse's own speed: a parse slower at everything gets
/// a longer bound, which only a benchmark catches. Measure `REFERENCE`
/// again when the parse's speed moves.
pub fn slowdown() -> f64 {
    let document = reference_document();
    let best = (0..20).map(|_| time(&document)).min().unwrap_or_default();
    (best.as_secs_f64() / REFERENCE.as_secs_f64()).max(1.0)
}

/// A document like those in the corpus, about 100 KB.
fn reference_document() -> String {
    let section = r#"## A heading with `code`

Prose with *emphasis*, **strong**, ~~struck~~ and `code` text, a
[link](https://example.org "title"), an ![image](a.png), a footnote[^1],
"quotes", dashes -- and math: $x^2 + y^2$ and $`\sqrt{2}`$. See
https://example.org and www.example.org.

> A quotation, with a [reference link][ref].

- An item
- [ ] A task
  1. Nested

| Name | Value |
|------|------:|
| a    | 1     |

```rust
fn main() {}
```

$$
\int_0^1 f(x)\,dx
$$

:::theorem
Every *statement* here is true.
:::

[^1]: The footnote.

[ref]: https://example.org

"#;
    section.repeat(100_000 / section.len())
}

/// How long `text` takes to parse.
pub fn time(text: &str) -> Duration {
    let start = Instant::now();
    let _ = parse(text);
    start.elapsed()
}
