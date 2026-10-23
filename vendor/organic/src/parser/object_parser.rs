use super::org_source::OrgSource;
use super::plain_text::plain_text;
use super::regular_link::regular_link;
use super::subscript_and_superscript::detect_subscript_or_superscript;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::angle_link::angle_link;
use crate::parser::citation::citation;
use crate::parser::entity::entity;
use crate::parser::export_snippet::export_snippet;
use crate::parser::footnote_reference::footnote_reference;
use crate::parser::inline_babel_call::inline_babel_call;
use crate::parser::inline_source_block::inline_source_block;
use crate::parser::latex_fragment::latex_fragment;
use crate::parser::line_break::line_break;
use crate::parser::macros::element;
use crate::parser::org_macro::org_macro;
use crate::parser::plain_link::plain_link;
use crate::parser::radio_link::radio_link;
use crate::parser::radio_link::radio_target;
use crate::parser::statistics_cookie::statistics_cookie;
use crate::parser::subscript_and_superscript::subscript;
use crate::parser::subscript_and_superscript::superscript;
use crate::parser::target::target;
use crate::parser::text_markup::text_markup;
use crate::parser::timestamp::timestamp;
use crate::parser::util::WORD_CONSTITUENT_CHARACTERS;
use crate::types::Object;

pub(crate) fn standard_set_object<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    element!(standard_set_object_sans_plain_text, context, input);
    element!(
        plain_text(detect_standard_set_object_sans_plain_text),
        context,
        input,
        Object::PlainText
    );
    Err(nom::Err::Error(CustomError::Static("No object.")))
}

pub(crate) fn minimal_set_object<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    element!(minimal_set_object_sans_plain_text, context, input);
    element!(
        plain_text(detect_minimal_set_object_sans_plain_text),
        context,
        input,
        Object::PlainText
    );
    Err(nom::Err::Error(CustomError::Static("No object.")))
}

fn standard_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    element!(timestamp, context, input, Object::Timestamp);
    element!(subscript, context, input, Object::Subscript);
    element!(superscript, context, input, Object::Superscript);
    element!(statistics_cookie, context, input, Object::StatisticsCookie);
    element!(target, context, input, Object::Target);
    element!(line_break, context, input, Object::LineBreak);
    element!(
        inline_source_block,
        context,
        input,
        Object::InlineSourceBlock
    );
    element!(inline_babel_call, context, input, Object::InlineBabelCall);
    element!(citation, context, input, Object::Citation);
    element!(
        footnote_reference,
        context,
        input,
        Object::FootnoteReference
    );
    element!(export_snippet, context, input, Object::ExportSnippet);
    element!(entity, context, input, Object::Entity);
    element!(latex_fragment, context, input, Object::LatexFragment);
    element!(radio_link, context, input, Object::RadioLink);
    element!(radio_target, context, input, Object::RadioTarget);
    element!(text_markup, context, input);
    element!(regular_link, context, input, Object::RegularLink);
    element!(plain_link, context, input, Object::PlainLink);
    element!(angle_link, context, input, Object::AngleLink);
    element!(org_macro, context, input, Object::OrgMacro);

    Err(nom::Err::Error(CustomError::Static("No object.")))
}

fn minimal_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    element!(subscript, context, input, Object::Subscript);
    element!(superscript, context, input, Object::Superscript);
    element!(entity, context, input, Object::Entity);
    element!(latex_fragment, context, input, Object::LatexFragment);
    element!(text_markup, context, input);
    Err(nom::Err::Error(CustomError::Static("No object.")))
}

pub(crate) fn detect_standard_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    if !could_start_object(context, input) {
        return Err(nom::Err::Error(CustomError::Static("No object detected.")));
    }
    if detect_subscript_or_superscript(input).is_ok() {
        return Ok((input, ()));
    }
    if standard_set_object_sans_plain_text(context, input).is_ok() {
        return Ok((input, ()));
    }

    Err(nom::Err::Error(CustomError::Static("No object detected.")))
}

fn detect_minimal_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    if !could_start_object(context, input) {
        return Err(nom::Err::Error(CustomError::Static("No object detected.")));
    }
    if detect_subscript_or_superscript(input).is_ok() {
        return Ok((input, ()));
    }
    if minimal_set_object_sans_plain_text(context, input).is_ok() {
        return Ok((input, ()));
    }

    Err(nom::Err::Error(CustomError::Static("No object detected.")))
}

pub(crate) fn regular_link_description_set_object<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    // TODO: It can also contain another link, but only when it is a plain or angle link. It can contain square brackets, but not ]]
    element!(
        regular_link_description_set_object_sans_plain_text,
        context,
        input
    );
    element!(
        plain_text(detect_regular_link_description_set_object_sans_plain_text),
        context,
        input,
        Object::PlainText
    );
    Err(nom::Err::Error(CustomError::Static("No object.")))
}

