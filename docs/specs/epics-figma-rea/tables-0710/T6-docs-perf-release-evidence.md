# T6: perf measurement, docs, release evidence for tables

| Field | Value |
|---|---|
| Issues | #1231 edge case "Very large table (50x50)", epic #1215 perf budgets (§8), docs for #317 / #331 / #1231. Hands off to WB-18 #1233 (release notes and screenshots for the whole epic). |
| Size | 5 h |
| Tier | Claude Sonnet (perf and docs), Opus review |
| Depends on | T2, T3a, T3b, T4, T5, T5b merged |
| Blocks | 0.7.11 tag |
| Branch | `jangtrinh/tables-t6-perf-docs` |

## Goal

1. Measured numbers show that a large table keeps the canvas within the invariants: 8 ms interactive budget, 0% idle CPU.
2. The docs tell users and agents what tables do.

## Perf

Epic budgets (FACT, #1215 body):
- 2,000 items + 200 frames ≥ 55 fps;
- open 5,000 items ≤ 2 s.

A 50×50 table is 2,501 nodes (T1 model).

- `crates/designos-gpu/tests/table_perf.rs` (new, `#[ignore]`), modelled on `crates/designos-gpu/tests/connector_perf.rs` (FACT, it exists). It builds a page with one 50×50 table filled with 3-word texts and reports, in `--release`:
  - a. the median interactive compile time at fit zoom and at 100% zoom over 20 runs;
  - b. the deferred count (`stale_hits`) after the first frame;
  - c. whether frame 3 at rest is a reuse (`Cb { frame: None }` path).
- Idle CPU: the `ps -p <pid> -o time=` method from `CLAUDE.md` §4. Open the 50×50 fixture with `--gpu`, wait past the 250 ms idle refine, and take two 20 s windows. Target `dCPU = 0.000s`.
- Open time: save the 50×50 table plus 2,500 stickies as `.designos`, then time the open with the existing open path ([UNVERIFIED] CLI flag; use the method WB-8 #1223 uses if it has merged). Target ≤ 2 s.

## Docs

The builder reads each file before editing, and verifies each claim against the code after.
- `README.md`: add a "What's new" entry for 0.7.11 tables, with a capture (project memory: every release adds one; the newest entry has captures). Coordinate with WB-18 #1233 so there is one entry for the release.
- `CLAUDE.md` §2 MCP tool table: the `designos_apply_batch` row gains the `table` op.
  - If a generator owns that table, run it instead of hand-editing ([UNVERIFIED]; look for a script that writes the "Declared `designos-mcp` Tools" table).
- `CONTEXT.md`: glossary entries for "Table", "Table cell", "Track (row/column id)" and "Merge anchor".
- `.brv/context-tree/architecture/crate-map.md:59` is wrong today (FACT: it lists `Table`, `WashiTape`, `CodeBlock` and `Slice` as `NodeKind` variants, and `origin/main` has none of those). Correct it to the real list after T1.
- `docs/` user guide page for tables, if the docs tree has a whiteboard section ([UNVERIFIED]; find through the docs navigation).
- Release-notes line: files containing tables need 0.7.11 or later to open (T1 serde note).

## Files T6 may modify

- `crates/designos-gpu/tests/table_perf.rs` (new)
- `README.md`
- `CLAUDE.md` (the tool table row only)
- `CONTEXT.md`
- `.brv/context-tree/architecture/crate-map.md`
- the docs page(s) found
- the perf report `plans/261106-2245-epic-waves/reports/tables-perf-0710.md`

## Acceptance criteria

1. `cargo test -p designos-gpu --release -- --ignored table_perf` prints medians.
   - The test asserts:
     - the median interactive compile time is ≤ 16 ms at both zooms on the controller's M4 Max (one frame at 60 fps);
     - frame 3 at rest is a reuse.
   - The assertion either passes or fails; there is no fallback pass condition.
   - If it fails, that is an owner escalation by AskUserQuestion: ship with a filed follow-up, or hold the tag. Report the measured numbers honestly, and never loosen the threshold to make the test pass.
2. Idle CPU is `dCPU = 0.000s` over 2 × 20 s with the 50×50 fixture open, from a terminal capture.
3. The open time is reported as a number against the 2 s budget.
4. Docs: every new claim cites code. `grep -n "WashiTape\|CodeBlock" .brv/context-tree/architecture/crate-map.md` returns nothing unless the code has them.
5. Reference codes follow the global writing rule: no code such as "WB-16" leads a sentence or heading in user-facing docs.

## Non-goals

- Optimizing beyond the budget.
- Release notes for the whole epic (WB-18 #1233).

## Gates

- `cargo fmt`
- `clippy -D warnings`
- `cargo test -p designos-gpu`
- The doc link check the repo uses ([UNVERIFIED])

## Visual evidence

- Terminal captures of the perf and idle runs.
- A 1440 capture of the 50×50 table, zoomed to fit and at 100%.
- The README entry rendered on GitHub (or as a local markdown preview capture).

## Rollback

Docs and tests only; revert freely.
