//! The Document: one tree for both Source formats (#24). A parser's own
//! tree is lowered into it right after parsing and never seen again, so the
//! HTML writer, the Label index and smartypants handle one tree.
//!
//! A Document is immutable once built and shared through an `Arc`; what
//! only the whole Site knows, such as where a link goes, is kept beside it,
//! keyed by [`NodeId`].

use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

/// Where each resolved link goes, by the link's node.
pub type Links = BTreeMap<NodeId, String>;

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: Kind,
    /// Bytes in the Entry's file.
    pub span: Range<usize>,
    pub children: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Root,
    Paragraph,
    /// `id` is unique in the Document. `attrs` are the others an `{…}` on
    /// the heading gives it, such as `class`.
    Heading {
        level: u8,
        id: String,
        attrs: Vec<(String, String)>,
    },
    ThematicBreak,
    BlockQuote,
    List {
        ordered: bool,
        start: u64,
        tight: bool,
    },
    /// `checked` is set on a task-list item.
    Item {
        checked: Option<bool>,
    },
    CodeBlock {
        lang: Option<String>,
        text: String,
    },
    /// Raw HTML, written out as it is (ADR 0009).
    HtmlBlock(String),
    Table {
        align: Vec<Align>,
    },
    TableRow {
        head: bool,
    },
    TableCell,
    FootnoteDefinition {
        label: String,
    },
    /// A block directive whose name is not a Theorem kind.
    Directive {
        name: String,
    },
    Text(String),
    Code(String),
    Emphasis,
    Strong,
    Strikethrough,
    Link {
        url: String,
        title: Option<String>,
    },
    /// Its children are the alt text.
    Image {
        url: String,
        title: Option<String>,
    },
    LineBreak,
    HtmlInline(String),
    FootnoteReference {
        label: String,
    },
    Math {
        display: bool,
        tex: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// A heading, as `entry.headings` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

impl Document {
    pub const ROOT: NodeId = NodeId(0);

    pub(crate) fn new(span: Range<usize>) -> Self {
        let root = Node {
            kind: Kind::Root,
            span,
            children: Vec::new(),
        };
        Document { nodes: vec![root] }
    }

    pub(crate) fn push(&mut self, parent: NodeId, kind: Kind, span: Range<usize>) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).expect("under 4 billion nodes"));
        self.nodes.push(Node {
            kind,
            span,
            children: Vec::new(),
        });
        self.nodes[parent.0 as usize].children.push(id);
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    pub(crate) fn extend_span(&mut self, id: NodeId, end: usize) {
        self.nodes[id.0 as usize].span.end = end;
    }

    pub(crate) fn kind_mut(&mut self, id: NodeId) -> &mut Kind {
        &mut self.nodes[id.0 as usize].kind
    }

    /// Every node under `id`, `id` first, in document order.
    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            out.push(id);
            stack.extend(self.node(id).children.iter().rev());
        }
        out
    }

    /// The text under `id`: text, code and math, with no markup. An image
    /// below `id` adds nothing, as its alt text is an attribute.
    pub fn text(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_text(id, &mut out);
        out
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        let node = self.node(id);
        match &node.kind {
            Kind::Text(s) | Kind::Code(s) => out.push_str(s),
            Kind::Math { tex, .. } => out.push_str(tex),
            _ => {}
        }
        for &child in &node.children {
            if !matches!(self.node(child).kind, Kind::Image { .. }) {
                self.collect_text(child, out);
            }
        }
    }

    /// mdast-util-to-string over the whole Document, which Astro's reading
    /// time counts: every text, code and raw HTML, and image alt text, with
    /// nothing between blocks.
    fn all_text(&self) -> String {
        let mut out = String::new();
        for id in self.descendants(Self::ROOT) {
            match &self.node(id).kind {
                Kind::Text(s) | Kind::Code(s) | Kind::HtmlBlock(s) | Kind::HtmlInline(s) => {
                    out.push_str(s)
                }
                Kind::CodeBlock { text, .. } => out.push_str(text),
                Kind::Math { tex, .. } => out.push_str(tex),
                _ => {}
            }
        }
        out
    }

    /// Words as the reading-time package counts them, which the Reference
    /// site shows (#18).
    pub fn word_count(&self) -> u64 {
        word_count(&self.all_text())
    }

    pub fn headings(&self) -> Vec<Heading> {
        self.descendants(Self::ROOT)
            .into_iter()
            .filter_map(|id| match &self.node(id).kind {
                Kind::Heading {
                    level, id: anchor, ..
                } => Some(Heading {
                    level: *level,
                    id: anchor.clone(),
                    text: self.text(id),
                }),
                _ => None,
            })
            .collect()
    }
}

/// reading-time 1.5's count, over UTF-16 code units as JavaScript indexes
/// them: a word ends at a space, tab or newline, and each CJK code unit is
/// a word of its own.
fn word_count(text: &str) -> u64 {
    let units: Vec<u16> = text.encode_utf16().collect();
    let bound = |i: usize| {
        units
            .get(i)
            .is_some_and(|&u| matches!(u, 0x20 | 0x0A | 0x0D | 0x09))
    };
    // Its last range is outside UTF-16 code units, so it never matches.
    let cjk = |i: usize| {
        units
            .get(i)
            .is_some_and(|&u| matches!(u, 0x3040..=0x309F | 0x4E00..=0x9FFF | 0xAC00..=0xD7A3))
    };
    let punctuation = |i: usize| {
        units.get(i).is_some_and(|&u| {
            matches!(u, 0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E | 0x3000..=0x303F | 0xFF00..=0xFFEF)
        })
    };
    let Some(mut start) = (0..units.len()).find(|&i| !bound(i)) else {
        return 0;
    };
    let end = (0..units.len())
        .rev()
        .find(|&i| !bound(i))
        .expect("start exists");
    // The text has a newline appended, so the last unit ends a word.
    let bound = |i: usize| i == units.len() || bound(i);
    let mut words = 0;
    while start <= end {
        let i = start;
        if cjk(i) || (!bound(i) && (bound(i + 1) || cjk(i + 1))) {
            words += 1;
        }
        let mut i = i;
        if cjk(i) {
            while i <= end && (punctuation(i + 1) || bound(i + 1)) {
                i += 1;
            }
        }
        start = i + 1;
    }
    words
}

/// Minutes at 200 words a minute, as `Math.ceil(minutes.toFixed(2))`.
pub fn reading_time(words: u64) -> u64 {
    let minutes = words as f64 / 200.0;
    // Rust rounds the double's exact value as toFixed does, and a double
    // that is a multiple of 0.005 is never exactly halfway.
    format!("{minutes:.2}")
        .parse::<f64>()
        .expect("a number")
        .ceil() as u64
}
