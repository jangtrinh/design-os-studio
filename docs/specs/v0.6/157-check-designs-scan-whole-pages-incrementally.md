# #157 Check designs: scan whole pages incrementally off the UI thread (E3-12)

| Field | Value |
|---|---|
| Issue | #157 (task E3-12; parent #16; label in-review at spec time) |
| Size | M |
| Depends on | #35 (the Check designs panel; its code is on `main`) |
| Unblocks | #158 (rule parity runs on the same scan job) |
| Touches | new `crates/designos-app/src/check_designs/job.rs` (resumable scan), `crates/designos-app/src/check_designs/mod.rs` (`MAX_SCANNED_LAYERS` `:31`, `audit_with_libraries` `:139-171`, `Report` `:106-118`), `crates/designos-app/src/check_designs/state.rs` (`refresh_check_designs` `:61-79`), `crates/designos-app/src/check_designs/detached.rs` (`Components::new` `:30`), `crates/designos-app/src/panels/check_designs.rs` (progress row, `truncation_note` `:262-270` removed), tests |

Code baseline: `origin/main@9f3f7a5`. Source note named by the issue (`plans/261017-epics-full-scope/full-scope-audit-e1-e2-e3-e5.md` line 183) describes the task as "incremental scan off the UI thread, no cut-off".

## 1. Summary and user story

As a designer with a huge page, I want Check designs to look at every layer, to keep the app smooth while it works, and to show me how far it has got. Today it stops silently at 20,000 layers.

## 2. Current state (FACT)

- `MAX_SCANNED_LAYERS = 20_000` (`check_designs/mod.rs:31`). The scan is an iterative pre-order loop over a stack and stops at the cap with `report.truncated = true` (`:155-159`). The panel then shows "Stopped after 20000 layers. Select fewer layers to check the rest." (`panels/check_designs.rs:262-270`). A test asserts the cap (`check_designs/tests.rs:466-472`).
- The audit runs **synchronously on the UI thread** inside `refresh_check_designs` whenever `(canvas.tiles.rev(), scope, enabled libraries)` changed; while the user is mid-gesture an existing report is kept (`check_designs/state.rs:61-79`).
- Before the loop, the audit builds `hardcoded::Index::with_libraries(doc, libs)` and `detached::Components::new(doc)` (`mod.rs:140-143`). `Components::new` walks **every node of the document** (`detached.rs:32`), so it is O(N) before the first layer is checked. UNKNOWN: its cost, and the cost of `Index::with_libraries`, on a 700,000-node document. Check: time both with `test_timing::fastest_within`.
- The document is owned by `App`, not shared. `Document.nodes` is a `BTreeMap<NodeId, Node>` (`designos-doc/src/lib.rs:147`), so a snapshot for a worker thread is a full copy. UNKNOWN: its time and memory at 700,000 nodes. Check: measure `doc.clone()` on the synthetic fixture below.
- The findings list is painted inside one `egui::ScrollArea::vertical().show(...)` (`panels/check_designs.rs:130`). UNKNOWN whether it paints only the visible rows. Check: read the closure and count painter calls for 50,000 findings.
- No large synthetic document exists on `main`: the existing large fixtures are the >25,000-node case in `live_sync_tests.rs:25` and the label test in `canvas/draw.rs:1223`. The issue's "723k-node fixture" has no generator in the repository (part 2's deterministic generator, E15-24, is not on `main`).
- The frame budget the issue quotes is 8 ms (CLAUDE.md section 4, item 3: `budget = Some(8ms)` for interactive rendering).
- Design laws that bind (CLAUDE.md section 4, item 1; `design/principles.md` P8): no work that stalls the UI thread; idle CPU 0.00%.

## 3. Behaviour rules

