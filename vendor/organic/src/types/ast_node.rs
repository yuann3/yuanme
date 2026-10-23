use super::macros::to_ast_node;
use super::CenterBlock;
use super::PostBlank;
use super::QuoteBlock;
use super::SpecialBlock;
use super::StandardProperties;
use crate::types::AngleLink;
use crate::types::BabelCall;
use crate::types::Bold;
use crate::types::Citation;
use crate::types::CitationReference;
use crate::types::Clock;
use crate::types::Code;
use crate::types::Comment;
use crate::types::CommentBlock;
use crate::types::DiarySexp;
use crate::types::Document;
use crate::types::DocumentElement;
use crate::types::Drawer;
use crate::types::DynamicBlock;
use crate::types::Element;
use crate::types::Entity;
use crate::types::ExampleBlock;
use crate::types::ExportBlock;
use crate::types::ExportSnippet;
use crate::types::FixedWidthArea;
use crate::types::FootnoteDefinition;
use crate::types::FootnoteReference;
use crate::types::Heading;
use crate::types::HorizontalRule;
use crate::types::InlineBabelCall;
use crate::types::InlineSourceBlock;
use crate::types::Italic;
use crate::types::Keyword;
use crate::types::LatexEnvironment;
use crate::types::LatexFragment;
use crate::types::LineBreak;
use crate::types::NodeProperty;
use crate::types::Object;
use crate::types::OrgMacro;
use crate::types::Paragraph;
use crate::types::PlainLink;
use crate::types::PlainList;
use crate::types::PlainListItem;
use crate::types::PlainText;
use crate::types::Planning;
use crate::types::PropertyDrawer;
use crate::types::RadioLink;
use crate::types::RadioTarget;
use crate::types::RegularLink;
use crate::types::Section;
use crate::types::SrcBlock;
use crate::types::StatisticsCookie;
use crate::types::StrikeThrough;
use crate::types::Subscript;
use crate::types::Superscript;
use crate::types::Table;
use crate::types::TableCell;
use crate::types::TableRow;
use crate::types::Target;
use crate::types::Timestamp;
use crate::types::Underline;
use crate::types::Verbatim;
use crate::types::VerseBlock;

pub enum AstNode<'r, 's> {
    // Document Nodes
    Document(&'r Document<'s>),
    Heading(&'r Heading<'s>),
    Section(&'r Section<'s>),
    // Elements
    Paragraph(&'r Paragraph<'s>),
    PlainList(&'r PlainList<'s>),
    PlainListItem(&'r PlainListItem<'s>),
    CenterBlock(&'r CenterBlock<'s>),
    QuoteBlock(&'r QuoteBlock<'s>),
    SpecialBlock(&'r SpecialBlock<'s>),
    DynamicBlock(&'r DynamicBlock<'s>),
    FootnoteDefinition(&'r FootnoteDefinition<'s>),
    Comment(&'r Comment<'s>),
    Drawer(&'r Drawer<'s>),
    PropertyDrawer(&'r PropertyDrawer<'s>),
    NodeProperty(&'r NodeProperty<'s>),
    Table(&'r Table<'s>),
    TableRow(&'r TableRow<'s>),
    VerseBlock(&'r VerseBlock<'s>),
    CommentBlock(&'r CommentBlock<'s>),
    ExampleBlock(&'r ExampleBlock<'s>),
    ExportBlock(&'r ExportBlock<'s>),
    SrcBlock(&'r SrcBlock<'s>),
    Clock(&'r Clock<'s>),
    DiarySexp(&'r DiarySexp<'s>),
    Planning(&'r Planning<'s>),
    FixedWidthArea(&'r FixedWidthArea<'s>),
    HorizontalRule(&'r HorizontalRule<'s>),
    Keyword(&'r Keyword<'s>),
    BabelCall(&'r BabelCall<'s>),
    LatexEnvironment(&'r LatexEnvironment<'s>),
    // Objects
    Bold(&'r Bold<'s>),
    Italic(&'r Italic<'s>),
    Underline(&'r Underline<'s>),
    StrikeThrough(&'r StrikeThrough<'s>),
    Code(&'r Code<'s>),
    Verbatim(&'r Verbatim<'s>),
    PlainText(&'r PlainText<'s>),
    RegularLink(&'r RegularLink<'s>),
    RadioLink(&'r RadioLink<'s>),
    RadioTarget(&'r RadioTarget<'s>),
    PlainLink(&'r PlainLink<'s>),
    AngleLink(&'r AngleLink<'s>),
    OrgMacro(&'r OrgMacro<'s>),
    Entity(&'r Entity<'s>),
    LatexFragment(&'r LatexFragment<'s>),
    ExportSnippet(&'r ExportSnippet<'s>),
    FootnoteReference(&'r FootnoteReference<'s>),
    Citation(&'r Citation<'s>),
    CitationReference(&'r CitationReference<'s>),
    InlineBabelCall(&'r InlineBabelCall<'s>),
    InlineSourceBlock(&'r InlineSourceBlock<'s>),
    LineBreak(&'r LineBreak<'s>),
    Target(&'r Target<'s>),
    StatisticsCookie(&'r StatisticsCookie<'s>),
    Subscript(&'r Subscript<'s>),
    Superscript(&'r Superscript<'s>),
    TableCell(&'r TableCell<'s>),
    Timestamp(&'r Timestamp<'s>),
}

