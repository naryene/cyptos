//! Bump heap allocator for the CyptOS kernel.
//!
//! A simple, non-freeing allocator that advances a pointer through a fixed
//! heap region. Suitable for single-threaded kernel use (hart 0 only).
//! Reference: RISC-V linker symbols _sheap/_eheap provide the heap bounds.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

/// A bump allocator that advances a pointer through the heap region.
/// Deallocation is a no-op (memory is never freed).
pub struct BumpAllocator {
    heap_start: AtomicUsize,
    heap_end: AtomicUsize,
    next: AtomicUsize,
}

impl BumpAllocator {
    /// Creates an uninitialized allocator. Must call `init` before use.
    pub const fn new() -> Self {
        Self {
            heap_start: AtomicUsize::new(0),
            heap_end: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
        }
    }

    /// Initialize the allocator with heap bounds from linker symbols.
    /// Must be called exactly once before any allocation.
    pub fn init(&self, heap_start: usize, heap_end: usize) {
        self.heap_start.store(heap_start, Ordering::Relaxed);
        self.heap_end.store(heap_end, Ordering::Relaxed);
        self.next.store(heap_start, Ordering::Relaxed);
    }
}

// SAFETY: This allocator is only used on single-hart (hart 0) kernel code.
// AtomicUsize ensures the next pointer is updated atomically.
unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let end = self.heap_end.load(Ordering::Relaxed);
        loop {
            let current = self.next.load(Ordering::Relaxed);
            // Align the current pointer up to the required alignment
            let align = layout.align();
            let aligned = (current + align - 1) & !(align - 1);
            let new_next = aligned + layout.size();
            if new_next > end {
                // OOM: return null pointer (will cause alloc error → panic)
                return core::ptr::null_mut();
            }
            // Atomically advance the pointer
            if self
                .next
                .compare_exchange(current, new_next, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator: deallocation is a no-op
    }
}
