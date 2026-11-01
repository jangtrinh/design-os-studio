# Report: `#1464` — Instance Fill and Stroke Overrides Take Their Style Link From the Override

## 1. Line-Anchor Drift (`v0.7.5` / Audit Citations vs `HEAD` `7ce3a092`)

- [`crates/designos-fig/src/rest_import.rs:2822-2833`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/rest_import.rs#L2822-L2833) (cited in `#1464` and the 2026-10-31 audit as `rest_import.rs:2788-2795`, **+34 lines drift** due to `#1427` instance-swap overrides): `if set.get("fills").is_some()` and `if set.get("strokes").is_some()` replace `n.fills` / `n.stroke.paints` and strip `fills/*` / `strokes/*` variable bindings, without updating `n.style_refs["fill"]` or `n.style_refs["stroke"]`.
- [`crates/designos-fig/src/rest_import.rs:258-290`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/rest_import.rs#L258-L290), [`3148`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/rest_import.rs#L3148), [`3526-3558`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/rest_import.rs#L3526-L3558) (`#1348` commit `f1baf5b7`): `apply_text_style_link` and `override_field` (`_clearTextStyle`, `"styles" if is_text`).
- [`crates/designos-fig/src/fig.rs:2334-2341`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2334-L2341) (**0 lines drift**): `convert_override` checked `if obj.contains_key(src)` before `obj.get(style).and_then(|r| lk.style_node(r))` and never emitted `styles.fill`, `styles.stroke`, `_clearFillStyle`, or `_clearStrokeStyle`.
- [`crates/designos-fig/src/fig.rs:2509-2525`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2509-L2525) (**0 lines drift**): `text_style_override_link` from `#1348`.
- [`crates/designos-doc/src/variables_apply.rs:567-584`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-doc/src/variables_apply.rs#L567-L584) (**0 lines drift**): `resolve_node` applies `doc.styles.get(style_id)` for each slot in `node.style_refs` (`"fill"`, `"stroke"`, `"text"`, `"effect"`, `"grid"`).

## 2. Epistemic Ledger

- **[FACT]** On `origin/main` (`7ce3a092`), `convert_override` in `crates/designos-fig/src/fig.rs` never wrote `styleIdForFill` or `styleIdForStrokeFill` into `m["styles"]` or a detach marker, and `apply_override` in `crates/designos-fig/src/rest_import.rs` never updated `n.style_refs["fill"]` or `n.style_refs["stroke"]`. Consequently, every cloned instance child kept the main component's `style_refs["fill"]` / `style_refs["stroke"]`, and `resolve_node` overwrote the overridden paints with the main component's style.
- **[FACT]** Across all 113 pages (864,170 hydrated nodes) of the Untitled UI PRO VARIABLES v8.0 kit, `kit_detect` reported 119 `Fills` overrides (`117` with `via_var=false` and `2` with `via_var=true`) and `0` `Stroke` overrides whose resolved value diverged from raw via an inherited style link (`via_style=true`): 110 swatches on Page 4 Frame `1137` (`Gradients`) and 9 nodes across 5 Landing frames on Page 84 (`88899`, `89183`, `89194`, `89205`, `89216`).
- **[FACT]** On Frame `1137`, 14 Neutral swatches (including swatch `212423`, override target `1525:273952`) carry both a named `styleIdForFill` (`1272:5915` `Gradient/Neutral/600 -> 500 (90deg)`, whose style node has `#525252 → #737373` matching the swatch's text labels) and stale pre-v8 inline `fillPaints` (`#475466 → #667084`, plus stale `stopsVar` `#f5f5f5 → #e5e5e5` on `7942:72516` / `215717` and `11255:6363` / `215762`). Matching `convert_node`'s rule (`fig.rs:1535-1542`: a local style node wins over stale inline paints unless `colorVar` is bound) in `convert_override` and `bound_variables` resolves swatch `212423` to `#525252 → #737373` and reduces `kit_detect`'s `via_style=true` override divergences from `119` to `0`.
- **[FACT]** On Page 84's dark banking Landing frames (`89216`, `89205`, `89183`, `89194`, `88899`), the 9 overrides link `1272:5920` (`Gradient/Neutral/900 -> 600 (45deg)`), `1272:5919` (`Gradient/Neutral/800 -> 700 (45deg)`), and `1033:618` (image fill). Because those styles bind primitive `Colors/Neutral/*` variables (single-mode `_Primitives` collection), the relinked styles resolve to the exact same dark colours (`#171717 → #525252`, `#262626 → #404040`, and `img`) even inside dark-mode frames (`0` mode-difference conflicts).
- **[FACT]** `styleIdForEffect` is intentionally left unchanged per Ruling #1: in the Untitled UI kit (331 nodes, e.g. `1256:132203` `Buttons/Social button`), the instance node itself carries `styleIdForEffect: 7395:8771` (`Shadows/shadow-xs`) while its self-override in `symbolOverrides` carries stale `effects` without repeating `styleIdForEffect`. Clearing `style_refs["effect"]` when `effects` is present without `styleIdForEffect` would detach valid effect styles on those instances.

## 3. Summary

- Extended `#1348`'s style-link override handling to fills (`fillPaints` / `styleIdForFill`) and strokes (`strokePaints` / `styleIdForStrokeFill`) across the shared `.fig` → REST import pipeline (`fig.rs` + `rest_import.rs`).
- Aligned `convert_override` and `bound_variables` in `fig.rs` with `convert_node` (`fig.rs:1535-1542`) so a local style named by `styleIdForFill` / `styleIdForStrokeFill` supplies the paints and variable bindings over stale inline caches unless `colorVar` is bound.
- Added `apply_paint_style_links` and `_clearFillStyle` / `_clearStrokeStyle` mapping in `rest_import.rs` to relink named styles, clear inherited links when no style or the `4294967295:4294967295` sentinel is present, and record `OverrideField::Fills` / `OverrideField::Stroke`.
- Added 7 unit tests in `crates/designos-fig/tests/fig_library.rs` (all 7 red on `origin/main`, green with the fix) covering fill and stroke relink, no-style detach, sentinel detach, unchanged copies, and `sync_instances` preservation.
- Verified on an APFS clone of the Untitled UI v8.0 kit: `kit_detect` `Fills`/`Stroke` `via_style=true` override divergences dropped from `119` to `0`, Frame `1137` swatch `212423` resolves to `#525252 → #737373`, and before/after CPU renders of Frames `1137` and `89216` were captured to `out/evidence/1464-*.png`.

## 4. Files Touched

- [`crates/designos-fig/src/fig.rs:1867-1898`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L1867-L1898) — Skip extracting stale inline paint variable bindings in `bound_variables` when a local `styleIdForFill` / `styleIdForStrokeFill` supplies the paints.
- [`crates/designos-fig/src/fig.rs:2339-2357`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2339-L2357) — In `convert_override`, let local style nodes supply paints (matching `convert_node`) and emit `styles.fill` / `styles.stroke` or `_clearFillStyle` / `_clearStrokeStyle`.
- [`crates/designos-fig/src/fig.rs:2523-2536`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2523-L2536) — Replace `text_style_override_link` with `insert_style_override_link` merging into `m["styles"]` so text and paint style overrides on the same node do not overwrite each other.
- [`crates/designos-fig/src/rest_import.rs:292-314`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/rest_import.rs#L292-L314) — Add `apply_paint_style_links` to relink or clear `style_refs["fill"]` and `style_refs["stroke"]` and record `OverrideField::Fills` / `OverrideField::Stroke`.
- [`crates/designos-fig/src/rest_import.rs:3171-3172`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/rest_import.rs#L3171-L3172) — Invoke `apply_paint_style_links(n, set)` alongside `apply_text_style_link(n, set)` during override application.
- [`crates/designos-fig/src/rest_import.rs:3559-3563`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/rest_import.rs#L3559-L3563) — Map `_clearFillStyle` and `_clearStrokeStyle` in `override_field`.
- [`crates/designos-fig/tests/fig_library.rs:707-1095`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/tests/fig_library.rs#L707-L1095) — Add 7 unit tests for `#1464` (fill/stroke named style relink, no-style detach, sentinel detach, and unchanged copy keeping main's link).

## 5. Verification

### 5.1 Acceptance #1 — Unit Tests (Red on Base, Green on Final Commit)

- **Red on base commit (`origin/main` `7ce3a092`):**
  Log: [`plans/reports/agy-1464-fill-style-override-evidence/unit-tests-red-on-base.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/plans/reports/agy-1464-fill-style-override-evidence/unit-tests-red-on-base.log)
  ```text
  failures:
      instance_copy_without_fill_override_keeps_main_style_link
      instance_fill_override_with_named_style_relinks
      instance_fill_override_with_no_style_clears_inherited_link
      instance_fill_override_with_sentinel_clears_inherited_link
      instance_stroke_override_with_named_style_relinks
      instance_stroke_override_with_no_style_clears_inherited_link
      instance_stroke_override_with_sentinel_clears_inherited_link

  test result: FAILED. 10 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
  ```
- **Green on final commit:**
  ```text
  running 17 tests
  test instance_copy_without_fill_override_keeps_main_style_link ... ok
  test instance_fill_override_with_named_style_relinks ... ok
  test instance_fill_override_with_no_style_clears_inherited_link ... ok
  test instance_fill_override_with_sentinel_clears_inherited_link ... ok
  test instance_stroke_override_with_named_style_relinks ... ok
  test instance_stroke_override_with_no_style_clears_inherited_link ... ok
  test instance_stroke_override_with_sentinel_clears_inherited_link ... ok
  ...
  test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
  ```

### 5.2 Acceptance #2 — Untitled UI v8.0 Kit Divergence Detector & Swatch `212423`

Log: [`plans/reports/agy-1464-fill-style-override-evidence/kit-detect-summary.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/plans/reports/agy-1464-fill-style-override-evidence/kit-detect-summary.log)

```text
=== BEFORE (detect-v075.tsv.gz / origin/main) ===
('effect', 'false', 'true', 'false') 7
('effect', 'true', 'false', 'false') 331
('fill', 'false', 'true', 'false') 3500
('fill', 'false', 'true', 'true') 139
('fill', 'true', 'false', 'false') 117
('fill', 'true', 'true', 'false') 2
('stroke', 'false', 'true', 'false') 8176
('stroke', 'false', 'true', 'true') 2

=== AFTER (jangtrinh/agy-1464-fill-style-override) ===
('effect', 'false', 'true', 'false') 7
('effect', 'true', 'false', 'false') 331
('fill', 'false', 'true', 'false') 3500
('fill', 'false', 'true', 'true') 139
('stroke', 'false', 'true', 'false') 8176
('stroke', 'false', 'true', 'true') 2
```

- `Fills` or `Stroke` overrides diverging via style (`override=true, via_style=true`): **`119` (`117 + 2` fill, `0` stroke) → `0` (`0` fill, `0` stroke)**.
- Frame `1137` swatch `212423` (`1525:273952`):
  - `style_refs`: `{"fill": "1272:5915"}` (`Gradient/Neutral/600 -> 500 (90deg)`)
  - `overrides`: `{Fills}`
  - Raw and resolved gradient stops: `#525252 → #737373` (`r=0.32156864, g=0.32156864, b=0.32156864` → `r=0.4509804, g=0.4509804, b=0.4509804`).

### 5.3 Acceptance #3 — Before/After Headless CPU Captures (Local Only, Not Committed)

- Colors → Gradients (Frame `1137`):
  - Before: `out/evidence/1464-gradients-1137-before.png` (`/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/out/evidence/1464-gradients-1137-before.png`)
  - After: `out/evidence/1464-gradients-1137-after.png` (`/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/out/evidence/1464-gradients-1137-after.png`)
- Landing page 13 → Desktop credit-card frame (Frame `89216`):
  - Before: `out/evidence/1464-landing-89216-before.png` (`/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/out/evidence/1464-landing-89216-before.png`)
  - After: `out/evidence/1464-landing-89216-after.png` (`/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/out/evidence/1464-landing-89216-after.png`)

### 5.4 Acceptance #4 — Formatting, Clippy, and Test Gates

Logs: [`plans/reports/agy-1464-fill-style-override-evidence/clippy.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/plans/reports/agy-1464-fill-style-override-evidence/clippy.log), [`plans/reports/agy-1464-fill-style-override-evidence/test-gates.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/plans/reports/agy-1464-fill-style-override-evidence/test-gates.log)

- `cargo fmt --all -- --check`: exit `0`
- `cargo clippy --workspace --all-targets --locked`: exit `0` (only `main`'s existing unneeded-`return` warnings in `designos-app`)
- `cargo test -p designos-fig --locked`: exit `0`
- `cargo test -p designos-doc --locked --lib`: exit `0`
- `cargo test --workspace --locked --lib`: exit `0`

### 5.5 Acceptance #5 — Draft PR & Cleanup

- Draft PR URL: https://github.com/jangtrinh/design-os-studio/pull/1484
- `out/cargo` deleted (`rm -rf out/cargo`).

## 6. Risks, Follow-Ups & Unresolved Questions

- **Risks:** None identified for fills or strokes. Multi-entry `symbolOverrides` targeting the same `guidPath` (where entry 1 sets `strokePaints` with `colorVar` and entry 2 detaches `styleIdForStrokeFill` with `4294967295:4294967295` without `strokePaints`, e.g. `1726:458793`) preserve the earlier entry's variable binding because `fills/*` and `strokes/*` bindings are only cleared when paints are replaced.
- **Follow-ups:** `styleIdForEffect` on instance self-overrides (`331` kit nodes where the instance node carries `styleIdForEffect` and its `symbolOverrides` self-entry carries stale `effects` without `styleIdForEffect`) remains left alone per Ruling #1 and would require reconciling the outer instance node's `styleIdForEffect` with its self-override entry if revisited.
- **Unresolved questions:** None.
