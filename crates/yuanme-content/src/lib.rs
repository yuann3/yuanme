//! Site config and content: Entries, the Document tree, math and links.

mod config;
mod data;
mod document;
mod entry;
mod fields;
mod label;
mod markdown;
mod math;
mod org;
mod site;
mod slug;
mod smartypants;
mod theorem;
mod yaml;

pub use config::{
    Adapter, Budget, Budgets, Collection, Config, Field, FieldType, HeaderRule, Host,
    Images as ImagesConfig, RouterMode, data_schema, entry_schema, json_schema as config_schema,
    parse as parse_config, similar,
};
pub use document::{Align, Checkbox, Document, Kind, Links, NodeId, footnote_identifier};
pub use entry::{Entry, SourceFormat};
pub use fields::{Image as FieldImage, Value as FieldValue};
pub use markdown::{document as parse_markdown, options as markdown_options};
pub use org::document as parse_org;
pub use site::{
    Project, Route, RouteKind, Site, collections, read as read_entry, resolve as resolve_site,
    sources as sort_sources,
};
pub use slug::{slug, term_slug, url_path};
pub use yaml::{Node as YamlNode, Yaml, parse as parse_yaml};
