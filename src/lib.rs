#![feature(placement_in_syntax, placement_new_protocol)]
#![cfg_attr(feature = "unstable", feature(plugin))]
#![cfg_attr(feature = "unstable", plugin(clippy))]

//! A lightweight, placement based memory arena for any types which are `Sized + Copy`.
//! This crate uses the placement in syntax and placement new protocol and
//! thus **requires nightly Rust**.
//!
//! This crate is written to solve a specific problem I have in
//! [tray\_rust](https://github.com/Twinklebear/tray_rust), where I want to
//! store trait objects and f32 arrays in a memory arena which is then reset
//! and reused for each pixel rendered (but not free'd and reallocated!).
//! The key features to enable this are the use of the nightly placement new feature, letting us
//! actually construct objects in place instead of copying from a stack temporary,
//! and reusing the previously allocated space via the `Allocator` scopes.
//! If you have a similar problem, this might be the right crate for you!
//! ## Examples
//!
//! Allocations in a `MemoryArena` are made using an allocator and the
//! placement in syntax. The `Allocator` grants exclusive access to the
//! arena while it's in scope, allowing to make allocations. Once the `Allocator`
//! is dropped the space used is marked available again for subsequent allocations.
//! Note that **Drop is never called** on objects allocated in the arena,
//! and thus the restriction that `T: Sized + Copy`.
//!
//! ```rust
//! #![feature(placement_in_syntax)]
//! use light_arena;
//!
//! let mut arena = light_arena::MemoryArena::new(8);
//! let alloc = arena.allocator();
//! // This would overflow the stack without placement new!
//! let bytes: &[u8] = &alloc <- [0u8; 8 * 1024 * 1024];
//! ```
//!
//! The arena is untyped and can store anything which is `Sized + Copy`.
//!
//! ```rust
//! #![feature(placement_in_syntax)]
//!
//! trait Foo {
//!     fn speak(&self);
//! }
//!
//! #[derive(Copy, Clone)]
//! struct Bar(i32);
//! impl Foo for Bar {
//!     fn speak(&self) {
//!         println!("Bar! val = {}", self.0);
//!     }
//! }
//!
//! #[derive(Copy, Clone)]
//! struct Baz;
//! impl Foo for Baz {
//!     fn speak(&self) {
//!         println!("Baz!");
//!     }
//! }
//!
//! let mut arena = light_arena::MemoryArena::new(2);
//! let allocator = arena.allocator();
//! let a: &Foo = &allocator <- Baz;
//! let b: &Foo = &allocator <- Bar(10);
//! let c: &Foo = &allocator <- Bar(14);
//! a.speak();
//! b.speak();
//! c.speak();
//! // Storing 0-sized types can give some interesting results
//! println!("a = {:p}", a as *const Foo);
//! println!("b = {:p}", b as *const Foo);
//! println!("c = {:p}", c as *const Foo);
//! ```
//!
//! ## Blockers
//!
//! - placement\_in\_syntax and placement\_new\_protocol are required,
//! see https://github.com/rust-lang/rust/issues/27779

use std::ops::{Placer, Place, InPlace};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::{cmp, mem, ptr};

/// A block of bytes used to back allocations requested from the `MemoryArena`.
struct Block {
    buffer: Vec<u8>,
    size: usize,
}
impl Block {
    /// Create a new block of some fixed size, in bytes
    fn new(size: usize) -> Block {
        Block {
            buffer: Vec::with_capacity(size),
            size: 0,
        }
    }
    /// Reserve `size` bytes at alignment `align`. Returns null if the block doesn't
    /// have enough room.
    unsafe fn reserve(&mut self, size: usize, align: usize) -> *mut u8 {
        if self.has_room(size, align) {
            let align_offset = align_address(self.buffer.as_ptr().offset(self.size as isize), align);
            let ptr = self.buffer.as_mut_ptr().offset((self.size + align_offset) as isize);
            self.size += size + align_offset;
            ptr
        } else {
            ptr::null_mut()
        }
    }
    /// Check if this block has `size` bytes available at alignment `align`
    fn has_room(&self, size: usize, align: usize) -> bool {
        let ptr = unsafe { self.buffer.as_ptr().offset(self.size as isize) };
        let align_offset = align_address(ptr, align);
        self.buffer.capacity() - self.size >= size + align_offset
    }
}

/// Compute the number of bytes we need to offset the `ptr` by to align
/// it to the desired alignment.
fn align_address(ptr: *const u8, align: usize) -> usize {
    let addr = ptr as usize;
    if addr % align != 0 {
        align - addr % align
    } else {
        0
    }
}

/// Provides the backing storage to serve allocations requested by an `Allocator`.
///
/// The `MemoryArena` allocates blocks of fixed size on demand as its existing
/// blocks get filled by allocation requests. To make allocations in the
/// arena use the `Allocator` returned by `allocator`. Only one `Allocator`
/// can be active for an arena at a time, after the allocator is dropped
/// the space used by its allocations is made available again.
///
/// # Example
/// Allocations are made using the allocator and the placement in syntax.
///
/// ```
/// #![feature(placement_in_syntax)]
/// use light_arena;
///
/// let mut arena = light_arena::MemoryArena::new(8);
/// let alloc = arena.allocator();
/// // This would overflow the stack without placement new!
/// let bytes: &[u8] = &alloc <- [0u8; 8 * 1024 * 1024];
/// ```
pub struct MemoryArena {
    blocks: Vec<Block>,
    block_size: usize,
}

