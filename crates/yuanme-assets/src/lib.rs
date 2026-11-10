//! CSS, JS, images, fonts and the Managed tools the Engine runs for them.
//! Names that would clash at the root carry their module's name.

mod css;
mod font;
mod html;
mod image;
mod js;
mod tailwind;
mod tool;
mod typescript;

pub use css::{
    Compound, Error as CssError, ErrorKind as CssErrorKind, Import, Packages,
    Project as CssProject, bundle as bundle_css,
};
pub use font::{NCM_MATH, subset as subset_font, woff2};
pub use html::{Kind as ReferenceKind, Scan, scan as scan_html, urls as html_urls};
pub use image::{
    Error as ImageError, Format, Probe, Settings, encode as encode_image, probe as probe_image,
};
pub use js::{
    At as ProblemAt, Entry as ScriptEntry, Problem as ScriptProblem, ProblemKind,
    Project as JsProject, bundle as bundle_js, strings as js_strings,
};
pub use tailwind::{
    ENV as TAILWIND_ENV, Error as TailwindError, VERSION as TAILWIND_VERSION,
    Watch as TailwindWatch, imported as imports_tailwind, input as tailwind_input, is_tailwind,
    run as run_tailwind, tokens as tailwind_tokens, tool as tailwind_tool,
};
pub use tool::{cache_dir, fetch, listed, sha256};
pub use typescript::{
    ENV as TSC_ENV, TYPES as SCRIPT_TYPES, TypeError, VERSION as TSC_VERSION,
    errors as type_errors, run as run_tsc, tool as tsc_tool,
};
