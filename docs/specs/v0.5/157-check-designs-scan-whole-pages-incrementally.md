# #157 Check designs: scan whole pages incrementally off the UI thread

- Issue: https://github.com/jangtrinh/design-os-studio/issues/157 (task E3-12, epic #16, M / P1)
- Spec base: `main` at `767acf9`.
- Open PR: none.
- Files: `crates/designos-app/src/check_designs/mod.rs`, `state.rs`, `detached.rs`, `panels/check_designs.rs`, and `tests/scale.rs` (new).

## 1. Summary and user story

As a designer with a very large page (the 112,474-scene-node VSF subset), I run Check designs on the whole page. I need:
- every layer scanned, with no 20,000-layer cut-off;
- a progress indicator while it runs;
- the canvas to stay at full frame rate throughout.

## 2. Current state

- FACT: `MAX_SCANNED_LAYERS = 20_000` (`check_designs/mod.rs:30`). `audit` stops there and sets `truncated` (`mod.rs:127-130`). The panel then shows `truncation_note` (`panels/check_designs.rs:239`).
- FACT: `audit` runs synchronously in `refresh_check_designs` (`state.rs:42-58`), called from the panel each frame (`panels/check_designs.rs:70`) and from `shell.rs:299`. It recomputes whenever `(tiles.rev(), scope)` changes.
- FACT: `detached::Components::new` iterates every node of the document (`detached.rs:28-35`). `hardcoded::Index::new` iterates every colour variable (`hardcoded.rs:39-66`).
- FACT: `App.doc` is an owned `Document` (`app.rs:131`), not shared. Sending it to another thread would need a full clone.
- FACT, from `CLAUDE.md`: the VSF benchmark is 112,474 scene nodes from 723,136 kiwi records on the 7-page subset. "723k" in the issue counts kiwi records, not scene nodes.

## 3. Behaviour rules

1. **Execution model: time-sliced, resumable, on the UI thread.**
   - A background thread is rejected. It would need `app.doc.clone()` on every revision, gigabytes on VSF-scale files, which breaks the spirit of `CLAUDE.md` §4.1 and the PR #501 review rule of no whole-document clones.
   - The scan keeps an explicit cursor: the DFS stack, the visited count, and partial findings. Each frame it advances until a **2.0 ms slice budget** is spent, measured with `std::time::Instant`, then yields and calls `ctx.request_repaint()`.
   - The UI-thread cost of the scan stays far below the 8 ms frame build budget.
   - Recorded as a technical decision: the issue title says "off the UI thread", while the acceptance criteria require "no frame over budget". The slices meet the criteria.
2. **No cut-off.** Delete `MAX_SCANNED_LAYERS`, `Report::truncated` and `truncation_note`. Every layer in scope is scanned.
3. **Index building is sliced too.**
   - `Components::new`'s whole-document walk becomes a phase-0 slice that iterates `doc.nodes` with a resumable key cursor (`BTreeMap::range(next..)`).
   - The variable `Index` (O(variables × modes)) stays one step. It is small.
   - Phase order: build indexes, then walk the scope.
