use std::borrow::Cow;

/// Removes all whitespace from a string if any line breaks are present.
///
/// Example: "foo  bar" => "foo  bar" but "foo \n bar" => "foobar".
pub(crate) fn remove_whitespace_if_line_break(input: &str) -> Cow<'_, str> {
    let mut state = RemoveWhitespaceIfLineBreakState::Normal;
    for (offset, c) in input.char_indices() {
        match (&mut state, c) {
            (RemoveWhitespaceIfLineBreakState::Normal, '\n') => {
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..offset]);
                state = RemoveWhitespaceIfLineBreakState::HasLineBreak(ret);
            }
            (RemoveWhitespaceIfLineBreakState::Normal, ' ' | '\t') => {
                state = RemoveWhitespaceIfLineBreakState::HasWhitespace(offset);
            }
            (RemoveWhitespaceIfLineBreakState::Normal, _) => {}
            (RemoveWhitespaceIfLineBreakState::HasWhitespace(first_whitespace_offset), '\n') => {
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..*first_whitespace_offset]);
                for c in input[*first_whitespace_offset..offset].chars() {
                    if !c.is_ascii_whitespace() {
                        ret.push(c);
                    }
                }
                state = RemoveWhitespaceIfLineBreakState::HasLineBreak(ret);
            }
            (RemoveWhitespaceIfLineBreakState::HasWhitespace(_), _) => {}
            (RemoveWhitespaceIfLineBreakState::HasLineBreak(_), ' ' | '\t' | '\r' | '\n') => {}
            (RemoveWhitespaceIfLineBreakState::HasLineBreak(ret), _) => {
                ret.push(c);
            }
        }
    }
    match state {
        RemoveWhitespaceIfLineBreakState::Normal => Cow::Borrowed(input),
        RemoveWhitespaceIfLineBreakState::HasWhitespace(_) => Cow::Borrowed(input),
        RemoveWhitespaceIfLineBreakState::HasLineBreak(ret) => Cow::Owned(ret),
    }
}

enum RemoveWhitespaceIfLineBreakState {
    Normal,
    HasWhitespace(usize),
    HasLineBreak(String),
}

/// Removes all line breaks from a string
///
/// Example: "foo bar" => "foo bar" but "foo \n bar" => "foo  bar".
pub(crate) fn remove_line_break(input: &str) -> Cow<'_, str> {
    let mut state = RemoveLineBreakState::Normal;
    for (offset, c) in input.char_indices() {
        match (&mut state, c) {
            (RemoveLineBreakState::Normal, '\n') => {
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..offset]);
                state = RemoveLineBreakState::HasLineBreak(ret);
            }
            (RemoveLineBreakState::Normal, _) => {}
            (RemoveLineBreakState::HasLineBreak(_), '\n') => {}
            (RemoveLineBreakState::HasLineBreak(ret), _) => {
                ret.push(c);
            }
        }
    }
    match state {
        RemoveLineBreakState::Normal => Cow::Borrowed(input),
        RemoveLineBreakState::HasLineBreak(ret) => Cow::Owned(ret),
    }
}

enum RemoveLineBreakState {
    Normal,
    HasLineBreak(String),
}

