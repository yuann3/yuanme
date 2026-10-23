macro_rules! parser_with_context {
    ($target:expr) => {
        move |context| move |i| $target(context, i)
    };
}
pub(crate) use parser_with_context;

macro_rules! bind_context {
    ($target:expr, $context:expr) => {
        |i| $target($context, i)
    };
}
pub(crate) use bind_context;
