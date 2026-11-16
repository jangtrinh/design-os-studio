# #1494 An instance's effect style wins over stale effects in its own override entry

| Field | Value |
|---|---|
| Issue | #1494 (epic #1487; part of #18) |
| Code baseline | `origin/main@d22dad970`. Re-anchor before coding. |
| Verdict | **Still real.** The fill/stroke style rule from #1464 (`e531a9d91`, PR #1484) was never extended to effects. Contributor PR #1636 was rejected on kit evidence (dropped 1,316 effect style links, emptied shared style `1186:100350`). |
| Size | M: 6–8 h (code 2 h, kit census and round-trip 4–6 h) |
| Tier | Claude Sonnet (import path, kit-wide census, a previous attempt regressed) |
| Touches | `crates/designos-fig/src/fig.rs` (`convert_override`, `override_entries`), `crates/designos-fig/src/rest_import.rs` (`apply_paint_style_links` → effect slot, `override_field` mapping), tests in `crates/designos-fig/tests/` |
| Depends on | Open PRs editing `fig.rs`: #1664 (nested overrides on save), #1671 (padding), #1578 (toolchain). Land #1664 first (same override path). Owner Q6. |

## 1. Still real? (FACT on `d22dad970`)

- Node conversion already lets an effect style win: `fig.rs:1695-1701` (`fx_style = style_node_of(nc, "styleIdForEffect", "inheritEffectStyleID")`, unless an effect has `colorVar`).
- Override conversion does not: `convert_override` writes raw override `effects` (`fig.rs:2364-2367`) and has no `styleIdForEffect` branch. The paint loop that does this for fills/strokes covers only `fillPaints`/`strokePaints` (`fig.rs:2488-2503`).
- Self-entry restatement (an override entry whose `guidPath` is the instance itself) preserves only `styleIdForFill` / `styleIdForStrokeFill` (`fig.rs:2776-2780`).
- `apply_paint_style_links` handles only `fill` and `stroke` slots (`rest_import.rs:439-458`); there is no `_clearEffectStyle`.
- Shared effect styles are created from node effects at import (`rest_import.rs:1554-1582`), so a stale override that empties a node's effects can empty the style itself (the #1636 failure, issue comment 2026-11-05).
- INFERENCE: the "mix" comes from the instance node taking the style's effects (node path) while its self-override restates the older list without a style key; whichever is applied last plus the still-linked `style_refs["effect"]` gives style effects in some passes and stale ones in others. The builder confirms with one kit instance before coding (print node `effects` + `style_refs` after import).

## 2. User-visible outcome

The 331 kit instances that link an effect style render exactly that style's shadows/blurs; editing the style updates them; a `.fig` save and reopen keeps the link. No other node loses its effect style, and shared style `1186:100350` keeps its effects.

## 3. Behaviour rules (decided, mirror #1464)

- R1 `convert_override`: when the entry names `styleIdForEffect` (or legacy `inheritEffectStyleID`) and the local style node exists, take the style's `effects` (unless an override effect binds `colorVar`) and link `styles.effect`; when the entry has `effects` but no style key, emit `_clearEffectStyle`.
- R2 `override_entries` self entry: add `("styleIdForEffect", "effect", "_clearEffectStyle")` to the loop at `fig.rs:2776`, so a self entry with effects and no style key keeps the style the instance node names.
- R3 `apply_paint_style_links`: add the `effects` / `effect` / `_clearEffectStyle` / `OverrideField::Effects` row.
- R4 Never derive a shared style's effects from an override entry (only from the style node); assert it in a test.
- R5 Empty effects in a self entry with an effect style on the node: per owner Q6 (recommended: the style wins).

## 4. Files likely touched

`fig.rs:2312-2420` (`convert_override`), `fig.rs:2776-2780` (`override_entries`), `rest_import.rs:439-458`, `rest_import.rs:3880-3890` (`override_field` mapping for `_clearEffectStyle`), tests.

## 5. Acceptance criteria (red on `d22dad970`)

- AC-1 Unit (synthetic Kiwi): instance with `styleIdForEffect` → style S (drop shadow 0/4/8); self entry with `effects` [inner shadow] and no style key → imported instance has `style_refs.effect == S` and effects == S's effects.
- AC-2 Same with an explicit `styleIdForEffect` on the entry pointing to S2 → links S2.
- AC-3 Entry with `effects` and the detach sentinel `4294967295:4294967295` → no effect style link, entry effects kept.
- AC-4 Self entry with `effects: []` and node style S → per Q6 (recommended: S wins, effects == S's).
- AC-5 Round trip: import → `.fig` export → re-import keeps `styleIdForEffect` on the instance (kiwi field check) and the same effects.
- AC-6 Kit census (Untitled UI PRO v8.0 copy, script under `../e19-e18/evidence/1494/`): instances with an effect-style link whose resolved effects differ from the style's: 331 before → 0 after; total effect style links before vs after: no decrease (the #1636 failure was −1,316); style `1186:100350` effect count unchanged and non-empty; list of nodes whose resolved effects changed, each with frame name.
- AC-7 Visual: CPU render of 3 affected kit frames before/after (`designos --screenshot`), side by side; plus the 2 instances #1636 broke render their shadow.
- AC-8 `cargo test -p designos-fig` and `-p designos-doc --lib` green.

## 6. Test steps

1. Write AC-1..AC-5 tests (`crates/designos-fig/tests/` next to the #1464 tests; grep `effect_style_link` / `_clearFillStyle` tests for the fixture helper). Red.
2. Run the kit census on `d22dad970` (baseline numbers, expect 331).
3. Implement R1–R5; green; rerun census; renders.

## 7. Edge cases

- Effect with `colorVar` bound in the override: override wins (same as fills).
- Library (remote) effect style (`key:` id): link kept even without a local node (`insert_style_override_link` rule, `fig.rs:2678-2685`).
- Nested override path (not self): R1 applies, R2 does not.
- Text nodes: effect style applies the same (unlike fill → TextStyle mapping).
- Grid style (`styleIdForGrid`) is out of scope even though it shares the shape.

## 8. Non-goals

Grid styles; effect rendering changes; REST (non-.fig) import unless the same helper serves it.

## 9. Figma-parity facts

- FACT (kit data, issue body): 331 instances carry an effect style on the instance node with a self-override holding older effects without a style key.
- INFERENCE (Figma model): a node's `styleIdForEffect` is authoritative and the override's raw `effects` is a cache; same model #1464 verified for fills/strokes on the kit (PR #1484 census).
- Live check (optional): open one of the 331 in Figma and read effects via `figma-agent inspect`; expect the style's list.

## 10. Undecided user-visible details

- Q6: empty effects in a self entry with an effect style on the node.

## 11. Builder tier

Claude Sonnet.

## Epistemic ledger

- FACT: no effect branch in `convert_override` / self-entry loop / `apply_paint_style_links` (lines above).
- FACT: #1636 regressed 1,316 links (owner comment on the issue).
- INFERENCE: mechanism of the "mix" (verify on one kit node first).
- ASSUMPTION (medium): the 331 count still holds on `d22dad970` after #1647/#1664. AC-6 measures it first.
