use crate::codes::{self, ALL, AREAS, Code};
use crate::*;

// A code that exists only for these tests, shaped like the #21 example.
const UNKNOWN_LABEL: Code = Code {
    name: "math/unknown-label",
    meaning: "test",
    example: "test",
    fix: "test",
};

fn span(file: FileId, range: std::ops::Range<usize>) -> Span {
    Span { file, range }
}

fn edit(file: FileId, range: std::ops::Range<usize>, replacement: &str) -> Edit {
    Edit {
        span: span(file, range),
        replacement: replacement.to_string(),
    }
}

/// The #21 example: `eq:gaus` is bytes 20..27, line 3, columns 11..18.
fn unknown_label() -> (Files, Diagnostic) {
    let mut files = Files::default();
    let text = "title: x\n\nby \\eqref{eq:gaus} we get\n";
    let file = files.add("content/posts/maxwell.md", text);
    assert_eq!(&text[20..27], "eq:gaus");
    let d = Diagnostic::error(
        &UNKNOWN_LABEL,
        "no Label `eq:gaus` in this Entry",
        span(file, 20..27),
    )
    .label("referenced here")
    .suggest(
        "a Label with a similar name exists: `eq:gauss`",
        Applicability::MachineApplicable,
        vec![edit(file, 20..27, "eq:gauss")],
    );
    (files, d)
}

#[test]
fn json_has_rustc_positions_and_the_fix() {
    let (files, d) = unknown_label();
    let json = d.to_json(&files);
    let s = &json["spans"][0];
    assert_eq!(
        (
            &s["line_start"],
            &s["column_start"],
            &s["line_end"],
            &s["column_end"]
        ),
        (&3.into(), &11.into(), &3.into(), &18.into())
    );
    assert_eq!(s["text"], "by \\eqref{eq:gaus} we get");
    let fix = &json["children"][0]["suggestions"][0];
    assert_eq!(fix["applicability"], "machine-applicable");
    assert_eq!(fix["edits"][0]["byte_start"], 20);
    assert_eq!(json["rendered"], d.render(&files, false));
    insta::assert_snapshot!(serde_json::to_string_pretty(&json).unwrap());
}

#[test]
fn rendered_output() {
    let (files, d) = unknown_label();
    insta::assert_snapshot!(d.render(&files, false));
    assert_eq!(
        d.render_short(&files),
        "content/posts/maxwell.md:3:11: error[math/unknown-label]: no Label `eq:gaus` in this Entry"
    );
}

#[test]
fn columns_count_unicode_scalar_values() {
    let mut files = Files::default();
    // é is 2 bytes and 😀 is 4, so `c` is byte 9 but the 4th character of line 2.
    let file = files.add("a.md", "é\nab😀c\r\nnext");
    assert_eq!(files.position(file, 9), Position { line: 2, column: 4 });
    assert_eq!(files.line_text(file, 9), "ab😀c");
}

