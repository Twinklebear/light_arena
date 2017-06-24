#![feature(placement_in_syntax)]
extern crate light_arena;

use light_arena::MemoryArena;

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
    let mut raw_arena = MemoryArena::new(2);
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
