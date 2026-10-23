use std::marker::PhantomData;

use nom::combinator::eof;
use nom::IResult;

use super::exiting::ExitClass;
use super::global_settings::GlobalSettings;
use super::list::List;
use super::DynContextMatcher;
use super::RefContext;
use crate::error::CustomError;
use crate::error::Res;
use crate::parser::OrgSource;

pub(crate) enum ContextElement<'r, 's> {
    /// Stores a parser that indicates that children should exit upon matching an exit matcher.
    ExitMatcherNode(ExitMatcherNode<'r>),

    /// Stores the name of the current element to prevent directly nesting elements of the same type.
    Context(&'r str),

    /// Indicates if elements should consume the whitespace after them.
    ConsumeTrailingWhitespace(bool),

    /// This is just here to use the 's lifetime until I'm sure we can eliminate it from ContextElement.
    #[allow(dead_code)]
    Placeholder(PhantomData<&'s str>),
}

pub(crate) struct ExitMatcherNode<'r> {
    // TODO: Should this be "&'r DynContextMatcher<'c>" ?
    pub(crate) exit_matcher: &'r DynContextMatcher<'r>,
    pub(crate) class: ExitClass,
}

pub(crate) struct Context<'g, 'r, 's> {
    global_settings: &'g GlobalSettings<'g, 's>,
    tree: List<'r, &'r ContextElement<'r, 's>>,
}

impl<'g, 'r, 's> Context<'g, 'r, 's> {
    pub(crate) fn new(
        global_settings: &'g GlobalSettings<'g, 's>,
        tree: List<'r, &'r ContextElement<'r, 's>>,
    ) -> Self {
        Self {
            global_settings,
            tree,
        }
    }

    pub(crate) fn with_additional_node(&'r self, new_element: &'r ContextElement<'r, 's>) -> Self {
        let new_tree = self.tree.push(new_element);
        Self::new(self.global_settings, new_tree)
    }

    pub(crate) fn iter(&'r self) -> super::list::Iter<'r, &'r ContextElement<'r, 's>> {
        self.tree.iter()
    }

    fn iter_context(&'r self) -> Iter<'g, 'r, 's> {
        Iter {
            next: self.tree.iter_list(),
            global_settings: self.global_settings,
        }
    }

    pub(crate) fn get_parent(&'r self) -> Option<Self> {
        self.tree.get_parent().map(|parent_tree| Self {
            global_settings: self.global_settings,
            tree: parent_tree.clone(),
        })
    }

    fn get_data(&self) -> &ContextElement<'r, 's> {
        self.tree.get_data()
    }

    pub(crate) fn get_global_settings(&self) -> &'g GlobalSettings<'g, 's> {
        self.global_settings
    }

    pub(crate) fn with_global_settings<'gg>(
        &self,
        new_settings: &'gg GlobalSettings<'gg, 's>,
    ) -> Context<'gg, 'r, 's> {
        Context {
            global_settings: new_settings,
            tree: self.tree.clone(),
        }
    }

    pub(crate) fn check_exit_matcher(
        &'r self,
        i: OrgSource<'s>,
    ) -> IResult<OrgSource<'s>, OrgSource<'s>, CustomError> {
        let mut current_class_filter = ExitClass::Gamma;
        for current_node in self.iter_context() {
            let context_element = current_node.get_data();
            if let ContextElement::ExitMatcherNode(exit_matcher) = context_element {
                if exit_matcher.class as u32 <= current_class_filter as u32 {
                    current_class_filter = exit_matcher.class;
                    let local_result = (exit_matcher.exit_matcher)(&current_node, i);
                    if local_result.is_ok() {
                        return local_result;
                    }
                }
            }
        }
        // TODO: Make this a specific error instead of just a generic MyError
        Err(nom::Err::Error(CustomError::Static("NoExit")))
    }

    /// Indicates if elements should consume the whitespace after them.
    ///
    /// Defaults to true.
    pub(crate) fn should_consume_trailing_whitespace(&self) -> bool {
        self._should_consume_trailing_whitespace().unwrap_or(true)
    }

    fn _should_consume_trailing_whitespace(&self) -> Option<bool> {
        for current_node in self.iter() {
            if let ContextElement::ConsumeTrailingWhitespace(should) = current_node {
                return Some(*should);
            }
        }
        None
    }
}

fn document_end<'b, 'g, 'r, 's>(
    _context: RefContext<'b, 'g, 'r, 's>,
    input: OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>> {
    eof(input)
}

struct Iter<'g, 'r, 's> {
    global_settings: &'g GlobalSettings<'g, 's>,
    next: super::list::IterList<'r, &'r ContextElement<'r, 's>>,
}

impl<'g, 'r, 's> Iterator for Iter<'g, 'r, 's> {
    type Item = Context<'g, 'r, 's>;

    fn next(&mut self) -> Option<Self::Item> {
        let next_tree = self.next.next();
        let ret =
            next_tree.map(|parent_tree| Context::new(self.global_settings, parent_tree.clone()));
        ret
    }
}

impl<'r, 's> ContextElement<'r, 's> {
    pub(crate) fn document_context() -> Self {
        Self::ExitMatcherNode(ExitMatcherNode {
            exit_matcher: &document_end,
            class: ExitClass::Document,
        })
    }
}
