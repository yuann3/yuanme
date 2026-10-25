use super::*;

#[test]
fn block_directives() {
    html_opts!(
        [extension.block_directive],
        concat!(
            ":::foo bar\n",
            "Paragraph 1\n",
            "\n",
            "Paragraph 2\n",
            ":::\n"
        ),
        concat!(
            "<div class=\"foo bar\">\n",
            "<p>Paragraph 1</p>\n",
            "<p>Paragraph 2</p>\n",
            "</div>\n",
        ),
    );

    html_opts!(
        [extension.block_directive],
        concat!(
            "- item one\n",
            "\n",
            "  :::foo bar\n",
            "  Paragraph 1\n",
            "\n",
            "  Paragraph 2\n",
            "  :::\n",
            "- item two\n"
        ),
        concat!(
            "<ul>\n",
            "<li>\n",
            "<p>item one</p>\n",
            "<div class=\"foo bar\">\n",
            "<p>Paragraph 1</p>\n",
            "<p>Paragraph 2</p>\n",
            "</div>\n",
            "</li>\n",
            "<li>\n",
            "<p>item two</p>\n",
            "</li>\n",
            "</ul>\n",
        ),
    );
}

#[test]
fn sourcepos() {
    assert_ast_match!(
        [extension.block_directive],
        "- item one\n"
        "\n"
        "  :::foo bar\n"
        "  Paragraph 1\n"
        "  :::\n"
        "- item two\n",
        (document (1:1-6:10) [
            (list (1:1-6:10) [
                (item (1:1-5:5) [
                    (paragraph (1:3-1:10) [
                        (text (1:3-1:10) "item one")
                    ])
                    (block_directive (3:3-5:5) [
                        (paragraph (4:3-4:13) [
                            (text (4:3-4:13) "Paragraph 1")
                        ])
                    ])
                ])
                (item (6:1-6:10) [
                    (paragraph (6:3-6:10) [
                        (text (6:3-6:10) "item two")
                    ])
                ])
            ])
        ])
    );
}

#[test]
fn sourcepos_with_block_quote() {
    assert_ast_match!(
        [extension.block_directive],
        ":::foo bar\n"
        "> a\n"
        ":::\n",
        (document (1:1-3:3) [
            (block_directive (1:1-3:3) [
                (block_quote (2:1-2:3) [
                    (paragraph (2:3-2:3) [
                        (text (2:3-2:3) "a")
                    ])
                ])
            ])
        ])
    );
}

#[test]
fn sourcepos_with_list() {
    assert_ast_match!(
        [extension.block_directive],
        ":::foo bar\n"
        "* a\n"
        ":::\n",
        (document (1:1-3:3) [
            (block_directive (1:1-3:3) [
                (list (2:1-2:3) [
                    (item (2:1-2:3) [
                        (paragraph (2:3-2:3) [
                            (text (2:3-2:3) "a")
                        ])
                    ])
                ])
            ])
        ])
    );
}

#[test]
fn sourcepos_with_list_ends_at_the_items_last_byte() {
    // The item and its paragraph end where their line does, not at their
    // start, which split the `é` they end in.
    assert_ast_match!(
        [extension.block_directive],
        ":::foo\n"
        "1. aé\n"
        ":::\n",
        (document (1:1-3:3) [
            (block_directive (1:1-3:3) [
                (list (2:1-2:6) [
                    (item (2:1-2:6) [
                        (paragraph (2:4-2:6) [
                            (text (2:4-2:6) "aé")
                        ])
                    ])
                ])
            ])
        ])
    );
}

#[test]
fn sourcepos_in_blockquote() {
    assert_ast_match!(
        [extension.block_directive],
        "> :::foo bar\n"
        "> a\n"
        "> :::\n",
        (document (1:1-3:5) [
            (block_quote (1:1-3:5) [
                (block_directive (1:3-3:5) [
                    (paragraph (2:3-2:3) [
                        (text (2:3-2:3) "a")
                    ])
                ])
            ])
        ])
    );
}

#[test]
fn info_with_colons() {
    html_opts!(
        [extension.block_directive],
        concat!("::::theorem[Proof: one]{#thm:a}\n", "a\n", "::::\n",),
        concat!(
            "<div class=\"theorem[Proof: one]{#thm:a}\">\n",
            "<p>a</p>\n",
            "</div>\n",
        ),
    );
}
