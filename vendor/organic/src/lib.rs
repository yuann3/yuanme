// TODO: #![warn(missing_docs)]
#![allow(clippy::bool_assert_comparison)] // Sometimes you want the long form because its easier to see at a glance.

mod context;
mod error;
mod iter;
pub mod parser;
pub mod types;

pub mod settings {
    pub use crate::context::FileAccessInterface;
    pub use crate::context::GlobalSettings;
    pub use crate::context::HeadlineLevelFilter;
    pub use crate::context::LocalFileAccessInterface;
    pub use crate::context::DEFAULT_TAB_WIDTH;
}
