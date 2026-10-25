use alloc::vec::Vec;
use core::ops::Range;

use kstring::KString;

use crate::FxHashMap;
use crate::custom_cmds::CustomCmds;

#[derive(Debug, Default)]
pub(crate) struct GlobalState {
    /// This is used for numbering equations in the document.
    pub(crate) equation_count: usize,
    /// This is used for resolving references to equations in the document. The keys are the labels
    /// defined in the document, and the values are the corresponding equation numbers (as strings).
    pub(crate) label_map: FxHashMap<KString, KString>,
    /// The commands which the document defines for itself, with `\newcommand`, when the
    /// conversion runs in the global group. They stay defined for the snippets which follow.
    pub(crate) custom_cmds: CustomCmds,
    /// Every `\label`, in order, with where it is written and the tag it was given, if its
    /// equation has a number. Unlike `label_map`, this keeps a label which is given twice.
    pub(crate) labels: Vec<(KString, Range<usize>, Option<KString>)>,
    /// Every `\eqref`, in order, with where it is written.
    pub(crate) references: Vec<(KString, Range<usize>)>,
    /// The bytes of LaTeX converted so far, and the tokens expansions of custom commands have
    /// produced from them. See [`GlobalState::charge_expansion`].
    pub(crate) written: usize,
    pub(crate) expanded: usize,
}

/// How many tokens expansions may produce for each byte of LaTeX, and once more.
const TOKENS_PER_BYTE: usize = 8;
const TOKENS: usize = 10_000;

impl GlobalState {
    /// Accounts for `tokens` more tokens from expanding a custom command, and returns `false`
    /// if expansions have now produced too many for the LaTeX converted so far.
    ///
    /// `MaxExpansions` limits how many expansions there are, not what they produce: a command
    /// whose body is long, or which expands to others several times over, turns a few bytes
    /// into a great many tokens, and so a snippet into a conversion that takes seconds. The
    /// budget is shared by every snippet converted with this state, so that one with many
    /// small snippets can't claim the constant part over and over.
    pub(crate) fn charge_expansion(&mut self, tokens: usize) -> bool {
        self.expanded = self.expanded.saturating_add(tokens);
        self.expanded <= TOKENS.saturating_add(self.written.saturating_mul(TOKENS_PER_BYTE))
    }

    /// Records the tag of the equation which `label` names.
    pub(crate) fn tag_label(&mut self, label: &str, tag: &str) {
        self.label_map
            .insert(KString::from_ref(label), KString::from_ref(tag));
        if let Some((_, _, t)) = self
            .labels
            .iter_mut()
            .rev()
            .find(|(name, _, t)| name.as_str() == label && t.is_none())
        {
            *t = Some(KString::from_ref(tag));
        }
    }
}
