# #5 Import: global clone budget and remaining review backlog

| Field | Value |
|---|---|
| Issue | #5 (follow-up from PR #1; milestone v0.6) |
| Size | L (five items; items 1 and 5 carry the weight) |
| Depends on | none |
| Unblocks | none |
| Touches | `crates/designos-fig/src/rest_import.rs` (hot file: `expand_instance` `:2007-2056`, `fresh_id` `:1703`, clone ids `:1393`), `crates/designos-fig/src/kiwi.rs` (`MAX_ZERO_WIRE_INSTANCES` `:35`, `charge_zero_wire` `:216-219`), `crates/designos-app/src/proto/smart.rs` (`:14-31`), `crates/designos-app/src/io.rs` (`:154`, the import call and its toast), `crates/designos-library/src/store.rs` (`:147`, the second call), tests in `designos-fig/tests/` and `kiwi.rs` |

Code baseline: `origin/main@9f3f7a5`. The five items are separate subsections; each can be one commit.

## 1. Summary and user story

As a designer opening a large or odd Figma file, I want the import to finish in bounded time, to tell me when it left something unexpanded, and never to crash or collide ids at the edge of the number range. As the maintainer, I want the remaining review findings turned into verified decisions.

## 2. Current state (FACT)

- Item 1. `expand_instance` copies a component's children with a per-instance budget of 50,000 nodes (`rest_import.rs:2041-2049`); nothing bounds the total over the whole file. Clone ids come from one counter `next_clone` started above the highest imported id (`:1393`, `fresh_id` `:1703-1707`). `import_rest_json` returns `Result<Document, FigError>`, with no channel for a warning (`:505`). The issue's measurement (5,000 chained links give 25,000,000 nodes in 24 s) was not reproduced here. UNKNOWN: the fixture. Check: build a chain where component `i` holds an instance of component `i+1`, and count `next_clone` growth.
- Item 2. The zero-size struct count is capped at `MAX_ZERO_WIRE_INSTANCES = 1 << 16` (`kiwi.rs:35`); one more fails with "zero-size struct count exceeds budget" (`:218`). A struct of zero wire bytes costs no input, so the cap exists to stop a tiny file from asking for unbounded work. The existing test builds 20 messages of 50,000 empty structs (`kiwi.rs:1316`). UNKNOWN: the largest count in a real file. Check: log the maximum while decoding every available kit.
- Item 3. `self.doc.nodes.keys().next_back().map(|k| k.0).unwrap_or(0) + 1` (`rest_import.rs:1393`) and `NodeId(max + 1)` plus `out.reserve_ids(NodeId(max + 1))` (`proto/smart.rs:14-15`, `:31`) overflow `u64` when the highest id is `u64::MAX`. `fresh_id` also does `self.next_clone += 1` unchecked. `Document` has an exhausted-counter fallback in its own allocator (`designos-doc/src/lib.rs`, the `gap_cursor` field); UNKNOWN whether `alloc_id` is overflow-safe at the top of the range. Check: a test with `next_id` at `u64::MAX`.
- Item 4. A NaN once serialized as `null` and failed to reload with "invalid type: null, expected f32". This is already fixed on `main`: `designos-doc/src/finite_json/` writes finite numbers and repairs old files (`finite_json/mod.rs:1-12`), with `fig_export_non_finite.rs` and `fig_non_finite_derived.rs` among the tests; the change shipped in 0.4.1. The issue item is stale. UNKNOWN: whether a real `.fig` import, save and reopen was re-run after that fix. Check: run that sequence on a kit and on the NaN-poisoned synthetic file.
- Item 5. The review is the 2026-10-16 full-repository code review that PR #1 came from. It is not in the repo, so its MEDIUM and LOW findings are copied into the appendix "Review backlog" at the end of this spec, as written on 2026-10-16 (baseline before 0.4.0). Several have probably been fixed since; rule 13's table gives each one a verdict on the current `main`. The unreviewed app-root slice (specs, ops, cmdk, shell, video, present, dev) is still unreviewed.
- The sibling importer in `designos-fig` already has a document-wide clone budget that leaves further instances unexpanded and returns a warning string; it is the pattern to follow (module not named here on purpose).

## 3. Behaviour rules

### Item 1: a document-wide clone budget

1. The importer counts every cloned node across the whole import. At `clone_budget_total` it stops expanding instances. The remaining instances stay as instances with their component link intact; the document is valid and opens.
2. Order is deterministic (the existing instance order), so the same input always leaves the same instances unexpanded and always yields the same ids.
3. The per-instance 50,000 budget stays.
4. Value: UNKNOWN until measured (section 2, item 1 check). Rule for choosing it: at least 2,000,000 and at least four times the largest clone count observed on the available kits; recorded as a named constant with the measurement in a comment.
5. A new `import_rest_json_with_report(json) -> Result<(Document, ImportReport), FigError>` returns `ImportReport { instances_left_unexpanded: usize }`; `import_rest_json` stays as a wrapper. The `.fig` entry point gets the same report. The app shows one toast when the count is above zero: "N instances were left unexpanded to keep the import fast."
6. A cycle or a chain never panics and never exceeds the budget by more than one instance's 50,000.

