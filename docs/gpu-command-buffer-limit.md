# GPU command buffer limit (Metal)

The limit that made the idle refine lose the Metal device in the 0.3.0 release (issue #467), measured on an Apple M4 Max with wgpu 29.0.4.

## The limit

wgpu's Metal backend (`wgpu-hal/src/metal/adapter.rs`, `MAX_COMMAND_BUFFERS = 4096`, also the `maxCommandBufferCount` of its command queue) counts the command buffers it has created and not yet submitted, per queue. It refuses to start the 4,097th and reports a lost device. Every later call fails, and egui-wgpu then panics with `Failed to create staging buffer for index data`, exit 101.

The log line that names it (`wgpu_hal::metal::command`, a warning):

> metal: refusing to create new command buffer; 4097 outstanding command buffers exceeds the limit of 4096. Treating this as device lost.

The counter drops back to zero when the command buffers are submitted. It is a software constant of the backend, so it does not depend on the GPU model.

## What costs command buffers

Measured with bare wgpu in one encoder (`tests/command_buffer_limit.rs`):

| Recorded into one encoder | Largest count that survives |
|---|---|
| Render passes | 2,047 (2,048 loses the device) |
| Texture copies | at least 8,192 (a run of copies shares one command buffer) |
| Alternating render pass and copy | 2,729 operations |

wgpu-core records each render pass in two command buffers: one for the pass and one in front of it for the resource transitions the pass needs. A run of copies between passes costs one more. Nothing else the renderer does counts.

A blurred layer is 4 passes (the layer, two blur passes, the composite). That is why the program count alone does not predict the loss.

## Evidence on the Untitled UI kit

20 large component sets, 6 zooms (10% to 100%), 2880 x 1800 viewport at 2x, the first refine after the capped frame converged, 120 runs:

- The refine lost the device in 11 runs. Each lost refine needs 2,083 render passes or more (up to 3,876).
- Every refine that survived needs 1,940 passes or fewer, with up to 952 programs. Lost refines had 970 programs or more.
- With the fix, 0 of the 120 runs lose the device. The largest frames are cut into 4 submissions.

## The fix

`Renderer::render_submitted` and `render_to_rgba` count the estimated command buffers they record (2 per render pass, 1 per copy) and submit the encoder before the count passes `SUBMIT_HAL_BUDGET` (2,048, half the limit), then go on in a new encoder. Queue order keeps the passes in recording order: the pixels are the same as in one submission (`splitting_a_frame_into_submissions_changes_no_pixel`).

Half the limit leaves room for egui's own encoders (open while the paint callback runs), wgpu's pending-writes encoder and estimate error.

`Renderer::render`, which records into the caller's encoder, cannot split. Its frame must stay within 2,047 passes; `ExecStats::hal_buffers_max` is the estimate.

If a wgpu upgrade changes these numbers, `metal_holds_2047_render_passes_in_one_submission_and_loses_the_device_at_2048` fails.
