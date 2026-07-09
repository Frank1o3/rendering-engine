// src/renderer/triple_buffer.rs
use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct SharedState<T> {
    // Wrap the slots in UnsafeCell to allow safe interior mutability via raw pointers
    slots: [UnsafeCell<T>; 3],
    back: AtomicUsize,
    mid: AtomicUsize,
    front: AtomicUsize,
    fresh: AtomicBool,
}

// SAFETY: We manually manage access to the slots via atomics.
// The 'back' slot is only accessed mutably by the single WriteHandle.
// The 'front'/'mid' slots are only accessed mutably by the single ReadHandle.
// Therefore, it is safe to send and share this state across threads.
unsafe impl<T: Send> Send for SharedState<T> {}
unsafe impl<T: Send> Sync for SharedState<T> {}

/// The handle given to the Game Logic thread to write data.
pub struct WriteHandle<T> {
    state: Arc<SharedState<T>>,
}

/// The handle given to the Renderer thread to read data.
pub struct ReadHandle<T> {
    state: Arc<SharedState<T>>,
}

/// Creates a new lock-free triple buffer, returning the write and read handles.
pub fn new_triple_buffer<T: Default + Clone>() -> (WriteHandle<T>, ReadHandle<T>) {
    let state = Arc::new(SharedState {
        slots: [
            UnsafeCell::new(T::default()),
            UnsafeCell::new(T::default()),
            UnsafeCell::new(T::default()),
        ],
        back: AtomicUsize::new(0),
        mid: AtomicUsize::new(1),
        front: AtomicUsize::new(2),
        fresh: AtomicBool::new(false),
    });

    (
        WriteHandle {
            state: state.clone(),
        },
        ReadHandle { state },
    )
}

impl<T: Default + Clone> WriteHandle<T> {
    /// Get a mutable reference to the back slot to write your FrameData.
    #[allow(clippy::mut_from_ref)]
    pub fn write_slot(&self) -> &mut T {
        let idx = self.state.back.load(Ordering::Relaxed);
        // SAFETY: UnsafeCell::get() returns a *mut T directly, bypassing the creation
        // of an intermediate &T. We have exclusive logical access to the 'back' slot.
        unsafe { &mut *self.state.slots[idx].get() }
    }

    /// Publish the written data, swapping 'back' and 'mid'.
    pub fn publish(&self) {
        let b = self.state.back.load(Ordering::Relaxed);
        let m = self.state.mid.swap(b, Ordering::AcqRel);
        self.state.back.store(m, Ordering::Relaxed);
        self.state.fresh.store(true, Ordering::Release);
    }
}

impl<T: Default + Clone> ReadHandle<T> {
    /// Try to consume the latest published data.
    /// Returns true if new data was consumed, false if no new data is available.
    pub fn consume(&self, out: &mut T) -> bool {
        if !self.state.fresh.load(Ordering::Acquire) {
            return false;
        }
        self.state.fresh.store(false, Ordering::Relaxed);

        let f = self.state.front.load(Ordering::Relaxed);
        let m = self.state.mid.swap(f, Ordering::AcqRel);
        self.state.front.store(m, Ordering::Relaxed);

        // SAFETY: We have exclusive logical access to the 'mid' slot during consume.
        unsafe {
            *out = (*self.state.slots[m].get()).clone();
        }
        true
    }
}
