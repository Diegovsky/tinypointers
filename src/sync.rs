use std::{fmt::Debug, ops::Deref, sync::atomic::AtomicU32};

use crate::TinyPtr;

#[derive(Debug)]
struct RefCounted<T> {
    count: AtomicU32,
    value: T,
}

#[derive(Debug)]
/// A weak reference to a [`TinyArc`], which is a thread-safe reference-counting tiny pointer.
/// Essentially, it is non owning, and can be upgraded to a [`TinyArc`] at any time to access the
/// data.
/// ## Example
/// ```rust
/// use tinypointers::TinyArc;
///
/// let owned = TinyArc::new(42);
/// let non_owned = TinyArc::downgrade(&owned);
/// assert_eq!(*owned, 42);
/// assert_eq!(*non_owned.upgrade(), 42);
/// ```
pub struct TinyWeak<T>(TinyPtr<RefCounted<T>>);

unsafe impl<T: Send + Sync> Send for TinyWeak<T> {}
unsafe impl<T: Send + Sync> Sync for TinyWeak<T> {}

impl<T> Clone for TinyWeak<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

crate::boxed::impl_traits!(TinyArc);

impl<T> TinyWeak<T> {
    /// Attempts to upgrade the `TinyWeak` pointer to an `TinyArc`, extending the lifetime of the
    /// data if successful.
    /// ## Example
    /// ```rust
    /// use tinypointers::TinyArc;
    ///
    /// let owned = TinyArc::new(42);
    /// let non_owned = TinyArc::downgrade(&owned);
    ///
    /// drop(owned);
    ///
    /// let owned = non_owned.upgrade(); // Panics
    /// ```
    ///
    /// ## Panics
    /// This panics if the data has since been dropped. I.E. if the `TinyArc` count is zero.
    pub fn upgrade(&self) -> TinyArc<T> {
        let arc = TinyArc(self.0);
        TinyArc::increase_count(&arc);
        arc
    }
}

/// A thread-safe reference-counting tiny pointer. As with all types of this crate, memory is
/// allocated on the heap. It is equivalent to [`std::sync::Arc`].
///
/// ```rust
/// use tinypointers::TinyArc;
///
/// let x = TinyArc::new(42);
/// let y = x.clone();
/// println!("{}", *x); // prints 42
/// println!("{}", *y); // prints 42
/// // both x and y point to the same memory location
/// ```
pub struct TinyArc<T>(TinyPtr<RefCounted<T>>);

unsafe impl<T: Send + Sync> Send for TinyArc<T> {}
unsafe impl<T: Send + Sync> Sync for TinyArc<T> {}

