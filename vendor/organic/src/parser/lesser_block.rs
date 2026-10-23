use nom::branch::alt;
use nom::bytes::complete::is_not;
use nom::bytes::complete::tag;
use nom::bytes::complete::tag_no_case;
use nom::character::complete::anychar;
use nom::character::complete::line_ending;
use nom::character::complete::space0;
use nom::character::complete::space1;
use nom::combinator::consumed;
use nom::combinator::eof;
use nom::combinator::map;
use nom::combinator::opt;
use nom::combinator::peek;
use nom::combinator::recognize;
use nom::combinator::verify;
use nom::multi::many_till;
use nom::sequence::tuple;
use nom::InputTake;

use super::affiliated_keyword::parse_affiliated_keywords;
use super::closer::expect_end;
use super::closer::End;
use super::org_source::OrgSource;
use super::util::maybe_consume_trailing_whitespace_if_not_exiting;
use crate::context::parser_with_context;
use crate::context::ContextElement;
use crate::context::ContextMatcher;
use crate::context::ExitClass;
use crate::context::ExitMatcherNode;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::object_parser::standard_set_object;
use crate::parser::util::blank_line;
use crate::parser::util::exit_matcher_parser;
use crate::parser::util::get_consumed;
use crate::parser::util::start_of_line;
use crate::parser::util::text_until_exit;
use crate::types::CharOffsetInLine;
use crate::types::CommentBlock;
use crate::types::ExampleBlock;
use crate::types::ExportBlock;
use crate::types::Keyword;
use crate::types::LineNumber;
use crate::types::Object;
use crate::types::PlainText;
use crate::types::RetainLabels;
use crate::types::SrcBlock;
use crate::types::SwitchNumberLines;
use crate::types::VerseBlock;

