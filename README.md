# light\_arena

**Temporarily a more simple memory pool for keeping stack alloc objects
in copied into a shared heap rather than a true placement new memory arena.**
Unfortunately the path forward for placement new in Rust does not look
good right now, so I've reverted this crate to work more like a memory
heap where stuff can be put, but not constructed in place. This mimics
similar behavior, but allocations are limited to the stack size and
must first be made on the stack then copied in.

This crate is written to solve a specific problem I have in
[tray\_rust](https://github.com/Twinklebear/tray_rust), where I want to
store trait objects and f32 arrays in a memory arena which is then reset
and reused for each pixel rendered (but not free'd and reallocated!).
The key features to enable this are the use of the nightly placement new feature, letting us
actually construct objects in place instead of copying from a stack temporary,
and reusing the previously allocated space via the `Allocator` scopes.
If you have a similar problem, this might be the right crate for you!

![Crate Version Badge](https://img.shields.io/crates/v/light_arena.svg)
[![Build Status](https://travis-ci.org/Twinklebear/light_arena.svg?branch=master)](https://travis-ci.org/Twinklebear/light_arena)

## Examples

Allocations in a `MemoryArena` are made using an allocator and the
placement in syntax. The `Allocator` grants exclusive access to the
arena while it's in scope, allowing to make allocations. Once the `Allocator`
is dropped the space used is marked available again for subsequent allocations.
Note that **Drop is never called** on objects allocated in the arena,
and thus the restriction that `T: Sized + Copy`.

```rust
#![feature(placement_in_syntax)]
use light_arena;

let mut arena = light_arena::MemoryArena::new(8);
let alloc = arena.allocator();
// This would overflow the stack without placement new!
let bytes: &[u8] = &alloc <- [0u8; 8 * 1024 * 1024];
```

The arena is untyped and can store anything which is `Sized + Copy`.

```rust
#![feature(placement_in_syntax)]
use light_arena;

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

fn main() {
	let mut arena = light_arena::MemoryArena::new(2);
	let allocator = arena.allocator();
	let a: &Foo = &allocator <- Baz;
	let b: &Foo = &allocator <- Bar(10);
	let c: &Foo = &allocator <- Bar(14);
	a.speak();
	b.speak();
	c.speak();
	// Storing 0-sized types can give some interesting results
	println!("a = {:p}", a as *const Foo);
	println!("b = {:p}", b as *const Foo);
	println!("c = {:p}", c as *const Foo);
}
```

## Documentation

Rustdoc can be found [here](http://www.willusher.io/light_arena/light_arena/)

## Blockers

- placement\_in\_syntax and placement\_new\_protocol are required,
	see https://github.com/rust-lang/rust/issues/27779

