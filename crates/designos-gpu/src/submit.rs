//! Splits one frame over several queue submissions so that no submission asks
//! the Metal backend for more command buffers than it can create.
//!
//! The limit (issue #467): wgpu-hal's Metal backend counts the command buffers
//! it has created and not yet submitted, and refuses to start the 4,097th
//! (`MAX_COMMAND_BUFFERS = 4096` in `wgpu-hal/src/metal/adapter.rs`, the
//! `maxCommandBufferCount` of its command queue). The refusal is reported as a
//! lost device, after which every call fails; egui-wgpu then panics with
//! "Failed to create staging buffer for index data". The count is per queue and
//! drops back when the command buffers are submitted.
//!
//! What costs command buffers (measured on an Apple M4 Max with wgpu 29.0.4,
//! `docs/gpu-command-buffer-limit.md`):
//!
//! - every render pass: 2 (wgpu-core records the pass and, in front of it, the
//!   resource transitions the pass needs, each in a command buffer of its own);
//! - every run of copies between passes: 1;
//! - nothing else this renderer does.
//!
//! One encoder therefore survives at most 2,047 render passes. A frame of a few
//! hundred offscreen programs (a blurred layer is 4 passes) is past that, so the
//! executor submits what it has recorded whenever the running estimate nears
//! [`SUBMIT_HAL_BUDGET`] and goes on in a fresh encoder. Queue order keeps the
//! passes in recording order, so splitting changes nothing about the pixels.

/// Command buffers wgpu-hal's Metal backend lets exist unsubmitted, per queue.
pub const MAX_HAL_COMMAND_BUFFERS: u32 = 4096;

/// Command buffers wgpu-core records for one render pass.
pub const HAL_BUFFERS_PER_PASS: u32 = 2;

/// Command buffers wgpu-core records for a run of copies (counted per copy: an
/// upper bound).
pub const HAL_BUFFERS_PER_COPY: u32 = 1;

/// Estimated command buffers one submission may hold: half the limit. The rest
/// is for the host's own encoders (egui keeps a few open while a paint callback
/// runs), wgpu's pending-writes encoder and estimate error.
pub const SUBMIT_HAL_BUDGET: u32 = MAX_HAL_COMMAND_BUFFERS / 2;

/// Counts the command buffers recorded into an encoder and, when the executor
/// owns that encoder, submits it before the count passes its budget
/// (`Renderer::submit_budget`, [`SUBMIT_HAL_BUDGET`] by default).
pub(crate) struct Splitter {
    dev: wgpu::Device,
    queue: wgpu::Queue,
    /// Command buffers one submission may hold, or `None` when the encoder may
    /// not be submitted mid-frame (a caller's encoder is submitted by the
    /// caller).
    budget: Option<u32>,
    /// Estimated command buffers recorded since the last submission.
    used: u32,
    /// Largest `used` of any submission of the frame.
    pub max_used: u32,
    /// Submissions made by the splitter (the frame's last one is the caller's).
    pub submissions: u32,
}

impl Splitter {
    pub fn new(dev: &wgpu::Device, queue: &wgpu::Queue, budget: Option<u32>) -> Self {
        Self { dev: dev.clone(), queue: queue.clone(), budget, used: 0, max_used: 0, submissions: 0 }
    }

    /// Call before recording something that costs `cost` command buffers into
    /// `enc`. May swap `enc` for a fresh encoder after submitting the old one.
    pub fn reserve(&mut self, enc: &mut wgpu::CommandEncoder, cost: u32) {
        if self.budget.is_some_and(|b| self.used > 0 && self.used.saturating_add(cost) > b) {
            let next = self.dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("designos-gpu frame (continued)") });
            let full = std::mem::replace(enc, next);
            self.queue.submit([full.finish()]);
            self.submissions += 1;
            self.used = 0;
        }
        self.used += cost;
        self.max_used = self.max_used.max(self.used);
    }
}