pub(crate) fn verse_block<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, VerseBlock<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, _) = lesser_block_begin("verse")(context, remaining)?;
    let (remaining, parameters) = opt(tuple((space1, data)))(remaining)?;
    let (remaining, _nl) = recognize(tuple((space0, line_ending)))(remaining)?;
    let lesser_block_end_specialized = lesser_block_end("verse");
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("lesser block"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &lesser_block_end_specialized,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let parameters = parameters.map(|(_ws, parameters)| parameters);

    let object_matcher = parser_with_context!(standard_set_object)(&parser_context);
    let exit_matcher = parser_with_context!(exit_matcher_parser)(&parser_context);
    // Check for a completely empty block
    let (remaining, contents, children) =
        match consumed(many_till(blank_line, exit_matcher))(remaining) {
            Ok((remaining, (whitespace, (_children, _exit_contents)))) => (
                remaining,
                whitespace,
                if whitespace.len() > 0 {
                    vec![Object::PlainText(PlainText {
                        source: whitespace.into(),
                    })]
                } else {
                    Vec::new()
                },
            ),
            Err(_) => {
                let (remaining, (contents, (children, _exit_contents))) =
                    consumed(many_till(object_matcher, exit_matcher))(remaining)?;
                (remaining, contents, children)
            }
        };
    let (remaining, _end) = lesser_block_end_specialized(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        VerseBlock {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            data: parameters.map(Into::<&str>::into),
            children,
            contents: Into::<&str>::into(contents),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn comment_block<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, CommentBlock<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, _) = lesser_block_begin("comment")(context, remaining)?;
    let (remaining, _parameters) = opt(tuple((space1, data)))(remaining)?;
    let (remaining, _nl) = recognize(tuple((space0, line_ending)))(remaining)?;
    let lesser_block_end_specialized = lesser_block_end("comment");
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("lesser block"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &lesser_block_end_specialized,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);

    let (remaining, contents) = parser_with_context!(text_until_exit)(&parser_context)(remaining)?;
    let (remaining, _end) = lesser_block_end_specialized(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        CommentBlock {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            contents: contents.into(),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn example_block<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ExampleBlock<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, _) = lesser_block_begin("example")(context, remaining)?;
    let (remaining, parameters) = opt(alt((
        map(tuple((space1, example_switches)), |(_, switches)| switches),
        map(space1, |ws: OrgSource<'_>| {
            let source = ws.take(0);
            ExampleSrcSwitches {
                source: source.into(),
                number_lines: None,
                retain_labels: RetainLabels::Yes,
                preserve_indent: None,
                use_labels: true,
                label_format: None,
            }
        }),
    )))(remaining)?;
    let (remaining, _nl) = recognize(tuple((space0, line_ending)))(remaining)?;
    let lesser_block_end_specialized = lesser_block_end("example");
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("lesser block"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &lesser_block_end_specialized,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);

    let (remaining, contents) = text_until_exit(&parser_context, remaining)?;
    let (remaining, _end) = lesser_block_end_specialized(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    let (switches, number_lines, preserve_indent, retain_labels, use_labels, label_format) = {
        if let Some(parameters) = parameters {
            (
                Some(parameters.source),
                parameters.number_lines,
                parameters.preserve_indent,
                parameters.retain_labels,
                parameters.use_labels,
                parameters.label_format,
            )
        } else {
            (None, None, None, RetainLabels::Yes, true, None)
        }
    };
    Ok((
        remaining,
        ExampleBlock {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            switches,
            number_lines,
            preserve_indent,
            retain_labels,
            use_labels,
            label_format,
            value: Into::<&str>::into(contents),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn export_block<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ExportBlock<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, _) = lesser_block_begin("export")(context, remaining)?;
    // https://orgmode.org/worg/org-syntax.html#Blocks claims that export blocks must have a single word for data but testing shows no data and multi-word data still parses as an export block.
    let (remaining, export_type) = opt(map(
        tuple((space1, switch_word, peek(tuple((space0, line_ending))))),
        |(_, export_type, _)| export_type,
    ))(remaining)?;
    let (remaining, parameters) =
        opt(map(tuple((space1, data)), |(_, parameters)| parameters))(remaining)?;
    let (remaining, _nl) = recognize(tuple((space0, line_ending)))(remaining)?;
    let lesser_block_end_specialized = lesser_block_end("export");
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("lesser block"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &lesser_block_end_specialized,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);

    let (remaining, contents) = text_until_exit(&parser_context, remaining)?;
    let (remaining, _end) = lesser_block_end_specialized(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        ExportBlock {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            export_type: export_type.map(Into::<&str>::into),
            data: parameters.map(Into::<&str>::into),
            value: Into::<&str>::into(contents),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

pub(crate) fn src_block<'b, 'g, 'r, 's, AK>(
    affiliated_keywords: AK,
    remaining: OrgSource<'s>,
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, SrcBlock<'s>>
where
    AK: IntoIterator<Item = Keyword<'s>>,
{
    let (remaining, _) = lesser_block_begin("src")(context, remaining)?;
    // https://orgmode.org/worg/org-syntax.html#Blocks claims that data is mandatory and must follow the LANGUAGE SWITCHES ARGUMENTS pattern but testing has shown that no data and incorrect data here will still parse to a src block.
    let (remaining, language) =
        opt(map(tuple((space1, switch_word)), |(_, language)| language))(remaining)?;
    let (remaining, switches) = opt(src_switches)(remaining)?;
    let (remaining, parameters) = opt(map(tuple((space1, src_parameters)), |(_, parameters)| {
        parameters
    }))(remaining)?;
    let (remaining, _nl) = recognize(tuple((space0, line_ending)))(remaining)?;
    let lesser_block_end_specialized = lesser_block_end("src");
    let contexts = [
        ContextElement::ConsumeTrailingWhitespace(true),
        ContextElement::Context("lesser block"),
        ContextElement::ExitMatcherNode(ExitMatcherNode {
            class: ExitClass::Alpha,
            exit_matcher: &lesser_block_end_specialized,
        }),
    ];
    let parser_context = context.with_additional_node(&contexts[0]);
    let parser_context = parser_context.with_additional_node(&contexts[1]);
    let parser_context = parser_context.with_additional_node(&contexts[2]);
    let (remaining, contents) = text_until_exit(&parser_context, remaining)?;
    let (remaining, _end) = lesser_block_end_specialized(&parser_context, remaining)?;

    let (remaining, post_blank) =
        maybe_consume_trailing_whitespace_if_not_exiting(context, remaining)?;
    let source = get_consumed(input, remaining);
    let (switches, number_lines, preserve_indent, retain_labels, use_labels, label_format) = {
        if let Some(switches) = switches {
            (
                if switches.source.is_empty() {
                    None
                } else {
                    Some(switches.source)
                },
                switches.number_lines,
                switches.preserve_indent,
                switches.retain_labels,
                switches.use_labels,
                switches.label_format,
            )
        } else {
            (None, None, None, RetainLabels::Yes, true, None)
        }
    };
    Ok((
        remaining,
        SrcBlock {
            source: source.into(),
            affiliated_keywords: parse_affiliated_keywords(
                context.get_global_settings(),
                affiliated_keywords,
            ),
            language: language.map(Into::<&str>::into),
            switches,
            parameters: parameters.map(Into::<&str>::into),
            number_lines,
            preserve_indent,
            retain_labels,
            use_labels,
            label_format,
            value: Into::<&str>::into(contents),
            post_blank: post_blank.map(Into::<&str>::into),
        },
    ))
}

fn name<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    is_not(" \t\r\n")(input)
}

fn data<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    is_not("\r\n")(input)
}

fn lesser_block_end(current_name: &str) -> impl ContextMatcher + '_ {
    // Since the lesser block names are statically defined in code, we can simply assert that the name is lowercase instead of causing an allocation by converting to lowercase.
    debug_assert!(current_name == current_name.to_lowercase());
    move |context, input: OrgSource<'_>| _lesser_block_end(context, input, current_name)
}

fn _lesser_block_end<'b, 'g, 'r, 's, 'c>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    current_name_lower: &'c str,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    let (remaining, _leading_whitespace) = space0(input)?;
    let (remaining, (_begin, _name, _ws, _ending)) = tuple((
        tag_no_case("#+end_"),
        tag_no_case(current_name_lower),
        space0,
        alt((eof, line_ending)),
    ))(remaining)?;
    let source = get_consumed(input, remaining);
    Ok((remaining, source))
}

/// Parser for the beginning of a lesser block
///
/// current_name MUST be lowercase. We do not do the conversion ourselves because it is not allowed in a const fn.
const fn lesser_block_begin(current_name: &str) -> impl ContextMatcher + '_ {
    // TODO: Since this is a const fn, is there ANY way to "generate" functions at compile time?
    move |context, input: OrgSource<'_>| _lesser_block_begin(context, input, current_name)
}

fn _lesser_block_begin<'b, 'g, 'r, 's, 'c>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    current_name_lower: &'c str,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    start_of_line(input)?;
    let (remaining, _leading_whitespace) = space0(input)?;
    let (remaining, (_begin, name)) = tuple((
        tag_no_case("#+begin_"),
        // The names are ASCII without a k, so no other character folds to
        // them and Unicode case folding would agree; it allocated a String
        // for every line starting with #+begin_.
        verify(name, |name: &OrgSource<'_>| {
            Into::<&str>::into(name).eq_ignore_ascii_case(current_name_lower)
        }),
    ))(remaining)?;
    expect_end(remaining, End::Block(current_name_lower))?;
    Ok((remaining, name))
}

#[derive(Debug)]
struct ExampleSrcSwitches<'s> {
    source: &'s str,
    number_lines: Option<SwitchNumberLines>,
    retain_labels: RetainLabels,
    preserve_indent: Option<CharOffsetInLine>,
    use_labels: bool,
    label_format: Option<&'s str>,
}