1. **No cut-off.** `MAX_SCANNED_LAYERS`, `Report.truncated`, `truncation_note` and the cap test are removed. A page of any size is scanned completely.
2. **Resumable job.** A new `AuditJob` holds everything the loop holds today (the stack, `visited`, the report, the indexes) and has `step(&mut self, doc: &Document, budget: Duration) -> JobState { Running { scanned }, Done }`. The loop body is the existing one, unchanged, so one-shot and sliced results are equal. The clock is read every 64 layers.
3. **Time slices on the frame loop.** While a job runs, `refresh_check_designs` calls `step` once per frame with a budget of 4 ms (half the 8 ms frame budget; initial value, measured) and requests the next frame with `request_repaint()`. The job pauses while the user is mid-gesture (`interacting`) and resumes after. Nothing is copied: the job reads `&App.doc`, so no snapshot and no UI-thread `to_json` are needed.
4. **Why not a worker thread.** The issue title says "off the UI thread". A worker needs a copy of the page's nodes (a `BTreeMap` clone), which costs time and memory of the same order as the scan on a page this size. The time-sliced job meets the three acceptance criteria without the copy. If the measurements of section 2 show the clone is cheap, the job's `step` runs unchanged on a worker over the snapshot; the choice is open question 1.
5. **Setup is incremental too.** If `Components::new` or `Index::with_libraries` exceeds the slice budget, build them inside the job as their own steps (a `Setup` phase that walks nodes in chunks), or cache `Components` by `Document::mutation_epoch()` so a rescan after an unrelated edit does not rebuild it.
6. **Restart.** A change of document revision, scope or enabled libraries drops the running job and starts a new one after the user has been idle for 250 ms. The previous complete report stays on screen, with the progress row, until the new one finishes. With no previous report the partial findings found so far are listed as they arrive, in document order.
7. **Progress.** While a job runs, the panel shows the number of layers checked so far. No percentage is shown unless the total is known (a count of the scope's nodes is not taken: that is another O(N) walk).
8. **Idle.** When the job is `Done`, no repaint is requested. A closed panel runs no job. A cancelled job (panel closed, scope changed) frees its stack at once.
9. **Equivalence.** For a scope under 20,000 layers the sliced result equals the one-shot `audit_with_libraries` result, finding for finding, in the same order.
10. The findings list paints only visible rows, whatever the count (use `show_rows` or the existing virtual list helper if the check in section 2 shows the current code paints all rows).

## 4. Data model / schema changes

None in the document. In memory: `AuditJob`; `CheckDesignsState.job: Option<AuditJob>`; `Report.truncated` removed, `Report.scanned` kept.

## 5. Contracts

- `audit_with_libraries(doc, scope, libs) -> Report` stays (it runs a job to completion; tests and any caller that wants a result at once use it).
- `Report` loses the `truncated` field; the one reader (the panel) is changed in this task.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` P1 to P10 and `design/soul.md`.

- A progress row under the scope label, `CONTROL_DENSE` (24 px) high: a 2 px rule across the width in the `INFO` colour that advances in steps as the count rises (it is a running state), and the text "Checking… 120,000 layers" in caption type, `TEXT_SECONDARY`, with the number in `font_mono` in its own place after the word. It disappears when the job is done.
- The yellow "Stopped after 20000 layers" note is removed; the panel's literal status colours for that note go with it.
- Rows keep their present look; no new colour literal is added (the panel's `STATUS_*` imports stay only where already used).
- No cancel button; closing the panel cancels.
- Desktop capture at 1440: the progress row mid-scan on the synthetic page with partial findings, the finished list with the count of layers checked. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. The user edits during a scan: the job pauses during the gesture, restarts 250 ms after the last revision change; the old report stays.
2. The scope's page is deleted mid-scan: the existing "follow the page on screen" branch runs and the job restarts.
3. A page with 723,000 layers and zero findings: the job ends with "checked on 723,000 layers" and the existing "No issues found" state.
4. A page with 100,000 findings: the list is virtualized (rule 10); the count pill is a number, not a label string.
5. Selection scope (a few layers): finishes inside one frame, as today.
6. Hidden layers and layers inside instances: skipped exactly as before.
7. The clock stalls (a system sleep): the slice measures wall time; the next frame continues.
8. Two documents (tabs): each tab's state owns its job; a background tab runs no job.

## 9. Non-goals

- New rules or thresholds (#158). Caching reports across sessions. A cancel button. Auto-fix over huge reports (the existing Fix all stays one undo step).

## 10. Test plan (red first)

Seams: a deterministic synthetic page generator in `check_designs/tests/` (frames with text, fills, a few variables; sizes 50,000 non-ignored and 723,000 ignored); a manual clock for the job; `test_timing::fastest_within` for per-step cost.

| Case | Expected | Fails before |
|---|---|---|
| `no_layer_cutoff` | a 50,000-layer page: `report.scanned` covers all layers; no `truncated` | yes (cap 20,000) |
| `sliced_result_equals_one_shot` | on a 15,000-layer page the findings are identical and ordered | guard |
| `each_step_stays_within_the_budget` | on the 723,000-layer page (ignored) every `step(4 ms)` call returns within 8 ms including the clock check | yes (a single call takes the whole scan) |
| `setup_phase_respects_the_slice_budget` | `Components::new` work is chunked or cached; no step over 8 ms | yes |
| `progress_counts_up_and_ends` | counter strictly rises; state `Done` | yes |
| `edit_during_scan_restarts_after_250_ms_and_keeps_the_old_report` | old report visible until done | yes |
| `done_job_requests_no_repaint` | repaint requests drop to zero | yes |
| `closing_the_panel_cancels_the_job` | job dropped | yes |
| `findings_list_paints_only_visible_rows` | painter shape count bounded for 50,000 findings | yes if the check in section 2 finds all rows painted |
| `old_cap_test_is_replaced` | the test at `tests.rs:466-472` now asserts completion | yes |

Run: `cargo test -p designos-app check_designs`; the 723,000-layer test with `cargo test -p designos-app check_designs -- --ignored` in release mode. Evidence: red/green capture, the per-step timing output on the large page, the 1440 captures, and the idle measurement (20 s window, `ps -p <pid> -o time=` delta 0.00 s after the job is done).

## 11. Performance budgets

- Per frame while scanning: at most 4 ms of audit work plus the panel's own paint (under 2 ms), so no frame passes 8 ms because of Check designs. Measured per step, not averaged.
- Total time for 723,000 layers: UNKNOWN; recorded in the PR (it is bounded by 4 ms of every frame, so it can take many seconds; the issue sets no limit).
- Memory: the report only; no document copy.
- Idle: 0.00% CPU after `Done`.

## 12. Dependencies and build order

Needs the panel of #35 (on `main`). Part 3's FU-663 edits the same module (`mod.rs`, `contrast.rs`, `tests/`) and part 1's E15-9 and E15-11 turn it into a consumer of the unified linter: land FU-663 first or rebase (README-part3). #158 follows #157 and builds on `AuditJob`. Order in README-part4: FU-663, then #157, then #158.

## 13. Owner rulings

- The full page completes with no frame over the 8 ms budget, progress is shown, and the layer cut-off is removed; perf test on the 723k-node fixture with budgets unchanged. (Issue acceptance criteria.)
- No work that stalls the UI thread; idle CPU 0.00% (CLAUDE.md section 4).

## Open questions for the owner (recommended first)

1. "Off the UI thread" versus time slices. Recommended: time slices on the frame loop (rule 4), because a snapshot of a 700,000-node document costs as much as the scan; revisit if the clone measurement proves cheap.
2. The 723k fixture. Recommended: a deterministic synthetic generator in the test module now; swap for part 2's generator (E15-24) when it lands.
