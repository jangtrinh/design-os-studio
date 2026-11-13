# #834 .fig save keeps lists and icon strokes: finish the nested-override gap and add a pixel gate to release smoke step 4a

| Field | Value |
|---|---|
| Issue | #834 (bug, P1; epic #21; milestone Backlog) |
| Code baseline | `origin/main@d22dad970`. Re-anchor every line against HEAD and report drift before coding. |
| Verdict | **Partly fixed.** Done-when 1 and 2 are reported met for top-level instances by #1647 (INFERENCE, not re-measured here). Nested-instance overrides are still dropped (#1649, PR #1664 open). Done-when 3 (pixel compare in smoke 4a) is **not done**. |
| Size | M: 5–7 h builder (headless `--export-png` CLI flag + smoke 4a pixel step + one kit re-measure). Nested part is PR #1664's work, not this spec. |
| Tier | Grok build-fast (CLI flag with a unit test + shell step, machine-checkable on the kit; reversible). Review by Claude. |
| Touches | `crates/designos-app/src/cli.rs` (flag parse, next to `--export-spec` `:393`), `crates/designos-app/src/main.rs` (headless branch next to `--import-capture` `:431`), `scripts/release-smoke-macos.sh` (step 4a `:400-451`; step 5a `:478-494` can reuse the flag). |
| Depends on | PR #1664 (#1649 nested overrides) merged first, else AC-2 fails on the kit for the known reason. |

## 1. Still real? (FACT unless tagged)

- #1647 (`faeb53987`, merged 2026-11-06) body reports: "Welcome to Untitled UI" frame 32,581 → 0 differing pixels; override flags added that the file did not have 9,465 / 18,620 → 0 / 0; numbered and bulleted list lines on an overridden text are written. Says "Part of #834", not "Closes". INFERENCE: done-when 1 and 2 hold for top-level instances; not re-measured in this pass.
- Same body: "overrides set on instances nested inside a main component are still dropped (#1649). On this kit, 3,684 fill-flagged and 4,811 stroke-flagged nodes resolve to a different first paint after a save." → done-when 2 not met kit-wide. PR #1664 (open, `jangtrinh/agy-1649-nested-overrides`, fix round 3 in flight per `controller-ledger.md`) targets exactly this.
- List writer exists: `crates/designos-fig/src/fig_export.rs:3240-3243` writes `lineType` ORDERED_LIST / UNORDERED_LIST and `indentationLevel`.
- Smoke step 4a (`scripts/release-smoke-macos.sh:400-451`) compares node count, page structure, auto-layout and grid spec signatures, then takes an app screenshot (`4a.screenshot`, `:449`). **No pixel comparison** of a kit frame before vs after the `.fig` save. → done-when 3 is real.
- No headless render seam for the kit: the smoke tries `designos-mcp` `designos_render_png` (`:484`), but "That server refuses imports above 64 MiB" (`:480-481`) and the kit copy is 336,639,376 bytes (measured `ls -l`), so 5a falls to OWNER-MANUAL (`:493`). "The designos CLI has no PNG export flag" (`:479`; confirmed: no `--export-png` among the flags parsed in `cli.rs`).
- `designos-diff compare A B --threshold <ratio> --json --out diff.png` exists (`crates/designos-diff/src/main.rs`, `Compare` subcommand) and ships in the bundle (`:268`).
- Decision (technical, recorded): add a headless `designos <file> --select <name|id:n|fig:id> --export-png <out.png> [--scale s]` that calls the same `designos_export` PNG path as File > Export (the path `designos_render_png` uses), CPU renderer, no window. It removes the 64 MiB limit for the smoke and turns 5a from OWNER-MANUAL into an automatic row.

## 2. User-visible outcome

A designer who opens a Figma file in Studio, saves it as `.fig` and opens it again (in Studio or Figma) sees the same pictures: numbered lists keep their numbers, icons keep their stroke weight, instances keep following their variables. Release smoke fails loudly if a save changes pixels.

