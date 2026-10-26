# DEV Report — `#1197`: Frame Name Labels Behave Exactly Like Figma's

## 1. What Could NOT Be Verified First
- **Figma's exact narrow-width threshold for a component/instance diamond glyph + `"…"` (`UNKNOWN` in Figma 126.10.11 evidence):** The reference Figma captures in `plans/reports/1197-evidence/` test narrow plain frames (`40px`, `30px`, `15px` at 68% zoom) and a `300px` component (`Card`), but do not include a `15px` component/instance in Figma 126.10.11. Per task spec item 1 ("For components and instances, the glyph comes first. If the glyph plus '…' does not fit, paint '…' alone. This is an ASSUMPTION (Figma not tested); state it in the report"), Studio's behaviour is verified by unit test `frame_label_f2_narrow_40_30_15_frames_and_component_at_zoom_068`, while Figma's own glyph threshold on narrow components remains unverified against live Figma (per rule: never open Figma).
- **Items explicitly marked `UNKNOWN` in the spec (hover, sections as covered label or cover, `CANVAS_LABEL_MAX_ZOOM_HIDE`, and dev-status / in-progress badges):** Preserved unchanged from `9680eb2e` as instructed.

---

## 2. Conclusion & Epistemic Ledger

### Conclusion
Studio's canvas frame/component/instance name labels now match Figma 126.10.11 across **F1–F6**:
1. **Width & Real-Font Elision (`F1`, `F2`)**:
   - Added [`widgets::elide_measured`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/widgets/mod.rs#L35-L45) using a thread-local `egui::Context` configured with `theme::install_fonts` so `label_entries` / `label_rects` and the overlay painter measure label text with the exact same Inter 11px font metrics instead of the old `chars × 0.66 × font_size` estimate.
   - Removed the `.max(24.0)` width floor in [`canvas::draw::label_entries`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/canvas/draw.rs#L94-L131) and the `+ 8.0` painter overflow in [`canvas::draw::overlays`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/canvas/draw.rs#L906-L947). Label rects never exceed `w_px` (`b.w * zoom`) and are clamped to `text_w.min(w_px)`.
   - When not even one character plus `"…"` fits inside `w_px`, `"…"` alone is painted and the label is never hidden for narrowness.
2. **Top-Level Filled Frame Cover Clipping (`F4`, `F5`)**:
   - Inside the cached [`canvas::draw::label_entries`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/canvas/draw.rs#L133-L234) computation (keyed by `(doc_Ptr, page_id, tiles_rev, zoom_bits, min_x_bits, min_y_bits, max_x_bits, max_y_bits)` so viewport-only frames recompute nothing when static), Studio collects the on-screen rect of every visible top-level `Frame`, `Component`, `ComponentSet`, or `Instance` on the page that has at least one visible fill (`visible && opacity > 0.0`, via [`has_covering_fill`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/canvas/draw.rs#L46-L52)) and intersects the union of viewport label rects.
   - Each label rect subtracts every other top-level covering rect via [`subtract_rect`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/canvas/draw.rs#L56-L90), regardless of layer-stack order. Frames without a visible fill and child shapes inside a frame do not cover labels; `Section`s neither cover nor get covered.
   - [`canvas::draw::overlays`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/canvas/draw.rs#L918-L946) paints each visible piece in `entry.pieces` with `p.with_clip_rect(*piece)`, cutting the label cleanly at the covering frame's edge without adding `"…"`.
3. **Hit Testing**:
   - [`canvas::interact::label_hit`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/canvas/interact.rs#L918-L920) checks `entry.pieces.iter().any(|p| p.contains(pos))`, so clicking a covered portion of a label falls through to whatever node is under the pointer.

### Epistemic Ledger
- **`FACT`**: On `9680eb2e`, `label_rects` enforced `w_px.max(24.0)` with a `0.66 * font_size` character width estimate, and `overlays` elided to `r.width() - dx + 8.0` and painted with the full canvas clip rect. All 6 new `#1197` tests failed on `9680eb2e` (`plans/reports/agy-1197-frame-labels-evidence/red.log`).
- **`FACT`**: With the fix applied, all 56 `label` tests in `designos-app` pass (`plans/reports/agy-1197-frame-labels-evidence/green.log`), including `large_page_label_rects_hit_marquee_and_snap_under_5ms` with cover clipping included in `label_rects`.
- **`FACT`**: On `crates/designos-app/tests/fixtures/frame-labels-1197.designos` at zoom `0.68`, `"A frame with a very long name that overflows"` (`200px` wide, `136px` on screen) elides to `"A frame with a very lon…"` (`129.95px <= 136.0px`), `"Tiny"` (`15px` wide, `10.2px` on screen) paints `"…"` alone, `"Right overlapping"` inside `"Left frame"` has `0` visible pieces, and `"Half covered label"` is clipped cleanly at `"Blocker"`'s left edge (`x = 1190` in world space) with no `"…"`.
- **`INFERENCE`**: Keying `LABEL_RECTS_CACHE` identically to `9680eb2e` and pre-filtering covering rects against `labels_bounds` preserves `O(V)` steady-state frame cost on large pages because off-screen covers outside `labels_bounds` are skipped before per-label rect subtraction.
- **`ASSUMPTION`**: For components, component sets, and instances (`COMP_LABEL_GLYPH_W = 18.0`), the diamond glyph is drawn first when `18.0 + ellipsis_w <= w_px` and the text is elided to `w_px - 18.0`; when `18.0 + ellipsis_w > w_px`, the glyph is omitted and `"…"` alone is painted at `r.left()`.
- **`UNKNOWN`**: Figma's exact glyph-drop threshold on `< 24px` on-screen components/instances, hover behaviour on elided labels, and section cover interactions.

- **Weakest link in one line:** Thread-local font measurement in `widgets::elide_measured` relies on `theme::install_fonts` matching the active app context's default `FontFamily::Proportional` (`Inter` regular), which holds across all Studio canvases and tests.
- **Unresolved questions:** None.

---

## 3. Acceptance Table

| # | Gate / Requirement | Verdict | Evidence |
|---|---|---|---|
| 1 | `red.log` (6 new tests fail on `9680eb2e`) & `green.log` (all 56 `label` tests pass) | **PASS** | `plans/reports/agy-1197-frame-labels-evidence/red.log`, `plans/reports/agy-1197-frame-labels-evidence/green.log` |
| 2 | List of existing tests whose assertion changed | **PASS** | **None** — zero existing test assertions were modified. |
| 3a | `cargo fmt --all -- --check` | **PASS** | Exits `0` (pasted below). |
| 3b | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** | Exits `0` (final lines pasted below). |
| 3c | `cargo test --workspace --locked --lib` | **PASS** | Exits `0` (`1,284` lib tests passed, `0` failed; pasted below). |
| 3d | `cargo test -p designos-app --bin designos --locked` | **PASS** | `3,325` passed; `#1211` (`agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one`) passed `3/3` isolated reruns (pasted below). |
| 3e | `cargo test -p designos-gpu` | **PASS** | Exits `0` (`60` GPU tests passed, `0` failed; pasted below). |
| 3f | `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py` | **PASS** | Both exit `0` (`17/17` invariants satisfied; no spacing count above baseline). |
| 4 | `1440x900` capture at zoom `0.68` + match/differ list vs `figma-fit-68pct.png` | **PASS** | `plans/reports/agy-1197-frame-labels-evidence/studio-fit-68pct.png` (+ `studio-top-left-68pct.png`, `studio-grid-card-68pct.png`) & `figma-fit-68pct.png`. |
| 5 | Commits `test(app): …` then `fix(app): …`, push branch, delete `out/cargo` | **PASS** | Branch `jangtrinh/agy-1197-frame-labels` pushed to `origin`; `out/cargo` deleted. |

---

## 4. Changed Existing Tests

**None.** No existing test or assertion in the repository was changed. Six new tests were added to [`crates/designos-app/src/shell/tests/canvas_label_rename.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/shell/tests/canvas_label_rename.rs#L683-L865):
- `frame_label_f1_long_name_elides_at_zoom_068_and_shows_full_at_zoom_2`
- `frame_label_f2_narrow_40_30_15_frames_and_component_at_zoom_068`
- `frame_label_f3_legacy_0_4_0_rich_fig_keeps_both_grid_and_card_labels`
- `frame_label_f4_blocker_clips_label_and_fully_inside_label_has_no_visible_piece`
- `frame_label_f5_nofill_frame_and_white_child_rect_inside_nofill_frame_cover_nothing`
- `frame_label_hit_testing_covered_part_ignores_label_and_visible_part_selects_frame`

---

## 5. Assumptions

1. **Component / ComponentSet / Instance narrow glyph fallback (`ASSUMPTION`, required by spec item 1):** For component-like nodes, the `18px` glyph slot (`COMP_LABEL_GLYPH_W = 18.0`) comes first when `18.0 + ellipsis_w <= w_px`, and the label text is elided to `w_px - 18.0`. When `w_px < 18.0 + ellipsis_w` (so the glyph plus `"…"` cannot fit inside the frame's on-screen width), the glyph is omitted and `"…"` alone is painted at `r.left()`.
2. **Top-level fill visibility (`INFERENCE` from F4/F5):** A top-level `Frame`, `Component`, `ComponentSet`, or `Instance` covers other labels if and only if `n.visible && n.opacity > 0.0` and `n.fills.iter().any(|f| f.visible && f.opacity > 0.0)`.
3. **Combined 13-node fixture viewport framing at `--zoom 68` (`FACT`):** Because combining `build-fixture.js` (`0..900 × 0..1250`) and `build-cover.js` (`Blocker` at `1190, 1200` and `Half covered label` at `1150, 1300`) into one page expands `page_bounds()` to `0..1450 × 0..1400` (`986 × 952 px` at 68% zoom inside an `880 × 820 px` canvas viewport), `studio-fit-68pct.png` (`--zoom 68 --select none`) centers on `(725, 700)` and shows the right/lower cases (`Second frame with a lo…`, `Thirty`, `Tiny`, `Left frame` covering `Right overlapping`, `Upper`/`Lower`, and `Blocker` clipping `Half covered label`), while companion captures `studio-top-left-68pct.png` and `studio-grid-card-68pct.png` at the same `--size 1440x900 --zoom 68` frame the top-left `(0, 0)` cases (`Grid` + `Card`, full `A frame with a very lon…`, and `Narrow frame name`).

---

## 6. Visual Capture & Match / Differ List (`1440x900`, Zoom `68%`)

- **Reference:** `plans/reports/agy-1197-frame-labels-evidence/figma-fit-68pct.png` (and `/Users/jangtrinh/Products/design-os-studio/plans/reports/1197-evidence/figma-cover.png`)
- **Studio Captures (`1440x900`, `--zoom 68`):**
  - `plans/reports/agy-1197-frame-labels-evidence/studio-fit-68pct.png` (`--size 1440x900 --zoom 68 --select none --no-toast`)
  - `plans/reports/agy-1197-frame-labels-evidence/studio-top-left-68pct.png` (`--size 1440x900 --zoom 68 --select "A frame with a very long name that overflows" --no-toast`)
  - `plans/reports/agy-1197-frame-labels-evidence/studio-grid-card-68pct.png` (`--size 1440x900 --zoom 68 --select "Grid" --no-toast`)

| Case | Fixture Nodes | Verdict | Observation (Figma 126.10.11 vs Studio) |
|---|---|---|---|
| **F3 — Same-origin frame + component collision** | `Grid` (`0,0,600,400`) + `Card` (`0,0,300,200`, component) | **Match** | Both labels are drawn at `(0,0)` with `Card` (component glyph + `"Card"`) painted on top of `"Grid"`, reading overlapping `GridCard` (`studio-grid-card-68pct.png`). |
| **F1 — Long frame name elision to frame on-screen width** | `A frame with a very long name that overflows` (`0,600,200,100`) & `Second frame with a long name too` (`210,600,200,100`) | **Match** | At 68% zoom (`136px` on-screen width), both elide with `"…"` (`"A frame with a very lon…"` and `"Second frame with a lo…"`) inside their 200px frame width without overflowing into the 10px gap between `x=200` and `x=210` (`studio-fit-68pct.png`, `studio-grid-card-68pct.png`). |
| **F2 — Narrow frames (`40px`, `30px`, `15px` at 68% zoom)** | `Narrow frame name` (`0,800,40,100`), `Thirty` (`100,800,30,100`), `Tiny` (`200,800,15,100`) | **Match** | Never hidden for narrowness; `40px` (`27.2px` on screen) shows `"Na…"`, `30px` (`20.4px` on screen) shows `"T…"`, and `15px` (`10.2px` on screen) shows `"…"` alone, each within its frame's on-screen width (`studio-top-left-68pct.png`, `studio-fit-68pct.png`). |
| **F4a — Label fully inside another filled top-level frame** | `Left frame` (`0,1000,300,200`) & `Right overlapping` (`150,1050,300,200`) | **Match** | `"Left frame"` is visible above its top-left corner; `"Right overlapping"`'s label origin `(150, 1050)` lies inside `"Left frame"`'s white fill (`0..300 × 1000..1200`) and is completely hidden (`studio-fit-68pct.png`, `studio-top-left-68pct.png`). |
| **F3 — Vertical overlap over empty canvas (`10px` apart)** | `Upper` (`600,1000,300,200`) & `Lower` (`600,1010,300,200`) | **Match** | Both `"Upper"` and `"Lower"` labels are painted over empty canvas above `y=1000`/`y=1010` and overlap vertically (`studio-fit-68pct.png`). |
| **F4b — Lower-in-stack filled frame partially covering a later frame's label** | `Blocker` (`1190,1200,200,150`, first in stack) & `Half covered label` (`1150,1300,300,100`, second in stack) | **Match** | `"Blocker"`'s white fill (`x=1190..1390, y=1200..1350`) cuts `"Half covered label"` sharply at `x=1190` (`"Half c"`) with no `"…"` added at the cut (`studio-fit-68pct.png`). |
| **F6 — Selected frame with long name** | `A frame with a very long name that overflows` selected | **Match** | Selection recolours `"A frame with a very lon…"` in selection blue and does not reveal the full name (`studio-top-left-68pct.png`). |

---

## 7. Skills Loaded & What Changed Because of Them

- **`/es:dev` -> `es:feature-dev`**: Followed the DEV phase directly from the accepted controller spec without stopping for brainstorm/plan approval, wrote red tests before production changes, and ran the verification gates.
- **`es:rust`**: Used a dedicated thread-local `egui::Context` in `widgets::elide_measured` instead of locking the active UI context inside nested closures (Law 4: no reentrant `egui::Context` lock deadlocks), and avoided extra allocations by reusing `LABEL_RECTS_CACHE`.
- **`es:rust-gpu-canvas`**: Kept `LABEL_RECTS_CACHE`'s viewport/rev key unchanged so viewport-only frames do not invalidate `TileView::rev`, pre-filtered covering top-level frames by `labels_bounds` before per-label clipping, and verified `large_page_label_rects_hit_marquee_and_snap_under_5ms` stays under 5ms.
- **`es-designer`**: Loaded `design/principles.md`, `design/soul.md`, and `craft-floor.md`; avoided introducing any new numeric offset/inset/radius/font literals in `crates/designos-app/src/canvas/draw.rs` so `scripts/check_spacing_relationships.py` and `scripts/zero_fake_ui_gate.py` pass with zero regressions, and inspected the `1440x900` screenshots against Figma 126.10.11.
- **`es:astra-work` & `es:fable-thinking`**: Maintained the epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION` / `UNKNOWN`), stated `PREDICT` before running red/green experiments and gates, and verified all claims against command logs and rendered screenshots.
- **`es:orca-native-orchestration`**: Sent periodic heartbeats with both `taskId` and `dispatchId`, checked coordinator messages at checkpoints, and reported completion once via `worker_done`.

---

## 8. Exact Commands & Outputs

### 8.1 `red.log` (`cargo test -p designos-app --bin designos --locked label` on `9680eb2e` with new tests)
```text
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked label
```
```text
running 56 tests
...
test shell::tests::canvas_label_rename::frame_label_f5_nofill_frame_and_white_child_rect_inside_nofill_frame_cover_nothing ... FAILED
test shell::tests::canvas_label_rename::frame_label_f4_blocker_clips_label_and_fully_inside_label_has_no_visible_piece ... FAILED
test shell::tests::canvas_label_rename::frame_label_f1_long_name_elides_at_zoom_068_and_shows_full_at_zoom_2 ... FAILED
test shell::tests::canvas_label_rename::frame_label_f3_legacy_0_4_0_rich_fig_keeps_both_grid_and_card_labels ... FAILED
test shell::tests::canvas_label_rename::frame_label_f2_narrow_40_30_15_frames_and_component_at_zoom_068 ... FAILED
test shell::tests::canvas_label_rename::frame_label_hit_testing_covered_part_ignores_label_and_visible_part_selects_frame ... FAILED
...
failures:

---- shell::tests::canvas_label_rename::frame_label_f5_nofill_frame_and_white_child_rect_inside_nofill_frame_cover_nothing stdout ----
thread 'shell::tests::canvas_label_rename::frame_label_f5_nofill_frame_and_white_child_rect_inside_nofill_frame_cover_nothing' (75604930) panicked at crates/designos-app/src/shell/tests/canvas_label_rename.rs:830:9:
Under nofill label clip [[298.0 0.0] - [1359.0 1088.0]] must equal full label_r [[332.0 118.0] - [478.7 132.0]]

---- shell::tests::canvas_label_rename::frame_label_f4_blocker_clips_label_and_fully_inside_label_has_no_visible_piece stdout ----
thread 'shell::tests::canvas_label_rename::frame_label_f4_blocker_clips_label_and_fully_inside_label_has_no_visible_piece' (75604909) panicked at crates/designos-app/src/shell/tests/canvas_label_rename.rs:788:9:
visible piece clip [[298.0 0.0] - [1359.0 1088.0]] must exclude Blocker's screen rect [[1141.2 170.0] - [1277.2 272.0]] (overlap [[1141.2 170.0] - [1277.2 272.0]])

---- shell::tests::canvas_label_rename::frame_label_f1_long_name_elides_at_zoom_068_and_shows_full_at_zoom_2 stdout ----
thread 'shell::tests::canvas_label_rename::frame_label_f1_long_name_elides_at_zoom_068_and_shows_full_at_zoom_2' (75604895) panicked at crates/designos-app/src/shell/tests/canvas_label_rename.rs:692:5:
painted label text width 140.1875 must not exceed 136px (no +8 overflow)

---- shell::tests::canvas_label_rename::frame_label_f3_legacy_0_4_0_rich_fig_keeps_both_grid_and_card_labels stdout ----
thread 'shell::tests::canvas_label_rename::frame_label_f3_legacy_0_4_0_rich_fig_keeps_both_grid_and_card_labels' (75604903) panicked at crates/designos-app/src/shell/tests/canvas_label_rename.rs:762:5:
Grid label_rect width 45.04001 must match painted width 22

---- shell::tests::canvas_label_rename::frame_label_f2_narrow_40_30_15_frames_and_component_at_zoom_068 stdout ----
thread 'shell::tests::canvas_label_rename::frame_label_f2_narrow_40_30_15_frames_and_component_at_zoom_068' (75604902) panicked at crates/designos-app/src/shell/tests/canvas_label_rename.rs:726:9:
30px frame at zoom 0.68 must have label_r.width() <= 20.4, got 24

---- shell::tests::canvas_label_rename::frame_label_hit_testing_covered_part_ignores_label_and_visible_part_selects_frame stdout ----
thread 'shell::tests::canvas_label_rename::frame_label_hit_testing_covered_part_ignores_label_and_visible_part_selects_frame' (75604933) panicked at crates/designos-app/src/shell/tests/canvas_label_rename.rs:857:5:
assertion `left == right` failed: clicking the covered part of a label must hit the covering frame under the pointer
  left: [NodeId(3)]
 right: [NodeId(2)]

test result: FAILED. 50 passed; 6 failed; 0 ignored; 0 measured; 3281 filtered out; finished in 6.55s
```

### 8.2 `green.log` (`cargo test -p designos-app --bin designos --locked label` on final commit)
```text
running 56 tests
test panels::layers::tests::row_text_split_keeps_a_readable_name_or_drops_the_label ... ok
test dev::annot::tests::callout_moves_left_without_room_and_avoids_other_labels ... ok
test history::tests::labels_for_the_panel ... ok
test panels::layers::tests::kind_label_covers_every_node_type ... ok
test agent_mention::tests::deleted_label_is_not_sent ... ok
test canvas::draw::tests::header_pills_keep_their_label_size_when_the_tracks_have_room ... ok
test check_designs::tests::safety::a_detached_frame_whose_label_was_edited_is_reported_without_a_reconnect ... ok
test check_designs::tests::instances::overridden_sibling_behind_label_in_opaque_instance_reports_contrast_on_instance_text ... ok
test panels::export::tests::format_labels_fit_their_dropdown ... ok
test proto::tests::gamepad_trigger_indexes_and_labels ... ok
test dev::annot::tests::labels_list_the_pinned_property_values ... ok
test check_designs::tests::fix_all::fix_all_on_a_detached_frame_keeps_the_contrast_fix_of_its_label ... ok
test check_designs::tests::covering_sibling::a_light_label_on_a_white_rectangle_is_flagged_with_a_fix ... ok
test canvas::draw::tests::large_page_label_rects_hit_marquee_and_snap_under_5ms ... ok
test panels::layers::tests::hidden_layers_grey_the_type_label_with_the_name ... ok
test panels::layers::tests::only_rows_inside_the_viewport_paint_a_type_label ... ok
test panels::right::tests::frame_preset_names_keep_a_grid_gap_to_their_size_label ... ok
test proto::tests::labels_name_destinations ... ok
test proto::tests::new_actions_have_labels_and_defaults ... ok
test panels::layers::tests::type_label_stays_on_wide_rows_and_gives_way_to_the_name_on_narrow_ones ... ok
test panels::layers::tests::type_label_never_overlaps_the_name_or_the_layer_icon_at_any_depth ... ok
test panels::layers::tests::type_label_is_clipped_to_the_text_column ... ok
test shell::tests::board::facilitator_timer_label_uses_tabular_digits ... ok
test agent_open_flow::tests::sessions_dropdown_lists_cli_label_time_status ... ok
test shell::tests::canvas_label_rename::frame_label_f3_legacy_0_4_0_rich_fig_keeps_both_grid_and_card_labels ... ok
test shell::tests::canvas_label_rename::frame_label_f4_blocker_clips_label_and_fully_inside_label_has_no_visible_piece ... ok
test shell::tests::canvas_label_rename::frame_label_f2_narrow_40_30_15_frames_and_component_at_zoom_068 ... ok
test shell::tests::home::account_and_plan_labels_are_gone_from_the_ui_sources ... ok
test shell::tests::canvas_label_rename::frame_label_f1_long_name_elides_at_zoom_068_and_shows_full_at_zoom_2 ... ok
test shell::tests::lint::a_label_poking_into_the_row_above_is_flagged_when_its_clip_cuts_at_105 ... ok
test shell::tests::lint::a_label_poking_into_the_row_above_is_flagged_when_its_clip_cuts_at_106 ... ok
test shell::tests::lint::a_label_poking_into_the_row_below_is_flagged_when_its_clip_starts_at_14 ... ok
test shell::tests::canvas_label_rename::frame_label_f5_nofill_frame_and_white_child_rect_inside_nofill_frame_cover_nothing ... ok
test skills::hook_delivery::tests::a_skill_over_one_chunk_is_split_at_line_breaks_into_labelled_parts_that_rebuild_it ... ok
test skills::tests::the_chip_shows_a_count_pill_beside_the_label_and_names_itself_for_screen_readers ... ok
test styles::instance_refit_tests::a_style_edit_on_an_instance_label_is_one_undo_step ... ok
test styles::instance_refit_tests::explicit_height_edit_on_height_mode_instance_label_survives_main_sync ... ok
test shell::tests::home::the_editor_header_shows_project_and_file_and_none_of_the_account_labels ... ok
test shell::tests::components::inspector_text_property_relabels_the_instance ... ok
test shell::tests::canvas_label_rename::frame_label_hit_testing_covered_part_ignores_label_and_visible_part_selects_frame ... ok
test terminal::gemini_narrow_tests::a_hook_wait_on_a_cut_label_stands_while_the_dialog_is_on_screen_and_ends_when_it_is_gone ... ok
test shell::tests::board::connector_labels_can_be_typed_by_double_clicking_the_line ... ok
test terminal::gemini_narrow_tests::a_label_cut_to_fit_is_still_the_dialog_down_to_the_minimum_panel_width ... ok
test shell::tests::history::labeled_live_patch_shows_step_label_in_history_panel ... ok
test terminal::tests::qa_p0_relabelled_shell_tab_gets_zero_bytes ... ok
test shell::tests::scrubbing_x_label_changes_value_in_one_undo_step ... ok
test terminal::gemini_narrow_tests::a_tab_with_no_hooks_file_starts_its_wait_from_a_cut_label ... ok
test shell::tests::canvas_label_rename::click_selects_one_shape_component_and_double_click_enters_shape ... ok
test shell::tests::canvas_label_rename::repro_qa_1193_rectangle_ctrl_alt_k_agent_rename_updates_canvas_frame_label ... ok
test shell::tests::canvas_label_rename::off_screen_and_unloaded_page_renames_paint_updated_canvas_label_when_scrolled_or_switched_to ... ok
test shell::tests::canvas_label_rename::renaming_main_component_updates_instance_canvas_labels_and_preserves_overridden_instance_names ... ok
test shell::tests::canvas_label_rename::several_renames_in_one_transaction_show_only_the_final_name_on_canvas_label ... ok
test shell::tests::canvas_label_rename::undo_and_redo_after_component_rename_keep_canvas_label_layers_and_properties_in_sync ... ok
test shell::tests::canvas_label_rename::component_and_inner_shape_names_stay_independent ... ok
test shell::tests::canvas_label_rename::revert_on_agent_turn_card_restores_old_name_on_canvas_label_in_fast_and_fallback_paths ... ok
test shell::tests::canvas_label_rename::multi_child_and_container_child_components_still_select_through_and_marquee_is_unchanged ... ok

test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 3281 filtered out; finished in 14.23s
```

### 8.3 `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`
```text
$ cargo fmt --all -- --check
FMT_EXIT=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.2 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-mcp)
    Checking designos-app v0.6.2 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1197-frame-labels/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.47s
CLIPPY_EXIT=0
```

### 8.4 `cargo test --workspace --locked --lib`
```text
$ cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.38s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.57s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 58.49s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.64s
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.48s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 59.84s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
WORKSPACE_LIB_EXIT=0
```

### 8.5 `cargo test -p designos-app --bin designos --locked` (+ 3 isolated reruns of `#1211`)
```text
$ cargo test -p designos-app --bin designos --locked
test result: FAILED. 3325 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 496.25s
(1 failure under full concurrency: shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one, allow-listed as #1211)

=== RERUN 1 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3336 filtered out; finished in 0.59s

=== RERUN 2 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3336 filtered out; finished in 0.60s

=== RERUN 3 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3336 filtered out; finished in 0.63s
```

### 8.6 `cargo test -p designos-gpu`
```text
$ cargo test -p designos-gpu
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-34492fc035346394)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/compare.rs (out/cargo/debug/deps/compare-14912d60fd54ff2b)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.62s
     Running tests/large_file_perf.rs (out/cargo/debug/deps/large_file_perf-919c2cf06f457945)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.05s
     Running tests/mask_without_own_fill.rs (out/cargo/debug/deps/mask_without_own_fill-94c9f7da7ce2c0f2)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/program_cap.rs (out/cargo/debug/deps/program_cap-24b4cb601e5148b5)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.82s
     Running tests/refine_vram_budget.rs (out/cargo/debug/deps/refine_vram_budget-870ad0fee1e860f7)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.02s
     Running tests/resized_shapes.rs (out/cargo/debug/deps/resized_shapes-276444f4c9016dad)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/text_hug_box.rs (out/cargo/debug/deps/text_hug_box-c609eeb8ec21af2d)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
GPU_EXIT=0
```

### 8.7 `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`
```text
$ python3 scripts/zero_fake_ui_gate.py
============================================================================================================
 design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)
============================================================================================================
...
Z16   | PASS   | registered label helpers in designos-app/src | UX writing: action labels at registered button/menu helpers and label fns <=3 words, no count in them (ratchet vs ux_writing_baseline.json) — 18 labelled violation(s) all in scripts/ux_writing_baseline.json; none new (design/ux-writing.md rules 1 and 3)
Z17   | PASS   | terminal.rs + mcp/server.rs      | No global /tmp/designos-live-* or designos-live-selection.spec.json fallback in shipped code — no shipped (non-test) Rust source under designos-app or designos-mcp contains a global `/tmp/designos-live-` path or `designos-live-selection.spec.json`
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2
note: 14 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

Status: PASS — frame name labels match Figma width elision, narrow-frame '…' fallback, top-level filled-frame cover clipping, and visible-piece hit testing (#1197).