/// Removes all whitespace from a string if any line breaks are present.
///
/// Example: "foo  bar" => "foo  bar" but "foo \n bar" => "foo bar".
pub(crate) fn coalesce_whitespace_if_line_break(input: &str) -> Cow<'_, str> {
    let mut state = CoalesceWhitespaceIfLineBreakState::Normal;
    for (offset, c) in input.char_indices() {
        match (&mut state, c) {
            (CoalesceWhitespaceIfLineBreakState::Normal, '\n') => {
                // Hit line break without any preceding whitespace
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..offset]);
                ret.push(' ');
                state = CoalesceWhitespaceIfLineBreakState::HasLineBreak {
                    in_whitespace: true,
                    ret,
                };
            }
            (CoalesceWhitespaceIfLineBreakState::Normal, ' ' | '\t') => {
                state = CoalesceWhitespaceIfLineBreakState::HasWhitespace {
                    in_whitespace: true,
                    first_whitespace_offset: offset,
                };
            }
            (CoalesceWhitespaceIfLineBreakState::Normal, _) => {}

            (
                CoalesceWhitespaceIfLineBreakState::HasWhitespace {
                    in_whitespace,
                    first_whitespace_offset,
                },
                '\n',
            ) => {
                // Hit line break with preceding whitespace so we add all the text up to the first whitespace and then process the remaining text coalescing the whitespace.
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..*first_whitespace_offset]);
                let mut sub_loop_in_whitespace = false;
                for c in input[*first_whitespace_offset..offset].chars() {
                    if sub_loop_in_whitespace {
                        if !c.is_ascii_whitespace() {
                            // Preceding character was whitespace but this is not.
                            sub_loop_in_whitespace = false;
                            ret.push(c);
                        }
                        // Do nothing if preceding character was whitespace and this character also is whitespace.
                    } else if c.is_ascii_whitespace() {
                        // Preceding character was not whitespace but this is.
                        sub_loop_in_whitespace = true;
                        ret.push(' ');
                    } else {
                        // Preceding character was not whitespace and this is not either.
                        ret.push(c);
                    }
                }
                if !*in_whitespace {
                    // If this line break was the start of whitespace then we need to inject a space character for it.
                    ret.push(' ');
                }
                state = CoalesceWhitespaceIfLineBreakState::HasLineBreak {
                    in_whitespace: true, // This was triggered by a line break which is whitespace.
                    ret,
                };
            }
            (
                CoalesceWhitespaceIfLineBreakState::HasWhitespace {
                    in_whitespace,
                    first_whitespace_offset: _,
                },
                ' ' | '\t',
            ) => {
                *in_whitespace = true;
            }
            (
                CoalesceWhitespaceIfLineBreakState::HasWhitespace {
                    in_whitespace,
                    first_whitespace_offset: _,
                },
                _,
            ) => {
                *in_whitespace = false;
            }
            (
                CoalesceWhitespaceIfLineBreakState::HasLineBreak { in_whitespace, ret },
                ' ' | '\t' | '\r' | '\n',
            ) => {
                if !*in_whitespace {
                    ret.push(' ');
                }
                *in_whitespace = true;
            }
            (CoalesceWhitespaceIfLineBreakState::HasLineBreak { in_whitespace, ret }, _) => {
                *in_whitespace = false;
                ret.push(c);
            }
        }
    }
    match state {
        CoalesceWhitespaceIfLineBreakState::Normal => Cow::Borrowed(input),
        CoalesceWhitespaceIfLineBreakState::HasWhitespace {
            in_whitespace: _,
            first_whitespace_offset: _,
        } => Cow::Borrowed(input),
        CoalesceWhitespaceIfLineBreakState::HasLineBreak {
            in_whitespace: _,
            ret,
        } => Cow::Owned(ret),
    }
}

enum CoalesceWhitespaceIfLineBreakState {
    Normal,
    HasWhitespace {
        in_whitespace: bool,
        first_whitespace_offset: usize,
    },
    HasLineBreak {
        in_whitespace: bool,
        ret: String,
    },
}

/// Removes all whitespace from a string.
///
/// Example: "foo bar" => "foobar" and "foo \n bar" => "foobar".
#[allow(dead_code)]
pub(crate) fn coalesce_whitespace(input: &str) -> Cow<'_, str> {
    let mut state = CoalesceWhitespace::Normal;
    for (offset, c) in input.char_indices() {
        match (&mut state, c) {
            (CoalesceWhitespace::Normal, ' ' | '\t' | '\r' | '\n') => {
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..offset]);
                ret.push(' ');
                state = CoalesceWhitespace::HasWhitespace {
                    in_whitespace: true,
                    ret,
                };
            }
            (CoalesceWhitespace::Normal, _) => {}
            (
                CoalesceWhitespace::HasWhitespace { in_whitespace, ret },
                ' ' | '\t' | '\r' | '\n',
            ) => {
                if !*in_whitespace {
                    *in_whitespace = true;
                    ret.push(' ');
                }
            }
            (CoalesceWhitespace::HasWhitespace { in_whitespace, ret }, _) => {
                *in_whitespace = false;
                ret.push(c);
            }
        }
    }
    match state {
        CoalesceWhitespace::Normal => Cow::Borrowed(input),
        CoalesceWhitespace::HasWhitespace {
            in_whitespace: _,
            ret,
        } => Cow::Owned(ret),
    }
}

enum CoalesceWhitespace {
    Normal,
    HasWhitespace { in_whitespace: bool, ret: String },
}

