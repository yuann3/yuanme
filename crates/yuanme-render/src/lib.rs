//! HTML writing, highlighting and Templates (#24).

mod highlight;
mod html;
mod image;
mod template;

pub use highlight::{
    Code, Language, block as highlight_block, classes as code_classes, css as code_css,
    document as highlight_document, grammar, language, names as language_names,
};
pub use html::{Pictures, alt as alt_text, body as body_html};
pub use image::{Error as ImageError, Image, Images, Picture, Request, picture};
pub use template::scope::{attribute as scope_attribute, elements as scoped_elements, sidecar};
pub use template::{Page, context_schemas, escape, render as render_page, route_templates};
