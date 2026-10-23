//! Small inputs that upstream organic crashed on or took exponential time
//! over. Each expected shape is what `org-element-parse-buffer` gives in
//! Org 9.8.7 (Emacs 31.1).

use yuanme_org::parser::parse;
use yuanme_org::parser::ParseError;
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

/// The first list item's counter, whether it has a checkbox, and the text
/// of its paragraph.
fn first_item(input: &str) -> (Option<u64>, bool, String) {
    let document = parse(input).unwrap_or_else(|error| panic!("{input:?}: {error}"));
    let section = document.zeroth_section.unwrap();
    let Element::PlainList(list) = &section.children[0] else {
        panic!("not a list: {:?}", section.children[0]);
    };
    let item = &list.children[0];
    let Element::Paragraph(paragraph) = &item.children[0] else {
        panic!("not a paragraph: {:?}", item.children[0]);
    };
    (
        item.counter,
        item.checkbox.is_some(),
        paragraph.source.to_owned(),
    )
}

#[test]
fn a_list_counter_can_be_a_large_number() {
    assert_eq!(
        first_item("999999997. [@999999997] x\n"),
        (Some(999_999_997), false, "x\n".to_owned())
    );
}

#[test]
fn text_can_follow_a_counter_but_not_a_checkbox_directly() {
    assert_eq!(
        first_item("3. [@10]Blue\n"),
        (Some(10), false, "Blue\n".to_owned())
    );
    assert_eq!(
        first_item("- [X]Blue\n"),
        (None, false, "[X]Blue\n".to_owned())
    );
    assert_eq!(
        first_item("- [X] Blue\n"),
        (None, true, "Blue\n".to_owned())
    );
}

#[test]
fn a_document_can_hold_more_unclosed_brackets_than_an_i16() {
    let input = format!("{}\n", "[ ".repeat(33_000));
    assert_eq!(shape_of(&input), ["Paragraph PlainText"]);
}

/// Openers that never close. Upstream took exponential or quadratic time
/// over these (#30); Emacs reads each as one run of plain text.
#[test]
fn unclosed_openers_are_plain_text() {
    let inputs = [
        "*a /b ".repeat(12),
        "char *p, *q, *r, *s, *t, *u, *v, *w, *x, *y;".to_owned(),
        "+a _b =c ~d ".repeat(12),
        "[fn:: ".repeat(14),
        "See[fn:: a note ".repeat(10),
        "[fn:x: ".repeat(14),
        "a_{b ".repeat(20),
        "a^{b ".repeat(20),
        "x^{a_{b ".repeat(20),
        "Let x_{i be y and ".repeat(20),
        "[[a][".repeat(800),
        "[[a]".repeat(800),
        "[cite:@a ".repeat(800),
        "{{{m(".repeat(800),
        "<%%(a".repeat(800),
        "\\(a".repeat(800),
    ];
    for input in inputs {
        assert_eq!(
            shape_of(&format!("{input}\n")),
            ["Paragraph PlainText"],
            "{input:?}"
        );
    }
}

#[test]
fn markup_in_a_verse_block_can_span_a_blank_line() {
    let document = parse("#+begin_verse\n~/\n\nc~\n#+end_verse\n").unwrap();
    let section = document.zeroth_section.unwrap();
    let Element::VerseBlock(verse) = &section.children[0] else {
        panic!("not a verse block: {:?}", section.children[0]);
    };
    assert_eq!(kind(&verse.children[0]), "Code");
}

/// The `]` of a labelled footnote is no closer for an anonymous one that
/// opened before it.
#[test]
fn an_unclosed_footnote_ignores_closed_ones_after_it() {
    let input = format!("x{}\n", " [fn:a] [fn:: ".repeat(14));
    let mut expected = "Paragraph PlainText".to_owned();
    for _ in 0..14 {
        expected.push_str(" FootnoteReference PlainText");
    }
    assert_eq!(shape_of(&input), [expected]);
}

#[test]
fn a_link_ends_at_a_blank_line() {
    assert_eq!(
        shape_of("x [[a][b\n\nc]] y\n"),
        ["Paragraph PlainText", "Paragraph PlainText"]
    );
}

