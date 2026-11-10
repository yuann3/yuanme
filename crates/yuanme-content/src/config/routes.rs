//! `[routes]` and `[feeds]`: Pages made by a Template alone.

use std::ops::Range;

/// A Page made by a Template alone (#19).
#[derive(Debug)]
pub struct TemplateRoute {
    pub path: String,
    pub template: String,
    pub template_at: Range<usize>,
}

#[derive(Debug)]
pub struct Feed {
    pub collections: Vec<String>,
    pub path: String,
    pub template: String,
    /// Where `template` is set, or the feed named when it isn't.
    pub template_at: Range<usize>,
}
