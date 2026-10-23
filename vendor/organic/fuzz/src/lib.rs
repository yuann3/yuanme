//! Fuzzing for the Org parser's guarantees (#15): no input panics, none
//! fails except by running out of fuel or nesting too deep, and none parses
//! slower than 1 ms/KB + 1 ms. From vendor/organic, on stable Rust:
//!
//! ```text
//! cargo fuzz run --sanitizer none adversarial fuzz/corpus/adversarial fuzz/seeds/adversarial
//! cargo run --release --manifest-path fuzz/Cargo.toml --example replay -- adversarial fuzz/corpus/adversarial fuzz/seeds/adversarial
//! ```
//!
//! The seeds are inputs the targets found that broke a guarantee once.

use std::time::Duration;
use std::time::Instant;

use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use yuanme_org::parser::ParseError;
use yuanme_org::parser::parse;

/// Pieces of Org that open, close or interrupt something. Random bytes
/// almost never nest unclosed openers, which is how every slow path found
/// so far starts (#15). The seeds are encoded against this list, so add to
/// its end.
#[rustfmt::skip]
const PIECES: &[&str] = &[
    // Objects that open and may never close.
    "*", "/", "_", "=", "~", "+", "[[", "[[a][", "[[a]", "[fn::", "[fn:a:", "[fn:a]",
    "[cite:", "[cite:@a ", "[cite/s:@a ", "{{{", "{{{m(", "<<", "<<<", "<%%(", "\\(", "\\[",
    "$", "_{", "^{", "x_{", "a^{", "src_x{", "src_x[", "call_x(", "call_x[", "@@a:", "\\alpha",
    "\\alpha{", "\\begin{x}", "[1/", "[%", "<2024-01-01 Mon ", "[2024-01-01 Mon ",
    "https://e.org", "<https://e.org", "\\\\",
    // Their closers.
    "]]", "][", "]", "}", "}}}", ")", ">>", ">>>", ">", "\\)", "\\]", "@@", "\\end{x}",
    // Text between them.
    "a", "word", " ", " ", ",", ".", ":", ";", "|", "é", "\t",
    // Line breaks and what starts a line.
    "\n", "\n", "\n\n", "\n* ", "\n** TODO [#A] ", " :tag:", "\n- ", "\n1. ", "\n  - ",
    "\n- [ ] ", "\n- a :: ", "\n| ", "\n|-", "\n#+BEGIN_QUOTE\n", "\n#+END_QUOTE\n",
    "\n#+BEGIN_SRC x\n", "\n#+END_SRC\n", "\n#+begin_verse\n", "\n#+BEGIN: x\n", "\n#+END:\n",
    "\n:PROPERTIES:\n", "\n:END:\n", "\n:DRAWER:\n", "\n\\begin{equation}\n",
    "\n\\end{equation}\n", "\n#+NAME: x\n", "\n#+CAPTION: c\n", "\n#+TODO: A | B\n",
    "\n[fn:1] ", "\n: ", "\n# ", "\n-----\n", "\n%%(x)\n", "\nCLOCK: [2024-01-01 Mon 10:00]",
    "\nSCHEDULED: <2024-01-01 Mon>",
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

/// Parses `text` and panics if the parse failed for any reason but running
/// out of fuel or nesting too deep, which is how the parser reports a
/// runaway input.
pub fn check(text: &str) {
    if let Err(ParseError::Failed(error)) = parse(text) {
        panic!("{text:?} failed to parse: {error}");
    }
}

/// How long #15 lets `text` take to parse.
pub fn bound(text: &str) -> Duration {
    Duration::from_millis(1) + Duration::from_secs_f64(text.len() as f64 / 1024.0 / 1000.0)
}

/// How long `text` takes to parse.
pub fn time(text: &str) -> Duration {
    let start = Instant::now();
    let _ = parse(text);
    start.elapsed()
}