### Item 2: the zero-size struct cap

7. Replace the fixed cap by one derived from the input: `max(65_536, 16 * input_len)` bytes-based, bounded above by `1 << 24` (initial values; the measurement of item 2 decides). A file at or under the old cap decodes exactly as before. A tiny hostile file still stops at the old cap.
8. Raising a safety cap is the owner's call (open question 1).

### Item 3: id arithmetic

9. All three sites use `checked_add(1)`. In the importer an overflow ends the import with a typed `FigError` ("node ids exhausted"); in Smart Animate it returns the dissolve fallback the module already documents. `fresh_id` is checked the same way.

### Item 4: confirm and close

10. Add no code. Re-run the real-file sequence of section 2 and the NaN-poisoned synthetic file, attach the output to the issue, and close the item.

### Item 5: the review backlog

11. Obtain the report. For each finding give one verdict with evidence: fixed on current `main` (commit), rejected (reason: verified decision or non-issue), or filed as its own issue (number). A finding that is small, in a file this task already edits, and has a failing test may be fixed inline.
12. Review the app-root slice (`specs.rs`, `ops.rs`, `cmdk.rs`, `shell.rs`, `video.rs`, `present.rs`, `dev/`) by an independent reviewer, not by the author of any fix in this task, using the same severity scale. Its findings go through rule 11.
13. The deliverable of item 5 is a table in the PR description (finding, verdict, evidence). No finding is left without a verdict.

## 4. Data model / schema changes

None in the document. `ImportReport` is an in-memory return value.

## 5. Contracts

- `designos-fig` public API gains `import_rest_json_with_report` and the `.fig` equivalent; existing functions keep their signatures.
- `kiwi` decode limits stay an internal constant plus the derivation of rule 7.

## 6. File-format mapping

None. Unexpanded instances export as instances; the component link survives a `.fig` round trip as it does for any instance.

## 7. UI

One toast line (rule 5), no new component. Evidence is a terminal capture: the chain fixture import before (time, node count) and after (time, node count, the report), and the toast in a 1440 screenshot of the app after opening it. Responsive: SKIPPED (owner default). The toast text only; es-designer is loaded if the toast layout changes.

## 8. Edge cases

1. A chain whose first links are legitimate and large: the early instances expand, the later ones stay; the toast says how many.
2. The budget is reached exactly at the end of an instance: that instance completes; the next does not start.
3. A component cycle: the existing `done` and `expanded_comps` guards keep it finite; the budget is a second guard.
4. A document with the highest id `u64::MAX - 1`: the first clone id works, the second reports exhaustion.
5. An input exactly at the old zero-size cap: decodes as before.
6. A file with more zero-size structs than the derived cap: fails with the same message as today.
7. Item 5: a finding that no longer reproduces: "rejected: not reproducible on `9f3f7a5`" with the command run.
8. A finding whose fix would change a public contract: filed, not fixed inline.

## 9. Non-goals

- Making expansion itself faster (a smarter copy); raising the per-instance 50,000 budget; lazy instance expansion; re-reviewing the slices the report already covers.

## 10. Test plan (red first)

Seams: `designos-fig/tests/` synthetic REST fixtures; `kiwi.rs` unit tests; a `smart.rs` unit test.

| Case | Expected | Fails before |
|---|---|---|
| `chained_links_stop_at_the_global_clone_budget` | 5,000-link chain: cloned nodes at most budget plus 50,000; finishes within the recorded time; report counts the left-over instances | yes (no budget) |
| `a_budgeted_import_is_deterministic` | two imports give identical documents and ids | yes |
| `an_unexpanded_instance_survives_a_fig_round_trip` | instance kind and component link kept | yes |
| `per_instance_budget_still_applies` | one huge component stops at 50,000 | guard |
| `zero_size_cap_follows_the_input_length_and_keeps_the_hostile_bound` | big honest input decodes; tiny input asking for 1 << 24 fails | yes |
| `old_cap_inputs_decode_unchanged` | byte-identical result | guard |
| `max_id_overflow_is_an_error_in_import` | typed error, no panic | yes |
| `max_id_overflow_in_smart_animate_uses_the_dissolve_fallback` | no panic | yes |
| `fresh_id_is_checked` | exhaustion reports | yes |
| `nan_round_trip_import_save_reopen` | reopens; values finite (confirms item 4) | guard (expected green) |
| item 5 table | every finding has a verdict and evidence | n/a (a process check in review) |

