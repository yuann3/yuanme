use std::collections::VecDeque;

use super::ast_node_iter::AstNodeIter;
use crate::types::AstNode;

pub struct AllAstNodeIter<'r, 's> {
    root: Option<AstNode<'r, 's>>,
    queue: VecDeque<AstNodeIter<'r, 's>>,
}

impl<'r, 's> Iterator for AllAstNodeIter<'r, 's> {
    type Item = AstNode<'r, 's>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(root) = self.root.take() {
            self.queue.push_back(AstNodeIter::from_ast_node(&root));
            return Some(root);
        }
        while let Some(child) = self.queue.front_mut() {
            let next_elem_this_iter = match child {
                AstNodeIter::Document(ref mut i) => i.next(),
                AstNodeIter::Heading(ref mut i) => i.next(),
                AstNodeIter::Section(ref mut i) => i.next(),
                AstNodeIter::Paragraph(ref mut i) => i.next(),
                AstNodeIter::PlainList(ref mut i) => i.next(),
                AstNodeIter::PlainListItem(ref mut i) => i.next(),
                AstNodeIter::CenterBlock(ref mut i) => i.next(),
                AstNodeIter::QuoteBlock(ref mut i) => i.next(),
                AstNodeIter::SpecialBlock(ref mut i) => i.next(),
                AstNodeIter::DynamicBlock(ref mut i) => i.next(),
                AstNodeIter::FootnoteDefinition(ref mut i) => i.next(),
                AstNodeIter::Comment(ref mut i) => i.next(),
                AstNodeIter::Drawer(ref mut i) => i.next(),
                AstNodeIter::PropertyDrawer(ref mut i) => i.next(),
                AstNodeIter::NodeProperty(ref mut i) => i.next(),
                AstNodeIter::Table(ref mut i) => i.next(),
                AstNodeIter::TableRow(ref mut i) => i.next(),
                AstNodeIter::VerseBlock(ref mut i) => i.next(),
                AstNodeIter::CommentBlock(ref mut i) => i.next(),
                AstNodeIter::ExampleBlock(ref mut i) => i.next(),
                AstNodeIter::ExportBlock(ref mut i) => i.next(),
                AstNodeIter::SrcBlock(ref mut i) => i.next(),
                AstNodeIter::Clock(ref mut i) => i.next(),
                AstNodeIter::DiarySexp(ref mut i) => i.next(),
                AstNodeIter::Planning(ref mut i) => i.next(),
                AstNodeIter::FixedWidthArea(ref mut i) => i.next(),
                AstNodeIter::HorizontalRule(ref mut i) => i.next(),
                AstNodeIter::Keyword(ref mut i) => i.next(),
                AstNodeIter::BabelCall(ref mut i) => i.next(),
                AstNodeIter::LatexEnvironment(ref mut i) => i.next(),
                AstNodeIter::Bold(ref mut i) => i.next(),
                AstNodeIter::Italic(ref mut i) => i.next(),
                AstNodeIter::Underline(ref mut i) => i.next(),
                AstNodeIter::StrikeThrough(ref mut i) => i.next(),
                AstNodeIter::Code(ref mut i) => i.next(),
                AstNodeIter::Verbatim(ref mut i) => i.next(),
                AstNodeIter::PlainText(ref mut i) => i.next(),
                AstNodeIter::RegularLink(ref mut i) => i.next(),
                AstNodeIter::RadioLink(ref mut i) => i.next(),
                AstNodeIter::RadioTarget(ref mut i) => i.next(),
                AstNodeIter::PlainLink(ref mut i) => i.next(),
                AstNodeIter::AngleLink(ref mut i) => i.next(),
                AstNodeIter::OrgMacro(ref mut i) => i.next(),
                AstNodeIter::Entity(ref mut i) => i.next(),
                AstNodeIter::LatexFragment(ref mut i) => i.next(),
                AstNodeIter::ExportSnippet(ref mut i) => i.next(),
                AstNodeIter::FootnoteReference(ref mut i) => i.next(),
                AstNodeIter::Citation(ref mut i) => i.next(),
                AstNodeIter::CitationReference(ref mut i) => i.next(),
                AstNodeIter::InlineBabelCall(ref mut i) => i.next(),
                AstNodeIter::InlineSourceBlock(ref mut i) => i.next(),
                AstNodeIter::LineBreak(ref mut i) => i.next(),
                AstNodeIter::Target(ref mut i) => i.next(),
                AstNodeIter::StatisticsCookie(ref mut i) => i.next(),
                AstNodeIter::Subscript(ref mut i) => i.next(),
                AstNodeIter::Superscript(ref mut i) => i.next(),
                AstNodeIter::TableCell(ref mut i) => i.next(),
                AstNodeIter::Timestamp(ref mut i) => i.next(),
            };
            if let Some(next_elem_this_iter) = next_elem_this_iter {
                self.queue
                    .push_back(AstNodeIter::from_ast_node(&next_elem_this_iter));
                return Some(next_elem_this_iter);
            } else {
                self.queue.pop_front();
            }
        }

        None
    }
}

impl<'r, 's> AstNode<'r, 's> {
    /// Iterate all AST nodes.
    ///
    /// This is different from the iter/into_iter functions which iterate a single level of the children. This iterates the entire tree including returning the root node itself.
    pub fn iter_all_ast_nodes(self) -> AllAstNodeIter<'r, 's> {
        AllAstNodeIter {
            root: Some(self),
            queue: VecDeque::new(),
        }
    }
}
