#![feature(placement_in_syntax, placement_new_protocol, plugin)]
#![plugin(clippy)]

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
use std::{cmp, mem};

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
            let align_offset = align_address(b.buffer.as_ptr().offset(b.size as isize), align);
            if b.buffer.capacity() - b.size - align_offset >= size {
                let ptr = b.buffer.as_mut_ptr().offset((b.size + align_offset) as isize);
                b.size += size + align_offset;
                return ptr;
            }
        }
        // No free blocks with enough room, we have to allocate
        let new_block_size = cmp::max(self.block_size, size);
        self.blocks.push(Block::new(new_block_size));

        let b = &mut self.blocks.last_mut().unwrap();
        let align_offset = align_address(b.buffer.as_ptr(), align);
        let ptr = b.buffer.as_mut_ptr().offset(align_offset as isize);
        b.size += align_offset;
        ptr
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

