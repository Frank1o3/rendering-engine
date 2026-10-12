use std::sync::{Arc, Mutex};

struct Published<T> {
    value: T,
    version: u64,
}

struct SharedState<T> {
    published: Mutex<Published<T>>,
}

/// Single-producer handle. Frame construction happens in its private back slot;
/// only `publish` briefly locks while swapping that slot with the latest value.
pub struct WriteHandle<T> {
    state: Arc<SharedState<T>>,
    back: T,
}

/// Single-consumer handle. `consume` moves the latest published value into the
/// caller's reusable frame and returns that frame's old storage to the writer.
pub struct ReadHandle<T> {
    state: Arc<SharedState<T>>,
    consumed_version: u64,
}

/// Create a latest-value triple buffer with separate producer, published, and
/// consumer slots. Publication and consumption use a short mutex-protected swap;
/// frame construction, rendering, and vector processing happen outside the lock.
pub fn new_triple_buffer<T: Default>() -> (WriteHandle<T>, ReadHandle<T>) {
    let state = Arc::new(SharedState {
        published: Mutex::new(Published {
            value: T::default(),
            version: 0,
        }),
    });

    (
        WriteHandle {
            state: state.clone(),
            back: T::default(),
        },
        ReadHandle {
            state,
            consumed_version: 0,
        },
    )
}

impl<T: Default> WriteHandle<T> {
    /// Get the producer-owned slot to fill before calling `publish`.
    pub fn write_slot(&mut self) -> &mut T {
        &mut self.back
    }

    /// Publish the frame and recycle the previously published storage.
    pub fn publish(&mut self) {
        let mut published = self
            .state
            .published
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::swap(&mut self.back, &mut published.value);
        published.version = published.version.wrapping_add(1);
    }
}

impl<T: Default> ReadHandle<T> {
    /// Consume the newest published value, skipping intermediate publications.
    /// Returns false when no newer value is available.
    pub fn consume(&mut self, out: &mut T) -> bool {
        let mut published = self
            .state
            .published
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if published.version == self.consumed_version {
            return false;
        }

        std::mem::swap(out, &mut published.value);
        self.consumed_version = published.version;
        true
    }
}