impl<T> TinyArc<T> {
    /// Allocates memory on the heap and then places `value` into it.
    /// ## Example
    /// ```rust
    /// use tinypointers::TinyArc;
    ///
    /// let x = TinyArc::new(42);
    /// ```
    pub fn new(value: T) -> Self {
        Self(TinyPtr::new(RefCounted {
            count: AtomicU32::new(1),
            value,
        }))
    }
    /// Constructs a new `TinyArc<T>` while giving you a `TinyWeak<T>` to the allocation, to allow
    /// you to construct a `T` which holds a weak pointer to itself.
    ///
    /// `new_cyclic` first allocates the managed allocation for the `TinyArc<T>`,
    /// then calls your closure, giving it a `TinyWeak<T>` to this allocation,
    /// and only afterwards completes the construction of the `TinyArc<T>` by placing
    /// the `T` returned from your closure into the allocation.
    ///
    /// ## Panic
    /// Keep in mind that the `TinyArc<T>` is not fully constructed until `TinyArc<T>::new_cyclic`
    /// returns. Calling [`TinyWeak::upgrade`] will cause a panic.
    pub fn new_cyclic<F>(data_fn: F) -> Self where F: FnOnce(TinyWeak<T>) -> T {
        let mut ptr = TinyPtr::new(RefCounted {
            count: AtomicU32::new(0),
            value: unsafe { std::mem::MaybeUninit::<T>::uninit().assume_init() },
        });
        let data = data_fn(TinyWeak(ptr));
        unsafe {
            let ptr = ptr.get_mut();
            std::ptr::addr_of_mut!(ptr.value).write(data);
        }
        let this = Self(ptr);
        Self::increase_count(&this);
        this
    }
    /// Returns a raw pointer to the inner value.
    ///
    /// The pointer will be valid for as long as there are strong references to this allocation.
    pub fn as_ptr(this: &Self) -> *const T {
        &this.get().value
    }
    /// Checks whether the two `TinyArc`s point to the same allocation.
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.0.id() == other.0.id()
    }
    /// Creates a [`TinyWeak`] pointer to this allocation.
    ///
    /// Weak references do not keep the allocation alive, and cannot access the inner value.
    pub fn downgrade(this: &Self) -> TinyWeak<T> {
        TinyWeak(this.0)
    }

    // internal apis

    fn get(&self) -> &RefCounted<T> {
        unsafe { &*self.0.get() }
    }
    fn increase_count(this: &Self) -> u32 {
        this.get()
            .count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
    fn decrease_count(this: &Self) -> u32 {
        this.get()
            .count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed)
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
        let refcounted = self.get();
        if dbg!(refcounted.count.load(std::sync::atomic::Ordering::Relaxed)) == 0 {
            panic!("Attempted to dereference a TinyArc before it was built")
        }
        &refcounted.value
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
        let owners = Self::decrease_count(self);
        if owners == 1 {
            // Drop the value if we're the last owner
            self.0.take();
        }
    }
}

#[cfg(test)]
mod tests {

    use std::sync::atomic::AtomicBool;

    use super::*;

    use crate::tests::{*, make_drop_indicator};

    #[test]
    fn multiple_thread_access() {
        make_drop_indicator!(__ind, p2, 42);
        let p2 = TinyArc::new(p2);
        let p1 = p2.clone();
        let t1 = std::thread::spawn(move || {
            assert_eq!(*p1, 42);
        });
        let t2 = std::thread::spawn(move || {
            assert_eq!(*p2, 42);
        });
        t1.join().unwrap();
        t2.join().unwrap();
        assert_dropped!(__ind);
    }
    #[test]
    fn assert_optimization_test() {
        assert_eq!(
            std::mem::size_of::<Option<TinyArc<u8>>>(),
            std::mem::size_of::<TinyArc<u8>>()
        );
    }

    #[test]
    fn single_arc_test() {
        make_drop_indicator!(__ind, b, 42);
        let b = TinyArc::new(b);
        assert_eq!(*b, 42);
        std::mem::drop(b);
        assert_dropped!(__ind)
    }

    #[test]
    #[cfg_attr(feature = "1byteid", ignore = "uses too much memory")]
    fn multiple_arc_test() {
        for i in 0..100 {
            make_drop_indicator!(__ind, val, i);
            {
                let b = TinyArc::new(val);
                assert_eq!(*b, i);
            }
            assert_dropped!(__ind)
        }
    }

    #[test]
    fn multiple_refs_test() {
        make_drop_indicator!(__ind, v, 30);
        let i = TinyArc::new(v);
        for _x in 0..200 {
            let j = i.clone();
            assert_eq!(*j, 30);
        }
        std::mem::drop(i);
        assert_dropped!(__ind)
    }

    #[test]
    fn make_cyclic_test() {
        #[derive(Debug)]
        struct Narcissus {
            _drop_indicator: DropIndicator<()>,
            self_: TinyWeak<Narcissus>,
        }

        make_drop_indicator!(__ind, ind, ());
        let narc = TinyArc::new_cyclic(|weak| {
            Narcissus{self_: weak, _drop_indicator: ind}
        });

        assert!(TinyArc::ptr_eq(&narc, &narc.self_.upgrade()));
        std::mem::drop(narc);
        assert_dropped!(__ind);
    }

    #[test]
    #[should_panic]
    fn make_cyclic_panic_test() {
        TinyArc::<()>::new_cyclic(|weak| {
            weak.upgrade();
        });
    }
}
