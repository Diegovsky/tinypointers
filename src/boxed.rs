use std::{ops::{Deref, DerefMut}, fmt::Debug};

use crate::TinyPtr;

#[repr(transparent)]
/// A tiny pointer to a heap allocated memory. As with all types of this crate, memory is
/// allocated on the heap. It is equivalent to [`std::boxed::Box`].
///
/// ```rust
/// use tinypointers::TinyBox;
/// let x = TinyBox::new(42);
/// println!("{}", *x); // prints 42
/// ```
pub struct TinyBox<T>(TinyPtr<T>);

macro_rules! impl_traits {
    ($derefable:ident) => {
        impl<T: std::fmt::Display> std::fmt::Display for $derefable<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.deref().fmt(f)
            }
        }

        impl<T: std::hash::Hash> std::hash::Hash for $derefable<T> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                std::hash::Hash::hash(self.deref(), state)
            }
        }

    };
}

impl<T: Debug> std::fmt::Debug for TinyBox<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TinyBox").field(self.deref()).finish()
    }
}

impl<T: Clone> Clone for TinyBox<T> {
    fn clone(&self) -> Self {
        Self::new(self.deref().clone())
    }
}

pub(crate) use impl_traits;

impl_traits!(TinyBox);

impl<T> TinyBox<T> {
    /// Allocates memory on the heap and then places `value` into it.
    ///
    /// It always allocates memory, even if `value` is zero-sized.
    /// ## Example
    /// ```rust
    /// use tinypointers::TinyBox;
    /// let x = TinyBox::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        Self(TinyPtr::new(value))
    }
}

impl<T> Deref for TinyBox<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

impl<T> DerefMut for TinyBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0.get_mut() }
    }
}

impl<T> std::ops::Drop for TinyBox<T> {
    fn drop(&mut self) {
        self.0.take();
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::*;

    use super::*;

    #[test]
    fn assert_optimization_test() {
        assert_eq!(std::mem::size_of::<Option<TinyBox<u8>>>(), std::mem::size_of::<TinyBox<u8>>());
    }

    #[test]
    fn single_box_test() {
        make_drop_indicator!(__ind, b, 42i32);
        let mut b = TinyBox::new(b);
        **b += 5;
        assert_eq!(*b, 47);

        std::mem::drop(b);
        assert_dropped!(__ind);
    }

    #[test]
    fn multiple_box_test() {
        for i in 0..100 {
            make_drop_indicator!(__ind, b, i);
            let mut b = TinyBox::new(b);
            **b += i;
            assert_eq!(*b, i * 2);

            std::mem::drop(b);
            assert_dropped!(__ind);
        }
    }
}