/// Removes all whitespace from a string and handle escaping characters.
///
/// Example: "foo bar" => "foobar" and "foo \n bar" => "foobar" but if the escape character is backslash and comma is an escapable character than "foo\,bar" becomes "foo,bar".
pub(crate) fn coalesce_whitespace_escaped<C: Fn(char) -> bool>(
    escape_character: char,
    escapable_characters: C,
) -> impl for<'s> Fn(&'s str) -> Cow<'s, str> {
    move |input| impl_coalesce_whitespace_escaped(input, escape_character, &escapable_characters)
}

fn impl_coalesce_whitespace_escaped<C: Fn(char) -> bool>(
    input: &str,
    escape_character: char,
    escapable_characters: C,
) -> Cow<'_, str> {
    let mut state = CoalesceWhitespaceEscaped::Normal {
        in_whitespace: false,
    };
    for (offset, c) in input.char_indices() {
        state = match (state, c) {
            (CoalesceWhitespaceEscaped::Normal { in_whitespace: _ }, c)
                if c == escape_character =>
            {
                CoalesceWhitespaceEscaped::NormalEscaping {
                    escape_offset: offset,
                }
            }
            (CoalesceWhitespaceEscaped::Normal { in_whitespace }, ' ') => {
                if in_whitespace {
                    let mut ret = String::with_capacity(input.len());
                    ret.push_str(&input[..offset]);
                    CoalesceWhitespaceEscaped::RequiresMutation {
                        in_whitespace: true,
                        ret,
                    }
                } else {
                    CoalesceWhitespaceEscaped::Normal {
                        in_whitespace: true,
                    }
                }
            }
            (CoalesceWhitespaceEscaped::Normal { in_whitespace: _ }, '\t' | '\r' | '\n') => {
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..offset]);
                ret.push(' ');
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: true,
                    ret,
                }
            }
            (CoalesceWhitespaceEscaped::Normal { in_whitespace: _ }, _) => {
                CoalesceWhitespaceEscaped::Normal {
                    in_whitespace: false,
                }
            }
            (CoalesceWhitespaceEscaped::NormalEscaping { escape_offset }, c)
                if escapable_characters(c) =>
            {
                // We escaped a character so we need mutation
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..escape_offset]);
                ret.push(c);
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: false,
                    ret,
                }
            }

            (CoalesceWhitespaceEscaped::NormalEscaping { escape_offset: _ }, ' ') => {
                // We didn't escape the character so continue as normal.
                CoalesceWhitespaceEscaped::Normal {
                    in_whitespace: true,
                }
            }
            (
                CoalesceWhitespaceEscaped::NormalEscaping { escape_offset: _ },
                '\t' | '\r' | '\n',
            ) => {
                // We didn't escape the character but we hit whitespace anyway.
                let mut ret = String::with_capacity(input.len());
                ret.push_str(&input[..offset]);
                ret.push(' ');
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: true,
                    ret,
                }
            }
            (CoalesceWhitespaceEscaped::NormalEscaping { escape_offset: _ }, _) => {
                // We didn't escape the character so continue as normal.
                CoalesceWhitespaceEscaped::Normal {
                    in_whitespace: false,
                }
            }

            (
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: _,
                    ret,
                },
                c,
            ) if c == escape_character => CoalesceWhitespaceEscaped::RequiresMutationEscaping {
                ret,
                matched_escape_character: c,
            },
            (
                CoalesceWhitespaceEscaped::RequiresMutation {
                    mut in_whitespace,
                    mut ret,
                },
                ' ' | '\t' | '\r' | '\n',
            ) => {
                if !in_whitespace {
                    in_whitespace = true;
                    ret.push(' ');
                }
                CoalesceWhitespaceEscaped::RequiresMutation { in_whitespace, ret }
            }
            (
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: _,
                    mut ret,
                },
                _,
            ) => {
                ret.push(c);
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: false,
                    ret,
                }
            }
            (
                CoalesceWhitespaceEscaped::RequiresMutationEscaping {
                    mut ret,
                    matched_escape_character: _,
                },
                c,
            ) if escapable_characters(c) => {
                ret.push(c);
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: false,
                    ret,
                }
            }
            (
                CoalesceWhitespaceEscaped::RequiresMutationEscaping {
                    mut ret,
                    matched_escape_character: _,
                },
                ' ' | '\t' | '\r' | '\n',
            ) => {
                ret.push(' ');
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: true,
                    ret,
                }
            }
            (
                CoalesceWhitespaceEscaped::RequiresMutationEscaping {
                    mut ret,
                    matched_escape_character,
                },
                c,
            ) => {
                ret.push(matched_escape_character);
                ret.push(c);
                CoalesceWhitespaceEscaped::RequiresMutation {
                    in_whitespace: false,
                    ret,
                }
            }
        }
    }
    match state {
        CoalesceWhitespaceEscaped::Normal { in_whitespace: _ } => Cow::Borrowed(input),
        CoalesceWhitespaceEscaped::NormalEscaping { escape_offset: _ } => Cow::Borrowed(input),
        CoalesceWhitespaceEscaped::RequiresMutation {
            in_whitespace: _,
            ret,
        } => Cow::Owned(ret),
        CoalesceWhitespaceEscaped::RequiresMutationEscaping {
            mut ret,
            matched_escape_character,
        } => {
            ret.push(matched_escape_character);
            Cow::Owned(ret)
        }
    }
}

