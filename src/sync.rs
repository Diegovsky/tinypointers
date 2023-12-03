use std::{sync::atomic::AtomicU32, fmt::Debug, ops::{Deref, DerefMut}};

use crate::TinyPtr;


#[derive(Debug)]
struct RefCounted<T> {
    count: AtomicU32,
    value: T,
}

#[derive(Debug)]
pub struct TinyWeak<T>(TinyPtr<RefCounted<T>>);

unsafe impl<T: Send + Sync> Send for TinyWeak<T> {}
unsafe impl<T: Send + Sync> Sync for TinyWeak<T> {}

impl<T> Clone for TinyWeak<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T> TinyWeak<T> {
    fn upgrade(&self) -> TinyArc<T> {
        let arc = TinyArc(self.0);
        TinyArc::increase_count(&arc);
        arc
    }
}

pub struct TinyArc<T>(TinyPtr<RefCounted<T>>);

unsafe impl<T: Send + Sync> Send for TinyArc<T> {}
unsafe impl<T: Send + Sync> Sync for TinyArc<T> {}

impl<T> TinyArc<T> {
    pub fn new(value: T) -> Self {
        Self(TinyPtr::new(RefCounted { count: AtomicU32::new(1), value }))
    }
    pub fn as_ptr(this: &Self) -> *const T {
        &this.get().value
    }
    pub fn downgrade(this: &Self) -> TinyWeak<T> {
        TinyWeak(this.0)
    }
    // internal apis

    fn get(&self) -> &RefCounted<T> {
        unsafe { &*self.0.get() }
    }
    fn increase_count(this: &Self) -> u32 {
        this.get().count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    }
}

impl<T: Debug> Debug for TinyArc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TinyArc")
            .field("refcount", self.get())
            .finish()
    }
}

impl<T> Deref for TinyArc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.get().value
    }
}

impl<T> Clone for TinyArc<T> {
    fn clone(&self) -> Self {
        Self::increase_count(self);
        Self(self.0)
    }
}

impl<T> std::ops::Drop for TinyArc<T> {
    fn drop(&mut self) {
        let owners = Self::increase_count(self);
        if owners == 0 {
            // Drop the value if we're the last owner
            self.0.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::TinyPtr;

    use super::*;

    #[test]
    fn multiple_thread_access() {
        let p2 = TinyArc::new(42);
        let p1 = p2.clone();
        let t1 = std::thread::spawn(move || {
            assert_eq!(*p1, 42);
        });
        let t2 = std::thread::spawn(move || {
            assert_eq!(*p2, 42);
        });
        t1.join().unwrap();
        t2.join().unwrap();
    }
    #[test]
    fn assert_optimization_test() {
        assert_eq!(std::mem::size_of::<Option<TinyArc<u8>>>(), std::mem::size_of::<TinyArc<u8>>());
    }

    #[test]
    fn single_arc_test() {
        let b = TinyArc::new(42);
        assert_eq!(*b, 42);
    }

    #[test]
    #[cfg_attr(feature="1byteid", ignore="uses too much memory")]
    fn multiple_arc_test() {
        for i in 0..100 {
            let b = TinyArc::new(i);
            assert_eq!(*b, i);
        }
    }

    #[test]
    fn multiple_refs_test() {
        let i = TinyArc::new(30);
        for x in 0..200 {
            let j = i.clone();
            assert_eq!(*j, 30);
        }
    }
}
