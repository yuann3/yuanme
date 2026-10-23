//! Small inputs that upstream organic crashed on or took exponential time
//! over. Each expected shape is what `org-element-parse-buffer` gives in
//! Org 9.8.7 (Emacs 31.1).

use yuanme_org::parser::parse;
use yuanme_org::types::Document;
use yuanme_org::types::Element;

/// The variant name from a node's Debug output, such as `Paragraph`.
fn kind(node: &impl std::fmt::Debug) -> String {
    let debug = format!("{node:?}");
    debug[..debug.find('(').unwrap_or(debug.len())].to_owned()
}

/// The top-level elements, each with its objects when it is a paragraph.
/// Adjacent plain text is one run, as Emacs prints it.
fn shape(document: &Document) -> Vec<String> {
    let section = document.zeroth_section.as_ref().expect("a zeroth section");
    section
        .children
        .iter()
        .map(|element| {
            let mut shape = kind(element);
            if let Element::Paragraph(paragraph) = element {
                for object in &paragraph.children {
                    let object = kind(object);
                    if !(object == "PlainText" && shape.ends_with(" PlainText")) {
                        shape.push(' ');
                        shape.push_str(&object);
                    }
                }
            }
            shape
        })
        .collect()
}

fn shape_of(input: &str) -> Vec<String> {
    let document = parse(input).unwrap_or_else(|error| panic!("{input:?}: {error}"));
    shape(&document)
}

#[test]
fn a_stray_end_line_before_an_object_is_a_paragraph() {
    assert_eq!(
        shape_of(":END:\n[2024-01-01 Mon]\n"),
        ["Paragraph PlainText Timestamp PlainText"]
    );
    assert_eq!(
        shape_of(":END:\nhttps://example.org\n"),
        ["Paragraph PlainText PlainLink PlainText"]
    );
    assert_eq!(
        shape_of("#+BEGIN: x\n<2024-01-01 Mon> y\n"),
        ["Paragraph PlainText Timestamp PlainText"]
    );
}

#[test]
fn a_lone_bar_is_a_table() {
    assert_eq!(
        shape_of("a\n|\nb\n"),
        ["Paragraph PlainText", "Table", "Paragraph PlainText"]
    );
}
