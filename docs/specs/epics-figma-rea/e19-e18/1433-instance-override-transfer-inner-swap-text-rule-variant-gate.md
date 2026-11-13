# #1433 Instance override transfer: inner-swap remap, single text rule, variant-switch "originally matched", NIVB

| Field | Value |
|---|---|
| Issue | #1433 (epic #1487) |
| Code baseline | `origin/main@d22dad970`. Re-anchor every line against HEAD and report drift before coding. |
| Verdict | **Still real** for all four items (one partly done: drops are counted, not reported). |
| Size | L: 12–16 h builder time (items 1–3) + 1 h owner-manual Figma probes; item 4 (NIVB) is a research spike of 2–3 h with no code unless the probe finds a rule |
| Tier | Claude Sonnet (judgment-heavy, sensitive `components.rs`, 8k-line file shared with two open PRs) |
| Touches | `crates/designos-doc/src/components.rs` (`swap_main` and its tests), `crates/designos-app/src/comp_ops.rs` (surface the drop count), `crates/designos-mcp/src/*` reply field (only if the owner picks the MCP option, Q3) |
| Depends on | PR #1673 and PR #1672 (both edit `components.rs`; land first). Owner answers Q3, Q4, Q5. |

## 1. Still real? (FACT, file:line on `d22dad970`)

