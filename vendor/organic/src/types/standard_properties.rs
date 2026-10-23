// TODO: What is an anonymous AST node and how can I trigger one?
pub trait StandardProperties<'s> {
    /// Get the slice of the entire AST node.
    ///
    /// This corresponds to :begin to :end in upstream org-mode's standard properties.
    fn get_source<'b>(&'b self) -> &'s str;

    /// Get the slice of the AST node's contents.
    ///
    /// This corresponds to :contents-begin to :contents-end
    fn get_contents<'b>(&'b self) -> Option<&'s str>;

    /// Get the ast node's post-blank.
    ///
    /// For objects this is a count of the characters of whitespace after the object. For elements this is a count of the line breaks following an element.
    fn get_post_blank(&self) -> PostBlank;
}

// TODO: Write some debugging code to alert when any of the unknown fields below are non-nil in our test data so we can see what these fields represent.

// Order of upstream org-mode's standard properties array:
//
// :begin :post-affiliated :contents-begin :contents-end :end :post-blank :secondary :mode :granularity :cached :org-element--cache-sync-key :robust-begin :robust-end :true-level :buffer :deferred :structure :parent
//
// Per-field notes: (Leading character: 'X' for not going to include, 'Y' for going to include but not included yet ("Yes"), 'D' for already included ("Done"), '?' for undecided)
//
// D :begin - Number of characters (NOT bytes!) since the beginning of the file.
//
// ? :post-affiliated - ?
//
// Y :contents-begin - Number of characters (NOT bytes!) since the beginning of the file.
//
// Y :contents-end - Number of characters (NOT bytes!) since the beginning of the file.
//
// D :end - Number of characters (NOT bytes!) since the beginning of the file.
//
// Y :post-blank - Number of characters after :contents-end but before :end. This is the trailing whitespace.
//
// X :secondary - List of properties that may contain AST nodes. This will be important to reference for implementing TokenIter properly, but I see no value in including this in the StandardProperties trait since which properties contain AST nodes will be self-evident in the struct definition.
//
// ? :mode - ?
//
// ? :granularity - ?
//
// X :cached - ? Based on the name, I'm guessing this is a runtime-optimization rather than something relevant to export from a parser, so (unless I'm wrong about the purpose) I see no reason to include this.
//
// X :org-element--cache-sync-key - ? Based on the name, I'm guessing this is a runtime-optimization rather than something relevant to export from a parser, so (unless I'm wrong about the purpose) I see no reason to include this.
//
// ? :robust-begin - ? uhh what? What makes this begin/end "robust" and the others not? I have no idea.
//
// ? :robust-end - ? uhh what? What makes this begin/end "robust" and the others not? I have no idea.
//
// ? :true-level - This seems to correspond to the REAL star count for headlines (as opposed to the headline level we set for when "odd" is enabled instead of the default "oddeven"). This is great information to have, but is this a "standard" property? Does anything other than headlines have this set? I don't know, so I need to investigate. If it is headline-specific then we will not be including this in the StandardProperties trait even though it is in the :standard-properties array in org-mode.
//
// X :buffer - This is the Emacs buffer name containing the org-mode document. This seems more like a runtime thing than something we would want to export from our parser so this will not be included.
//
// X :deferred - Seems to be a runtime optimization about only calculating some properties when requested.
//
// ? :structure - ?
//
// X :parent - Some weird numeric reference to the containing object. Since we output a tree structure, I do not see any value in including this, especially considering the back-references would be a nightmare in rust.

// Special case: Plain text. Plain text counts :begin and :end from the start of the text (so :begin is always 0 AFAICT) and instead of including the full set of standard properties, it only includes :begin, :end, and :parent.

pub type PostBlank = u8;