4. **Revision changes.** If `(tiles.rev(), scope)` changes mid-scan, discard the cursor and restart. The previous **complete** report stays visible, marked stale (the progress row reads `Updating…`), until the new scan completes. A pointer-down gesture (`interacting`) pauses slicing, as the current code pauses re-audits.
5. **Progress.** While scanning, the panel shows a progress row: a determinate bar, with the text `Checking… {scanned} of {total} layers`. `total` is the scope's node count, computed in phase 0 with the same slicing. During phase 0 the text is `Preparing…`. `Fix all` is disabled until the scan completes, with the tooltip `Wait for the check to finish`.
6. **Determinism.** The final report equals what one synchronous `audit` of the same document and scope produces (same findings, same order). Tests assert this.
7. **Fixes during a scan.** `Fix` on a listed row (from the last complete report) stays allowed. Fixes re-validate at apply time, as today (PR #45 L2). Applying a fix bumps `rev`, so a restart follows (rule 4).

## 4. Data model / schema changes

None in the document. In memory:

```rust
pub struct ScanCursor { phase: Phase, stack: Vec<NodeId>, node_iter_from: Option<NodeId>, total: usize, scanned: usize, partial: Vec<Finding>, key: (u64, Scope) }
enum Phase { Prepare, Walk, Done }
```

It lives in `CheckDesignsState`. `Report` loses `truncated` and gains `complete: bool`.

## 5. Contracts

```rust
pub fn audit(doc: &Document, scope: &Scope) -> Report;                     // unchanged signature; runs to completion (tests, headless)
pub fn audit_step(doc: &Document, cur: &mut ScanCursor, budget: Duration) -> bool; // true when Done
impl App { pub fn refresh_check_designs(&mut self, interacting: bool, ctx: &egui::Context); }
```

`refresh_check_designs` gains `ctx` so it can request a repaint. Update both call sites (`shell.rs:299`, `panels/check_designs.rs:70`).

## 6. File-format mapping

None. RESOLVED.

## 7. UI

- **Progress row.** Shown under the scope label: a 4 px bar in brand orange `#E36323` on the card surface, and caption text 10 px (P5). `scanned` and `total` use `font_mono`.
- **Empty state.** After a complete scan with no findings, the existing `empty_state`. The truncation note is removed.
- **Zero fake UI.** `Fix all` is honestly disabled while scanning.
- **Evidence.** A 1440 capture of the progress row mid-scan on the large synthetic page, plus the complete state.

## 8. Edge cases

1. A scope of 112,474 nodes: completes, no layer is skipped, and the final count equals the node count of the scope (minus hidden subtrees, which are skipped by rule as today).
2. An edit mid-scan: restart, and the previous report stays visible as stale.
3. Scope page deleted mid-scan: the scope re-targets (existing behaviour, `state.rs:44-49`) and the scan restarts.
4. Panel closed mid-scan: the cursor is dropped, and nothing keeps running.
5. A tiny scope (5 nodes): completes within the first slice. No progress row flashes, because the row is shown only if the scan lasts more than 1 frame.
6. A pointer held down during a drag: slicing pauses. No `request_repaint` loop runs while idle, which keeps the 0% idle-CPU rule (§4.4).

## 9. Non-goals

- Multi-threaded rule evaluation.
- Changing the rules themselves.
- Background scanning while the panel is closed.

## 10. Test plan

Seam: `crates/designos-app/src/check_designs/tests/scale.rs`.

- `sliced_audit_equals_synchronous_audit`: on the PR #45 fixture (`FIXTURE`) plus a generated page of 5,000 nodes, run `audit_step` with a 0.05 ms budget until done. The report equals `audit`.
  - Fails before: yes (no `audit_step`).
- `no_layer_cut_off_on_large_page`: generate a page with 30,000 visible nodes. Assert `scanned == 30_000` and no `truncated` field.
  - Fails before: yes (stops at 20,000).
- `each_slice_respects_budget_on_112k_page` (release only; `#[ignore = "release perf"]` in debug): generate 112,474 nodes (frames, rects, text and instances in roughly kit proportions). Run slices with a 2 ms budget. Assert every slice is ≤ 4 ms wall time, using `test_timing::fastest_within` (`test_timing.rs:12`) for the overall run, and that the scan completes.
  - Fails before: yes (one 20k-capped synchronous pass far over 8 ms).
- `fix_all_disabled_while_scanning` (shell test).
- Real-file run (manual, owner machine): the VSF subset page completes. Record its duration in the PR. The owner's file stays local.

## 11. Performance budgets

- `CLAUDE.md` §4.1: no `to_json` or `doc_hash`; the scan reads the document in place.
- §4.4 idle: `request_repaint` only while a scan is unfinished. Zero repaint requests once done or paused.
- P8 / 8 ms frame budget: a slice is ≤ 2 ms. No GPU budgets change.
- Budgets in other tests must not be loosened.

## 12. Dependencies and build order

- Depends on #35 (merged).
- Do it after #52, #65, #154 and #155, so every rule runs inside the sliced walk. Otherwise each later rule must be re-sliced.

## 13. Owner rulings that apply

- Brainstorm no-gos: no loosening of performance budgets.
- `CLAUDE.md` §4 invariants (owner rulings of 2026-10-16 and 2026-10-17).
- The execution-model choice (time-sliced instead of a thread) is a technical decision, recorded here and in the README.
- Owner default: 1440 capture only.
