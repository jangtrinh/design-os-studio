# #5 Import: global clone budget and remaining review backlog

- Issue: https://github.com/jangtrinh/design-os-studio/issues/5 (follow-up from PR #1)
- Spec base: `main` at `767acf9`.
- Open PR: none.
- Hot files: `crates/designos-fig/src/rest_import.rs`, `crates/designos-fig/src/kiwi.rs`; also `crates/designos-app/src/proto/smart.rs`.

## 1. Summary and user story

As a user opening files from anywhere:
- a crafted or pathological `.fig` or REST file must not stall or exhaust memory through chains of nested component instances;
- a legitimate large file must still open;
- id arithmetic must never overflow.

The issue also asks to close the remaining backlog of the PR #1 full review.

## 2. Current state

- FACT (item 1): the per-instance clone budget is `let mut budget = 50_000usize` (`rest_import.rs:1850`), reset for every instance. No document-wide budget exists. The issue measured 5k swap or component links → 25M nodes in 24 s.
- FACT (item 2): `MAX_ZERO_WIRE_INSTANCES = 1 << 16` (`kiwi.rs:35`), checked at `kiwi.rs:218` with the error `"zero-size struct count exceeds budget"`. The value budget is `MAX_DECODED_VALUES = 1 << 28` (`kiwi.rs:29`).
- FACT (item 3):
  - `self.next_clone = self.doc.nodes.keys().next_back().map(|k| k.0).unwrap_or(0) + 1` (`rest_import.rs:1300`);
  - `fresh_id` does `self.next_clone += 1` (`rest_import.rs:1588-1592`);
  - `subtree_doc` does `let pid = NodeId(max + 1)` (`designos-app/src/proto/smart.rs:14`), plus `out.reserve_ids(NodeId(max + 1))` (`smart.rs:31`, the line the issue cites).

  All three overflow at `u64::MAX`. The load repair keeps `MIN_ID_HEADROOM = 1 << 32` (`designos-doc/src/load_repair.rs:16`) for loaded documents, but import does not go through that check before computing `next_clone`.
- FACT (item 4): **fixed on `main`.** `Document::to_json` writes non-finite numbers as their neutral value (`designos-doc/src/lib.rs:700-711`, `finite_json/mod.rs:1-22`, `finite_json/ser.rs:60-68`). `from_json_with_report` repairs older files and counts the repairs (`lib.rs:737-742`). PRs #120, #318 and #464 landed this. No work is left except a regression test covering the issue's exact scenario.
- FACT (item 5): the PR #1 review report listing about 30 MEDIUM and 13 LOW findings is not in the repo (`plans/` searched; no `MEDIUM` list for PR #1). The app-root slice (`specs`, `ops`, `cmdk`, `shell`, `video`, `present`, `dev`) has not been reviewed.

## 3. Behaviour rules

