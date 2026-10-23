// ref https://orgmode.org/manual/Per_002dfile-keywords.html
// ref https://orgmode.org/manual/Workflow-states.html
// Case is significant.

/// Reads the value of a #+TODO keyword as Org 9.8.7's
/// `org-set-regexps-and-options` does: words split on whitespace, the
/// first `|` word between the states in progress and the done ones (or the
/// last word done when there is none), and a `(key)` suffix dropped.
/// Upstream parsed it with a grammar that failed the whole document on a
/// value Org accepts, such as `(` or `A(b)c`.
///
/// Example input: "foo bar baz | lorem ipsum"
pub(crate) fn todo_keywords(input: &str) -> (Vec<&str>, Vec<&str>) {
    let words: Vec<&str> = input.split_whitespace().collect();
    let (in_progress, done) = match words.iter().position(|word| *word == "|") {
        Some(pipe) => (&words[..pipe], &words[pipe + 1..]),
        None => match words.split_last() {
            Some((last, rest)) => (rest, std::slice::from_ref(last)),
            None => (&words[..], &words[..]),
        },
    };
    (names(in_progress), names(done))
}

fn names<'s>(words: &[&'s str]) -> Vec<&'s str> {
    words
        .iter()
        .filter(|word| **word != "|")
        .map(|word| without_key(word))
        .collect()
}

/// What `^\\(.*?\\)\\(?:(\\([^!@/]\\)?.*?)\\)?$` captures first: the word
/// before a final parenthesised suffix, such as `WAIT` in `WAIT(w@/!)`.
fn without_key(word: &str) -> &str {
    match word.find('(') {
        Some(open) if word.ends_with(')') && open < word.len() - 1 => &word[..open],
        _ => word,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_and_after() {
        assert_eq!(
            todo_keywords("foo bar baz | lorem ipsum"),
            (vec!["foo", "bar", "baz"], vec!["lorem", "ipsum"])
        );
    }

    #[test]
    fn no_pipe() {
        assert_eq!(
            todo_keywords("foo bar baz"),
            (vec!["foo", "bar"], vec!["baz"])
        );
    }

    #[test]
    fn early_pipe() {
        assert_eq!(
            todo_keywords("| foo bar baz"),
            (vec![], vec!["foo", "bar", "baz"])
        );
    }

    #[test]
    fn late_pipe() {
        assert_eq!(
            todo_keywords("foo bar baz |"),
            (vec!["foo", "bar", "baz"], vec![])
        );
    }

    #[test]
    fn keys_are_dropped_and_odd_words_kept() {
        assert_eq!(
            todo_keywords("TODO(t) WAIT(w@/!) ( A(b)c || | DONE(d!) |"),
            (vec!["TODO", "WAIT", "(", "A(b)c", "||"], vec!["DONE"])
        );
    }
}
