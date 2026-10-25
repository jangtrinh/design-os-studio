# E15-41 Stream generated SVG into frames as it arrives

- Issue: #631 (task E15-41, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-mcp/src/tools/assets.rs` (`kind: "svg"`), `crates/designos-mcp/src/tools/svg_stream.rs` (new: incremental parser and inserter), `crates/designos-eval/src/agent_job.rs` (streaming stdout option, SC-5), `crates/designos-doc/src/vector_fit.rs` (use only), `crates/designos-svg/src/lib.rs` (use only).

## 1. Summary and user story

As an agent, I want logos and illustrations generated as real vectors by my agent CLI and drawn into a frame path by path as the SVG streams in, so the design shows progress and stays editable.

As a user, I want stopping to keep what was complete and mark the frame as partial.

## 2. Current state

- FACT: `designos-svg` converts an SVG string into nodes (`svg_to_nodes`, `import_svg`, `import_svg_named`; `crates/designos-svg/src/lib.rs:97-118`) through `usvg`.
- FACT: `vector_fit` provides curve fitting: `fit_polyline(points, closed, tol, corner_deg) -> Vec<Anchor>` and cubic helpers (`crates/designos-doc/src/vector_fit.rs:47-139,434`).
- FACT: E15-39 defines asset jobs, the pending state and the overlay; E15-7 (#592) flags frames in progress.
- FACT: an MCP server receives a complete `tools/call`; progress can be reported with MCP progress notifications when the client supplies a progress token.

## 3. Behaviour rules

1. **Call.** `designos_generate_asset({ kind: "svg", prompt, target, wait?, fit?: "contain" | "cover" | "none", simplify?: bool })`. `target` must be a frame (`asset_target_invalid` otherwise). Defaults: `fit: "contain"`, `simplify: true`.
2. **Job.** The agent CLI is told (our own prompt) to print one SVG document to stdout, elements in drawing order, one top-level element per line where possible, ending with `</svg>`; optionally also writing it to `{output_dir}/final.svg`. The runner reads stdout incrementally (line-buffered, 64 KiB max line).
3. **Incremental parsing.** A small tokenizer tracks the open `<svg …>` tag (to learn `viewBox`, `width`, `height`), `<defs>` (gradients, clip paths kept for later elements), group nesting and element boundaries. An element is **complete** when its end tag (or self-closing `/>`) has arrived and every `href` it references is defined. Each complete drawable element (`path`, `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `text`, or a closed `g`) is wrapped as `<svg viewBox=…>{defs so far}{element with its ancestor group attributes}</svg>` and converted with `svg_to_nodes`.
4. **Fit.** The stream's `viewBox` maps to the target frame: `contain` = uniform scale `min(W/vbw, H/vbh)` centred; `cover` = `max(…)` centred and the frame clips; `none` = scale 1 at the origin. The transform is fixed by the first `<svg>` tag and applies to every element.
5. **Simplify.** When `simplify` and a converted path has more than 64 segments, it is flattened at 0.1 px tolerance (in frame space) and refitted with `fit_polyline(tol = 0.25, corner_deg = 30)`; closed paths stay closed. Paths with ≤ 64 segments are kept exactly.
6. **Insertion.** Each converted element is inserted as the next child of a group `Generated SVG` created inside the target at stream start, in arrival order. Insertions are applied to the live document immediately (so Studio shows them through the normal live patch path) but recorded into one pending batch; at stream end (or stop) the batch becomes **one undo step** `Generate SVG`. A user undo while the stream runs first stops the stream, closes the batch, then undoes it.
7. **In progress.** The target is flagged in progress (E15-7) and has `agent.asset = Pending { kind: Svg, partial: false }` from start to end. Progress notifications (when a token was given and `wait: true`) report `{ paths_inserted, bytes_read }` after each insertion.
8. **End.** On `</svg>`: if `final.svg` exists, it is the reference for the acceptance check; the flag and pending state clear. Result: `{ status: "done", paths: n }`.
9. **Stop.** `designos_cancel_asset({ job_id })` or agent stop: the stream is cancelled, any incomplete trailing element is discarded, complete paths stay, the in-progress flag clears, and `agent.asset = Failed { reason: "partial: stopped after <n> paths" }`. The frame also gets the name suffix ` (partial)` on the `Generated SVG` group so it is visible in the layers list. AC2.
10. **Errors.** Malformed SVG at an element: that element is skipped and counted in `skipped` with the parser message; three consecutive failures abort the stream as `failed` (complete paths kept, as in rule 9).

## 4. Data model / schema

No new document fields beyond E15-39's `AssetState` (`partial` is reused). In-memory: `SvgStreamState { view_box, transform, defs: String, open_groups: Vec<GroupAttrs>, buffer: String, inserted: Vec<NodeId>, skipped: u32 }`.

## 5. Contracts

- Input `{ "kind": "svg", "prompt": "Minimal fox logo, two colours", "target": 410, "wait": true }` → `{ "job_id": "a-13", "status": "done", "target": 410, "paths": 20, "skipped": 0 }`.
- Progress notification payload: `{ "progress": 7, "total": null, "message": "7 paths" }`.
- `designos_cancel_asset({ job_id })` (new here; applies to every asset kind) → `{ "job_id": "a-13", "status": "cancelled", "paths_kept": 9 }`. For image and stock jobs it cancels the job and sets the E15-39 failed state with reason `cancelled`.
- Errors: E15-39's, plus `svg_stream_invalid` when no `<svg` tag arrives within the first 8 KiB.

## 6. File-format mapping

Inserted nodes are ordinary vector, shape and text nodes; exports are unchanged.

## 7. UI

While streaming, Studio shows the paths appearing (no extra animation) and the E15-39 pending overlay only on the frame's border region (a 2 px hatch frame edge, so the drawing stays visible). Partial frames show the failed overlay edge and the `(partial)` group name. Evidence: three 1440 captures during and after a stream.

## 8. Edge cases

1. SVG without `viewBox`: `width`/`height` attributes define it; neither → `0 0 W H` of the target (scale 1).
2. `<use>` referencing an element defined later: held until the definition arrives; if the stream ends first, skipped.
3. Embedded raster `<image>`: converted only for `data:` URIs (as `designos-svg` does today); external hrefs are skipped and counted.
4. Very long single line containing the whole SVG: still parsed; insertion happens element by element after the line arrives (no visible streaming, same result).
5. Text elements: converted to text nodes with the bundled font fallback rules of `designos-svg`.
6. Target resized during the stream: the transform stays as fixed at start; the result may no longer fit; documented.

## 9. Non-goals

- Hand-drawn vector generation by the engine.
- Streaming partial tool input from the model (an MCP server only receives complete calls).

## 10. Test plan

Seam: shell test with a stub streaming agent (a script printing a 20-path SVG one element per 50 ms, also writing `final.svg`), plus screenshots.

Fail-before: `kind: "svg"` is rejected.

| Test | Input | Expected |
|---|---|---|
| `streams_path_by_path_and_matches_final` (AC1) | 20-path stub | insert events observed one by one (20); final render of the frame vs `import_svg` render of `final.svg` fitted the same way: `designos-diff` SSIM ≥ 0.98 and mismatched ratio ≤ 1% at tolerance 16 |
| `stop_keeps_complete_paths` (AC2) | stop after 9 paths while path 10 is half written | 9 paths kept; in-progress flag cleared; state `Failed { reason: "partial: …9 paths" }` |
| `one_undo_step` | full stream then undo | frame back to its pre-stream hash |
| `undo_during_stream_stops_first` | undo at path 5 | stream stopped, 5-path batch undone |
| `simplify_reduces_segments` | one path with 400 segments | output segments < 400; Hausdorff distance ≤ 0.5 px |
| `malformed_element_skipped` | element 4 broken | 19 paths, `skipped = 1` |

Visual evidence: the three captures in §7.

## 11. Performance budgets

- Per element: parse + convert + insert ≤ 5 ms for paths ≤ 500 segments.
- Live updates use the existing patch path; no full-document sync per path, no `to_json` on the UI thread (CLAUDE.md §4.1).
- Studio repaints only when a patch arrives (no idle repaint, §4.4).

## 12. Dependencies and build order

- Needs E15-39, E15-7 (#592). Parallel with E15-40.

## 13. Owner rulings that apply

- R4.13: the agent CLI generates the SVG; the engine only parses and inserts.
- Nothing vanishes silently: partial results stay and are labelled.
