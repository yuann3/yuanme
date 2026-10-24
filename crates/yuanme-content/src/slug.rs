//! The shared slugger (#18), used for file names here and heading ids later.
//! Letters stay letters in any script, lowercased; a URL percent-encodes
//! what isn't ASCII when it is written out.

/// github-slugger's rule: lowercase, spaces become `-`, and only letters,
/// digits, `-` and `_` are kept. `Foo Bar` → `foo-bar`.
pub fn slug(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            ' ' => vec!['-'],
            '-' | '_' => vec![c],
            c if c.is_alphanumeric() => c.to_lowercase().collect(),
            _ => vec![],
        })
        .collect()
}

/// A Term's Slug (ADR 0005): the symbols that carry meaning are spelled out
/// first, so `C++` → `cplusplus` and `C#` → `csharp` instead of both `c`.
pub fn term_slug(term: &str) -> String {
    let term = term.trim();
    let spelled = match term.strip_prefix('.') {
        Some(rest) => format!("dot{rest}"),
        None => term.to_string(),
    };
    slug(&spelled.replace('+', "plus").replace('#', "sharp"))
}

/// Whether `s` is already a valid Slug: segments separated by `/`, each one
/// non-empty and unchanged by [`slug`].
pub fn is_valid(s: &str) -> bool {
    !s.is_empty() && s.split('/').all(|seg| !seg.is_empty() && slug(seg) == seg)
}

/// The Slug `s` most likely meant, for the fix on an invalid one.
pub fn corrected(s: &str) -> String {
    let segments: Vec<String> = s
        .split('/')
        .map(slug)
        .filter(|seg| !seg.is_empty())
        .collect();
    segments.join("/")
}

/// A URL path with everything outside unreserved ASCII and `/`
/// percent-encoded in uppercase, which every host tested serves without a
/// redirect (ADR 0005).
pub fn url_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_slug_like_github_slugger() {
        assert_eq!(slug("Foo Bar"), "foo-bar");
        assert_eq!(slug("Pew"), "pew");
        assert_eq!(slug("Rust's 2024 Edition!"), "rusts-2024-edition");
        assert_eq!(slug("snake_case-and-kebab"), "snake_case-and-kebab");
        assert_eq!(slug("Café 中文"), "café-中文");
    }

    #[test]
    fn term_slugs_spell_out_the_symbols_that_carry_meaning() {
        // ADR 0005: plain github-slugger files C++ and C# under `c`.
        assert_eq!(term_slug("C++"), "cplusplus");
        assert_eq!(term_slug("c++"), "cplusplus");
        assert_eq!(term_slug("C#"), "csharp");
        assert_eq!(term_slug("F#"), "fsharp");
        assert_eq!(term_slug(".NET"), "dotnet");
        assert_eq!(term_slug("Node.js"), "nodejs");
        assert_eq!(term_slug("Rust"), "rust");
        assert_eq!(term_slug("Machine Learning"), "machine-learning");
        assert_eq!(term_slug("!!!"), "");
    }

    #[test]
    fn urls_percent_encode_non_ascii_in_uppercase() {
        assert_eq!(url_path("/tags/中文/"), "/tags/%E4%B8%AD%E6%96%87/");
        assert_eq!(url_path("/blog/a-b_c.d/"), "/blog/a-b_c.d/");
    }

    #[test]
    fn an_override_must_already_be_a_slug() {
        assert!(is_valid("2025/foo-bar"));
        assert!(!is_valid("My Post"));
        assert!(!is_valid("a//b"));
        assert!(!is_valid("/a"));
        assert_eq!(corrected("My Post/Two"), "my-post/two");
    }
}
