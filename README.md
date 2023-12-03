# Tiny Pointers
This crate implements pointer types that take less space than the `std` equivalents. You can choose between 8 or 16-bit using the flags `1byteid` and `2byteid` respectively.

[`TinyBox`], [`TinyArc`] and [`TinyPtr`] are equivalent to `Box`, `Arc` and `*mut T`,respectively.

Some care has been taken to ensure it's a mostly painless transition from `rust` types to the equivalent `tinypointers` type. Feel free to open a PR if functionality you need is missing!

## How
To accomplish this, memory is allocated on the heap and inserted into a global array. You're given an index inside the array, and this is what is called an `id`.

## Size optimizations
Since this crate strives to minimize memory footprint, `NonZero*` are used internally to enable memory layout optimizations. This means both structs have the same size in the following example:
```rust
struct Bar(TinyBox<List>);

struct Foo(Option<TinyBox<List>>);

// 2 == 2
assert_eq!(std::mem::size_of::<Bar>(), std::mem::size_of::<Foo>())

```
