use std::marker::PhantomData;

use super::macros::children_iter;
use super::macros::empty_iter;
use super::macros::multi_field_iter;
use crate::types::AngleLink;
use crate::types::AstNode;
use crate::types::BabelCall;
use crate::types::Bold;
use crate::types::CenterBlock;
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
use crate::types::QuoteBlock;
use crate::types::RadioLink;
use crate::types::RadioTarget;
use crate::types::RegularLink;
use crate::types::Section;
use crate::types::SpecialBlock;
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

/// Iterator over the AST nodes contained within the starting node.
///
/// This only iterates over the children, not the starting node itself. So an AstNodeIter::PlainList would only return PlainListItems, not the PlainList.
///
/// This only iterates over AST nodes, so an AstNodeIter::Heading would iterate over both the title and section contents, but it would not iterate over simple strings like the TODO keyword or priority.
pub(crate) enum AstNodeIter<'r, 's> {
    // Document Nodes
    Document(DocumentIter<'r, 's>),
    Heading(HeadingIter<'r, 's>),
    Section(SectionIter<'r, 's>),
    // Elements
    Paragraph(ParagraphIter<'r, 's>),
    PlainList(PlainListIter<'r, 's>),
    PlainListItem(PlainListItemIter<'r, 's>),
    CenterBlock(CenterBlockIter<'r, 's>),
    QuoteBlock(QuoteBlockIter<'r, 's>),
    SpecialBlock(SpecialBlockIter<'r, 's>),
    DynamicBlock(DynamicBlockIter<'r, 's>),
    FootnoteDefinition(FootnoteDefinitionIter<'r, 's>),
    Comment(CommentIter<'r, 's>),
    Drawer(DrawerIter<'r, 's>),
    PropertyDrawer(PropertyDrawerIter<'r, 's>),
    NodeProperty(NodePropertyIter<'r, 's>),
    Table(TableIter<'r, 's>),
    TableRow(TableRowIter<'r, 's>),
    VerseBlock(VerseBlockIter<'r, 's>),
    CommentBlock(CommentBlockIter<'r, 's>),
    ExampleBlock(ExampleBlockIter<'r, 's>),
    ExportBlock(ExportBlockIter<'r, 's>),
    SrcBlock(SrcBlockIter<'r, 's>),
    Clock(ClockIter<'r, 's>),
    DiarySexp(DiarySexpIter<'r, 's>),
    Planning(PlanningIter<'r, 's>),
    FixedWidthArea(FixedWidthAreaIter<'r, 's>),
    HorizontalRule(HorizontalRuleIter<'r, 's>),
    Keyword(KeywordIter<'r, 's>),
    BabelCall(BabelCallIter<'r, 's>),
    LatexEnvironment(LatexEnvironmentIter<'r, 's>),
    // Objects
    Bold(BoldIter<'r, 's>),
    Italic(ItalicIter<'r, 's>),
    Underline(UnderlineIter<'r, 's>),
    StrikeThrough(StrikeThroughIter<'r, 's>),
    Code(CodeIter<'r, 's>),
    Verbatim(VerbatimIter<'r, 's>),
    PlainText(PlainTextIter<'r, 's>),
    RegularLink(RegularLinkIter<'r, 's>),
    RadioLink(RadioLinkIter<'r, 's>),
    RadioTarget(RadioTargetIter<'r, 's>),
    PlainLink(PlainLinkIter<'r, 's>),
    AngleLink(AngleLinkIter<'r, 's>),
    OrgMacro(OrgMacroIter<'r, 's>),
    Entity(EntityIter<'r, 's>),
    LatexFragment(LatexFragmentIter<'r, 's>),
    ExportSnippet(ExportSnippetIter<'r, 's>),
    FootnoteReference(FootnoteReferenceIter<'r, 's>),
    Citation(CitationIter<'r, 's>),
    CitationReference(CitationReferenceIter<'r, 's>),
    InlineBabelCall(InlineBabelCallIter<'r, 's>),
    InlineSourceBlock(InlineSourceBlockIter<'r, 's>),
    LineBreak(LineBreakIter<'r, 's>),
    Target(TargetIter<'r, 's>),
    StatisticsCookie(StatisticsCookieIter<'r, 's>),
    Subscript(SubscriptIter<'r, 's>),
    Superscript(SuperscriptIter<'r, 's>),
    TableCell(TableCellIter<'r, 's>),
    Timestamp(TimestampIter<'r, 's>),
}