/// A tag ends at the last ` :: ` on its line. Upstream found it in time
/// exponential in the number of dividers.
#[test]
fn a_tag_ends_at_the_last_divider_on_its_line() {
    let input = format!("- a{} last\n", " :: b".repeat(30));
    let (_, _, paragraph) = first_item(&input);
    assert_eq!(paragraph, "b last\n");
}

#[test]
fn a_comment_at_the_end_of_the_input_keeps_its_text() {
    for (input, value) in [("#", ""), ("# a", "a"), ("# a\n", "a")] {
        let document = parse(input).unwrap();
        let section = document.zeroth_section.unwrap();
        let Element::Comment(comment) = &section.children[0] else {
            panic!("not a comment: {:?}", section.children[0]);
        };
        assert_eq!(comment.value, [value], "{input:?}");
    }
}

/// An object inside another can run past the outer one's closer. Upstream
/// panicked when the outer one's end matcher then found itself past it.
#[test]
fn an_object_past_its_parents_closer_does_not_panic() {
    for input in [
        ":_{[[}]]",
        "}^{http:}(E",
        ")^{src_}{}",
        "[cite:\\[[]]\\]",
        "[cite:src_]{}",
        "[cite:[[\\]]]",
    ] {
        assert!(parse(input).is_ok(), "{input:?}");
    }
}

#[test]
fn a_table_can_end_the_input_without_a_line_break() {
    assert_eq!(shape_of("a\n| b | c"), ["Paragraph PlainText", "Table"]);
}

#[test]
fn any_todo_keyword_value_parses() {
    for value in ["(", "||", "A(b)c", "TODO(t) | DONE(d!) |"] {
        let input = format!("#+TODO: {value}\n* TODO x\n");
        assert!(parse(&input).is_ok(), "{input:?}");
    }
}

/// Unclosed blocks nested in each other take polynomial time, each block
/// re-parsing the rest, so the parse runs out of fuel and names the first
/// block (#15).
#[test]
fn a_runaway_parse_runs_out_of_fuel_at_its_opener() {
    let prose = "Some prose.\n\n";
    let input = format!("{prose}{}text\n", "#+BEGIN_QUOTE\n#+BEGIN: x\n".repeat(8));
    match parse(&input) {
        Err(ParseError::OutOfFuel { opener }) => assert_eq!(opener, prose.len()),
        other => panic!("expected OutOfFuel, got {:?}", other.map(|_| ())),
    }
}

/// Upstream failed the whole parse on each of these.
#[test]
fn a_diary_sexp_runs_to_the_end_of_its_line() {
    assert_eq!(shape_of("%%("), ["DiarySexp"]);
    assert_eq!(shape_of("a\n%%(\n"), ["Paragraph PlainText", "DiarySexp"]);
    assert_eq!(
        shape_of("%%(a\rb)\nc\n"),
        ["DiarySexp", "Paragraph PlainText"]
    );
}

/// Upstream took `<<<a >>>` for a radio target, then failed the whole parse
/// when its link ran from a table cell into the next line.
#[test]
fn a_radio_target_does_not_end_with_whitespace() {
    assert_eq!(shape_of("|<<<a >>>a\nb"), ["Table", "Paragraph PlainText"]);
}

#[test]
fn a_radio_link_spans_lines_only_inside_a_paragraph() {
    let links = |input: &str| {
        format!("{:?}", parse(input).unwrap())
            .matches("RadioLink {")
            .count()
    };
    assert_eq!(links("<<<a b>>>\n\nx a\nb c"), 1);
    assert_eq!(links("<<<a b>>>\n\n|x a\nb"), 0);
    assert_eq!(links("<<<a b>>>\n\n- x a\nb"), 0);
}

/// Citations nested this deep recursed past a 2 MiB stack, which aborts the
/// process instead of unwinding. Test threads get 2 MiB.
#[test]
fn a_parse_nested_too_deeply_stops_at_its_opener() {
    let input = format!(
        "{}{}{}{}",
        "[cite:".repeat(44),
        "][".repeat(43),
        "[".repeat(130),
        "@".repeat(1152)
    );
    match parse(&input) {
        Err(ParseError::TooDeep { opener }) => assert_eq!(opener, 0),
        other => panic!("expected TooDeep, got {:?}", other.map(|_| ())),
    }
}
