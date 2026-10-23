use super::greater_element::DynamicBlock;
use super::greater_element::FootnoteDefinition;
use super::greater_element::PlainList;
use super::greater_element::PropertyDrawer;
use super::greater_element::Table;
use super::lesser_element::BabelCall;
use super::lesser_element::Clock;
use super::lesser_element::Comment;
use super::lesser_element::CommentBlock;
use super::lesser_element::DiarySexp;
use super::lesser_element::ExampleBlock;
use super::lesser_element::ExportBlock;
use super::lesser_element::FixedWidthArea;
use super::lesser_element::HorizontalRule;
use super::lesser_element::Keyword;
use super::lesser_element::LatexEnvironment;
use super::lesser_element::Paragraph;
use super::lesser_element::Planning;
use super::lesser_element::SrcBlock;
use super::lesser_element::VerseBlock;
use super::CenterBlock;
use super::Drawer;
use super::PostBlank;
use super::QuoteBlock;
use super::SpecialBlock;
use super::StandardProperties;

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Element<'s> {
    Paragraph(Paragraph<'s>),
    PlainList(PlainList<'s>),
    CenterBlock(CenterBlock<'s>),
    QuoteBlock(QuoteBlock<'s>),
    SpecialBlock(SpecialBlock<'s>),
    DynamicBlock(DynamicBlock<'s>),
    FootnoteDefinition(FootnoteDefinition<'s>),
    Comment(Comment<'s>),
    Drawer(Drawer<'s>),
    PropertyDrawer(PropertyDrawer<'s>),
    Table(Table<'s>),
    VerseBlock(VerseBlock<'s>),
    CommentBlock(CommentBlock<'s>),
    ExampleBlock(ExampleBlock<'s>),
    ExportBlock(ExportBlock<'s>),
    SrcBlock(SrcBlock<'s>),
    Clock(Clock<'s>),
    DiarySexp(DiarySexp<'s>),
    Planning(Planning<'s>),
    FixedWidthArea(FixedWidthArea<'s>),
    HorizontalRule(HorizontalRule<'s>),
    Keyword(Keyword<'s>),
    BabelCall(BabelCall<'s>),
    LatexEnvironment(LatexEnvironment<'s>),
}

impl<'s> StandardProperties<'s> for Element<'s> {
    fn get_source<'b>(&'b self) -> &'s str {
        match self {
            Element::Paragraph(inner) => inner.get_source(),
            Element::PlainList(inner) => inner.get_source(),
            Element::CenterBlock(inner) => inner.get_source(),
            Element::QuoteBlock(inner) => inner.get_source(),
            Element::SpecialBlock(inner) => inner.get_source(),
            Element::DynamicBlock(inner) => inner.get_source(),
            Element::FootnoteDefinition(inner) => inner.get_source(),
            Element::Comment(inner) => inner.get_source(),
            Element::Drawer(inner) => inner.get_source(),
            Element::PropertyDrawer(inner) => inner.get_source(),
            Element::Table(inner) => inner.get_source(),
            Element::VerseBlock(inner) => inner.get_source(),
            Element::CommentBlock(inner) => inner.get_source(),
            Element::ExampleBlock(inner) => inner.get_source(),
            Element::ExportBlock(inner) => inner.get_source(),
            Element::SrcBlock(inner) => inner.get_source(),
            Element::Clock(inner) => inner.get_source(),
            Element::DiarySexp(inner) => inner.get_source(),
            Element::Planning(inner) => inner.get_source(),
            Element::FixedWidthArea(inner) => inner.get_source(),
            Element::HorizontalRule(inner) => inner.get_source(),
            Element::Keyword(inner) => inner.get_source(),
            Element::BabelCall(inner) => inner.get_source(),
            Element::LatexEnvironment(inner) => inner.get_source(),
        }
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        match self {
            Element::Paragraph(inner) => inner.get_contents(),
            Element::PlainList(inner) => inner.get_contents(),
            Element::CenterBlock(inner) => inner.get_contents(),
            Element::QuoteBlock(inner) => inner.get_contents(),
            Element::SpecialBlock(inner) => inner.get_contents(),
            Element::DynamicBlock(inner) => inner.get_contents(),
            Element::FootnoteDefinition(inner) => inner.get_contents(),
            Element::Comment(inner) => inner.get_contents(),
            Element::Drawer(inner) => inner.get_contents(),
            Element::PropertyDrawer(inner) => inner.get_contents(),
            Element::Table(inner) => inner.get_contents(),
            Element::VerseBlock(inner) => inner.get_contents(),
            Element::CommentBlock(inner) => inner.get_contents(),
            Element::ExampleBlock(inner) => inner.get_contents(),
            Element::ExportBlock(inner) => inner.get_contents(),
            Element::SrcBlock(inner) => inner.get_contents(),
            Element::Clock(inner) => inner.get_contents(),
            Element::DiarySexp(inner) => inner.get_contents(),
            Element::Planning(inner) => inner.get_contents(),
            Element::FixedWidthArea(inner) => inner.get_contents(),
            Element::HorizontalRule(inner) => inner.get_contents(),
            Element::Keyword(inner) => inner.get_contents(),
            Element::BabelCall(inner) => inner.get_contents(),
            Element::LatexEnvironment(inner) => inner.get_contents(),
        }
    }

    fn get_post_blank(&self) -> PostBlank {
        match self {
            Element::Paragraph(inner) => inner.get_post_blank(),
            Element::PlainList(inner) => inner.get_post_blank(),
            Element::CenterBlock(inner) => inner.get_post_blank(),
            Element::QuoteBlock(inner) => inner.get_post_blank(),
            Element::SpecialBlock(inner) => inner.get_post_blank(),
            Element::DynamicBlock(inner) => inner.get_post_blank(),
            Element::FootnoteDefinition(inner) => inner.get_post_blank(),
            Element::Comment(inner) => inner.get_post_blank(),
            Element::Drawer(inner) => inner.get_post_blank(),
            Element::PropertyDrawer(inner) => inner.get_post_blank(),
            Element::Table(inner) => inner.get_post_blank(),
            Element::VerseBlock(inner) => inner.get_post_blank(),
            Element::CommentBlock(inner) => inner.get_post_blank(),
            Element::ExampleBlock(inner) => inner.get_post_blank(),
            Element::ExportBlock(inner) => inner.get_post_blank(),
            Element::SrcBlock(inner) => inner.get_post_blank(),
            Element::Clock(inner) => inner.get_post_blank(),
            Element::DiarySexp(inner) => inner.get_post_blank(),
            Element::Planning(inner) => inner.get_post_blank(),
            Element::FixedWidthArea(inner) => inner.get_post_blank(),
            Element::HorizontalRule(inner) => inner.get_post_blank(),
            Element::Keyword(inner) => inner.get_post_blank(),
            Element::BabelCall(inner) => inner.get_post_blank(),
            Element::LatexEnvironment(inner) => inner.get_post_blank(),
        }
    }
}
