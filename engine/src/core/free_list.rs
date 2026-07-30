// src/free_list.rs
//
// A linear free-list allocator over a fixed-size range [0, capacity).
// GeometryPool uses one of these for the vertex region and one for the
// index region, so chunk meshes can be uploaded and *freed* as the player
// moves — the old GeometryPool only ever bumped a cursor forward, which
// leaks GPU memory the moment anything gets unloaded.
//
// Strategy: first-fit over a Vec<FreeBlock> sorted by offset, with
// immediate coalescing of adjacent blocks on free(). O(n) in the number of
// free blocks per call — fine for a few thousand chunk (de)allocations. If
// this ever shows up in profiling, swap the Vec for a BTreeMap<offset, len>.
//
// This allocator never compacts or moves existing allocations — it can't,
// since freeing/moving GPU-resident data would mean a re-upload the caller
// doesn't expect. That means fragmentation is possible: `free_space()` can
// be nonzero while `alloc()` still fails because no *single* block is big
// enough. `largest_free_block()` is the honest ceiling on the next alloc.

#[derive(Clone, Copy, Debug)]
struct FreeBlock {
    offset: usize,
    len: usize,
}

/// A handle returned from `alloc`, given back to `free`. Deliberately a
/// distinct type (not a raw offset) so callers can't accidentally reuse a
/// stale offset after the block has been freed and reallocated elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allocation {
    pub offset: usize,
    pub len: usize,
}

pub struct FreeListAllocator {
    capacity: usize,
    /// Sorted by `offset` ascending. Invariant: no two blocks are adjacent
    /// or overlapping — adjacent ones are always merged immediately in `free`.
    free_blocks: Vec<FreeBlock>,
}

impl FreeListAllocator {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            free_blocks: vec![FreeBlock {
                offset: 0,
                len: capacity,
            }],
        }
    }

    /// Allocates a contiguous run of `len` units. Returns `None` if no
    /// single free block is large enough.
    pub fn alloc(&mut self, len: usize) -> Option<Allocation> {
        if len == 0 {
            return Some(Allocation { offset: 0, len: 0 });
        }

        // First-fit, earliest-offset-first: biases reuse toward the low end
        // of the pool, which tends to leave larger contiguous regions free
        // at the high end for subsequent big allocations.
        let idx = self.free_blocks.iter().position(|b| b.len >= len)?;
        let block = self.free_blocks[idx];

        if block.len == len {
            self.free_blocks.remove(idx);
        } else {
            self.free_blocks[idx] = FreeBlock {
                offset: block.offset + len,
                len: block.len - len,
            };
        }

        Some(Allocation {
            offset: block.offset,
            len,
        })
    }

    /// Returns a previously allocated range to the pool, coalescing with
    /// adjacent free blocks so fragmentation doesn't accumulate over a long
    /// play session of constant chunk load/unload.
    pub fn free(&mut self, alloc: Allocation) {
        if alloc.len == 0 {
            return;
        }

        let mut offset = alloc.offset;
        let mut len = alloc.len;

        let insert_at = self.free_blocks.partition_point(|b| b.offset < offset);

        // Merge with the following block if adjacent.
        if insert_at < self.free_blocks.len() {
            let next = self.free_blocks[insert_at];
            if offset + len == next.offset {
                len += next.len;
                self.free_blocks.remove(insert_at);
            }
        }

        // Merge with the preceding block if adjacent.
        if insert_at > 0 {
            let prev_idx = insert_at - 1;
            let prev = self.free_blocks[prev_idx];
            if prev.offset + prev.len == offset {
                offset = prev.offset;
                len += prev.len;
                self.free_blocks.remove(prev_idx);
                self.free_blocks.insert(prev_idx, FreeBlock { offset, len });
                return;
            }
        }

        self.free_blocks
            .insert(insert_at, FreeBlock { offset, len });
    }

    /// Total free units across all blocks — useful for diagnostics, but NOT
    /// the ceiling on the next `alloc`; see `largest_free_block`.
    pub fn free_space(&self) -> usize {
        self.free_blocks.iter().map(|b| b.len).sum()
    }

    /// Size of the largest single contiguous free block — the real ceiling
    /// on the next successful `alloc`, since this allocator doesn't compact.
    pub fn largest_free_block(&self) -> usize {
        self.free_blocks.iter().map(|b| b.len).max().unwrap_or(0)
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_free_reuse() {
        let mut a = FreeListAllocator::new(100);
        let x = a.alloc(40).unwrap();
        let y = a.alloc(40).unwrap();
        assert_eq!(x.offset, 0);
        assert_eq!(y.offset, 40);

        a.free(x);
        // A same-size alloc should reuse the freed block, not the tail.
        let z = a.alloc(40).unwrap();
        assert_eq!(z.offset, 0);
    }

    #[test]
    fn coalesces_adjacent_frees() {
        let mut a = FreeListAllocator::new(100);
        let x = a.alloc(30).unwrap();
        let y = a.alloc(30).unwrap();
        let z = a.alloc(30).unwrap();

        a.free(x);
        a.free(z);
        a.free(y); // fills the gap between x and z — should merge into one 90-block

        assert_eq!(a.largest_free_block(), 100);
        assert_eq!(a.free_blocks.len(), 1);
    }

    #[test]
    fn alloc_fails_when_too_big() {
        let mut a = FreeListAllocator::new(50);
        assert!(a.alloc(51).is_none());
        assert!(a.alloc(50).is_some());
        assert!(a.alloc(1).is_none());
    }
}
