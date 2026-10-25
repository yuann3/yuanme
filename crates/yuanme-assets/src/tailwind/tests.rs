use std::path::Path;

use super::*;

fn rewritten(css: &str) -> Option<String> {
    input(css, Path::new("/tmp/tokens.txt"))
}

#[test]
fn an_import_of_tailwindcss_turns_it_on_and_stops_it_scanning_the_project() {
    assert_eq!(
        rewritten("@import \"tailwindcss\";\n.card { @apply p-4; }\n").as_deref(),
        Some(
            "@import \"tailwindcss\" source(none);\n.card { @apply p-4; }\n\n@source \"/tmp/tokens.txt\";\n"
        )
    );
    // The Author's own `source()` would scan a folder: it goes, and the
    // import's other conditions stay.
    assert_eq!(
        rewritten("@IMPORT 'tailwindcss' layer(x) source(\"../src\") prefix(tw)").as_deref(),
        Some(
            "@IMPORT 'tailwindcss' source(none) layer(x) prefix(tw);\n@source \"/tmp/tokens.txt\";\n"
        )
    );
    // Its parts, imported one by one: only the utilities scan.
    assert_eq!(
        rewritten(
            "@import \"tailwindcss/theme.css\" layer(theme);\n@import \"tailwindcss/utilities.css\" layer(utilities) source(none);\n"
        )
        .as_deref(),
        Some(
            "@import \"tailwindcss/theme.css\" layer(theme);\n@import \"tailwindcss/utilities.css\" source(none) layer(utilities);\n\n@source \"/tmp/tokens.txt\";\n"
        )
    );
}

#[test]
fn a_stylesheet_without_that_import_is_plain_css() {
    for css in [
        "/* @import \"tailwindcss\"; */ a { color: red }",
        "@import \"./tailwindcss.css\";",
        "@import \"tailwindcss-animate\";",
        "a::before { content: '@import \"tailwindcss\";' }",
        "@importx \"tailwindcss\";",
    ] {
        assert_eq!(rewritten(css), None, "{css}");
    }
}

#[test]
fn the_token_file_holds_each_token_once_in_order() {
    let file = tokens([
        "px-4  hover:bg-sky-500\nflex",
        "flex",
        "a { color: var(--color-red-500); }",
    ]);
    assert_eq!(
        file,
        "a\ncolor:\nflex\nhover:bg-sky-500\npx-4\nvar(--color-red-500);\n{\n}\n"
    );
}

/// The pinned release is checked against the checksums the Engine
/// embeds; one the Author chose, against its own list.
#[test]
fn the_pinned_release_has_its_checksum_embedded() {
    let Some(pinned) = tool(VERSION) else {
        return;
    };
    let Sha256::Known(hex) = pinned.sha256 else {
        panic!("{VERSION} isn't pinned")
    };
    let asset = pinned.url.rsplit('/').next().unwrap();
    assert!(SHA256S.contains(&(asset, hex.as_str())));
    let Some(Sha256::Listed(list)) = tool("4.0.0").map(|t| t.sha256) else {
        panic!("4.0.0 has a checksum embedded")
    };
    assert_eq!(
        list,
        "https://github.com/tailwindlabs/tailwindcss/releases/download/v4.0.0/sha256sums.txt"
    );
}
