use crate::error::Res;
use crate::parser::OrgSource;

pub(crate) mod constants;
#[allow(clippy::module_inception)]
mod context;
mod exiting;
mod file_access_interface;
mod global_settings;
mod list;
mod parser_with_context;

pub(crate) type RefContext<'b, 'g, 'r, 's> = &'b Context<'g, 'r, 's>;
pub(crate) trait ContextMatcher:
    for<'b, 'g, 'r, 's> Fn(
    RefContext<'b, 'g, 'r, 's>,
    OrgSource<'s>,
) -> Res<OrgSource<'s>, OrgSource<'s>>
{
}
impl<T> ContextMatcher for T where
    T: for<'b, 'g, 'r, 's> Fn(
        RefContext<'b, 'g, 'r, 's>,
        OrgSource<'s>,
    ) -> Res<OrgSource<'s>, OrgSource<'s>>
{
}
type DynContextMatcher<'c> = dyn ContextMatcher + 'c;
pub(crate) trait Matcher:
    for<'s> Fn(OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>>
{
}
impl<T> Matcher for T where T: for<'s> Fn(OrgSource<'s>) -> Res<OrgSource<'s>, OrgSource<'s>> {}
#[allow(dead_code)]
type DynMatcher<'c> = dyn Matcher + 'c;

pub(crate) use context::Context;
pub(crate) use context::ContextElement;
pub(crate) use context::ExitMatcherNode;
pub(crate) use exiting::ExitClass;
pub use file_access_interface::FileAccessInterface;
pub use file_access_interface::LocalFileAccessInterface;
pub use global_settings::EntityDefinition;
pub use global_settings::GlobalSettings;
pub use global_settings::HeadlineLevelFilter;
pub use global_settings::DEFAULT_TAB_WIDTH;
pub(crate) use list::List;
pub(crate) use parser_with_context::bind_context;
pub(crate) use parser_with_context::parser_with_context;