impl<'r, 's> AstNodeIter<'r, 's> {
    pub(crate) fn from_ast_node(node: &AstNode<'r, 's>) -> AstNodeIter<'r, 's> {
        match node {
            AstNode::Document(inner) => AstNodeIter::Document(inner.into_iter()),
            AstNode::Heading(inner) => AstNodeIter::Heading(inner.into_iter()),
            AstNode::Section(inner) => AstNodeIter::Section(inner.into_iter()),
            AstNode::Paragraph(inner) => AstNodeIter::Paragraph(inner.into_iter()),
            AstNode::PlainList(inner) => AstNodeIter::PlainList(inner.into_iter()),
            AstNode::PlainListItem(inner) => AstNodeIter::PlainListItem(inner.into_iter()),
            AstNode::CenterBlock(inner) => AstNodeIter::CenterBlock(inner.into_iter()),
            AstNode::QuoteBlock(inner) => AstNodeIter::QuoteBlock(inner.into_iter()),
            AstNode::SpecialBlock(inner) => AstNodeIter::SpecialBlock(inner.into_iter()),
            AstNode::DynamicBlock(inner) => AstNodeIter::DynamicBlock(inner.into_iter()),
            AstNode::FootnoteDefinition(inner) => {
                AstNodeIter::FootnoteDefinition(inner.into_iter())
            }
            AstNode::Comment(inner) => AstNodeIter::Comment(inner.into_iter()),
            AstNode::Drawer(inner) => AstNodeIter::Drawer(inner.into_iter()),
            AstNode::PropertyDrawer(inner) => AstNodeIter::PropertyDrawer(inner.into_iter()),
            AstNode::NodeProperty(inner) => AstNodeIter::NodeProperty(inner.into_iter()),
            AstNode::Table(inner) => AstNodeIter::Table(inner.into_iter()),
            AstNode::TableRow(inner) => AstNodeIter::TableRow(inner.into_iter()),
            AstNode::VerseBlock(inner) => AstNodeIter::VerseBlock(inner.into_iter()),
            AstNode::CommentBlock(inner) => AstNodeIter::CommentBlock(inner.into_iter()),
            AstNode::ExampleBlock(inner) => AstNodeIter::ExampleBlock(inner.into_iter()),
            AstNode::ExportBlock(inner) => AstNodeIter::ExportBlock(inner.into_iter()),
            AstNode::SrcBlock(inner) => AstNodeIter::SrcBlock(inner.into_iter()),
            AstNode::Clock(inner) => AstNodeIter::Clock(inner.into_iter()),
            AstNode::DiarySexp(inner) => AstNodeIter::DiarySexp(inner.into_iter()),
            AstNode::Planning(inner) => AstNodeIter::Planning(inner.into_iter()),
            AstNode::FixedWidthArea(inner) => AstNodeIter::FixedWidthArea(inner.into_iter()),
            AstNode::HorizontalRule(inner) => AstNodeIter::HorizontalRule(inner.into_iter()),
            AstNode::Keyword(inner) => AstNodeIter::Keyword(inner.into_iter()),
            AstNode::BabelCall(inner) => AstNodeIter::BabelCall(inner.into_iter()),
            AstNode::LatexEnvironment(inner) => AstNodeIter::LatexEnvironment(inner.into_iter()),
            AstNode::Bold(inner) => AstNodeIter::Bold(inner.into_iter()),
            AstNode::Italic(inner) => AstNodeIter::Italic(inner.into_iter()),
            AstNode::Underline(inner) => AstNodeIter::Underline(inner.into_iter()),
            AstNode::StrikeThrough(inner) => AstNodeIter::StrikeThrough(inner.into_iter()),
            AstNode::Code(inner) => AstNodeIter::Code(inner.into_iter()),
            AstNode::Verbatim(inner) => AstNodeIter::Verbatim(inner.into_iter()),
            AstNode::PlainText(inner) => AstNodeIter::PlainText(inner.into_iter()),
            AstNode::RegularLink(inner) => AstNodeIter::RegularLink(inner.into_iter()),
            AstNode::RadioLink(inner) => AstNodeIter::RadioLink(inner.into_iter()),
            AstNode::RadioTarget(inner) => AstNodeIter::RadioTarget(inner.into_iter()),
            AstNode::PlainLink(inner) => AstNodeIter::PlainLink(inner.into_iter()),
            AstNode::AngleLink(inner) => AstNodeIter::AngleLink(inner.into_iter()),
            AstNode::OrgMacro(inner) => AstNodeIter::OrgMacro(inner.into_iter()),
            AstNode::Entity(inner) => AstNodeIter::Entity(inner.into_iter()),
            AstNode::LatexFragment(inner) => AstNodeIter::LatexFragment(inner.into_iter()),
            AstNode::ExportSnippet(inner) => AstNodeIter::ExportSnippet(inner.into_iter()),
            AstNode::FootnoteReference(inner) => AstNodeIter::FootnoteReference(inner.into_iter()),
            AstNode::Citation(inner) => AstNodeIter::Citation(inner.into_iter()),
            AstNode::CitationReference(inner) => AstNodeIter::CitationReference(inner.into_iter()),
            AstNode::InlineBabelCall(inner) => AstNodeIter::InlineBabelCall(inner.into_iter()),
            AstNode::InlineSourceBlock(inner) => AstNodeIter::InlineSourceBlock(inner.into_iter()),
            AstNode::LineBreak(inner) => AstNodeIter::LineBreak(inner.into_iter()),
            AstNode::Target(inner) => AstNodeIter::Target(inner.into_iter()),
            AstNode::StatisticsCookie(inner) => AstNodeIter::StatisticsCookie(inner.into_iter()),
            AstNode::Subscript(inner) => AstNodeIter::Subscript(inner.into_iter()),
            AstNode::Superscript(inner) => AstNodeIter::Superscript(inner.into_iter()),
            AstNode::TableCell(inner) => AstNodeIter::TableCell(inner.into_iter()),
            AstNode::Timestamp(inner) => AstNodeIter::Timestamp(inner.into_iter()),
        }
    }
}

