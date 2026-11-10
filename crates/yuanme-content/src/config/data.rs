//! `[data]`: the schemas of Data files.

use std::ops::Range;

use super::FieldType;

#[derive(Debug)]
pub struct DataSchema {
    pub kind: FieldType,
    /// Where `[data.<name>]` is named.
    pub at: Range<usize>,
}
