/// Write the implementation of From<> to convert a borrow of the type to an AstNode
macro_rules! to_ast_node {
    ($inp:ty, $enum:expr) => {
        impl<'r, 's> From<$inp> for AstNode<'r, 's> {
            fn from(value: $inp) -> Self {
                $enum(value)
            }
        }
    };
}

pub(crate) use to_ast_node;