#[derive(Debug)]
enum SwitchState {
    Normal,
    NewLineNumber,
    ContinuedLineNumber,
    LabelFormat,
}

fn src_parameters<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    verify(
        recognize(many_till(anychar, peek(tuple((space0, line_ending))))),
        |parameters: &OrgSource<'_>| parameters.len() > 0,
    )(input)
}

fn src_switches<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ExampleSrcSwitches<'s>> {
    let (remaining, leading_spaces) = space1(input)?;
    let offset = Into::<&str>::into(leading_spaces).chars().count();
    let offset = CharOffsetInLine::try_from(offset)
        .expect("Character offset should fit in CharOffsetInLine");
    example_src_switches(true, offset)(remaining)
}

fn example_switches<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, ExampleSrcSwitches<'s>> {
    let (remaining, switches) = example_src_switches(false, 0)(input)?;
    Ok((remaining, switches))
}

fn example_src_switches(
    stop_at_parameters: bool,
    additional_char_offset: CharOffsetInLine,
) -> impl for<'s> Fn(OrgSource<'s>) -> Res<OrgSource<'s>, ExampleSrcSwitches<'s>> {
    move |input| _example_src_switches(input, stop_at_parameters, additional_char_offset)
}

fn _example_src_switches<'s>(
    input: OrgSource<'s>,
    stop_at_parameters: bool,
    additional_char_offset: CharOffsetInLine,
) -> Res<OrgSource<'s>, ExampleSrcSwitches<'s>> {
    let mut number_lines = None;
    let mut retain_labels = RetainLabels::Yes;
    let mut preserve_indent = None;
    let mut use_labels = true;
    let mut label_format = None;
    let mut saw_r = false;
    let mut matched_a_word = false;
    let mut remaining = input;
    let mut last_match_remaining = input;
    let mut state = SwitchState::Normal;
    'outer: loop {
        let (remain, word) = opt(switch_word)(remaining)?;
        let word = match word {
            Some(word) => word,
            None => {
                break;
            }
        };

        let normalized_word = Into::<&str>::into(word);

        loop {
            match (&state, normalized_word) {
                (SwitchState::Normal, "-n") => {
                    state = SwitchState::NewLineNumber;
                }
                (SwitchState::Normal, "+n") => {
                    state = SwitchState::ContinuedLineNumber;
                }
                (SwitchState::Normal, "-r") => {
                    saw_r = true;
                    use_labels = false;
                    if let RetainLabels::Yes = retain_labels {
                        retain_labels = RetainLabels::No;
                    }
                }
                (SwitchState::Normal, "-l") => {
                    state = SwitchState::LabelFormat;
                }
                (SwitchState::Normal, "-k") => {
                    use_labels = false;
                    let text_until_flag = input.get_until(word);
                    let character_offset = Into::<&str>::into(text_until_flag).chars().count();
                    let character_offset = CharOffsetInLine::try_from(character_offset)
                        .expect("Character offset should fit in CharOffsetInLine");
                    retain_labels = RetainLabels::Keep(character_offset + additional_char_offset);
                }
                (SwitchState::Normal, "-i") => {
                    let text_until_flag = input.get_until(word);
                    let character_offset = Into::<&str>::into(text_until_flag).chars().count();
                    let character_offset = CharOffsetInLine::try_from(character_offset)
                        .expect("Character offset should fit in CharOffsetInLine");
                    preserve_indent = Some(character_offset + additional_char_offset);
                }
                (SwitchState::NewLineNumber, _) => {
                    let val = normalized_word.parse::<LineNumber>();
                    if let Ok(val) = val {
                        if val < 0 {
                            number_lines = Some(SwitchNumberLines::New(0));
                        } else {
                            // Note that this can result in a negative 1 if the val is originally 0.
                            number_lines = Some(SwitchNumberLines::New(val - 1));
                        }
                        state = SwitchState::Normal;
                    } else {
                        number_lines = Some(SwitchNumberLines::New(0));
                        state = SwitchState::Normal;
                        continue; // Re-processes the word
                    }
                }
                (SwitchState::ContinuedLineNumber, _) => {
                    let val = normalized_word.parse::<LineNumber>();
                    if let Ok(val) = val {
                        if val < 0 {
                            number_lines = Some(SwitchNumberLines::Continued(0));
                        } else {
                            // Note that this can result in a negative 1 if the val is originally 0.
                            number_lines = Some(SwitchNumberLines::Continued(val - 1));
                        }
                        state = SwitchState::Normal;
                    } else {
                        number_lines = Some(SwitchNumberLines::Continued(0));
                        state = SwitchState::Normal;
                        continue; // Re-processes the word
                    }
                }
                (SwitchState::LabelFormat, _) => {
                    label_format = Some(normalized_word);
                    state = SwitchState::Normal;
                }
                (SwitchState::Normal, _) if stop_at_parameters => {
                    break 'outer;
                }
                (SwitchState::Normal, _) => {}
            };
            matched_a_word = true;
            remaining = remain;
            last_match_remaining = remain;

            let (remain, divider) = opt(space1)(remaining)?;
            if divider.is_none() {
                break 'outer;
            }
            remaining = remain;
            break;
        }
    }
    if !matched_a_word {
        return Err(nom::Err::Error(CustomError::Static("No words.")));
    }
    let remaining = last_match_remaining;

    let (remaining, _post_spaces) = opt(tuple((space0, peek(line_ending))))(remaining)?;
    let source = input.get_until(remaining);

    // Handle state that didn't get processed because we ran out of words.
    match state {
        SwitchState::Normal => {}
        SwitchState::NewLineNumber => {
            number_lines = Some(SwitchNumberLines::New(0));
        }
        SwitchState::ContinuedLineNumber => {
            number_lines = Some(SwitchNumberLines::Continued(0));
        }
        SwitchState::LabelFormat => {}
    }

    let retain_labels = match retain_labels {
        RetainLabels::Keep(_) if !saw_r => RetainLabels::Yes,
        _ => retain_labels,
    };

    Ok((
        remaining,
        ExampleSrcSwitches {
            source: Into::<&str>::into(source),
            number_lines,
            retain_labels,
            preserve_indent,
            use_labels,
            label_format,
        },
    ))
}

fn switch_word<'s>(input: OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {
    alt((
        map(
            tuple((tag(r#"""#), is_not("\"\r\n"), tag(r#"""#))),
            |(_, contents, _)| contents,
        ),
        is_not(" \t\r\n"),
    ))(input)
}
