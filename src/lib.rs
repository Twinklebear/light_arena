#![feature(placement_in_syntax, placement_new_protocol)]

use std::ops::{Placer, Place, InPlace};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::{cmp, mem};

struct Block {
    buffer: Vec<u8>,
    size: usize,
}
impl Block {
    fn new(size: usize) -> Block {
        Block {
            buffer: Vec::with_capacity(size),
            size: 0,
        }
    }
}

pub struct MemoryArena {
    blocks: Vec<Block>,
    block_size: usize,
}

impl MemoryArena {
    pub fn new(capacity_mb: usize) -> MemoryArena {
        let block_size = capacity_mb * 1024 * 1024;
        MemoryArena {
            blocks: vec![Block::new(block_size)],
            block_size: block_size,
        }
    }
    pub fn allocator(&mut self) -> Allocator {
        Allocator { arena: RefCell::new(self) }
    }
    /// Reserve a chunk of bytes in some block of the memory arena
    unsafe fn reserve(&mut self, size: usize) -> *mut u8 {
        for b in &mut self.blocks[..] {
            if b.buffer.capacity() - b.size >= size {
                let ptr = b.buffer.as_mut_ptr().offset(b.size as isize);
                b.size += size;
                return ptr;
            }
        }
        // No free blocks with enough room, we have to allocate
        let new_block_size = cmp::max(self.block_size, size);
        self.blocks.push(Block::new(new_block_size));
        let ref mut b = self.blocks.last_mut().unwrap();
        let ptr = b.buffer.as_mut_ptr().offset(b.size as isize);
        b.size += size;
        ptr
    }
}

/// The allocator provides exclusive access to the memory arena, allowing
/// for allocation of objects in the arena. Objects allocated by
/// an allocated cannot outlive the allocator, as the memory space
/// will be made available for later allocations.
pub struct Allocator<'a> {
    arena: RefCell<&'a mut MemoryArena>,
}
impl<'a> Drop for Allocator<'a> {
    fn drop(&mut self) {
        let mut arena = self.arena.borrow_mut();
        for b in &mut arena.blocks[..] {
            b.size = 0;
        }
    }
}

pub struct AllocatorPlacer<'a, T: 'a + Sized + Copy> {
    ptr: *mut u8,
    phantom: PhantomData<&'a T>,
}

impl<'a, 'b, T: 'b + Sized + Copy> Placer<T> for &'a Allocator<'b> {
    type Place = AllocatorPlacer<'a, T>;

    fn make_place(self) -> Self::Place {
        let mut arena = self.arena.borrow_mut();
        let ptr = unsafe { arena.reserve(mem::size_of::<T>()) };
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
        println!("ptr = {:p}", self.ptr);
        (self.ptr as *mut T).as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    trait Foo {
        fn speak(&self);
    }

    #[derive(Copy, Clone)]
    struct Bar(i32);
    impl Foo for Bar {
        fn speak(&self) {
            println!("Bar! val = {}", self.0);
        }
    }

    #[derive(Copy, Clone)]
    struct Baz;
    impl Foo for Baz {
        fn speak(&self) {
            println!("Baz!");
        }
    }
    #[test]
    fn it_works() {
        let mut raw_arena = ::MemoryArena::new(2);
        let allocator = raw_arena.allocator();
        let a: &Foo = &allocator <- Baz;
        let b: &Foo = &allocator <- Bar(10);
        let c: &Foo = &allocator <- Bar(14);
        a.speak();
        b.speak();
        c.speak();
        println!("a = {:p}", a as *const Foo);
        println!("b = {:p}", b as *const Foo);
        println!("c = {:p}", c as *const Foo);
    }
}