#[test]
fn source_map_points_derived_spans_at_the_original() {
    let mut files = Files::default();
    let original = r#"<a href="x">hi</a>"#;
    let file = files.add("templates/row.html", original);
    let mut b = SourceMapBuilder::new(file, original);
    b.copy(0..2);
    b.insert(" data-y-1");
    b.copy(2..original.len());
    let (derived, map) = b.finish();
    assert_eq!(derived, r#"<a data-y-1 href="x">hi</a>"#);

    // Copied text maps to the same text in the original.
    assert_eq!(map.span(12..16).range, 3..7);
    assert_eq!(&original[3..7], "href");
    // A span ending exactly where an insertion begins stays in the copied text.
    assert_eq!(map.span(0..2).range, 0..2);
    // Inserted text has no original bytes: it maps to where it was inserted.
    assert_eq!(map.span(3..11).range, 2..2);
    // A span across the insertion covers the original bytes on both sides.
    assert_eq!(map.span(0..16).range, 0..7);
}

#[test]
fn source_map_keeps_spans_out_of_removed_text() {
    let mut files = Files::default();
    // Scoping unwraps `:global(...)`, so the derived text skips original bytes.
    let original = ".a :global(.b) {}";
    let file = files.add("templates/row.css", original);
    let mut b = SourceMapBuilder::new(file, original);
    b.copy(0..3);
    b.copy(11..13);
    b.copy(14..original.len());
    let (derived, map) = b.finish();
    assert_eq!(derived, ".a .b {}");
    // `.a ` ends at byte 3 in both; it must not jump past the removed `:global(`.
    assert_eq!(map.span(0..3).range, 0..3);
    assert_eq!(map.span(3..5).range, 11..13);
    // An empty span where text was removed points after the removal, not into it.
    assert_eq!(map.span(3..3).range, 11..11);
}

#[test]
#[should_panic(expected = "copies must be in original order")]
fn source_map_refuses_copies_out_of_order() {
    let mut files = Files::default();
    let file = files.add("a.css", "0123456789abc");
    let mut b = SourceMapBuilder::new(file, "0123456789abc");
    b.copy(10..12);
    b.copy(0..3);
}

#[test]
fn fixes_apply_in_order_and_skip_unsafe_ones() {
    let mut files = Files::default();
    let file = files.add("a.md", "teh cat adn teh dog");
    let other = files.add("b.md", "teh");
    let d = Diagnostic::error(&UNKNOWN_LABEL, "typos", span(file, 0..3))
        // Given out of order, so sorting matters.
        .suggest(
            "fix",
            Applicability::MachineApplicable,
            vec![edit(file, 12..15, "the"), edit(file, 0..3, "the")],
        )
        .suggest(
            "fix",
            Applicability::MaybeIncorrect,
            vec![edit(file, 8..11, "and")],
        )
        .suggest(
            "fix",
            Applicability::MachineApplicable,
            vec![edit(other, 0..3, "the")],
        );
    assert_eq!(
        apply_fixes(&files, file, &[d]).unwrap(),
        "the cat adn the dog"
    );
}

#[test]
fn overlapping_fixes_are_refused() {
    let mut files = Files::default();
    let file = files.add("a.md", "abcdef");
    let d = Diagnostic::error(&UNKNOWN_LABEL, "x", span(file, 0..1)).suggest(
        "fix",
        Applicability::MachineApplicable,
        vec![edit(file, 0..3, "X"), edit(file, 2..4, "Y")],
    );
    assert_eq!(apply_fixes(&files, file, &[d]), Err(OverlappingEdits));
}

#[test]
fn adjacent_and_repeated_fixes_apply() {
    let mut files = Files::default();
    let file = files.add("a.md", "abcdef");
    let fix = |edits| {
        Diagnostic::error(&UNKNOWN_LABEL, "x", span(file, 0..1)).suggest(
            "fix",
            Applicability::MachineApplicable,
            edits,
        )
    };
    // Replace a token and insert right after it: touching, not overlapping.
    let adjacent = fix(vec![edit(file, 0..3, "X"), edit(file, 3..3, "+")]);
    // The same mistake reported from two Pages carries the same fix twice.
    let twice = [
        fix(vec![edit(file, 4..5, "E")]),
        fix(vec![edit(file, 4..5, "E")]),
    ];
    assert_eq!(apply_fixes(&files, file, &[adjacent]).unwrap(), "X+def");
    assert_eq!(apply_fixes(&files, file, &twice).unwrap(), "abcdEf");
}

#[test]
#[should_panic(expected = "a suggestion edits one file")]
fn a_suggestion_cannot_span_files() {
    let mut files = Files::default();
    let a = files.add("a.md", "x");
    let b = files.add("b.md", "y");
    let _ = Diagnostic::error(&UNKNOWN_LABEL, "x", span(a, 0..1)).suggest(
        "fix",
        Applicability::MachineApplicable,
        vec![edit(a, 0..1, "X"), edit(b, 0..1, "Y")],
    );
}

#[test]
fn a_secondary_span_in_another_file_gets_its_own_snippet() {
    let mut files = Files::default();
    let post = files.add("content/posts/a.md", "see \\eqref{eq:x}\n");
    let other = files.add("content/posts/b.md", "$$ x \\label{eq:x} $$\n");
    let d = Diagnostic::error(
        &UNKNOWN_LABEL,
        "`eq:x` is in another Entry",
        span(post, 11..15),
    )
    .label("referenced here")
    .secondary(span(other, 12..16), "defined here");
    let json = d.to_json(&files);
    assert_eq!(json["spans"][1]["is_primary"], false);
    assert_eq!(json["spans"][1]["file"], "content/posts/b.md");
    insta::assert_snapshot!(d.render(&files, false));
}

#[test]
fn a_panic_on_an_empty_file_renders() {
    let mut files = Files::default();
    let file = files.add("content/posts/empty.org", "");
    let d = catch_panic(&files, file, || -> () { panic!("boom") }).unwrap_err();
    insta::assert_snapshot!(d.render(&files, false));
}

#[test]
fn a_span_at_the_end_of_a_file_is_on_the_line_after_its_last_newline() {
    let mut files = Files::default();
    let file = files.add("content/posts/a.org", "#+begin_src\n");
    let d = Diagnostic::error(&UNKNOWN_LABEL, "unclosed block", span(file, 12..12));
    let json = d.to_json(&files);
    assert_eq!(
        (
            &json["spans"][0]["line_start"],
            &json["spans"][0]["column_start"]
        ),
        (&2.into(), &1.into())
    );
    insta::assert_snapshot!(d.render(&files, false));
}

#[test]
fn a_panicking_task_becomes_internal_panic_on_its_file() {
    let mut files = Files::default();
    let file = files.add("content/posts/bad.org", "* x");
    assert_eq!(catch_panic(&files, file, || 7).unwrap(), 7);
    let d = catch_panic(&files, file, || -> () { panic!("unclosed block at {}", 3) }).unwrap_err();
    assert_eq!(d.code, &codes::INTERNAL_PANIC);
    assert_eq!(d.primary().file, file);
    assert!(d.message.contains("content/posts/bad.org"));
    assert!(
        d.children
            .iter()
            .any(|c| c.message.contains("unclosed block at 3"))
    );
}

#[test]
fn every_code_is_a_unique_area_slash_kebab_name_with_docs() {
    let mut names: Vec<_> = ALL.iter().map(|c| c.name).collect();
    for code in ALL {
        let (area, name) = code.name.split_once('/').expect("area/name");
        assert!(AREAS.contains(&area), "{}: unknown area", code.name);
        assert!(
            !name.is_empty()
                && name.split('-').all(|w| !w.is_empty()
                    && w.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())),
            "{}: not kebab-case",
            code.name
        );
        assert!(
            !code.meaning.is_empty() && !code.example.is_empty() && !code.fix.is_empty(),
            "{}: missing docs",
            code.name
        );
    }
    names.sort();
    names.dedup();
    assert_eq!(names.len(), ALL.len(), "duplicate code names");
}
