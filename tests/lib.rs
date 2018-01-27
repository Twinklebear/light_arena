#![feature(placement_in_syntax, attr_literals)]
extern crate light_arena;

use light_arena::MemoryArena;

#[test]
fn basic_usage() {
    let mut arena = MemoryArena::new(2);
    let allocator = arena.allocator();
    let x = &allocator <- [0usize; 16];
    for (i, v) in x.iter_mut().enumerate() {
        *v = i;
    }

    for (i, v) in x.iter().enumerate() {
        assert_eq!(*v, i);
    }
}

#[test]
fn buffer_reuse() {
    let mut arena = MemoryArena::new(2);
    let addr_a =  {
        let allocator = arena.allocator();
        let x = &allocator <- [0usize; 16];
        for (i, v) in x.iter_mut().enumerate() {
            *v = i;
        }

        for (i, v) in x.iter().enumerate() {
            assert_eq!(*v, i);
        }
        x.as_ptr() as usize
    };
    let addr_b = {
        let allocator = arena.allocator();
        let x = &allocator <- [0usize; 32];
        for (i, v) in x.iter_mut().enumerate() {
            *v = i * 2;
        }

        for (i, v) in x.iter().enumerate() {
            assert_eq!(*v, i * 2);
        }
        x.as_ptr() as usize
    };
    assert_eq!(addr_a, addr_b);
}

#[test]
fn on_demand_alloc() {
    let mut arena = MemoryArena::new(1);
    let addr_a =  {
        let allocator = arena.allocator();
        let x = &allocator <- [0u32; 128];
        for (i, v) in x.iter_mut().enumerate() {
            *v = i as u32;
        }

        for (i, v) in x.iter().enumerate() {
            assert_eq!(*v, i as u32);
        }
        x.as_ptr() as usize
    };
    let addr_b = {
        let allocator = arena.allocator();
        let x = &allocator <- [0u32; 128];
        for (i, v) in x.iter_mut().enumerate() {
            *v = i as u32;
        }

        for (i, v) in x.iter().enumerate() {
            assert_eq!(*v, i as u32);
        }

        x.as_ptr() as usize
    };
    let addr_c = {
        let allocator = arena.allocator();
        let y = &allocator <- [0u8; 2 * 1024 * 1024];

        y.as_ptr() as usize
    };
    assert_eq!(addr_a, addr_b);
    assert_ne!(addr_a, addr_c);
    assert_ne!(addr_b, addr_c);
}

#[repr(align(256))]
#[derive(Copy, Clone)]
#[allow(dead_code)]
struct FatAlignment {
    vals: [f32; 8],
}

#[test]
fn fat_align_struct() {
    use std::mem;
    assert_eq!(mem::align_of::<FatAlignment>(), 256);
    let mut arena = MemoryArena::new(1);
    let allocator = arena.allocator();
    let a = &allocator <- [FatAlignment { vals: [0f32; 8] }; 4];
    assert_eq!(a.len(), 4);
    assert_eq!(a.as_ptr() as usize % mem::align_of::<FatAlignment>(), 0);

    let b = &allocator <- [0u8; 128];

    let c = &allocator <- FatAlignment { vals: [0f32; 8] };
    assert_eq!(c as *const FatAlignment as usize % mem::align_of::<FatAlignment>(), 0);
    assert_eq!(c as *const FatAlignment as usize - b.as_ptr() as usize, 256);
}

#[test]
fn placement_alloc() {
    let mut arena = MemoryArena::new(16);
    let allocator = arena.allocator();
    // This would overflow the stack without proper in-place construction!
    let _b = &allocator <- [0u8; 8 * 1024 * 1024];
}

trait Eval {
    fn eval(&self, rhs: i32) -> i32;
}

#[derive(Copy, Clone)]
struct Add(i32);
impl Eval for Add {
    fn eval(&self, rhs: i32) -> i32 {
        self.0 + rhs
    }
}

#[derive(Copy, Clone)]
struct Subtract {
    x: i32,
}
impl Eval for Subtract {
    fn eval(&self, rhs: i32) -> i32 {
        self.x - rhs
    }
}

#[derive(Copy, Clone)]
struct Clamp {
    lo: i32,
    hi: i32,
}
impl Clamp {
    fn new(lo: i32, hi: i32) -> Clamp {
        Clamp { lo: lo, hi: hi }
    }
}
impl Eval for Clamp {
    fn eval(&self, rhs: i32) -> i32 {
        if rhs < self.lo {
            self.lo
        } else if rhs > self.hi {
            self.hi
        } else {
            rhs
        }
    }
}

#[derive(Copy, Clone)]
struct ShiftLeft;
impl Eval for ShiftLeft {
    fn eval(&self, rhs: i32) -> i32 {
        rhs << 1
    }
}

#[test]
fn trait_objects() {
    let mut arena = MemoryArena::new(2);
    let allocator = arena.allocator();
    let add: &Eval = &allocator <- Add(4);
    let sub: &Eval = &allocator <- Subtract { x: 2 };
    let shl: &Eval = &allocator <- ShiftLeft;
    let clamp: &Eval = &allocator <- Clamp::new(-2, 3);

    assert_eq!(add.eval(5), 4 + 5);
    assert_eq!(add.eval(sub.eval(8)), 2 - 8 + 4);
    assert_eq!(shl.eval(1), 1 << 1);
    assert_eq!(clamp.eval(4), 3);
    assert_eq!(clamp.eval(0), 0);
    assert_eq!(clamp.eval(-10), -2);
}

#[test]
fn dynamic_slice() {
    let mut arena = MemoryArena::new(2);
    let allocator = arena.allocator();
    let x = allocator.alloc_slice::<usize>(16);
    for (i, v) in x.iter_mut().enumerate() {
        *v = i;
    }

    for (i, v) in x.iter().enumerate() {
        assert_eq!(*v, i);
    }

    let y = allocator.alloc_slice::<usize>(16);
    for (i, v) in y.iter_mut().enumerate() {
        *v = i;
    }

    for (i, v) in x.iter().enumerate() {
        assert_eq!(*v, i);
    }
    for (i, v) in y.iter().enumerate() {
        assert_eq!(*v, i);
    }
    assert_eq!(x.as_ptr() as usize + std::mem::size_of::<usize>() * 16, y.as_ptr() as usize);
}

