use super::babel_call::babel_call;
use super::clock::clock;
use super::comment::comment;
use super::comment::detect_comment;
use super::diary_sexp::detect_diary_sexp;
use super::diary_sexp::diary_sexp;
use super::drawer::drawer;
use super::dynamic_block::dynamic_block;
use super::fixed_width_area::detect_fixed_width_area;
use super::fixed_width_area::fixed_width_area;
use super::footnote_definition::detect_footnote_definition;
use super::footnote_definition::footnote_definition;
use super::fuel;
use super::greater_block::greater_block;
use super::horizontal_rule::horizontal_rule;
use super::keyword::keyword;
use super::latex_environment::latex_environment;
use super::lesser_block::comment_block;
use super::lesser_block::example_block;
use super::lesser_block::export_block;
use super::lesser_block::src_block;
use super::lesser_block::verse_block;
use super::org_source::OrgSource;
use super::paragraph::paragraph;
use super::plain_list::detect_plain_list;
use super::plain_list::plain_list;
use super::table::detect_table;
use crate::context::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::affiliated_keyword::affiliated_keywords;
use crate::parser::bullshitium::bullshitium;
use crate::parser::bullshitium::detect_bullshitium;
use crate::parser::macros::ak_element;
use crate::parser::macros::element;
use crate::parser::table::org_mode_table;
use crate::types::Element;

pub(crate) const fn element(
    can_be_paragraph: bool,
) -> impl for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, Element<'s>> {
    move |context, input: OrgSource<'_>| _element(context, input, can_be_paragraph)
}

fn _element<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    can_be_paragraph: bool,
) -> Res<OrgSource<'s>, Element<'s>> {
    let _attempt = fuel::Attempt::start(input)?;
    let (post_affiliated_keywords_input, affiliated_keywords) = affiliated_keywords(input)?;

    let mut affiliated_keywords = affiliated_keywords.into_iter();

    ak_element!(
        plain_list,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::PlainList
    );

    ak_element!(
        greater_block,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input
    );

    ak_element!(
        dynamic_block,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::DynamicBlock
    );

    ak_element!(
        footnote_definition,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::FootnoteDefinition
    );

    element!(comment, context, input, Element::Comment);

    ak_element!(
        drawer,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::Drawer
    );

    ak_element!(
        org_mode_table,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::Table
    );

    ak_element!(
        verse_block,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::VerseBlock
    );

    ak_element!(
        comment_block,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::CommentBlock
    );

    ak_element!(
        example_block,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::ExampleBlock
    );

    ak_element!(
        export_block,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::ExportBlock
    );

    ak_element!(
        src_block,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::SrcBlock
    );

    element!(clock, context, input, Element::Clock);

    ak_element!(
        diary_sexp,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::DiarySexp
    );

    ak_element!(
        fixed_width_area,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::FixedWidthArea
    );

    ak_element!(
        horizontal_rule,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::HorizontalRule
    );

    ak_element!(
        latex_environment,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::LatexEnvironment
    );

    ak_element!(
        babel_call,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::BabelCall
    );

    // Keyword with affiliated keywords
    ak_element!(
        keyword,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input,
        Element::Keyword
    );

    if can_be_paragraph {
        // Paragraph with affiliated keyword
        ak_element!(
            paragraph,
            &mut affiliated_keywords,
            post_affiliated_keywords_input,
            context,
            input,
            Element::Paragraph
        );
    }

    // Keyword without affiliated keywords
    ak_element!(
        keyword,
        std::iter::empty(),
        input,
        context,
        input,
        Element::Keyword
    );

    if can_be_paragraph {
        // Fake paragraphs
        element!(bullshitium, context, input, Element::Paragraph);

        // Paragraph without affiliated keyword
        ak_element!(
            paragraph,
            std::iter::empty(),
            input,
            context,
            input,
            Element::Paragraph
        );
    }

    Err(nom::Err::Error(CustomError::Static("No element.")))
}

pub(crate) const fn detect_element(
    can_be_paragraph: bool,
) -> impl for<'b, 'g, 'r, 's> Fn(RefContext<'b, 'g, 'r, 's>, OrgSource<'s>) -> Res<OrgSource<'s>, ()>
{
    move |context, input: OrgSource<'_>| _detect_element(context, input, can_be_paragraph)
}

fn _detect_element<'b, 'g, 'r, 's>(
    context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
    can_be_paragraph: bool,
) -> Res<OrgSource<'s>, ()> {
    fuel::spend(fuel::ELEMENT, input)?;
    let (post_affiliated_keywords_input, affiliated_keywords) = affiliated_keywords(input)?;

    let mut affiliated_keywords = affiliated_keywords.into_iter();

    ak_element!(
        detect_plain_list,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input
    );

    ak_element!(
        detect_footnote_definition,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input
    );

    ak_element!(
        detect_diary_sexp,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input
    );

    element!(detect_comment, input);

    ak_element!(
        detect_fixed_width_area,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input
    );

    ak_element!(
        detect_table,
        &mut affiliated_keywords,
        post_affiliated_keywords_input,
        context,
        input
    );

    // Fake paragraphs
    if !can_be_paragraph {
        element!(detect_bullshitium, context, input);
    }

    if _element(context, input, can_be_paragraph).is_ok() {
        return Ok((input, ()));
    }

    Err(nom::Err::Error(CustomError::Static("No element detected.")))
}