impl<'r, 's> From<&'r DocumentElement<'s>> for AstNode<'r, 's> {
    fn from(value: &'r DocumentElement<'s>) -> Self {
        match value {
            DocumentElement::Heading(inner) => inner.into(),
            DocumentElement::Section(inner) => inner.into(),
        }
    }
}

impl<'r, 's> From<&'r Element<'s>> for AstNode<'r, 's> {
    fn from(value: &'r Element<'s>) -> Self {
        match value {
            Element::Paragraph(inner) => inner.into(),
            Element::PlainList(inner) => inner.into(),
            Element::CenterBlock(inner) => inner.into(),
            Element::QuoteBlock(inner) => inner.into(),
            Element::SpecialBlock(inner) => inner.into(),
            Element::DynamicBlock(inner) => inner.into(),
            Element::FootnoteDefinition(inner) => inner.into(),
            Element::Comment(inner) => inner.into(),
            Element::Drawer(inner) => inner.into(),
            Element::PropertyDrawer(inner) => inner.into(),
            Element::Table(inner) => inner.into(),
            Element::VerseBlock(inner) => inner.into(),
            Element::CommentBlock(inner) => inner.into(),
            Element::ExampleBlock(inner) => inner.into(),
            Element::ExportBlock(inner) => inner.into(),
            Element::SrcBlock(inner) => inner.into(),
            Element::Clock(inner) => inner.into(),
            Element::DiarySexp(inner) => inner.into(),
            Element::Planning(inner) => inner.into(),
            Element::FixedWidthArea(inner) => inner.into(),
            Element::HorizontalRule(inner) => inner.into(),
            Element::Keyword(inner) => inner.into(),
            Element::BabelCall(inner) => inner.into(),
            Element::LatexEnvironment(inner) => inner.into(),
        }
    }
}

impl<'r, 's> From<&'r Object<'s>> for AstNode<'r, 's> {
    fn from(value: &'r Object<'s>) -> Self {
        match value {
            Object::Bold(inner) => inner.into(),
            Object::Italic(inner) => inner.into(),
            Object::Underline(inner) => inner.into(),
            Object::StrikeThrough(inner) => inner.into(),
            Object::Code(inner) => inner.into(),
            Object::Verbatim(inner) => inner.into(),
            Object::PlainText(inner) => inner.into(),
            Object::RegularLink(inner) => inner.into(),
            Object::RadioLink(inner) => inner.into(),
            Object::RadioTarget(inner) => inner.into(),
            Object::PlainLink(inner) => inner.into(),
            Object::AngleLink(inner) => inner.into(),
            Object::OrgMacro(inner) => inner.into(),
            Object::Entity(inner) => inner.into(),
            Object::LatexFragment(inner) => inner.into(),
            Object::ExportSnippet(inner) => inner.into(),
            Object::FootnoteReference(inner) => inner.into(),
            Object::Citation(inner) => inner.into(),
            Object::CitationReference(inner) => inner.into(),
            Object::InlineBabelCall(inner) => inner.into(),
            Object::InlineSourceBlock(inner) => inner.into(),
            Object::LineBreak(inner) => inner.into(),
            Object::Target(inner) => inner.into(),
            Object::StatisticsCookie(inner) => inner.into(),
            Object::Subscript(inner) => inner.into(),
            Object::Superscript(inner) => inner.into(),
            Object::Timestamp(inner) => inner.into(),
        }
    }
}

