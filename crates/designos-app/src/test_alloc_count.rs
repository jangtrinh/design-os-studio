//! Counts the heap allocations the current thread makes while a closure runs, so a test can bound the work a
//! code path does without timing it. A count does not move with machine load or with the other tests running
//! beside it, which a wall-clock budget does.
//!
//! The counter belongs to the thread that asked for it: an allocation on any other thread is not counted, and
//! a thread that never asks pays one thread-local read per allocation.
//!
//! [`peak_bytes_in`] also reports the most heap the closure held at one time beyond what it started with, so a test can tell a
//! save that streams a document into its file from one that builds the whole file in memory first.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    /// Heap bytes held now, relative to when the measurement started (negative when the closure frees what it did not allocate).
    static LIVE_BYTES: Cell<isize> = const { Cell::new(0) };
    static PEAK_BYTES: Cell<isize> = const { Cell::new(0) };
}

/// The system allocator, counting for the threads that opted in through [`allocations_in`].
struct CountingAllocator;

fn note_allocation(grown_by: usize) {
    // `try_with`: a thread's locals may already be gone while its last allocations are freed.
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCATIONS.try_with(|n| n.set(n.get() + 1));
            note_bytes(grown_by as isize);
        }
    });
}

fn note_bytes(delta: isize) {
    let _ = LIVE_BYTES.try_with(|live| {
        live.set(live.get() + delta);
        let _ = PEAK_BYTES.try_with(|peak| peak.set(peak.get().max(live.get())));
    });
}

fn note_release(freed: usize) {
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            note_bytes(-(freed as isize));
        }
    });
}

// SAFETY: every method forwards to `System` with the arguments it was given, so the layout contract is the
// system allocator's own. The counter touches only thread-local `Cell`s and never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation(layout.size());
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation(layout.size());
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note_allocation(0);
        let _ = COUNTING.try_with(|on| {
            if on.get() {
                note_bytes(new_size as isize - layout.size() as isize);
            }
        });
        System.realloc(ptr, layout, new_size)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note_release(layout.size());
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Stops counting when dropped, so a panic in the measured closure leaves the thread uncounted.
struct Counting;

impl Drop for Counting {
    fn drop(&mut self) {
        COUNTING.with(|on| on.set(false));
    }
}

/// Runs `work` and returns its value with the number of allocations (including growth of an existing
/// buffer) the current thread made while it ran. Calls do not nest.
pub fn allocations_in<R>(work: impl FnOnce() -> R) -> (R, usize) {
    let (value, allocations, _) = measure(work);
    (value, allocations)
}

/// Runs `work` and returns its value with the most heap, in bytes, the current thread held at one time beyond what it held when
/// `work` began. Calls do not nest.
pub fn peak_bytes_in<R>(work: impl FnOnce() -> R) -> (R, usize) {
    let (value, _, peak) = measure(work);
    (value, peak)
}

fn measure<R>(work: impl FnOnce() -> R) -> (R, usize, usize) {
    assert!(!COUNTING.with(Cell::get), "allocations_in does not nest");
    ALLOCATIONS.with(|n| n.set(0));
    LIVE_BYTES.with(|n| n.set(0));
    PEAK_BYTES.with(|n| n.set(0));
    let guard = Counting;
    COUNTING.with(|on| on.set(true));
    let value = work();
    drop(guard);
    (value, ALLOCATIONS.with(Cell::get), PEAK_BYTES.with(Cell::get).max(0) as usize)
}

#[cfg(test)]
mod tests {
    use super::allocations_in;

    #[test]
    fn counts_the_allocations_of_the_closure_and_nothing_outside_it() {
        let (_, none) = allocations_in(|| 1 + 1);
        assert_eq!(none, 0);
        let (v, some) = allocations_in(|| (0..10).map(|i| vec![i; 4]).collect::<Vec<_>>());
        assert_eq!(v.len(), 10);
        assert!(some >= 10, "ten vectors were allocated, counted {some}");
        let (_, after) = allocations_in(|| ());
        assert_eq!(after, 0, "the count starts over for every call");
    }

    #[test]
    fn the_peak_is_the_most_held_at_once_not_the_total_allocated() {
        let (_, peak) = super::peak_bytes_in(|| (0..100).for_each(|_| drop(vec![0u8; 100_000])));
        assert!((100_000..400_000).contains(&peak), "one buffer at a time: peak {peak}");
        let (_, peak) = super::peak_bytes_in(|| (0..10).map(|_| vec![0u8; 100_000]).collect::<Vec<_>>());
        assert!(peak >= 1_000_000, "ten buffers held together: peak {peak}");
    }

    #[test]
    fn another_threads_allocations_are_not_counted() {
        let (_, n) = allocations_in(|| {
            std::thread::spawn(|| (0..500).map(|i| vec![i; 4]).collect::<Vec<_>>()).join().unwrap();
        });
        // spawning allocates a handful of times on this thread; the 500 vectors are the other thread's
        assert!(n < 100, "only this thread's own spawn bookkeeping is counted, saw {n}");
    }
}
