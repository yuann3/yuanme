use std::fmt::Debug;

#[derive(Debug, Clone)]
pub(crate) struct List<'parent, T> {
    data: T,
    parent: Link<'parent, T>,
}

// TODO: Should I be defining a lifetime for T in the generics here? Ref: https://quinedot.github.io/rust-learning/dyn-elision-advanced.html#iteraction-with-type-aliases
type Link<'parent, T> = Option<&'parent List<'parent, T>>;

impl<'parent, T> List<'parent, T> {
    pub(crate) fn new(first_item: T) -> Self {
        Self {
            data: first_item,
            parent: None,
        }
    }

    pub(crate) fn get_data(&self) -> &T {
        &self.data
    }

    pub(crate) fn get_parent(&'parent self) -> Link<'parent, T> {
        self.parent
    }

    pub(crate) fn iter(&self) -> Iter<'_, T> {
        Iter { next: Some(self) }
    }

    pub(crate) fn iter_list(&self) -> IterList<'_, T> {
        IterList { next: Some(self) }
    }

    pub(crate) fn push(&'parent self, item: T) -> Self {
        Self {
            data: item,
            parent: Some(self),
        }
    }
}

pub(crate) struct Iter<'a, T> {
    next: Link<'a, T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.next.map(|link| link.get_data());
        self.next = self.next.and_then(|link| link.get_parent());
        ret
    }
}

pub(crate) struct IterList<'a, T> {
    next: Link<'a, T>,
}

impl<'a, T> Iterator for IterList<'a, T> {
    type Item = &'a List<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.next;
        self.next = self.next.and_then(|link| link.get_parent());
        ret
    }
}
