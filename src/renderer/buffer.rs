use glow::HasContext;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub struct GpuBuffer {
    gl: Arc<glow::Context>,
    pub handle: glow::Buffer,
    pub target: u32,
}

impl GpuBuffer {
    pub fn new(gl: Arc<glow::Context>, target: u32) -> Self {
        unsafe {
            let handle = gl.create_buffer().expect("Failed to create buffer");
            Self {
                gl: gl,
                handle,
                target,
            }
        }
    }

    pub fn upload_data<T: bytemuck::Pod>(&self, data: &[T], usage: u32) {
        unsafe {
            self.gl.bind_buffer(self.target, Some(self.handle));
            self.gl
                .buffer_data_u8_slice(self.target, bytemuck::cast_slice(data), usage);
        }
    }
}

impl Drop for GpuBuffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.handle);
        }
    }
}

pub struct TripleBuffer<T> {
    slots: [T; 3],
    back: AtomicUsize, // producer writes to this index
    mid: AtomicUsize,  // staging index (latest published)
    front: usize,      // consumer reads from this index (non‑atomic, only consumer touches it)
    fresh: AtomicBool, // true if there's new data to consume
}

impl<T: Default + Clone> TripleBuffer<T> {
    pub fn new() -> Self {
        Self {
            slots: [T::default(), T::default(), T::default()],
            back: AtomicUsize::new(0),
            mid: AtomicUsize::new(1),
            front: 2,
            fresh: AtomicBool::new(false),
        }
    }

    // Producer side: get a mutable reference to the back slot
    pub fn write_slot(&mut self) -> &mut T {
        let idx = self.back.load(Ordering::Relaxed);
        &mut self.slots[idx]
    }

    // Producer side: publish the written data
    pub fn publish(&mut self) {
        let b = self.back.load(Ordering::Relaxed);
        let m = self.mid.swap(b, Ordering::AcqRel);
        self.back.store(m, Ordering::Relaxed);
        self.fresh.store(true, Ordering::Release);
    }

    // Consumer side: try to consume the latest published data into `out`
    pub fn consume(&mut self, out: &mut T) -> bool {
        if !self.fresh.load(Ordering::Acquire) {
            return false;
        }
        self.fresh.store(false, Ordering::Relaxed);

        let m = self.mid.swap(self.front, Ordering::AcqRel);
        self.front = m;
        *out = self.slots[self.front].clone();
        true
    }
}
