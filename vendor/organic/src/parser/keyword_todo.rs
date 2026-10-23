use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_till;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::eof;
use nom::combinator::opt;
use nom::combinator::verify;
use nom::multi::separated_list0;
use nom::sequence::tuple;

use crate::error::Res;

// ref https://orgmode.org/manual/Per_002dfile-keywords.html
// ref https://orgmode.org/manual/Workflow-states.html
// Case is significant.

/// Parses the text in the value of a #+TODO keyword.
///
/// Example input: "foo bar baz | lorem ipsum"
pub(crate) fn todo_keywords(input: &str) -> Res<&str, (Vec<&str>, Vec<&str>)> {
    let (remaining, mut before_pipe_words) = separated_list0(space1, todo_keyword_word)(input)?;
    let (remaining, after_pipe_words) = opt(tuple((
        tuple((space0, tag("|"), space0)),
        separated_list0(space1, todo_keyword_word),
    )))(remaining)?;
    let (remaining, _eol) = alt((line_ending, eof))(remaining)?;
    if let Some((_pipe, after_pipe_words)) = after_pipe_words {
        Ok((remaining, (before_pipe_words, after_pipe_words)))
    } else if !before_pipe_words.is_empty() {
        // If there was no pipe, then the last word becomes a completion state instead.
        let after_pipe_words = vec![before_pipe_words
            .pop()
            .expect("If-statement proves this is Some.")];
        Ok((remaining, (before_pipe_words, after_pipe_words)))
    } else {
        // No words founds
        Ok((remaining, (Vec::new(), Vec::new())))
    }
}

fn todo_keyword_word(input: &str) -> Res<&str, &str> {
    let (remaining, keyword) = verify(take_till(|c| "( \t\r\n|".contains(c)), |result: &str| {
        !result.is_empty()
    })(input)?;

    let (remaining, _) = opt(tuple((
        tag("("),
        take_till(|c| "() \t\r\n|".contains(c)),
        tag(")"),
    )))(remaining)?;

    Ok((remaining, keyword))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_and_after() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo bar baz | lorem ipsum";
        let (remaining, (before_pipe_words, after_pipe_words)) = todo_keywords(input)?;
        assert_eq!(remaining, "");
        assert_eq!(before_pipe_words, vec!["foo", "bar", "baz"]);
        assert_eq!(after_pipe_words, vec!["lorem", "ipsum"]);
        Ok(())
    }

    #[test]
    fn no_pipe() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo bar baz";
        let (remaining, (before_pipe_words, after_pipe_words)) = todo_keywords(input)?;
        assert_eq!(remaining, "");
        assert_eq!(before_pipe_words, vec!["foo", "bar"]);
        assert_eq!(after_pipe_words, vec!["baz"]);
        Ok(())
    }

    #[test]
    fn early_pipe() -> Result<(), Box<dyn std::error::Error>> {
        let input = "| foo bar baz";
        let (remaining, (before_pipe_words, after_pipe_words)) = todo_keywords(input)?;
        assert_eq!(remaining, "");
        assert_eq!(before_pipe_words, Vec::<&str>::new());
        assert_eq!(after_pipe_words, vec!["foo", "bar", "baz"]);
        Ok(())
    }

    #[test]
    fn late_pipe() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo bar baz |";
        let (remaining, (before_pipe_words, after_pipe_words)) = todo_keywords(input)?;
        assert_eq!(remaining, "");
        assert_eq!(before_pipe_words, vec!["foo", "bar", "baz"]);
        assert_eq!(after_pipe_words, Vec::<&str>::new());
        Ok(())
    }
}