multi_field_iter!(
    Document<'s>,
    DocumentIter,
    zeroth_section,
    std::option::Iter<'r, Section<'s>>,
    children,
    std::slice::Iter<'r, Heading<'s>>
);
multi_field_iter!(
    Heading<'s>,
    HeadingIter,
    title,
    std::slice::Iter<'r, Object<'s>>,
    children,
    std::slice::Iter<'r, DocumentElement<'s>>
);
children_iter!(Section<'s>, SectionIter, std::slice::Iter<'r, Element<'s>>);
children_iter!(
    Paragraph<'s>,
    ParagraphIter,
    std::slice::Iter<'r, Object<'s>>
);
children_iter!(
    PlainList<'s>,
    PlainListIter,
    std::slice::Iter<'r, PlainListItem<'s>>
);
multi_field_iter!(
    PlainListItem<'s>,
    PlainListItemIter,
    tag,
    std::slice::Iter<'r, Object<'s>>,
    children,
    std::slice::Iter<'r, Element<'s>>
);
children_iter!(
    CenterBlock<'s>,
    CenterBlockIter,
    std::slice::Iter<'r, Element<'s>>
);
children_iter!(
    QuoteBlock<'s>,
    QuoteBlockIter,
    std::slice::Iter<'r, Element<'s>>
);
children_iter!(
    SpecialBlock<'s>,
    SpecialBlockIter,
    std::slice::Iter<'r, Element<'s>>
);
children_iter!(
    DynamicBlock<'s>,
    DynamicBlockIter,
    std::slice::Iter<'r, Element<'s>>
);
children_iter!(
    FootnoteDefinition<'s>,
    FootnoteDefinitionIter,
    std::slice::Iter<'r, Element<'s>>
);
empty_iter!(Comment<'s>, CommentIter);
children_iter!(Drawer<'s>, DrawerIter, std::slice::Iter<'r, Element<'s>>);
children_iter!(
    PropertyDrawer<'s>,
    PropertyDrawerIter,
    std::slice::Iter<'r, NodeProperty<'s>>
);
empty_iter!(NodeProperty<'s>, NodePropertyIter);
children_iter!(Table<'s>, TableIter, std::slice::Iter<'r, TableRow<'s>>);
children_iter!(
    TableRow<'s>,
    TableRowIter,
    std::slice::Iter<'r, TableCell<'s>>
);
children_iter!(
    VerseBlock<'s>,
    VerseBlockIter,
    std::slice::Iter<'r, Object<'s>>
);
empty_iter!(CommentBlock<'s>, CommentBlockIter);
empty_iter!(ExampleBlock<'s>, ExampleBlockIter);
empty_iter!(ExportBlock<'s>, ExportBlockIter);
empty_iter!(SrcBlock<'s>, SrcBlockIter);
empty_iter!(Clock<'s>, ClockIter);
empty_iter!(DiarySexp<'s>, DiarySexpIter);
empty_iter!(Planning<'s>, PlanningIter);
empty_iter!(FixedWidthArea<'s>, FixedWidthAreaIter);
empty_iter!(HorizontalRule<'s>, HorizontalRuleIter);
empty_iter!(Keyword<'s>, KeywordIter);
empty_iter!(BabelCall<'s>, BabelCallIter);
empty_iter!(LatexEnvironment<'s>, LatexEnvironmentIter);
children_iter!(Bold<'s>, BoldIter, std::slice::Iter<'r, Object<'s>>);
children_iter!(Italic<'s>, ItalicIter, std::slice::Iter<'r, Object<'s>>);
children_iter!(
    Underline<'s>,
    UnderlineIter,
    std::slice::Iter<'r, Object<'s>>
);
children_iter!(
    StrikeThrough<'s>,
    StrikeThroughIter,
    std::slice::Iter<'r, Object<'s>>
);
empty_iter!(Code<'s>, CodeIter);
empty_iter!(Verbatim<'s>, VerbatimIter);
empty_iter!(PlainText<'s>, PlainTextIter);
empty_iter!(RegularLink<'s>, RegularLinkIter);
children_iter!(
    RadioLink<'s>,
    RadioLinkIter,
    std::slice::Iter<'r, Object<'s>>
);
children_iter!(
    RadioTarget<'s>,
    RadioTargetIter,
    std::slice::Iter<'r, Object<'s>>
);
empty_iter!(PlainLink<'s>, PlainLinkIter);
empty_iter!(AngleLink<'s>, AngleLinkIter);
empty_iter!(OrgMacro<'s>, OrgMacroIter);
empty_iter!(Entity<'s>, EntityIter);
empty_iter!(LatexFragment<'s>, LatexFragmentIter);
empty_iter!(ExportSnippet<'s>, ExportSnippetIter);
multi_field_iter!(
    FootnoteReference<'s>,
    FootnoteReferenceIter,
    definition,
    std::slice::Iter<'r, Object<'s>>,
);
empty_iter!(Citation<'s>, CitationIter);
empty_iter!(CitationReference<'s>, CitationReferenceIter);
empty_iter!(InlineBabelCall<'s>, InlineBabelCallIter);
empty_iter!(InlineSourceBlock<'s>, InlineSourceBlockIter);
empty_iter!(LineBreak<'s>, LineBreakIter);
empty_iter!(Target<'s>, TargetIter);
empty_iter!(StatisticsCookie<'s>, StatisticsCookieIter);
empty_iter!(Subscript<'s>, SubscriptIter);
empty_iter!(Superscript<'s>, SuperscriptIter);
children_iter!(
    TableCell<'s>,
    TableCellIter,
    std::slice::Iter<'r, Object<'s>>
);
empty_iter!(Timestamp<'s>, TimestampIter);