to_ast_node!(&'r Document<'s>, AstNode::Document);
to_ast_node!(&'r Heading<'s>, AstNode::Heading);
to_ast_node!(&'r Section<'s>, AstNode::Section);
to_ast_node!(&'r Paragraph<'s>, AstNode::Paragraph);
to_ast_node!(&'r PlainList<'s>, AstNode::PlainList);
to_ast_node!(&'r PlainListItem<'s>, AstNode::PlainListItem);
to_ast_node!(&'r CenterBlock<'s>, AstNode::CenterBlock);
to_ast_node!(&'r QuoteBlock<'s>, AstNode::QuoteBlock);
to_ast_node!(&'r SpecialBlock<'s>, AstNode::SpecialBlock);
to_ast_node!(&'r DynamicBlock<'s>, AstNode::DynamicBlock);
to_ast_node!(&'r FootnoteDefinition<'s>, AstNode::FootnoteDefinition);
to_ast_node!(&'r Comment<'s>, AstNode::Comment);
to_ast_node!(&'r Drawer<'s>, AstNode::Drawer);
to_ast_node!(&'r PropertyDrawer<'s>, AstNode::PropertyDrawer);
to_ast_node!(&'r NodeProperty<'s>, AstNode::NodeProperty);
to_ast_node!(&'r Table<'s>, AstNode::Table);
to_ast_node!(&'r TableRow<'s>, AstNode::TableRow);
to_ast_node!(&'r VerseBlock<'s>, AstNode::VerseBlock);
to_ast_node!(&'r CommentBlock<'s>, AstNode::CommentBlock);
to_ast_node!(&'r ExampleBlock<'s>, AstNode::ExampleBlock);
to_ast_node!(&'r ExportBlock<'s>, AstNode::ExportBlock);
to_ast_node!(&'r SrcBlock<'s>, AstNode::SrcBlock);
to_ast_node!(&'r Clock<'s>, AstNode::Clock);
to_ast_node!(&'r DiarySexp<'s>, AstNode::DiarySexp);
to_ast_node!(&'r Planning<'s>, AstNode::Planning);
to_ast_node!(&'r FixedWidthArea<'s>, AstNode::FixedWidthArea);
to_ast_node!(&'r HorizontalRule<'s>, AstNode::HorizontalRule);
to_ast_node!(&'r Keyword<'s>, AstNode::Keyword);
to_ast_node!(&'r BabelCall<'s>, AstNode::BabelCall);
to_ast_node!(&'r LatexEnvironment<'s>, AstNode::LatexEnvironment);
to_ast_node!(&'r Bold<'s>, AstNode::Bold);
to_ast_node!(&'r Italic<'s>, AstNode::Italic);
to_ast_node!(&'r Underline<'s>, AstNode::Underline);
to_ast_node!(&'r StrikeThrough<'s>, AstNode::StrikeThrough);
to_ast_node!(&'r Code<'s>, AstNode::Code);
to_ast_node!(&'r Verbatim<'s>, AstNode::Verbatim);
to_ast_node!(&'r PlainText<'s>, AstNode::PlainText);
to_ast_node!(&'r RegularLink<'s>, AstNode::RegularLink);
to_ast_node!(&'r RadioLink<'s>, AstNode::RadioLink);
to_ast_node!(&'r RadioTarget<'s>, AstNode::RadioTarget);
to_ast_node!(&'r PlainLink<'s>, AstNode::PlainLink);
to_ast_node!(&'r AngleLink<'s>, AstNode::AngleLink);
to_ast_node!(&'r OrgMacro<'s>, AstNode::OrgMacro);
to_ast_node!(&'r Entity<'s>, AstNode::Entity);
to_ast_node!(&'r LatexFragment<'s>, AstNode::LatexFragment);
to_ast_node!(&'r ExportSnippet<'s>, AstNode::ExportSnippet);
to_ast_node!(&'r FootnoteReference<'s>, AstNode::FootnoteReference);
to_ast_node!(&'r Citation<'s>, AstNode::Citation);
to_ast_node!(&'r CitationReference<'s>, AstNode::CitationReference);
to_ast_node!(&'r InlineBabelCall<'s>, AstNode::InlineBabelCall);
to_ast_node!(&'r InlineSourceBlock<'s>, AstNode::InlineSourceBlock);
to_ast_node!(&'r LineBreak<'s>, AstNode::LineBreak);
to_ast_node!(&'r Target<'s>, AstNode::Target);
to_ast_node!(&'r StatisticsCookie<'s>, AstNode::StatisticsCookie);
to_ast_node!(&'r Subscript<'s>, AstNode::Subscript);
to_ast_node!(&'r Superscript<'s>, AstNode::Superscript);
to_ast_node!(&'r TableCell<'s>, AstNode::TableCell);
to_ast_node!(&'r Timestamp<'s>, AstNode::Timestamp);