- Override carry on swap/variant switch is `swap_main`, `crates/designos-doc/src/components.rs:1617-1768`. It snapshots overridden descendants by base-name path (`collect`, `:1630-1662`), rebuilds children from the new main (`:1669-1678`), then re-applies by exact path (`:1730-1767`).
- Item 1 (inner swaps): an inner instance that is itself swapped (`MainComponent` override, or its main differs from the old main's copy) is **counted and dropped**: `:1643-1647` increments `dropped`, `:1663-1667` prints `eprintln!` and adds to `Ctx::dropped_inner_swaps`. The count is returned by `swap_instance_inner` (`:2025-2032`) and `set_instance_prop_inner` (`:2074`) but `swap_instance` / `set_instance_prop` discard it (`:2021-2023`, `:2055-2057`). No caller in `designos-app` or `designos-mcp` reads it (grep `dropped_inner` outside `components.rs`: 0 hits). Overrides **below** the dropped inner swap are still carried by name path and may land on the unswapped inner instance's children or miss. Test `:6418` pins "dropped as base does (#1433)". → Real.
- Item 2 (single text rule): carry is by exact name path only (`by_path.get(&path)`, `:1744`). Text fields are dropped when kinds differ (`:1746-1749`). No "exactly one text node on each side" fallback. → Real.
- Item 3 (variant switch "originally matched"): the gate exists for plain/nested swaps (`if !(top_swap && same_family)`, `:1753-1758`: keep a field only if `record_override(orig, new_orig)` shows the old and new main agree). Top-level variant switches (`top_swap && same_family`, reached from `sync_pair` `:1797-1805` via `set_instance_prop`) skip the gate and carry every field. → Real (unchanged behaviour, not a regression).
- Item 4 (NIVB): no code; meaning unknown (`plans/reports/researcher-261031-1130-figma-instance-swap-rules.md` §Not determined). → Real as an unknown.

## 2. User-visible outcome

When a user swaps an instance or switches its variant:
1. A nested instance they swapped inside it stays swapped where the new main has a matching layer, and the overrides they made inside that nested instance follow it.
2. A text override survives when old and new contain exactly one text layer each, even if the layer names differ.
3. On a variant switch, a changed colour (or other field) survives only if both variants originally had the same value for it, as Figma does.
4. Nothing is lost silently: when Studio cannot carry a nested swap or an override, the user is told how many (surface per Q3).

## 3. Scope

- Item 1: remap overrides through inner swaps (both "transferred" and "untransferred" cases, below).
- Item 2: single text rule.
- Item 3: "originally matched" gate on top-level variant switches.
- Item 4: research only; implement only if the live probe yields a rule; otherwise record "no observable effect" in the issue.
- Report drops (count) on every swap and variant switch.

## 4. Behaviour rules (decided; technical)

R1 Transferred inner swap. For each carried inner instance with a `MainComponent` override (old tree), find the node at the same base-name path in the new tree. If that node is an instance, swap it to the carried main with `swap_recorded` (same function the UI uses, `:2036`), then re-apply the overrides collected below the old inner instance relative to it (recursive use of the same path carry). Engine name: `remapOverridesThroughTransferredInnerSwaps` (research report §Q1, SEEN string).
R2 Untransferred inner swap. If no matching instance exists in the new tree, or swapping would make a main hold itself (`would_hold_itself`), count 1 drop for the swap plus 1 per override field below it that finds no target.
R3 Override target precedence: exact path first; then (text fields only) the single text rule; never a type-mismatched target for non-text fields (existing guard `:1746-1752` stays).
R4 Single text rule: when an override carrying `Characters` or `TextStyle` finds no exact path, and the old instance subtree and the new main subtree each contain exactly one `Text` node, apply it to that one. Skip when the override came from a hoisted/exposed property (`exposed_nested_value_overrides`, `:1050`) — research INFERENCE "unless hoisted".
R5 Variant-switch gate: for `top_swap && same_family`, keep a non-text field only if `record_override(orig_in_old_variant, counterpart_in_new_variant)` does not contain it. `Characters`, `TextStyle`, `Name` stay exempt as in `:1756`. `Size` on the instance root keeps today's rule (`:1693-1696`).
R6 Drop accounting: one counter struct (`dropped_inner_swaps`, `dropped_overrides`) on `Ctx`; replace the `eprintln!` at `:1665` with the counter; public `swap_instance_report` / `set_instance_prop_report` return it; the App and MCP read it (Q3 decides where it shows).

## 5. Files likely touched

- `crates/designos-doc/src/components.rs:1617-1768` (`swap_main`), `:1797-1805` (variant path), `:2021-2095` (public wrappers), tests near `:5303`, `:5398`, `:6418-6510`.
- `crates/designos-app/src/comp_ops.rs` (swap and variant handlers: read the report, show per Q3).
- MCP tool reply (only with Q3 option "MCP + toast"): the swap/variant write path in `crates/designos-mcp/src` (`[UNVERIFIED]` exact file; grep `set_instance_prop` there before coding).

## 6. Acceptance criteria (each red on `d22dad970`, green after)

- AC-1 Transferred inner swap: Card main holds Icon instance (main Star); user swaps Card instance's Icon to Heart and sets Heart's fill red; switch Card to its sibling variant that also holds an Icon at the same path → Icon is still Heart, fill red; drop count 0. Flips test `:6418` (update its message, do not delete the case).
- AC-2 Untransferred inner swap: same, but the new variant has no Icon layer → Icon override gone, drop count reports 1 swap + 1 field.
- AC-3 Single text rule: old main has one Text "Label", new variant has one Text "Title"; override Characters "Buy" → "Title" shows "Buy". With two Text nodes on the new side → not applied, counted as 1 drop.
- AC-4 Hoisted text skips R4: same as AC-3 but the text value comes from an exposed text property → not applied by R4 (property carry rules apply instead).
- AC-5 Variant gate kept: variants Primary/Hover both have fill blue; user sets fill green on Primary instance; switch to Hover → green kept.
- AC-6 Variant gate drops: Primary fill blue, Secondary fill grey; user sets green; switch to Secondary → fill grey (Secondary's), drop count 1 (per Q4 answer; if the owner keeps today's behaviour, this AC inverts).
- AC-7 Undo: each scenario's swap/switch is one undo step and undo restores the old tree and overrides exactly.
- AC-8 Report surface: per Q3 (toast text and/or MCP reply field `dropped_overrides`), asserted in an App test (toast string) and/or MCP test (JSON field).
- AC-9 Existing suites green: `cargo test -p designos-doc --lib`, `-p designos-app --lib`, `-p designos-mcp`; `.fig` round-trip tests in `designos-fig` unchanged.
- AC-10 NIVB: a research note appended to the issue (probe steps, observed result). Code only if a rule is observed; then its own AC.

## 7. Test steps

1. Red: write AC-1..AC-6 as `components.rs` unit tests on `d22dad970` (after #1673/#1672 land, rebase). Run `cargo test -p designos-doc --lib swap_ variant_` → expect the new cases to fail.
2. Implement R1–R6. Green.
3. Kit check: open the Untitled UI PRO v8.0 copy (`.designos`), switch 20 variant instances across Buttons/Inputs; count drops; screenshot before/after of 3 instances (visual evidence).
4. Owner-manual probes in live Figma (see §9) before merging R4/R5 if the owner wants runtime confirmation (Q5).

## 8. Edge cases

- Inner swap two levels deep (swap inside a swapped inner instance): R1 recursion; cap depth at 64 like `orig_in_main` (`:1543`).
- Inner swap target would hold itself: counted drop, never a cycle (existing `would_hold_itself`).
- Same base name used twice under one parent: `by_path` keeps the first (`or_insert`, `:1736`); keep that, and count the second as a drop if it has overrides.
- Variant switch where the property values select no variant (`variant_target_from` → None): no switch, no drops.
- Override with a variable binding (`fills/0` bound): the gate already compares bindings (`record_override` checks `variables.bindings`, `components.rs:518-526`).
- Library (remote) mains: `component_key` cleared on swap (`:1683`); gate must read the local copy.

## 9. Figma-parity facts checked

- SEEN (engine strings, research report §Q1): `remapOverridesThroughTransferredInnerSwaps`, `...UntransferredInnerSwaps` in `FGOverrideTransferrer.cpp`.
- SEEN (help 360039150733, "Apply changes to instances"): "When selecting variants, Figma also checks if the layer properties you've changed originally matched between variants … If so, Figma will preserve your changes."
- SEEN (engine): `handleSingleTextRule`; INFERRED condition "exactly one text node each side, unless hoisted".
- UNKNOWN: NIVB (`FGOverrideTransferForNivb.cpp`, "(W01) NIVB: Overrides are being dropped").
- Doc conflict: help 360039150413 says swaps keep only text overrides; research judged it stale. Runtime probes (owner-manual, live Figma, read with `figma-agent inspect`): (d) nested glyph fill override through a swap with same vs different glyph fill; (g, new) variant switch with fill matched vs not; (h, new) single-text rule with differently named text layers; (i, new) inner swap kept across a variant switch.

## 10. Non-goals

Plain-swap size/name policy (done in #1427); slots (E3); MCP component tools (#1396).

## 11. Undecided user-visible details → README owner questions

- Where a drop count shows (toast, MCP reply, both, log only) → Q3.
- Whether to apply the variant gate now (behaviour change: overrides users keep today will reset) → Q4.
- Whether R4/R5 wait for live Figma probes → Q5.
- Toast wording, if chosen: decided default "Couldn't keep {n} changes" (≤ 8 words, PCP ux-writing rule) — owner may override in Q3.

## 12. Builder tier

Claude Sonnet. Reason: rules interact with sync, undo and slots in an 8k-line file; two open PRs touch it.

## Epistemic ledger

- FACT: item 1 counted-not-reported; item 2 absent; item 3 gated only for non-variant swaps (lines above).
- INFERENCE: "transferred/untransferred" map to R1/R2 (from names only).
- ASSUMPTION (medium): applying R5 is wanted even though it changes current behaviour. Flips on owner Q4.
- FACT: `record_override` compares variable bindings (`components.rs:518-526`), so R5 covers bound paints.