Run: `cargo test -p designos-fig`, `cargo test -p designos-app proto`, `cargo test -p designos-doc finite_json`. Kit measurements (items 1 and 2) use the ignored real-file tests with `DESIGNOS_REFERENCE_DIR`.

## 11. Performance budgets

- Item 1: the 5,000-link chain imports in under 2 s on the M4 Max reference machine (initial target; measured and recorded; it was 24 s). Memory stays within the budgeted node count.
- Counting costs one integer increment per clone; no change for files under the budget (kit import wall time within noise, before and after).
- No UI-thread work is added; the import already runs off the frame loop through the app's existing path (UNKNOWN whether `io.rs:154` runs on the UI thread; check by reading its caller).

## 12. Dependencies and build order

No task dependency. `rest_import.rs` is a hot file with a one-at-a-time rule: #5 queues with #691 behind FU-669 and FU-582 and ahead of part 1's E1-4 (README-part4). `kiwi.rs` is also a hot file (`docs/releasing.md:21` lists `fig.rs`, `kiwi.rs` and `rest_import.rs`), so item 2 queues behind any other `kiwi.rs` task; `smart.rs` is free. Order inside the task: items 3, 2, 1, then 4 (evidence only), with item 5 running in parallel because it edits no code until its verdicts land.

## 13. Owner rulings

- Hot-file rule (`docs/releasing.md:21`) for `rest_import.rs` and `kiwi.rs`. Full scope: nothing in the five items is dropped; item 4 is closed with evidence, not with code.
- No safety cap is raised without the owner's approval (open question 1).

## Open questions for the owner (recommended first)

1. Raising the zero-size struct cap (item 2). Recommended: approve the input-derived cap with the hard upper bound `1 << 24`, because the old cap rejects honest files and the new one still stops hostile ones.
2. Where the review report is. Recommended: the owner or coordinator supplies the path; without it item 5 cannot start.
3. The clone budget value (item 1). Recommended: set it from the kit measurement as in rule 4.

## Appendix: Review backlog (full-repository review, 2026-10-16)

Copied verbatim from the review so workers on any machine can read it. (V) marks a finding the reviewer re-read in source. Each item needs a verdict on the current `main` (rule 13).

### MEDIUM findings (~30, condensed)
- **Rendering:** the mesh arena clears and re-uploads every frame once over 384 MiB (V). GPU effect layers go to GB scale at high zoom. Background-blur tile invalidation leaves stale edges. More than 1024 gradients corrupt the ramp rows.
- **Library:** retention deletes named versions after an index rebuild. `project.json` uses a fixed `.tmp` name with only partial locking, so a project can silently disappear. `move_file` and `restore_file` have no rollback. `write_atomic` never fsyncs.
- **Document model:** a stale `next_id` overwrites existing nodes. `Batch` is not atomic, and undo drops the command on error. `grid_columns` from MCP/spec is unclamped.
- **MCP / export / fonts:** `save overwrite` truncates in place. Every session writes to fixed world-readable `/tmp/designos-live-*`. A canvas reload keeps the old undo history. Polygon `count` is unclamped. Font family is interpolated unescaped into SwiftUI/CSS/Tailwind (code injection). Font cache: a failed write discards the download, and ids are unsanitised. Font fetching is unbounded with no negative cache.
- **Windows packaging:** the installer takes over `.fig` without saving the old handler. The uninstaller deletes the key and runs a recursive force delete on `-InstallRoot` with errors silenced.
- **App UX:** Z+click zooms to an absolute 200%/50% (V). Multi-select X/Y stacks every node at the same point. Paint rows edit by index across every selected node. 5 Zoom-menu rows do nothing, and "Multiplayer cursors" is hard-coded as checked. The Share link has no handler. The Dev Mode token estimate is a constant. Editing the radius flattens per-corner values. Duplicating a guide is outside undo history. A panel edit during text editing corrupts the undo order. Replace in Find gets stuck when the replacement contains the query. Publishing a library is allowed while viewing an old version. Variable import overwrites ids that collide.

### LOW findings (~13)
- **Docs vs code:** docs say "29 MCP tools", the server registers 22 (V).
- **Panic paths:** comments panic on foreign data. Typing `nan` in the zoom box breaks the viewport.
- **Edge cases:** the resize box covers only the first 512 nodes. A numeric index on variable-binding export can allocate billions of entries. XPath recursion has no depth limit. Manifest paths are not validated.
- **Silent failures:** `.fig` export reports success even when the write failed. The sticky-note author ignores the user's name setting.
- **CI and scripts:** `ci.yml` has no `permissions:` block and actions are not pinned to a SHA. The `.mjs` scripts hard-code `/Users/jangtrinh`, and the release manifest has a single macOS `download_url` for all platforms.
