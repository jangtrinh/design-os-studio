# #891 Every font_weight writer keeps the `wght` axis in sync — close-out

| Field | Value |
|---|---|
| Issue | #891 (bug, P2; epic #21, Backlog) |
| Code baseline | `origin/main@d22dad970` |
| Verdict | **Writer part fixed on main** (by #1616, which carried contributor PR #1414 "#891 (part)"). Two leftover bullets move to #941. Propose: add one guard test, then close #891. |
| Size | XS: 1 h builder |
| Tier | Grok build-fast (one source-scan test, machine-checkable) |
| Touches | new test file `crates/designos-doc/tests/font_weight_writers_use_helper.rs` (or a `#[test]` in `paint.rs`) only |
| Depends on | none |

## 1. Re-anchor (FACT)

- Shared helper exists: `TextStyle::set_font_weight` (`crates/designos-doc/src/paint.rs:760-765`) sets the weight and moves an existing `wght` axis.
- Every site the issue names now calls it:
  - `variables_apply.rs:226`, `:452`, `:508` (issue said "around 448 and 499");
  - `panels/style_edit.rs:255` (issue: "around 240");
  - `spec.rs:869`, `:1423`, `:1569` (MCP apply path goes through `spec.rs`);
  - also `typography.rs:233`, `textsession.rs:706`, importers (`rest_import.rs:466`, `pen/node.rs:841`, `:911`, `pen/instances.rs:795`, `fig/board.rs:35`, `webcapture/text_map.rs:329`).
- `find.rs` "around 880": the only `.font_weight =` in `find.rs` is `:1304`, inside `mod tests` (starts `:914`).
- Remaining `.font_weight =` assignments outside `tests.rs` files (20 hits) are all in `#[cfg(test)]` modules (`richtext.rs` tests start `:417`, `render/board.rs` `:362`, `render/text.rs` `:951`, `spec_bind.rs` `:1464`, `textsession.rs` `:867`), except:
  - `paint.rs:761` (the helper itself);
  - `panels/typography.rs:1067`, the Variable tab, which deliberately writes `set_axis("wght", v)` then `font_weight` (test `richtext.rs:763-768` pins "the Variable tab writes both and owns the fraction").
- Struct-literal writers (`font_weight: x` in `TextStyle { .. }`) all build a fresh style with `..Default::default()` (no axes), so no stale axis is possible (`protovars.rs:14`, `demo.rs:13`, `decks/demo.rs:11`, `showcase/mod.rs:169`, `svg/text.rs:97`).
- Leftover bullets of the issue, still real, **moved to #941** (spec `941-fig-font-weight-roundtrip-style-names-axis.md`, rules R4 and R5):
  - v0.6.0 files with weight 700 and `wght` 400 still draw regular;
  - `.fig` importer prefers `axisName` over `axisTag` (`rest_import.rs:551`) — and the real kit does write `axisName: "Weight"`.

## 2. Scope

One guard test so the bug cannot come back: scan `crates/*/src/**/*.rs`, strip `#[cfg(test)]` modules and `tests.rs` / `*_tests.rs` files, and fail on any `.font_weight =` assignment outside `paint.rs` (helper) and `panels/typography.rs` (Variable tab, allow-listed with a reason string).

## 3. Acceptance criteria

- **AC-1** The guard test passes on main.
- **AC-2** Red proof: add `t.style.font_weight = 700;` to a non-test function locally → the test fails naming file:line. Revert.
- **AC-3** Issue #891 closed with a comment linking #1616 / #1414 and #941 for the leftovers (controller action, not the builder's).

## 4. Edge cases

- A writer through a `&mut u16` alias (`let w = &mut s.font_weight;`) escapes a text scan. Accept: grep shows none today; note it in the test doc comment.

## 5. Non-goals

The two moved bullets (#941). Any behaviour change.

## 6. User-visible details

None.
