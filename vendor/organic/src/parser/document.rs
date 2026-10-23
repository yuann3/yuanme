use nom::combinator::all_consuming;
use nom::combinator::opt;
use nom::multi::many0;
use nom::InputTake;

use super::closer;
use super::fuel;
use super::headline::heading;
use super::in_buffer_settings::apply_in_buffer_settings;
use super::in_buffer_settings::apply_post_parse_in_buffer_settings;
use super::in_buffer_settings::scan_for_in_buffer_settings;
use super::org_source::OrgSource;
use super::section::zeroth_section;
use super::util::get_consumed;
use crate::context::bind_context;
use crate::context::parser_with_context;
use crate::context::Context;
use crate::context::ContextElement;
use crate::context::GlobalSettings;
use crate::context::List;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::util::blank_line;
use crate::types::AstNode;
use crate::types::Document;
use crate::types::Object;

/// Why a document didn't parse.
#[derive(Debug)]
pub enum ParseError {
    /// The parse spent more work than the input's length allows (#15).
    /// `opener` is the byte offset where the outermost element being parsed
    /// when the fuel ran out began: a top-level paragraph whose objects ran
    /// away, or the first of several unclosed blocks nested in each other.
    OutOfFuel { opener: usize },
    /// Org has no syntax errors, so this is a bug in the parser, or a
    /// `#+SETUPFILE` the settings' `FileAccessInterface` failed to read.
    Failed(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::OutOfFuel { opener } => {
                write!(f, "parsing ran out of fuel at byte {opener}")
            }
            ParseError::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a full org-mode document with the default settings.
pub fn parse(input: &str) -> Result<Document<'_>, ParseError> {
    parse_with_settings(input, &GlobalSettings::default())
}

/// Parse a full org-mode document with starting settings.
///
/// This will not prevent additional settings from being learned during parsing, for example when encountering a "#+TODO".
pub fn parse_with_settings<'g, 's>(
    input: &'s str,
    global_settings: &'g GlobalSettings<'g, 's>,
) -> Result<Document<'s>, ParseError> {
    let initial_context = ContextElement::document_context();
    let initial_context = Context::new(global_settings, List::new(&initial_context));
    let wrapped_input = OrgSource::new(input);
    closer::reset();
    fuel::fill(input.len());
    let document =
        all_consuming(bind_context!(document_org_source, &initial_context))(wrapped_input);
    if let Some(opener) = fuel::opener() {
        return Err(ParseError::OutOfFuel { opener });
    }
    document
        .map(|(_remaining, document)| document)
        .map_err(|error| ParseError::Failed(error.to_string()))
}

/// Parse a full org-mode document.
///
/// Use this entry point when you want to have direct control over the starting context or if you want to use this integrated with other nom parsers. For general-purpose usage, the `parse` and `parse_with_settings` functions are a lot simpler.
///
/// This will not prevent additional settings from being learned during parsing, for example when encountering a "#+TODO".
#[allow(dead_code)]
fn document<'s>(context: RefContext<'_, '_, '_, 's>, input: &'s str) -> Res<&'s str, Document<'s>> {
    let (remaining, doc) = document_org_source(context, input.into())?;
    Ok((Into::<&str>::into(remaining), doc))
}

#[allow(dead_code)]
fn document_org_source<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Document<'s>> {
    let mut final_settings = Vec::new();
    let (_, document_settings) = scan_for_in_buffer_settings(input)?;
    let setup_files: Vec<String> = document_settings
        .iter()
        .filter(|kw| kw.key.eq_ignore_ascii_case("setupfile"))
        .map(|kw| kw.value)
        .map(|setup_file| {
            context
                .get_global_settings()
                .file_access
                .read_file(setup_file)
                .map_err(|err| nom::Err::<CustomError>::Failure(err.into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for setup_file in setup_files.iter().map(String::as_str) {
        let (_, setup_file_settings) = scan_for_in_buffer_settings(setup_file.into())?;
        final_settings.extend(setup_file_settings);
    }
    final_settings.extend(document_settings);
    let new_settings = apply_in_buffer_settings(final_settings, context.get_global_settings())
        .map_err(nom::Err::Error)?;
    let new_context = context.with_global_settings(&new_settings);
    let context = &new_context;

    let (remaining, mut document) =
        _document(context, input).map(|(rem, out)| (Into::<&str>::into(rem), out))?;
    {
        // If there are radio targets in this document then we need to parse the entire document again with the knowledge of the radio targets.
        let all_radio_targets: Vec<&Vec<Object<'_>>> = Into::<AstNode>::into(&document)
            .iter_all_ast_nodes()
            .filter_map(|ast_node| {
                if let AstNode::RadioTarget(ast_node) = ast_node {
                    Some(ast_node)
                } else {
                    None
                }
            })
            .map(|rt| &rt.children)
            .collect();
        if !all_radio_targets.is_empty() {
            let mut new_global_settings = context.get_global_settings().clone();
            new_global_settings.radio_targets = all_radio_targets;
            let parser_context = context.with_global_settings(&new_global_settings);
            let (remaining, mut document) = _document(&parser_context, input)
                .map(|(rem, out)| (Into::<&str>::into(rem), out))?;
            apply_post_parse_in_buffer_settings(&mut document);
            return Ok((remaining.into(), document));
        }
    }

    // Find final in-buffer settings that do not impact parsing
    apply_post_parse_in_buffer_settings(&mut document);

    Ok((remaining.into(), document))
}

fn _document<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, Document<'s>> {
    let zeroth_section_matcher = parser_with_context!(zeroth_section)(context);
    let heading_matcher = parser_with_context!(heading(0))(context);
    let (remaining, _blank_lines) = many0(blank_line)(input)?;
    let contents_begin = remaining;
    let (remaining, zeroth_section) = opt(zeroth_section_matcher)(remaining)?;
    let (remaining, children) = many0(heading_matcher)(remaining)?;
    let contents = get_consumed(contents_begin, remaining);
    let source = get_consumed(input, remaining);
    Ok((
        remaining,
        Document {
            source: source.into(),
            category: None,
            zeroth_section,
            children,
            contents: if contents.len() > 0 {
                Into::<&str>::into(contents)
            } else {
                Into::<&str>::into(remaining.take(0))
            },
        },
    ))
}