impl MemoryArena {
    /// Create a new `MemoryArena` with the requested block size (in MB).
    /// The arena will allocate one initial block on creation, and further
    /// blocks of `block_size_mb` size, or larger if needed to meet a large
    /// allocation, on demand as allocations are made.
    pub fn new(block_size_mb: usize) -> MemoryArena {
        let block_size = block_size_mb * 1024 * 1024;
        MemoryArena {
            blocks: vec![Block::new(block_size)],
            block_size: block_size,
        }
    }
    /// Get an allocator for the arena. Only a single `Allocator` can be
    /// active for an arena at a time. Upon destruction of the `Allocator`
    /// its allocated data is marked available again.
    pub fn allocator(&mut self) -> Allocator {
        Allocator { arena: RefCell::new(self) }
    }
    /// Reserve a chunk of bytes in some block of the memory arena
    unsafe fn reserve(&mut self, size: usize, align: usize) -> *mut u8 {
        for b in &mut self.blocks[..] {
            if b.has_room(size, align) {
                return b.reserve(size, align);
            }
        }
        // No free blocks with enough room, we have to allocate. We also make
        // sure we've got align bytes of padding available as we don't assume
        // anything about the alignment of the underlying buffer.
        let new_block_size = cmp::max(self.block_size, size + align);
        self.blocks.push(Block::new(new_block_size));
        let b = &mut self.blocks.last_mut().unwrap();
        b.reserve(size, align)
    }
}

/// The allocator provides exclusive access to the memory arena, allowing
/// for allocation of objects in the arena.
///
/// Objects allocated by an allocated cannot outlive it, upon destruction
/// of the allocator the memory space it requested will be made available
/// again. **Drops of allocated objects are not called**, only
/// types which are `Sized + Copy` can be safely stored.
pub struct Allocator<'a> {
    arena: RefCell<&'a mut MemoryArena>,
}
impl<'a> Allocator<'a> {
    /// Get a dynamically sized slice of data from the allocator. The
    /// contents of the slice will be unintialized.
    pub fn alloc_slice<'b, T: Sized + Copy>(&'b self, len: usize) -> &'b mut [T] {
        let mut arena = self.arena.borrow_mut();
        let size = len * mem::size_of::<T>();
        unsafe {
            let ptr = arena.reserve(size, mem::align_of::<T>()) as *mut T;
            std::slice::from_raw_parts_mut(ptr, len)
        }
    }
}
impl<'a> Drop for Allocator<'a> {
    /// Upon dropping the allocator we mark all the blocks in the arena
    /// as empty again, "releasing" our allocations.
    fn drop(&mut self) {
        let mut arena = self.arena.borrow_mut();
        for b in &mut arena.blocks[..] {
            b.size = 0;
        }
    }
}

/// Object representing a place to put a newly requested allocation.
///
/// `Drop` is never called so the placement new can only be run on
/// `Sized + Copy` types. The lifetime of the place, and the subsequently
/// placed `T` is tied to the lifetime of the `Allocator` which created
/// the `AllocatorPlacer`.
pub struct AllocatorPlacer<'a, T: 'a + Sized + Copy> {
    ptr: *mut u8,
    phantom: PhantomData<&'a T>,
}

impl<'a, 'b, T: 'b + Sized + Copy> Placer<T> for &'a Allocator<'b> {
    type Place = AllocatorPlacer<'a, T>;

    fn make_place(self) -> Self::Place {
        let mut arena = self.arena.borrow_mut();
        let ptr = unsafe { arena.reserve(mem::size_of::<T>(), mem::align_of::<T>()) };
        AllocatorPlacer {
            ptr: ptr,
            phantom: PhantomData,
        }
    }
}

impl<'a, T: 'a + Sized + Copy> Place<T> for AllocatorPlacer<'a, T> {
    fn pointer(&mut self) -> *mut T {
        self.ptr as *mut T
    }
}
impl<'a, T: 'a + Sized + Copy> InPlace<T> for AllocatorPlacer<'a, T> {
    type Owner = &'a mut T;

    unsafe fn finalize(self) -> Self::Owner {
        (self.ptr as *mut T).as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligner() {
        assert_eq!(align_address(4 as *const u8, 4), 0);
        assert_eq!(align_address(5 as *const u8, 4), 3);
        assert_eq!(align_address(17 as *const u8, 1), 0);
    }

    #[test]
    fn block() {
        let mut b = Block::new(16);
        assert!(b.has_room(16, 1));
        let a = unsafe { b.reserve(3, 1) };
        let c = unsafe { b.reserve(4, 4) };
        assert_eq!(c as usize - a as usize, 4);
        // This check is kind of assuming that the block's buffer
        // is at least 4-byte aligned which is probably a safe assumption.
        assert_eq!(b.size, 8);

        assert!(!b.has_room(32, 4));
        let d = unsafe { b.reserve(32, 4) };
        assert_eq!(d, ptr::null_mut());
    }

    #[test]
    fn memory_arena() {
        let mut arena = MemoryArena::new(1);
        let a = unsafe { arena.reserve(1024, 4) };
        assert_eq!(align_address(a, 4), 0);
        assert_eq!(arena.blocks[0].size, 1024);

        let two_mb = 2 * 1024 * 1024;
        let b = unsafe { arena.reserve(two_mb, 32) };
        assert_eq!(align_address(b, 32), 0);
        assert_eq!(arena.blocks.len(), 2);
        assert_eq!(arena.blocks[1].buffer.capacity(), two_mb + 32);
        assert_eq!(arena.blocks[1].size, two_mb);
    }
}