enum CoalesceWhitespaceEscaped {
    Normal {
        in_whitespace: bool,
    },
    NormalEscaping {
        escape_offset: usize,
    },
    RequiresMutation {
        in_whitespace: bool,
        ret: String,
    },
    RequiresMutationEscaping {
        ret: String,
        matched_escape_character: char,
    },
}

pub(crate) fn to_lowercase(input: &str) -> Cow<'_, str> {
    if input.chars().any(|c| !c.is_lowercase()) {
        Cow::Owned(input.to_lowercase())
    } else {
        Cow::Borrowed(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesce_whitespace_escaped_default() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foobarbaz";
        let output = coalesce_whitespace_escaped('&', |c| "".contains(c))(input);
        assert_eq!(output, "foobarbaz");
        assert!(matches!(output, Cow::Borrowed(_)));
        Ok(())
    }

    #[test]
    fn coalesce_whitespace_escaped_whitespace_single() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo bar baz";
        let output = coalesce_whitespace_escaped('&', |c| "".contains(c))(input);
        assert_eq!(output, "foo bar baz");
        assert!(matches!(output, Cow::Borrowed(_)));
        Ok(())
    }

    #[test]
    fn coalesce_whitespace_escaped_whitespace_double() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo  bar  baz";
        let output = coalesce_whitespace_escaped('&', |c| "".contains(c))(input);
        assert_eq!(output, "foo bar baz");
        assert!(matches!(output, Cow::Owned(_)));
        Ok(())
    }

    #[test]
    fn coalesce_whitespace_escaped_escape_match() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo  &bar  baz";
        let output = coalesce_whitespace_escaped('&', |c| "b".contains(c))(input);
        assert_eq!(output, "foo bar baz");
        assert!(matches!(output, Cow::Owned(_)));
        Ok(())
    }

    #[test]
    fn coalesce_whitespace_escaped_escape_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo  b&ar  baz";
        let output = coalesce_whitespace_escaped('&', |c| "b".contains(c))(input);
        assert_eq!(output, "foo b&ar baz");
        assert!(matches!(output, Cow::Owned(_)));
        Ok(())
    }

    #[test]
    fn coalesce_whitespace_escaped_escape_mismatch_around_whitespace(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input = "foo& bar &baz";
        let output = coalesce_whitespace_escaped('&', |c| "z".contains(c))(input);
        assert_eq!(output, "foo& bar &baz");
        assert!(matches!(output, Cow::Borrowed(_)));
        Ok(())
    }

    #[test]
    fn test_coalesce_whitespace_if_line_break() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(coalesce_whitespace_if_line_break("foo  bar"), "foo  bar");
        assert_eq!(coalesce_whitespace_if_line_break("foo \n bar"), "foo bar");
        Ok(())
    }

    #[test]
    fn test_remove_whitespace_if_line_break() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(remove_whitespace_if_line_break("foo  bar"), "foo  bar");
        assert_eq!(remove_whitespace_if_line_break("foo \n bar"), "foobar");
        Ok(())
    }

    #[test]
    fn test_remove_line_break() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(remove_line_break("foo  bar"), "foo  bar");
        assert_eq!(remove_line_break("foo \n bar"), "foo  bar");
        Ok(())
    }
}
