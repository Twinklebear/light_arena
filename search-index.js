var searchIndex = {};
searchIndex["light_arena"] = {"doc":"A lightweight, placement based memory arena for any types which are `Sized + Copy`. This crate uses the placement in syntax and placement new protocol and thus requires nightly Rust.","items":[[3,"MemoryArena","light_arena","Provides the backing storage to serve allocations requested by an `Allocator`.",null,null],[3,"Allocator","","The allocator provides exclusive access to the memory arena, allowing for allocation of objects in the arena.",null,null],[3,"AllocatorPlacer","","Object representing a place to put a newly requested allocation.",null,null],[11,"new","","Create a new `MemoryArena` with the requested block size (in MB). The arena will allocate one initial block on creation, and further blocks of `block_size_mb` size, or larger if needed to meet a large allocation, on demand as allocations are made.",0,{"inputs":[{"name":"usize"}],"output":{"name":"memoryarena"}}],[11,"allocator","","Get an allocator for the arena. Only a single `Allocator` can be active for an arena at a time. Upon destruction of the `Allocator` its allocated data is marked available again.",0,{"inputs":[{"name":"self"}],"output":{"name":"allocator"}}],[11,"alloc_slice","","Get a dynamically sized slice of data from the allocator. The contents of the slice will be unintialized.",1,null],[11,"drop","","Upon dropping the allocator we mark all the blocks in the arena as empty again, \"releasing\" our allocations.",1,{"inputs":[{"name":"self"}],"output":null}],[11,"pointer","","",2,null],[11,"finalize","","",2,null]],"paths":[[3,"MemoryArena"],[3,"Allocator"],[3,"AllocatorPlacer"]]};
initSearch(searchIndex);
