/// Create iterators for ast nodes where it only has to iterate over children
macro_rules! children_iter {
    ($astnodetype:ty, $itertype:ident, $innertype:ty) => {
        pub struct $itertype<'r, 's> {
            next: $innertype,
        }

        impl<'r, 's> Iterator for $itertype<'r, 's> {
            type Item = AstNode<'r, 's>;

            fn next(&mut self) -> Option<Self::Item> {
                self.next.next().map(Into::<AstNode>::into)
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                let size = self.next.len();
                (size, Some(size))
            }
        }

        impl<'r, 's> ExactSizeIterator for $itertype<'r, 's> {}

        impl<'r, 's> IntoIterator for &'r $astnodetype {
            type Item = AstNode<'r, 's>;

            type IntoIter = $itertype<'r, 's>;

            fn into_iter(self) -> Self::IntoIter {
                $itertype {
                    next: self.children.iter(),
                }
            }
        }
    };
}

pub(crate) use children_iter;

/// Create iterators for ast nodes that do not contain any ast node children.
macro_rules! empty_iter {
    ($astnodetype:ty, $itertype:ident) => {
        pub struct $itertype<'r, 's> {
            phantom: PhantomData<&'r $astnodetype>,
        }

        impl<'r, 's> Iterator for $itertype<'r, 's> {
            type Item = AstNode<'r, 's>;

            fn next(&mut self) -> Option<Self::Item> {
                None
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (0, Some(0))
            }
        }

        impl<'r, 's> ExactSizeIterator for $itertype<'r, 's> {}

        impl<'r, 's> IntoIterator for &'r $astnodetype {
            type Item = AstNode<'r, 's>;

            type IntoIter = $itertype<'r, 's>;

            fn into_iter(self) -> Self::IntoIter {
                $itertype {
                    phantom: PhantomData,
                }
            }
        }
    };
}

pub(crate) use empty_iter;

/// Create iterators for ast nodes where it has to iterate over multiple child lists.
macro_rules! multi_field_iter {
    ($astnodetype:ty, $itertype:ident, $firstfieldname: ident, $firstinnertype:ty, $($fieldname: ident, $innertype:ty),*) => {
        pub struct $itertype<'r, 's> {
            $firstfieldname: $firstinnertype,
            $(
$fieldname: $innertype,
),*
        }

        impl<'r, 's> Iterator for $itertype<'r, 's> {
            type Item = AstNode<'r, 's>;

            fn next(&mut self) -> Option<Self::Item> {
                self.$firstfieldname.next().map(Into::<AstNode>::into)
                $(
.or_else(|| self.$fieldname.next().map(Into::<AstNode>::into))
),*
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                let size = self.$firstfieldname.len()$( + self.$fieldname.len() ),*;
                (size, Some(size))
            }
        }

        impl<'r, 's> ExactSizeIterator for $itertype<'r, 's> {}

        impl<'r, 's> IntoIterator for &'r $astnodetype {
            type Item = AstNode<'r, 's>;

            type IntoIter = $itertype<'r, 's>;

            fn into_iter(self) -> Self::IntoIter {
                $itertype {
            $firstfieldname: self.$firstfieldname.iter(),
            $(
$fieldname: self.$fieldname.iter(),
),*
                }
            }
        }
    };
}

pub(crate) use multi_field_iter;
