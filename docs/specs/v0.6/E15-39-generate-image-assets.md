# E15-39 Generate images into frames through the agent CLI with a pending placeholder

- Issue: #628 (task E15-39, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-mcp/src/tools/assets.rs` (new), `crates/designos-eval/src/agent_job.rs` (new: the agent job runner, SC-5; default features, so `designos-mcp` and `designos-app` share it without any inference code, and no dependency cycle arises because `designos-eval` depends on neither), the agent runtime bridge from E5-10 (#177) in `crates/designos-app/src/terminal.rs` (job execution in Studio), `crates/designos-doc/src/agent_meta.rs` (SC-4: `asset` state), `crates/designos-doc/src/image_key.rs` (new: SHA-1 image keys), `crates/designos-app/src/canvas/` overlay for pending and failed states.

## 1. Summary and user story

As an agent, I want to ask for an image in a frame and keep designing while it is produced; the frame shows a pending placeholder at once, and when the image arrives it replaces the placeholder in one undo step, following the active style's imagery recipe.

As a user, I want images made by my own agent CLI, never by a model bundled in the app, and a visible failed state if anything goes wrong.

## 2. Current state

- FACT: image paints are `PaintKind::Image { image_ref, scale_mode }` keyed into `Document.images` (`crates/designos-doc/src/paint.rs:209-213`, `crates/designos-doc/src/lib.rs:178-180`). The SVG importer keys embedded images by FNV (`svg-{:016x}`, `crates/designos-svg/src/convert.rs:395`); `.fig` export writes a hex key as Figma's image hash bytes and any other key as a string (`crates/designos-fig/src/fig_export.rs:1727-1733`).
- FACT: E5-10 (#177) routes AI image tools through the user's Studio agent runtime and requires "No agent configured: commands are disabled with an explanation" and "cargo tree shows no bundled model crate".
- FACT: E15-18 archetypes carry imagery recipes (`kind`, `prompt`, `aspect_ratios`, `avoid`); E15-7 (#592) flags frames in progress.
- FACT: idle CPU must stay 0% (CLAUDE.md §4.4): nothing may animate at rest.

## 3. Behaviour rules

1. **Agent job (SC-5, owned here).** A job = `{ id, kind, prompt_file, inputs[], output_dir, timeout_s, profile }`. The runner writes a prompt file, runs the profile's command with placeholders `{prompt_file}`, `{output_dir}`, `{model}` (E15-25 placeholder rules), and on exit 0 reads `output_dir/result.json` (`{ "status": "ok" | "miss" | "error", "files": [ … ], "meta": { … }, "message": "…" }`). Timeout → SIGTERM, then SIGKILL after 5 s, status `timeout`. Cancel = the same signals. In Studio the bridge from E5-10 runs jobs; in headless `designos-mcp` the same runner reads the profile from the settings file (`designos-eval.toml` format) or `DESIGNOS_AGENT_PROFILE`; with neither, asset tools are disabled.
2. **Call.** `designos_generate_asset({ kind: "image", prompt, target, scale_mode?, wait?, aspect?, recipe? })`:
   - `target` = a node id of a frame, rectangle or ellipse (image fills apply to shapes and frames);
   - immediately sets `target.agent.asset = Pending { job_id, kind, started_at }` and flags the target in progress (E15-7), then returns `{ job_id, status: "pending" }` unless `wait: true` (then it blocks up to the job timeout and returns the final status, streaming MCP progress notifications when the client sent a progress token).
3. **Prompt.** The job prompt is our own template: the agent's prompt, the target's size and aspect, and the active style's imagery recipe (`Document.applied_style` → E15-18 recipe: its `prompt`, preferred aspect ratios, `avoid` list) unless `recipe: "none"`. The template tells the agent CLI to write one PNG, JPEG or WebP file into `{output_dir}` and the `result.json`.
4. **Completion.** On `ok`: validate the file (decodes; ≤ 64 MiB; long edge ≤ 8192 px, larger images are downscaled to 8192 with Lanczos3 and the result notes it; WebP converted to PNG because the renderer decodes PNG and JPEG, matching `convert.rs:384-392`), store bytes under `image_key = lowercase hex SHA-1 of the stored bytes` (SC-7; Figma's hash format, so `.fig` export writes it as hash bytes), and replace the target's fills with one image paint (`scale_mode` default `fill`) in **one undo step**. Clear `agent.asset` and the in-progress flag.
5. **Pending is not a paint** (owner ruling 2026-10-19: canvas overlay). The pending and failed states live in `agent.asset`, not in `fills`, so exports, undo history and the document's real paints never contain placeholder data. Studio draws them as a canvas overlay: pending = 45° hatch in `BORDER_STRONG` over the node's bounds with the label `Generating image…` (11 px, `TEXT_SECONDARY`); failed = the same hatch in `DANGER` with `Image failed — <short reason>` and a `Retry` action in the context menu. The overlay is static (no spinner), so idle CPU stays 0%; it repaints only on state changes. Headless renders ignore it.
6. **Failure.** `error`, `timeout`, invalid file or agent stop → `agent.asset = Failed { job_id, reason, at }`, in-progress flag cleared, fills unchanged. Never an empty frame without a warning (AC rule). `designos_asset_status` reports it; the E15-12 issues block of the next write includes a warning row for every failed asset in the touched frames.
7. **No agent configured (AC2).** `designos_generate_asset` returns `agent_not_configured` with an explanation and how to configure a profile; Studio's image menu entries are disabled with the same explanation.
8. **No bundled model (AC3).** No crate in `designos-app` or `designos-mcp` trees performs model inference; CI checks `cargo tree -p designos-app -e normal` and `-p designos-mcp` against a deny list (`candle`, `ort`, `tch`, `tract`, `burn`, `llama`, `whisper`, `onnx`).
9. **Concurrency.** Up to 3 asset jobs per document run at once (setting `assets.max_parallel`); others queue with `Pending` state.
10. **Retry.** `designos_retry_asset({ node_id })` re-runs a failed job with the same prompt.

## 4. Data model / schema

```rust
// agent_meta.rs (SC-4; this task adds `asset`)
#[serde(default, skip_serializing_if = "Option::is_none")] pub asset: Option<AssetState>,
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)] #[serde(tag = "state", rename_all = "snake_case")]
pub enum AssetState { Pending { job_id: String, kind: AssetKind, started_at: String, partial: bool },
                      Failed { job_id: String, kind: AssetKind, reason: String, at: String } }
pub enum AssetKind { Image, Stock, Svg }
// designos-eval/src/agent_job.rs
pub trait AgentJobRunner: Send + Sync { fn run(&self, job: AgentJob, cancel: CancelToken, progress: &dyn Fn(JobProgress)) -> JobOutcome; }
pub struct AgentJob { pub id: String, pub kind: String, pub prompt: String, pub inputs: Vec<PathBuf>, pub output_dir: PathBuf, pub timeout: Duration, pub profile: AgentProfile }
pub enum JobOutcome { Ok { files: Vec<PathBuf>, meta: serde_json::Value }, Miss { message: String }, Error(String), Timeout, Cancelled }
```

A document saved while a job is pending reloads with `Pending` turned into `Failed { reason: "interrupted" }` (the process that ran the job is gone).

## 5. Contracts

- Input: `{ "kind": "image", "prompt": "Warm morning light over a quiet desk", "target": 204, "scale_mode": "fill" }` → `{ "job_id": "a-11", "status": "pending", "target": 204 }`.
- `designos_asset_status({ job_id })` → `{ "job_id": "a-11", "status": "pending" | "done" | "failed" | "miss", "target": 204, "image_key": "3f…", "reason": null }`.
- Errors: `agent_not_configured` (`safe_to_retry: false`), `asset_target_invalid` (node is text or a group; corrected example uses the nearest frame), `asset_job_unknown`, `asset_busy` (target already pending; `safe_to_retry: true` after completion).

## 6. File-format mapping

- `.designos`: image bytes in `images`, `agent.asset` while pending or failed.
- `.fig` export: the image paint exports with its SHA-1 as Figma's hash; `agent.asset` is not exported.

## 7. UI

Studio: the overlay in rule 5; a `Generate image…` entry in the context menu of frames and shapes (opens a one-line prompt field, Enter starts the job); disabled with a tooltip when no agent is configured. Evidence: desktop capture at 1440 of a pending and a completed frame.

## 8. Edge cases

1. Target deleted while pending: the result is discarded; the job's status is `failed` with `target_deleted`.
2. Target resized while pending: the image fills the new size (fill paint scales with the node).
3. The agent writes two files: the first valid image is used; the result notes the extra file.
4. Undo during pending: undo affects document history only; the pending state stays; completion still creates its own undo step.
5. The agent CLI returns a file with a misleading extension: format sniffed from bytes; unsupported → failed `unsupported_format`.
6. Identical bytes generated twice: same SHA-1 key, stored once.

## 9. Non-goals

- Stock search (E15-40), SVG (E15-41), editing existing images (E5-10's tools).
- Any built-in image model.

## 10. Test plan

Seam: shell test with a stub agent command (a script that writes a fixture PNG and `result.json` after 300 ms), plus a screenshot; MCP tests in `designos-mcp`.

Fail-before: the tool does not exist.

| Test | Input | Expected |
|---|---|---|
| `pending_then_replaced` (AC1) | generate on a 400×300 frame with the stub | state Pending immediately; after completion fills = one image paint, key = SHA-1 of stored bytes; one undo removes it |
| `disabled_without_agent` (AC2) | no profile | `agent_not_configured`; Studio menu entry disabled with the explanation |
| `no_model_crate` (AC3) | `cargo tree` checks | deny list absent |
| `failure_leaves_visible_state` | stub exits 1 | `Failed` with reason; fills unchanged; overlay drawn in the screenshot |
| `recipe_in_prompt` | applied style with a photo recipe | prompt file contains the recipe prompt and avoid list |
| `interrupted_on_reload` | save while pending, reload | state `Failed { reason: "interrupted" }` |
| `idle_cpu_zero_with_pending` | Studio idle with a pending overlay for 2 s | zero repaint requests |

Visual evidence: 1440 captures of pending, completed and failed frames.

## 11. Performance budgets

- Tool returns ≤ 20 ms (state write only).
- Image decode and SHA-1 off the UI thread; texture upload outside `ctx.data_mut` (CLAUDE.md §4.2).
- No periodic repaint while pending (§4.4).

## 12. Dependencies and build order

- Needs #177 (E5-10), E15-7 (#592), E15-18 (#604).
- Unblocks E15-40, E15-41, E15-48 (`job` image source).

## 13. Owner rulings that apply

- R4.13: the user's agent CLI produces every image; no model is bundled.
- Nothing vanishes silently: failures stay visible.