impl<'r, 's> StandardProperties<'s> for AstNode<'r, 's> {
    fn get_source<'b>(&'b self) -> &'s str {
        match self {
            AstNode::Document(inner) => inner.get_source(),
            AstNode::Heading(inner) => inner.get_source(),
            AstNode::Section(inner) => inner.get_source(),
            AstNode::Paragraph(inner) => inner.get_source(),
            AstNode::PlainList(inner) => inner.get_source(),
            AstNode::PlainListItem(inner) => inner.get_source(),
            AstNode::CenterBlock(inner) => inner.get_source(),
            AstNode::QuoteBlock(inner) => inner.get_source(),
            AstNode::SpecialBlock(inner) => inner.get_source(),
            AstNode::DynamicBlock(inner) => inner.get_source(),
            AstNode::FootnoteDefinition(inner) => inner.get_source(),
            AstNode::Comment(inner) => inner.get_source(),
            AstNode::Drawer(inner) => inner.get_source(),
            AstNode::PropertyDrawer(inner) => inner.get_source(),
            AstNode::NodeProperty(inner) => inner.get_source(),
            AstNode::Table(inner) => inner.get_source(),
            AstNode::TableRow(inner) => inner.get_source(),
            AstNode::VerseBlock(inner) => inner.get_source(),
            AstNode::CommentBlock(inner) => inner.get_source(),
            AstNode::ExampleBlock(inner) => inner.get_source(),
            AstNode::ExportBlock(inner) => inner.get_source(),
            AstNode::SrcBlock(inner) => inner.get_source(),
            AstNode::Clock(inner) => inner.get_source(),
            AstNode::DiarySexp(inner) => inner.get_source(),
            AstNode::Planning(inner) => inner.get_source(),
            AstNode::FixedWidthArea(inner) => inner.get_source(),
            AstNode::HorizontalRule(inner) => inner.get_source(),
            AstNode::Keyword(inner) => inner.get_source(),
            AstNode::BabelCall(inner) => inner.get_source(),
            AstNode::LatexEnvironment(inner) => inner.get_source(),
            AstNode::Bold(inner) => inner.get_source(),
            AstNode::Italic(inner) => inner.get_source(),
            AstNode::Underline(inner) => inner.get_source(),
            AstNode::StrikeThrough(inner) => inner.get_source(),
            AstNode::Code(inner) => inner.get_source(),
            AstNode::Verbatim(inner) => inner.get_source(),
            AstNode::PlainText(inner) => inner.get_source(),
            AstNode::RegularLink(inner) => inner.get_source(),
            AstNode::RadioLink(inner) => inner.get_source(),
            AstNode::RadioTarget(inner) => inner.get_source(),
            AstNode::PlainLink(inner) => inner.get_source(),
            AstNode::AngleLink(inner) => inner.get_source(),
            AstNode::OrgMacro(inner) => inner.get_source(),
            AstNode::Entity(inner) => inner.get_source(),
            AstNode::LatexFragment(inner) => inner.get_source(),
            AstNode::ExportSnippet(inner) => inner.get_source(),
            AstNode::FootnoteReference(inner) => inner.get_source(),
            AstNode::Citation(inner) => inner.get_source(),
            AstNode::CitationReference(inner) => inner.get_source(),
            AstNode::InlineBabelCall(inner) => inner.get_source(),
            AstNode::InlineSourceBlock(inner) => inner.get_source(),
            AstNode::LineBreak(inner) => inner.get_source(),
            AstNode::Target(inner) => inner.get_source(),
            AstNode::StatisticsCookie(inner) => inner.get_source(),
            AstNode::Subscript(inner) => inner.get_source(),
            AstNode::Superscript(inner) => inner.get_source(),
            AstNode::TableCell(inner) => inner.get_source(),
            AstNode::Timestamp(inner) => inner.get_source(),
        }
    }

    fn get_contents<'b>(&'b self) -> Option<&'s str> {
        match self {
            AstNode::Document(inner) => inner.get_contents(),
            AstNode::Heading(inner) => inner.get_contents(),
            AstNode::Section(inner) => inner.get_contents(),
            AstNode::Paragraph(inner) => inner.get_contents(),
            AstNode::PlainList(inner) => inner.get_contents(),
            AstNode::PlainListItem(inner) => inner.get_contents(),
            AstNode::CenterBlock(inner) => inner.get_contents(),
            AstNode::QuoteBlock(inner) => inner.get_contents(),
            AstNode::SpecialBlock(inner) => inner.get_contents(),
            AstNode::DynamicBlock(inner) => inner.get_contents(),
            AstNode::FootnoteDefinition(inner) => inner.get_contents(),
            AstNode::Comment(inner) => inner.get_contents(),
            AstNode::Drawer(inner) => inner.get_contents(),
            AstNode::PropertyDrawer(inner) => inner.get_contents(),
            AstNode::NodeProperty(inner) => inner.get_contents(),
            AstNode::Table(inner) => inner.get_contents(),
            AstNode::TableRow(inner) => inner.get_contents(),
            AstNode::VerseBlock(inner) => inner.get_contents(),
            AstNode::CommentBlock(inner) => inner.get_contents(),
            AstNode::ExampleBlock(inner) => inner.get_contents(),
            AstNode::ExportBlock(inner) => inner.get_contents(),
            AstNode::SrcBlock(inner) => inner.get_contents(),
            AstNode::Clock(inner) => inner.get_contents(),
            AstNode::DiarySexp(inner) => inner.get_contents(),
            AstNode::Planning(inner) => inner.get_contents(),
            AstNode::FixedWidthArea(inner) => inner.get_contents(),
            AstNode::HorizontalRule(inner) => inner.get_contents(),
            AstNode::Keyword(inner) => inner.get_contents(),
            AstNode::BabelCall(inner) => inner.get_contents(),
            AstNode::LatexEnvironment(inner) => inner.get_contents(),
            AstNode::Bold(inner) => inner.get_contents(),
            AstNode::Italic(inner) => inner.get_contents(),
            AstNode::Underline(inner) => inner.get_contents(),
            AstNode::StrikeThrough(inner) => inner.get_contents(),
            AstNode::Code(inner) => inner.get_contents(),
            AstNode::Verbatim(inner) => inner.get_contents(),
            AstNode::PlainText(inner) => inner.get_contents(),
            AstNode::RegularLink(inner) => inner.get_contents(),
            AstNode::RadioLink(inner) => inner.get_contents(),
            AstNode::RadioTarget(inner) => inner.get_contents(),
            AstNode::PlainLink(inner) => inner.get_contents(),
            AstNode::AngleLink(inner) => inner.get_contents(),
            AstNode::OrgMacro(inner) => inner.get_contents(),
            AstNode::Entity(inner) => inner.get_contents(),
            AstNode::LatexFragment(inner) => inner.get_contents(),
            AstNode::ExportSnippet(inner) => inner.get_contents(),
            AstNode::FootnoteReference(inner) => inner.get_contents(),
            AstNode::Citation(inner) => inner.get_contents(),
            AstNode::CitationReference(inner) => inner.get_contents(),
            AstNode::InlineBabelCall(inner) => inner.get_contents(),
            AstNode::InlineSourceBlock(inner) => inner.get_contents(),
            AstNode::LineBreak(inner) => inner.get_contents(),
            AstNode::Target(inner) => inner.get_contents(),
            AstNode::StatisticsCookie(inner) => inner.get_contents(),
            AstNode::Subscript(inner) => inner.get_contents(),
            AstNode::Superscript(inner) => inner.get_contents(),
            AstNode::TableCell(inner) => inner.get_contents(),
            AstNode::Timestamp(inner) => inner.get_contents(),
        }
    }

    fn get_post_blank(&self) -> PostBlank {
        match self {
            AstNode::Document(inner) => inner.get_post_blank(),
            AstNode::Heading(inner) => inner.get_post_blank(),
            AstNode::Section(inner) => inner.get_post_blank(),
            AstNode::Paragraph(inner) => inner.get_post_blank(),
            AstNode::PlainList(inner) => inner.get_post_blank(),
            AstNode::PlainListItem(inner) => inner.get_post_blank(),
            AstNode::CenterBlock(inner) => inner.get_post_blank(),
            AstNode::QuoteBlock(inner) => inner.get_post_blank(),
            AstNode::SpecialBlock(inner) => inner.get_post_blank(),
            AstNode::DynamicBlock(inner) => inner.get_post_blank(),
            AstNode::FootnoteDefinition(inner) => inner.get_post_blank(),
            AstNode::Comment(inner) => inner.get_post_blank(),
            AstNode::Drawer(inner) => inner.get_post_blank(),
            AstNode::PropertyDrawer(inner) => inner.get_post_blank(),
            AstNode::NodeProperty(inner) => inner.get_post_blank(),
            AstNode::Table(inner) => inner.get_post_blank(),
            AstNode::TableRow(inner) => inner.get_post_blank(),
            AstNode::VerseBlock(inner) => inner.get_post_blank(),
            AstNode::CommentBlock(inner) => inner.get_post_blank(),
            AstNode::ExampleBlock(inner) => inner.get_post_blank(),
            AstNode::ExportBlock(inner) => inner.get_post_blank(),
            AstNode::SrcBlock(inner) => inner.get_post_blank(),
            AstNode::Clock(inner) => inner.get_post_blank(),
            AstNode::DiarySexp(inner) => inner.get_post_blank(),
            AstNode::Planning(inner) => inner.get_post_blank(),
            AstNode::FixedWidthArea(inner) => inner.get_post_blank(),
            AstNode::HorizontalRule(inner) => inner.get_post_blank(),
            AstNode::Keyword(inner) => inner.get_post_blank(),
            AstNode::BabelCall(inner) => inner.get_post_blank(),
            AstNode::LatexEnvironment(inner) => inner.get_post_blank(),
            AstNode::Bold(inner) => inner.get_post_blank(),
            AstNode::Italic(inner) => inner.get_post_blank(),
            AstNode::Underline(inner) => inner.get_post_blank(),
            AstNode::StrikeThrough(inner) => inner.get_post_blank(),
            AstNode::Code(inner) => inner.get_post_blank(),
            AstNode::Verbatim(inner) => inner.get_post_blank(),
            AstNode::PlainText(inner) => inner.get_post_blank(),
            AstNode::RegularLink(inner) => inner.get_post_blank(),
            AstNode::RadioLink(inner) => inner.get_post_blank(),
            AstNode::RadioTarget(inner) => inner.get_post_blank(),
            AstNode::PlainLink(inner) => inner.get_post_blank(),
            AstNode::AngleLink(inner) => inner.get_post_blank(),
            AstNode::OrgMacro(inner) => inner.get_post_blank(),
            AstNode::Entity(inner) => inner.get_post_blank(),
            AstNode::LatexFragment(inner) => inner.get_post_blank(),
            AstNode::ExportSnippet(inner) => inner.get_post_blank(),
            AstNode::FootnoteReference(inner) => inner.get_post_blank(),
            AstNode::Citation(inner) => inner.get_post_blank(),
            AstNode::CitationReference(inner) => inner.get_post_blank(),
            AstNode::InlineBabelCall(inner) => inner.get_post_blank(),
            AstNode::InlineSourceBlock(inner) => inner.get_post_blank(),
            AstNode::LineBreak(inner) => inner.get_post_blank(),
            AstNode::Target(inner) => inner.get_post_blank(),
            AstNode::StatisticsCookie(inner) => inner.get_post_blank(),
            AstNode::Subscript(inner) => inner.get_post_blank(),
            AstNode::Superscript(inner) => inner.get_post_blank(),
            AstNode::TableCell(inner) => inner.get_post_blank(),
            AstNode::Timestamp(inner) => inner.get_post_blank(),
        }
    }
}