## 3. Scope

1. CLI flag `--export-png <out.png>` (requires `--select`; optional `--scale`, default 1; refuses to overwrite unless `--overwrite`, matching `--export-fig`). Exit non-zero with a one-line reason when the selection is missing or ambiguous.
2. Smoke step 4a renders the same kit frames (`$AL_FRAME`, and "Welcome to Untitled UI") from `kit.fig` and from `kit-saved.fig` with the new flag, then runs `designos-diff compare --threshold 0 --json --out`.
3. New result rows `4a.fig-pixels-<frame>`: PASS at 0 mismatched pixels; FAIL prints mismatched count, ratio and the diff PNG path. Step 5a uses the flag instead of the MCP call.
4. Re-measure the kit after PR #1664 merges and record the counts in the PR (fill/stroke first-paint differences kit-wide, and the two frames' pixel counts).
5. Close #834 when AC-1..AC-5 hold.

## 4. Acceptance criteria (red then green)

- **AC-1** Smoke 4a has a pixel step per listed frame; on current main before #1664 it is expected PASS for "Welcome to Untitled UI" (0 per #1647) — if it FAILs, that is a real regression to report, not to tune away.
- **AC-2** Red proof: with a deliberately broken writer (local patch dropping `lineType` at `fig_export.rs:3240`, not committed), the step FAILs with a non-zero count and writes the diff PNG. Revert, PASS again.
- **AC-3** Threshold is 0 mismatched pixels with `designos-diff` default anti-alias forgiveness; no `--ignore` regions.
- **AC-4** After #1664 merges: kit-wide first-paint differences for fill- and stroke-flagged nodes = 0 (the #1647 measure: 3,684 / 4,811 today), recorded in the PR.
- **AC-5** `--export-png` unit tests in `cli.rs` (parse, missing `--select` error, no-overwrite) fail before the flag exists and pass after; its PNG of a fixture frame equals `designos_export::export_png` bytes for the same node and scale.

## 5. Test steps

1. `caffeinate -u -d` (screenshots need the display awake), run `scripts/release-smoke-macos.sh` against a locally built bundle.
2. Read `step4-compare.json` + the new `4a.fig-pixels-*` rows; open the diff PNGs.
3. Red proof per AC-2, then revert.

## 6. Edge cases

- Frame name collides after reopen: select by name the way `$AL_FRAME` already does; if two match, FAIL with "ambiguous", never pick one.
- Fonts not yet fetched at render time → false diff. The flag waits for the font requests of the selected subtree (same fetch as the app) before rendering. The kit uses faces that are likely not fetchable (SF Pro Text, Euclid Circular A, Suisse Intl, Sharp Grotesk — kit dump 2026-11-06), so a missing face renders with the normal fallback and the flag prints the missing faces on stderr. The gate compares before-save vs after-save, so the same fallback on both sides is a valid comparison: the smoke row FAILs only when the two sides report different missing-face sets, and records the set in the result row.
- Render size differs (bounds changed) → `designos-diff` reports size mismatch: that is a FAIL, print both sizes.

## 7. Non-goals

- Fixing the nested-instance override drop (PR #1664).
- Whole-kit pixel parity (31 pages); two frames only, per the issue.
- Changing how the app's Save action writes `.fig` (4a is the CLI convert path, `:401-402`).

## 8. Format facts checked

- `.fig` list lines: `lineType` + `indentationLevel` per line (`fig_export.rs:3240-3243`, reads/writes symmetric per #1647 body). No new format claims.

## 9. User-visible details not settled by the issue

The new CLI flag name and its `--help` line (CLI users see it). Decided: `--export-png`, mirroring `--export-fig` / `--export-spec`; help line "Render the selected layer to a PNG file (headless)". Update `docs/llms.txt` / CLI docs only where they list export flags.

## 10. Rollback

Revert the smoke-script commit and the flag commit separately; no file-format change.