1. **Global clone budget.** One import allows at most `MAX_IMPORT_CLONES = 8_000_000` cloned nodes in total, across every `expand_instance` and `swap_nested`. That is 7.4 times the largest legitimate file measured: VSF, 1,082,324 full scene nodes (`CLAUDE.md` §3).
   - The per-instance 50,000 cap stays.
   - When the global budget runs out, the remaining instances stay unexpanded (no children). Each one counts in `ImportNotes.skipped_instance_expansions` (struct from #111).
   - The import still succeeds. Correctness is degraded but visible.
2. **Notice.** When `skipped_instance_expansions > 0`, the app shows an error toast once: `"This file has more nested component copies than Studio can expand (limit 8,000,000). {n} instances are shown empty."` The app's file-open path switches to `import_fig_with_notes` / `import_rest_json_with_notes` for this.
3. **Zero-size structs.** The separate 65,536 cap is removed. Zero-wire instances count against the existing `MAX_DECODED_VALUES` budget (`1 << 28`), the bound every other decoded value already shares. A file that is legitimate but exotic, with more than 65,536 zero-size structs, then decodes, and memory stays bounded.
4. **Overflow-safe ids.**
   - `next_clone` is computed as `max.checked_add(1)`. On `None`, the import fails with `FigError::Format("node id space exhausted")`.
   - `fresh_id` uses `checked_add`, and fails the import with the same error. The function returns `Result`, threaded to the import entry.
   - `subtree_doc` uses `checked_add(1)` at `smart.rs:14` and `:31`. On `None` it returns `Document::new("snapshot")` (empty), and the smart-animate caller draws the frame without a morph. This cannot happen for a loaded document, because load repair keeps `MIN_ID_HEADROOM`, so no further handling is needed.
5. **NaN.** No behaviour change. Add the regression test only.
6. **Review backlog.**
   - (a) Locate or reconstruct the PR #1 review findings. If the report cannot be found, run a fresh review of the same slices.
   - (b) Review the app-root slice (`specs`, `ops`, `cmdk`, `shell`, `video`, `present`, `dev`).
   - (c) File each confirmed MEDIUM or LOW as its own issue with a literal repro.
   - (d) Close #5 when (a) through (c) are filed and items 1–4 are merged. Fixing the backlog items is not part of #5; each filed issue is its own task.

## 4. Data model / schema changes

`ImportNotes.skipped_instance_expansions: usize` (on the struct added by #111). No document change.

## 5. Contracts

- `const MAX_IMPORT_CLONES: usize = 8_000_000;` in `rest_import.rs`.
- `fn fresh_id(&mut self) -> Result<NodeId, FigError>`.
- Error text for exhausted ids: `FigError::Format("node id space exhausted".into())`, using the existing variant (`designos-fig/src/lib.rs:49-50`).

## 6. File-format mapping

None. RESOLVED.

## 7. UI

One error toast (rule 2), on the existing toast surface. Evidence: a terminal capture of the budget test, plus a capture of the toast on the synthetic chain fixture opened in the app.

## 8. Edge cases

1. A chain of 5,000 components, each instancing the previous one: expansion stops at 8M clones and the import finishes. Time is bounded: test budget below 10 s in release.
2. The VSF file (owner machine): opens with `skipped_instance_expansions == 0`.
3. A kiwi message with 100,000 zero-size structs: decodes. Before the change it fails with `"zero-size struct count exceeds budget"`.
4. A file with a node id `18446744073709551615`: import fails with `"node id space exhausted"`, with no panic and no wrap.
5. `subtree_doc` on a document whose max id is `u64::MAX`: returns an empty document, and the smart-animate path draws without a morph.
6. Save then reopen a document holding a NaN: opens, and the count is reported once (existing behaviour).

## 9. Non-goals

- Fixing the backlog findings themselves (each becomes its own issue).
- Lazy expansion of instances on demand.

## 10. Test plan

Seams: `crates/designos-fig/tests/` (a new `import_limits.rs`), `designos-fig` kiwi unit tests, `designos-app/src/proto/smart.rs` tests and `designos-doc/src/finite_json_tests.rs`.

| Test | Fails before |
|---|---|
| `component_chain_import_stops_at_global_clone_budget_and_reports_skips` (5,000-deep chain built in REST JSON) | yes (24 s / 25M nodes) |
| `zero_size_structs_beyond_65536_decode_within_value_budget` | yes |
| `import_with_max_node_id_fails_with_id_space_exhausted` | yes (overflow) |
| `subtree_doc_with_max_id_does_not_overflow` | yes (panics in debug) |
| `nan_document_saves_and_reopens` (`to_json` then `from_json_with_report`: opens, `non_finite_numbers` as expected) | no (guard; item already fixed) |

## 11. Performance budgets

- No `CLAUDE.md` §4 invariant applies to import.
- The global counter is O(1) per clone.
- Budgets of other tests stay unchanged.

## 12. Dependencies and build order

- Needs `ImportNotes` from #111.
- Hot files `rest_import.rs` and `kiwi.rs`: run after #525, last in the import queue.

## 13. Owner rulings that apply

- Brainstorm no-gos: no loosened budgets or assertions. The new global budget tightens.
- Delivery law 1 ("nothing vanishes silently"): skipped expansions are counted and shown.
- UNRESOLVED (Q5-1): the location of the PR #1 review report with the ~30 MEDIUM and 13 LOW findings.
