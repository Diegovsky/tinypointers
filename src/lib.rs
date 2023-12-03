#![doc = include_str!("../README.md")]
use std::{marker::PhantomData, ptr::NonNull};

#[cfg(all(feature="1byteid", feature="2byteid"))]
compile_error!("Cannot enable both 1byteid and 2byteid features");

use parking_lot::{RwLock, Mutex};

#[cfg(feature="2byteid")]
type RawId = std::num::NonZeroU16;
#[cfg(feature="1byteid")]
type RawId = std::num::NonZeroU8;

mod boxed;
mod sync;

pub use boxed::TinyBox;
pub use sync::{TinyArc, TinyWeak};

#[derive(Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
/// A tiny pointer to a mutable value of type `T`. As with all types of this crate, memory is allocated on the heap.
/// ```rust
/// let x = TinyPtr::new(42);
/// println!("{}", unsafe { *x.get() }); // prints 42
/// ```
pub struct TinyPtr<T>(RawId, PhantomData<*mut T>);

impl<T> Clone for TinyPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TinyPtr<T> {}


impl<T> TinyPtr<T> {
    pub fn new(value: T) -> Self {
        MEMORY.insert_value(Value::from(Box::from(value)))
    }
}

impl<T> TinyPtr<T> {
    pub fn as_ptr(&self) -> *const T {
        unsafe { MEMORY.access(self) }
    }
    pub fn as_mut_ptr(&mut self) -> *mut T {
        unsafe { MEMORY.access(self) }
    }
    pub unsafe fn get<'a, 'b>(&'b self) -> &'a T {
        &*MEMORY.access(self)
    }
    pub unsafe fn get_mut<'a, 'b>(&'b mut self) -> &'a mut T {
        &mut *MEMORY.access(self)
    }
    pub fn take(self) -> T {
        unsafe { MEMORY.take(self) }
    }
}

impl<T> From<Box<T>> for TinyPtr<T> {
    fn from(value: Box<T>) -> Self {
        MEMORY.insert_value(Value::from(value))
    }
}

struct Value {
    val: NonNull<()>,
}

unsafe impl Send for Value {}
unsafe impl Sync for Value {}

impl<T> From<Box<T>> for Value {
    fn from(value: Box<T>) -> Self {
        Self {
            val: NonNull::from(Box::leak(value)).cast(),
        }
    }
}

impl Value {
    unsafe fn get<T>(&self) -> *mut T {
        std::mem::transmute(self.val)
    }
    unsafe fn into_box<T>(self) -> Box<T> {
        Box::from_raw(self.val.as_ptr() as *mut T)
    }
}

#[derive(Default)]
struct Memory {
    available: Mutex<Vec<RawId>>,
    map: RwLock<Vec<Option<Value>>>,
}

impl Memory {
    pub const fn new() -> Self {
        Self { available: Mutex::new(Vec::new()), map: RwLock::new(Vec::new()) }
    }
    fn insert_value<T>(&self, value: Value) -> TinyPtr<T> {
        if self.remaing_slots() == 0 {
            panic!("No more slots available. Consider increasing the id size.")
        }
        let mut map = self.map.write();
        let idx = match self.available.lock().pop() {
            None => {
                map.push(value.into());
                RawId::new(map.len() as _).unwrap()
            },
            Some(idx) => {
                map[idx.get() as usize - 1] = value.into();
                idx
            },
        };
        TinyPtr(idx, PhantomData)
    }
    fn remaing_slots(&self) -> usize {
        self.available.lock().len() + (RawId::MAX.get() as usize - self.map.read().len())
            
    }
    unsafe fn access<T>(&self, idx: &TinyPtr<T>) -> *mut T {
        let map = self.map.read();
        map.get(idx.0.get() as usize - 1).expect("Index out of bounds").as_ref().expect("Pointer already freed").get()
    }
    unsafe fn take<T>(&self, idx: TinyPtr<T>) -> T {
        let mut map = self.map.write();
        let value = map.get_mut(idx.0.get() as usize - 1).expect("Index out of bounds").take().expect("Pointer already freed");
        *value.into_box()
    }
}

static MEMORY: Memory = Memory::new();

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn access_raw_test() {
        let ptr = TinyPtr::new(42);
        assert_eq!(unsafe { *ptr.get() }, 42);
    }
    #[test]
    fn access_raw_string_test() {
        let ptr = TinyPtr::new(String::from("Hello, World!"));
        assert_eq!(unsafe { ptr.get() }, "Hello, World!");
    }
    #[test]
    #[cfg_attr(feature="1byteid", ignore="leaks too much memory")]
    fn access_after_multiple_test() {
        let ptrs = (0..100).map(|i| TinyPtr::new(i)).collect::<Vec<_>>();
        assert!(ptrs.iter().enumerate().all(|(i, ptr)| unsafe { *ptr.get() } == i));
    }

    #[test]
    fn drop_single_test() {
        let ptr = TinyPtr::new(42);
        assert_eq!(unsafe { *ptr.get() }, 42);
        assert_eq!(ptr.take(), 42);
    }

    #[test]
    fn multiple_thread_access() {
        let t1 = std::thread::spawn(|| {
            let ptr = TinyPtr::new(42);
            assert_eq!(unsafe { *ptr.get() }, 42);
            ptr.take();
        });
        let t2 = std::thread::spawn(|| {
            let ptr = TinyPtr::new(30);
            assert_eq!(unsafe { *ptr.get() }, 30);
            ptr.take();
        });
        t1.join().unwrap();
        t2.join().unwrap();
    }
    #[test]
    fn drop_multiple_test() {
        let ptrs = (0..100).map(|i| TinyPtr::new(i)).collect::<Vec<_>>();
        assert!(ptrs.iter().enumerate().all(|(i, ptr)| unsafe { *ptr.get() } == i));
        assert!(ptrs.into_iter().enumerate().all(|(i, ptr)| ptr.take() == i));
    }
    #[test]
    fn assert_optimization_test() {
        assert_eq!(std::mem::size_of::<TinyPtr<u8>>(), std::mem::size_of::<RawId>());
        assert_eq!(std::mem::size_of::<Option<TinyPtr<u8>>>(), std::mem::size_of::<TinyPtr<u8>>());
    }

}