fn regular_link_description_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    // TODO: It can also contain another link, but only when it is a plain or angle link. It can contain square brackets, but not ]]
    element!(export_snippet, context, input, Object::ExportSnippet);
    element!(statistics_cookie, context, input, Object::StatisticsCookie);
    element!(
        inline_source_block,
        context,
        input,
        Object::InlineSourceBlock
    );
    element!(inline_babel_call, context, input, Object::InlineBabelCall);
    element!(org_macro, context, input, Object::OrgMacro);
    element!(minimal_set_object_sans_plain_text, context, input);
    Err(nom::Err::Error(CustomError::Static("No object.")))
}

fn detect_regular_link_description_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    if !could_start_object(context, input) {
        return Err(nom::Err::Error(CustomError::Static("No object detected.")));
    }
    if detect_subscript_or_superscript(input).is_ok() {
        return Ok((input, ()));
    }
    if regular_link_description_set_object_sans_plain_text(context, input).is_ok() {
        return Ok((input, ()));
    }

    Err(nom::Err::Error(CustomError::Static("No object detected.")))
}

pub(crate) fn table_cell_set_object<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    element!(table_cell_set_object_sans_plain_text, context, input);
    element!(
        plain_text(detect_table_cell_set_object_sans_plain_text),
        context,
        input,
        Object::PlainText
    );
    Err(nom::Err::Error(CustomError::Static("No object.")))
}

fn table_cell_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Object<'s>> {
    element!(citation, context, input, Object::Citation);
    element!(export_snippet, context, input, Object::ExportSnippet);
    element!(
        footnote_reference,
        context,
        input,
        Object::FootnoteReference
    );
    element!(radio_link, context, input, Object::RadioLink);
    element!(regular_link, context, input, Object::RegularLink);
    element!(plain_link, context, input, Object::PlainLink);
    element!(angle_link, context, input, Object::AngleLink);
    element!(org_macro, context, input, Object::OrgMacro);
    element!(radio_target, context, input, Object::RadioTarget);
    element!(target, context, input, Object::Target);
    element!(timestamp, context, input, Object::Timestamp);
    element!(minimal_set_object_sans_plain_text, context, input);
    Err(nom::Err::Error(CustomError::Static("No object.")))
}

fn detect_table_cell_set_object_sans_plain_text<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, ()> {
    if !could_start_object(context, input) {
        return Err(nom::Err::Error(CustomError::Static("No object detected.")));
    }
    if detect_subscript_or_superscript(input).is_ok() {
        return Ok((input, ()));
    }
    if table_cell_set_object_sans_plain_text(context, input).is_ok() {
        return Ok((input, ()));
    }

    Err(nom::Err::Error(CustomError::Static("No object detected.")))
}

/// Whether any object can start at `input`. Plain text asks this at every
/// character, and upstream answered by trying every object parser (P3 and
/// P4 in #30). A false here must mean every parser would fail.
fn could_start_object(context: RefContext<'_, '_, '_, '_>, input: OrgSource<'_>) -> bool {
    // A radio link starts wherever its target's text does.
    if !context.get_global_settings().radio_targets.is_empty() {
        return true;
    }
    let rest: &str = input.into();
    let Some(first) = rest.chars().next() else {
        return false;
    };
    match first {
        '<' | '[' | '_' | '^' | '\\' | '@' | '$' | '{' | '*' | '/' | '=' | '~' | '+' => true,
        // The Kelvin sign lowercases to `k`, so a case-insensitive protocol
        // can start with it.
        letter if letter.is_ascii_alphabetic() || letter == '\u{212A}' => {
            let bytes = rest.as_bytes();
            let starts_with = |prefix: &str| {
                bytes.len() >= prefix.len()
                    && bytes[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
            };
            if starts_with("src_") || starts_with("call_") {
                return true;
            }
            // Otherwise only a plain link's protocol: after a non-word
            // character, and followed by `:` before any whitespace. The `:`
            // ends a protocol, so it is no further than the longest one.
            let after_word = input
                .get_preceding_character()
                .is_some_and(|c| WORD_CONSTITUENT_CHARACTERS.contains(c));
            if after_word {
                return false;
            }
            // Bytes, not chars: never fewer, and far cheaper to count.
            let longest = context
                .get_global_settings()
                .link_parameters
                .iter()
                .map(|protocol| protocol.len())
                .max()
                .unwrap_or(0);
            rest.chars()
                .take(longest + 1)
                .take_while(|c| !c.is_whitespace())
                .any(|c| c == ':')
        }
        _ => false,
    }
}
