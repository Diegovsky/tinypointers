use std::ops::{Deref, DerefMut};

use crate::TinyPtr;

#[derive(Debug)]
#[repr(transparent)]
pub struct TinyBox<T>(TinyPtr<T>);

impl<T: Clone> Clone for TinyBox<T> {
    fn clone(&self) -> Self {
        TinyBox::new(self.deref().clone())
    }
}

impl<T> TinyBox<T> {
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
    use crate::TinyPtr;

    use super::*;

    #[test]
    fn assert_optimization_test() {
        assert_eq!(std::mem::size_of::<Option<TinyBox<u8>>>(), std::mem::size_of::<TinyBox<u8>>());
    }

    #[test]
    fn single_box_test() {
        let mut b = TinyBox::new(42);
        *b += 5;
        assert_eq!(*b, 47);
    }

    #[test]
    fn multiple_box_test() {
        for i in 0..100 {
            let mut b = TinyBox::new(i);
            *b += i;
            assert_eq!(*b, i * 2);
        }
    }
}
