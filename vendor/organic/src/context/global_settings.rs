use std::collections::BTreeMap;
use std::collections::BTreeSet;

use super::constants::DEFAULT_ORG_ENTITIES;
use super::constants::DEFAULT_ORG_LINK_PARAMETERS;
use super::FileAccessInterface;
use super::LocalFileAccessInterface;
use crate::types::IndentationLevel;
use crate::types::Object;

// TODO: Ultimately, I think we'll need most of this: https://orgmode.org/manual/In_002dbuffer-Settings.html

#[derive(Clone)]
pub struct GlobalSettings<'g, 's> {
    pub radio_targets: Vec<&'g Vec<Object<'s>>>,
    pub file_access: &'g dyn FileAccessInterface,
    pub in_progress_todo_keywords: BTreeSet<String>,
    pub complete_todo_keywords: BTreeSet<String>,
    /// Set to true to allow for plain lists using single letters as the bullet in the same way that numbers are used.
    ///
    /// Corresponds to the org-list-allow-alphabetical elisp variable.
    pub list_allow_alphabetical: bool,

    /// How many spaces a tab should be equal to.
    ///
    /// Corresponds to the tab-width elisp variable.
    pub tab_width: IndentationLevel,

    /// Whether to only allow odd headline levels.
    ///
    /// Corresponds to org-odd-levels-only elisp variable.
    pub odd_levels_only: HeadlineLevelFilter,

    /// If a headline title matches this string exactly, then that section will become a "footnote section".
    ///
    /// Corresponds to org-footnote-section elisp variable.
    pub footnote_section: &'g str,

    /// The allowed protocols for links (for example, the "https" in "https://foo.bar/").
    ///
    /// Corresponds to org-link-parameters elisp variable.
    pub link_parameters: &'g [&'g str],

    /// Link templates where the key is the document text and the value is the replacement.
    ///
    /// For example, `"foo": "bar%s"` will replace `[[foo::baz]]` with `[[barbaz]]`
    ///
    /// This is set by including #+LINK in the org-mode document.
    pub link_templates: BTreeMap<String, String>,

    /// The special characters that can be written in org-mode like \infin for the infinity symbol.
    ///
    /// MUST be sorted with the largest names first. Otherwise the parser may match a shorter substring of a longer entity.
    ///
    /// Corresponds to org-entities elisp variable.
    pub entities: &'g [EntityDefinition<'s>],
}

pub const DEFAULT_TAB_WIDTH: IndentationLevel = 8;

#[derive(Debug, Clone)]
pub struct EntityDefinition<'a> {
    pub name: &'a str,
    pub latex_math_mode: bool,
    pub latex: &'a str,
    pub html: &'a str,
    pub ascii: &'a str,
    // Skipping latin1 because it is detrimental to the future. If anyone out there is using latin1, take a long look in the mirror and change your ways.
    pub utf8: &'a str,
}

impl<'g, 's> GlobalSettings<'g, 's> {
    fn new() -> GlobalSettings<'g, 's> {
        debug_assert!(DEFAULT_ORG_ENTITIES.is_sorted_by(|a, b| a.name.len() >= b.name.len()));
        GlobalSettings {
            radio_targets: Vec::new(),
            file_access: &LocalFileAccessInterface {
                working_directory: None,
            },
            in_progress_todo_keywords: BTreeSet::new(),
            complete_todo_keywords: BTreeSet::new(),
            list_allow_alphabetical: false,
            tab_width: DEFAULT_TAB_WIDTH,
            odd_levels_only: HeadlineLevelFilter::default(),
            footnote_section: "Footnotes",
            link_parameters: &DEFAULT_ORG_LINK_PARAMETERS,
            link_templates: BTreeMap::new(),
            entities: &DEFAULT_ORG_ENTITIES,
        }
    }
}

impl<'g, 's> Default for GlobalSettings<'g, 's> {
    fn default() -> GlobalSettings<'g, 's> {
        GlobalSettings::new()
    }
}

#[derive(Clone, PartialEq, Default)]
pub enum HeadlineLevelFilter {
    Odd,

    #[default]
    OddEven,
}
