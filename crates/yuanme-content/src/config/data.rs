//! `[data]`: the schemas of Data files.

use std::collections::BTreeMap;
use std::ops::Range;

use super::check::Checker;
use super::fields::field;
use super::{FieldType, Table, as_table, entries};

#[derive(Debug)]
pub struct DataSchema {
    pub kind: FieldType,
    /// Where `[data.<name>]` is named.
    pub at: Range<usize>,
}

pub(super) fn read(
    data: Option<&Table<'_>>,
    checker: &mut Checker,
) -> BTreeMap<String, DataSchema> {
    entries(data)
        .iter()
        .map(|(name, spec)| {
            let kind = field(as_table(spec), false, checker).kind;
            (
                name.get_ref().to_string(),
                DataSchema {
                    kind,
                    at: name.span(),
                },
            )
        })
        .collect()
}
