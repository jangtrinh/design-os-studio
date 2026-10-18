# E5-10 Route AI image tools through the agent CLI: remove background, isolate, expand, erase

| Field | Value |
|---|---|
| Issue | #177 (moved from v0.32 AI tools through your agent CLI to v0.6 AI design) |
| Size / priority | M / P2 |
| Depends on | none |
| Unblocks | part-2 E15-39 (generate images into frames through the agent CLI) |
| Touches | `crates/designos-app/src/terminal.rs` (agent runtime bridge: image jobs), new `crates/designos-app/src/image_jobs.rs`, image paint UI in the fill section and the canvas context menu (`crates/designos-app/src/panels/context.rs`) |

## 1. Summary and user story

As a designer, I select an image layer and choose Remove background, Isolate subject, Expand or Erase.
Studio hands the image and the instruction to my own agent CLI (Claude Code, Codex, Antigravity or Grok,
running in Studio's terminal with my subscription), waits for the result image, and inserts it as a new
layer in one undo step while keeping the original. No model ships inside Studio.

## 2. Current state (FACT)

- Studio runs the user's CLIs in PTY tabs with per-session directories and environment
  (`crates/designos-app/src/terminal.rs:62-176`, `220-300`, `668-760`); prompts are sent with
  `submit_chat_prompt` (`terminal.rs:1006`); run state arrives through `agent-status.json`
  (`terminal.rs:863-930`); `AgentCliPreset::is_installed` checks for each CLI (`terminal.rs:167`).
- There are no AI image commands and no bundled model crates (`crates/designos-app/Cargo.toml`).
- Owner ruling R4.13: remote AI features go through the agent CLI; no model is bundled, not even
  segmentation.

## 3. Behaviour rules

1. Commands (on a selected layer with exactly one visible image fill, or an image-filled rectangle):
   `Remove background`, `Isolate subject`, `Expand…` (top/right/bottom/left in px, 0..=4096, or a
   preset 25% each side), `Erase…` (an instruction such as "the person on the left", and an optional
   rectangular region drawn on the image as a mask).
2. Availability: enabled only when at least one agent CLI is installed and an agent tab for this file
   is running or can be started; otherwise disabled with the tooltip `Set up an agent CLI in the Agents
   panel to use AI image tools` (acceptance 2).
3. Job protocol (one job at a time per file; others queue):
   - Studio exports the image at its native resolution (the image fill's bytes, crop applied) to
     `<session_dir>/image-jobs/<job>/input.png` (and `mask.png` for Erase: white = erase region), plus
     `request.json` `{op, instruction, input, output, mask?, expand?, expected_size}`.
   - Studio sends one prompt to the active agent tab: "Image task <job>: <operation text>. Read
     <input>, write the result as PNG to <output>. Keep the same pixel size unless told otherwise. Do
     not change other files." (the operation text is fixed per command; the user's Erase instruction is
     appended inside quotes).
   - A watcher (off the UI thread) waits for `output.png` to appear and stay unchanged for 500 ms, or
     for the run state `done` without output, or for a 5-minute timeout. The user can Cancel.
4. Validation of the result: decodes as PNG; size equals `expected_size` (input size, or input plus the
   expansion for Expand); for Remove background / Isolate the image has an alpha channel with at least
   one transparent pixel (else a warning in the toast, still inserted). Failures produce a toast with
   the reason and nothing changes.
5. Insertion (one undo step `AI: <command>`): a new rectangle with the result as image fill, same
   transform as the original (Expand grows the bounds outward by the expansion, preserving scale),
   inserted directly above the original in the same parent, named `<name> · <command>`; the original is
   hidden (`visible = false`), never deleted (acceptance 3). Undo restores the original's visibility and
   removes the new layer.
6. The job directory is kept until the file closes (the user can inspect it); then removed.
7. No network access by Studio for these commands; whatever the agent CLI does is the user's own
   subscription and tools.

## 4. Data model / schema changes

`ImageJob { id, op, node, state: Queued | Running | Done | Failed(String) | Cancelled, started }` in
app state; job files in the session directory. No document change.

## 5. Contracts

No MCP tool. The request file format above is the contract with the agent; it is also described in the
rules pack (E15-15) so agents know the convention.

## 6. File-format mapping

None (results are ordinary image fills).

## 7. UI (load es-designer before UI code; desktop capture at 1440, responsive SKIPPED)

- Image fill row in the fill section gets an `AI` menu button (24 px) listing the four commands;
  the canvas context menu shows the same four items under `AI image`.
- Expand and Erase open a small popover (fields in mono, P5; region drawing for Erase uses a dashed
  rectangle overlay in Selection Blue).
- While running: a progress chip on the layer label `Agent working…` with Cancel; the Agents chat
  shows the prompt as a system line.
- Disabled state shows the tooltip from rule 2. Every control does real work (P6).

## 8. Edge cases

1. Image fill with crop/tile settings: the job uses the visible crop; tiled fills are refused with a
   reason (ambiguous source).
2. Multiple image fills on one layer: the topmost visible one is used, named in the popover.
3. Agent writes a JPEG with `.png` name: decoded by content sniffing; accepted if it decodes and sizes
   match; alpha warning applies.
4. Agent returns a different size: rejected with `expected WxH, got wxh`.
5. File closed during a job: job cancelled; no insertion.
6. Two commands in a row: the second queues; a toast shows queue position.

## 9. Non-goals

- Generating new images from prompts (part-2 E15-39), vectorizing, upscaling.
- Any bundled or downloaded model.

## 10. Test plan

Seams: shell test with a stub agent command and a screenshot.

| Case | Expected |
|---|---|
| `remove_background_inserts_result_one_undo` | stub agent (shell script) copies a fixture transparent PNG to `<output>` → new layer above the original with the result fill; original hidden; one undo restores |
| `no_agent_disables_commands` | `is_installed` false for all presets (test seam overrides PATH) → menu items disabled with the tooltip |
| `original_kept` | after the job, the original node exists with its image intact |
| `size_mismatch_rejected` | stub writes a smaller PNG → toast with sizes; document unchanged |
| `no_bundled_model_crate` | `cargo tree -p designos-app --edges normal` contains none of: `ort`, `onnxruntime`, `tract`, `candle`, `burn`, `tch`, `rust-bert`, `llama` |
| `expand_bounds` | Expand 10 px each side on 100x100 → new layer 120x120 at (-10,-10) relative to the original |

## 11. Performance budgets

UI thread work is limited to starting the job and applying the final command; export and watching run
off the UI thread; idle CPU stays 0% when no job runs.

## 12. Dependencies and build order

No upstream dependency. Before part-2 E15-39 (shares the job protocol and placeholder pattern).

## 13. Owner rulings

- R4.13: all AI through the agent CLI; no model bundled, not even segmentation.
- Nothing vanishes: the original image is kept.
- Responsive checks only on request.
