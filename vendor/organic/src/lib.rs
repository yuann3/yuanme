#![feature(exit_status_error)]
#![feature(trait_alias)]
#![feature(path_file_prefix)]
#![feature(test)]
#![feature(iter_intersperse)]
#![feature(exact_size_is_empty)]
// TODO: #![warn(missing_docs)]
#![allow(clippy::bool_assert_comparison)] // Sometimes you want the long form because its easier to see at a glance.

extern crate test;

#[cfg(feature = "compare")]
pub mod compare;
pub mod parse_cli;
#[cfg(any(feature = "compare", feature = "wasm", feature = "wasm_test"))]
mod util;
#[cfg(any(feature = "wasm", feature = "wasm_test"))]
mod wasm;
#[cfg(any(feature = "wasm", feature = "wasm_test"))]
pub mod wasm_cli;
#[cfg(feature = "wasm_test")]
pub mod wasm_test;

mod context;
mod error;
#[cfg(feature = "event_count")]
mod event_count;
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
