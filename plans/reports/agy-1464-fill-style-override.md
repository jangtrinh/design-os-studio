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

---

## Round 2

### R2.1 What Could NOT Be Verified

- **Figma desktop/cloud ground-truth pixel export of the Untitled UI v8.0 kit:** No Figma export of the kit exists on disk, and opening Figma is prohibited by task scope. Visual verification uses headless `designos-render` CPU renders (`kit_exp`) before (`64f2dd07`) and after (`HEAD`) across all affected frame families, cross-checked against adjacent text labels, contrast badges, and avatar row labels inside the kit itself.
- **Live GUI window screenshot:** Not captured because all Round-2 changes are in `designos-fig` import logic (`fig.rs` + `fig_library.rs`) and deterministic headless CPU frame renders (`out/evidence/1464-r2-*.png`) directly verify every affected frame family without touching WindowServer focus.

### R2.2 Corrections to Round-1 Report (`LOW-3` & `MINOR-2`)

1. **PR base commit (`LOW-3`):** Round 1 cited `7ce3a092` as the base commit, whereas PR #1484 branched from `64f2dd07` (3 commits after `7ce3a092`). All Round-2 base comparisons use `64f2dd07` (and `d4302880` for the Round-1 head).
2. **Workspace clippy with `-D warnings` (`LOW-3`):** Running `cargo clippy --workspace --all-targets --locked -- -D warnings` exits `101` due solely to 2 pre-existing `clippy::needless_return` lints on `origin/main` at [`crates/designos-app/src/agent_permissions.rs:1176`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-app/src/agent_permissions.rs#L1176) and [`:1194`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-app/src/agent_permissions.rs#L1194) (`#1417` had already removed the 3 `panels/components.rs` lints). Running `cargo clippy -p designos-fig --all-targets --locked -- -D warnings` exits `0` with `0` warnings.
3. **Swatch `NodeId` hydration order (`LOW-3`):** The `Gradient/Neutral/600 -> 500 (90deg)` swatch (`1525:273952`) on Page 4 Frame `1137` (`Gradients`) receives `NodeId(212423)` when Page 4 is hydrated first (as in `kit_exp`'s single-frame harness), and `NodeId(215654)` when all 113 pages are hydrated in page-index order (`0..112`, 864,057 total nodes).
4. **Effect overrides (`MINOR-2`):** Round-1 Ruling 1 stated *"apply it to `styleIdForEffect` only if the override carries effects"*. Leaving `styleIdForEffect` unchanged deviates from that wording because `331` instance self-overrides in the kit carry stale `effects` without repeating `styleIdForEffect` while the outer instance node itself links an effect style (e.g. `1256:132203` `Buttons/Social button` links `7395:8771`, which is `Shadows/shadow-xs-skeuomorphic`, not `Shadows/shadow-xs` as Round 1 wrote). Those `331` nodes already render as pre-existing hybrids on `main` (`kit_detect` reports `('effect', 'true', 'false', 'false') 331`). Per Round-2 `MINOR-2`, effects stay as built and the controller tracks the follow-up.

### R2.3 `MAJOR-1` — Option A (Style Wins Over Stale Override Paints) & Complete Kit Census

Per Round-2 `MAJOR-1`, **Option A replaces Round-1 Ruling 3**: when an override names a local style (`styleIdForFill` or `styleIdForStrokeFill`) that exists in the document (`lk.style_node(r)` is `Some`), the style's paints and variable bindings win over the inline paints saved in the override (unless `colorVar` is bound on the inline paint), matching `convert_node` ([`crates/designos-fig/src/fig.rs:1535-1542`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L1535-L1542)).

#### Why Option A Beats Option B and Option C

- **Option C (always keep override's inline `fillPaints`):** Fails Acceptance #2 on the `14` `Gradient/Neutral/*` and `Gradient/Skies/*` swatches on Page 4 Frame `1137` (`Gradients`), where the override's inline `fillPaints` hold stale pre-v8 Blue-Gray stops (`#475467 → #667085`, or stale `stopsVar` `#f5f5f5 → #e5e5e5` on `215717` and `215762`) while the named style `1272:5915` (`Gradient/Neutral/600 -> 500 (90deg)`) holds the true Neutral stops (`#525252 → #737373`) printed on the swatch card.
- **Option B (only replace inline paints when the style node has `bound_variables`):** Produces **2 broken hybrid avatars** (`NodeId(415527)` in Page 43 Frame `58648` and `NodeId(417278)` in Page 43 Frame `59012`), where the override links bound style `1237:1027` (`Avatar user square/Eduard Franz (neutral background)`, neutral backdrop `#e0e0e0` + Eduard Franz's portrait) while the inline `fillPaints` carry only a single `Image` paint for `Candice Wu`. Under Option B, the style's variable binding (`fills/0/color → Colors/Background/bg-tertiary` `#e0e0e0`) overwrites paint slot `0` (replacing the portrait image with a solid gray rectangle and no portrait on top), whereas the other `6` Eduard Franz avatars on Pages 42–43 (which link unbound style `1216:9` `#e5ddce + img`) would keep Candice Wu's face. Under **Option A**, all `8` Eduard Franz avatars on Pages 42–43 and all `70` `Avatar user square` rows on Page 13 Frame `21627` take both the backdrop and the portrait image from the named avatar style, so every row's portrait matches its printed name label (`Candice Wu`, `Demi Wilkinson`, `Drew Cano`, etc.).

#### Table 1 — All `202` Raw-Paint Replacements Across `96` Top-Level Frames (`64f2dd07` vs `HEAD`)

When an override carries both inline `fillPaints` and a valid `styleIdForFill` whose style paints differ from the inline cache, Option A replaces the stale inline paints with the named style's paints (`202` nodes across `96` top-level frames):

| Family / Named Style(s) | Style Bound? | Nodes | Frames | Inside C1 (`6` frames) | Outside C1 (`90` frames) | Raw Paint Before (`64f2dd07`) → After (`HEAD`) | Evidence Crop (`out/evidence/`) |
|---|---:|---:|---:|---:|---:|---|---|
| **`Neutral/900`** (`1:676`) on `Text` `"Label"` (`_Nav item base` / `_Footer link`) | Yes (`Colors/Neutral/900`) | `98` | `85` | `3` in `3` frames (`88899`, `89183`, `89194` on Page 84) | `95` in `82` frames (`81` mobile frames on Pages 79–90 `87152..90681` with 1 each + `14` in Page 99 Frame `97168` `Footer`) | `#101828` (stale Gray/900) → `#171717` (Neutral/900) | `1464-r2-label-87152-{before,after,crop}.png` |
| **`Avatar user square/* (color background)`** (`1031:41964`..`1031:42031`, `1216:9`) | No (`2` paints: solid `<bg>` + `img`) | `41` | `7` | `0` | `41` in `7` frames (`35` `Rect` `"Image"` in Page 13 Frame `21627` `Avatar users` + `6` `Frame` `"Avatar"` on Pages 42–43 frames `54868`, `55017`, `55227`, `55378`, `58797`, `59162`) | `img` (1-paint Candice Wu photo) → `<bg>,img` (2-paint tinted backdrop + matching user portrait) | `1464-r2-avatars-21627-{before,after,crop}.png` |
| **`Avatar user square/* (neutral background)`** (`1237:957`..`1237:1027`) | Yes (`fills/0/color` → `bg-tertiary` `#e0e0e0` + `img`) | `37` | `3` | `0` | `37` in `3` frames (`35` `Rect` `"Image"` in Page 13 Frame `21627` `Avatar users` + `2` `Frame` `"Avatar"` on Page 43 frames `58648`, `59012`) | `e6e6e6/1.00,img` (`35` in `21627`) or `img` (`2` in `58648`, `59012`) → `e0e0e0/1.00,img` (matching user portrait) | `1464-r2-avatars-21627-{before,after,crop}.png` |
| **`Gradient/Neutral/*` & `Gradient/Skies/*`** (`1272:5913`..`1272:5920`, `7942:72516`, `11255:6363`) | Yes (`stopsVar` → `_Primitives`) | `14` | `1` | `14` in `1` frame (Page 4 Frame `1137` `Gradients`) | `0` | Stale v7 Blue-Gray stops (e.g. `#475467 → #667085` on `212423`/`215654`) → true v8 Neutral/Sky stops (`#525252 → #737373`) | `1464-r2-gradients-1137-{before,after,crop}.png` |
| **`Gray/300`..`Gray/950`** (`1:492`, `1:498`, `1:500`, `1:506`..`1:514`, `1724:31215`) | Yes (`Colors/Gray (light mode)/*`) | `11` | `1` | `0` | `11` in `1` frame (Page 4 Frame `743` `Colors`: `8` `Frame` `"Swatch"` + `3` `Text` `"WCAG contrast"` with `Gray/600`) | Stale pre-v8 Gray (`#d2d6db`..`#090e15`, `#697586`) → v8 Gray (`#d1d5db`..`#030712`, `#4b5563`) | `1464-r2-colors-743-{before,after,crop}.png` |
| **`Base/Black`** (`800:25014`) on `Text` `"WCAG contrast"` | Yes (`Colors/Base/black`) | `1` | `1` | `0` | `1` in `1` frame (Page 4 Frame `743` `Colors`, on the `Gray/500` swatch card) | `#667085` (stale Gray/500 text on Gray/500 background) → `#000000` (`Base/Black`, readable contrast badge) | `1464-r2-colors-743-{before,after,crop}.png` |
| **Total Raw-Paint Replacements** | — | **`202`** | **`96`** | **`17` in `4` frames** | **`185` in `92` frames** | — | — |

#### Table 2 — All `308` Resolved-Paint Changes Across `99` Top-Level Frames (`64f2dd07` vs `HEAD`)

Across the entire kit, `308` nodes in `99` top-level frames change their **resolved** paints (`196` where both raw and resolved changed, plus `112` where raw paints were already right and relinking/detaching the style prevented `resolve_node` from clobbering them with the main component's `Gradient/Gray/600 -> 500` or `Base/White` style; `6` of the `202` raw-replaced `"Label"` nodes on dark-mode frames `88557`, `88565`, `88721`, `88899`, `89183`, `89194` resolve to `#f5f5f5` both before and after because `Neutral/900` maps to `#f5f5f5` in dark mode):

| Scope | Family / Component | Nodes | Frames | Resolved Paint Before (`64f2dd07`) → After (`HEAD`) | Cause (`raw_changed` / `relinked_only` / `detached`) | Evidence Crop (`out/evidence/`) |
|---|---|---:|---:|---|---|---|
| **Inside C1 (`6` frames)** | Page 4 Frame `1137` (`Gradients`) — `Vector` `"Swatch"` (`Gradient/*`) | `110` | `1` | Main component's `Gradient/Gray/600 -> 500 (90deg)` (`#4b5563 → #6b7280`) → each swatch's own `Gradient/*` (`108` relinked styles incl. `14` raw-replaced Neutral/Sky swatches + `2` detached custom gradients `214842`, `215622`) | `14` raw+relink, `94` relink, `2` detach | `1464-r2-gradients-1137-{before,after,crop}.png` |
| **Inside C1 (`6` frames)** | Page 84 Frames `88899`, `89183`, `89194`, `89205`, `89216` (`Landing page 13`) — `Content` (`1272:5920`), `Credit card mockup` (`1272:5919`), `Avatar` (`1033:618`) | `9` | `5` | Clobbered by main component's `Gradient/Gray/*` (`#1f2937 → #4b5563`, `#374151 → #1f2937`) or avatar `img` → dark Neutral gradients (`#171717 → #525252`, `#262626 → #404040`) and overridden avatar `img` | `9` relink (`raw` already matched) | `1464-r2-landing-89216-{before,after,crop}.png` |
| **Inside C1 (`6` frames)** | Page 84 Frames `88899`, `89183`, `89194` — `Text` `"Label"` (`Neutral/900` `1:676` in light-mode sub-tree) | `3` | `3` | `#101828` → `#171717` | `3` raw+relink | `1464-r2-label-87152-{before,after,crop}.png` |
| **Subtotal Inside C1** | — | **`122`** | **`6`** | — | — | — |
| **Outside C1 (`93` frames)** | Pages 79–90 (`81` mobile frames `87152..90681`, minus `3` dark-mode frames `88557`, `88565`, `88721` where dark mode resolves to `#f5f5f5`, plus `3` mobile frames with 2 `"Label"` nodes `87272`, `89620`, `90098`) + Page 99 Frame `97168` (`Footer`, `14` nodes) — `Text` `"Label"` (`Neutral/900` `1:676`) | `95` | `82` | `#101828` → `#171717` | `95` raw+relink | `1464-r2-label-87152-{before,after,crop}.png` |
| **Outside C1 (`93` frames)** | Page 13 Frame `21627` (`Avatar users`, `70` `Rect` `"Image"` nodes) + Pages 42–43 (`8` frames `54868`, `55017`, `55227`, `55378`, `58648`, `58797`, `59012`, `59162`, `8` `Frame` `"Avatar"` nodes) — `Avatar user square/*` | `78` | `9` | `1`-paint `img` (Candice Wu) or `#e6e6e6,img` → `<bg>,img` / `#e0e0e0,img` with each row's own user portrait | `78` raw+relink | `1464-r2-avatars-21627-{before,after,crop}.png` |
| **Outside C1 (`93` frames)** | Page 4 Frame `743` (`Colors`) — `8` `Frame` `"Swatch"` (`Gray/300`..`Gray/950`) + `3` `Text` `"WCAG contrast"` (`Gray/600`) + `1` `Text` `"WCAG contrast"` (`Base/Black`) | `12` | `1` | Stale v7 Gray (`#d2d6db`..`#090e15`, `#697586`) and low-contrast `#667085` badge → v8 Gray (`#d1d5db`..`#030712`, `#4b5563`) and `#000000` badge | `12` raw+relink | `1464-r2-colors-743-{before,after,crop}.png` |
| **Outside C1 (`93` frames)** | Page 34 Frame `41530` (`Forgot password`, `NodeId(285620)`) — `"Back to log in"` button `Text` `"Text"` relinked from main's `Base/White` (`800:25008`) to `Brand/700` (`159:659`) | `1` | `1` | `#ffffff` (invisible white text on white page, clobbered by main's `Base/White` style) → `#6941c6` (`Brand/700` purple link text matching adjacent arrow icon) | `1` relink (`raw` was already `#6941c6`) | `1464-r2-forgot-password-41530-{before,after,crop}.png` |
| **Subtotal Outside C1** | — | **`186`** | **`93`** | — | — | — |
| **Total Resolved-Paint Changes** | — | **`308`** | **`99`** | — | **`196` raw+relink, `110` relink-only, `2` detach** | — |

### R2.4 `MINOR-1` — Self-Override Style-Link Preservation

- **Root cause:** In the Untitled UI kit, `28` `Buttons/Button` instances carry `styleIdForStrokeFill: 7394:108475` (`Gradient/skeuemorphic-gradient-border`) on the `INSTANCE` record itself and also have a self-targeted entry (`guidPath.guids.len() == 1` matching the instance's own `symbolID` or `guid`) in `symbolOverrides` that repeats `strokePaints` without repeating `styleIdForStrokeFill`. On `d4302880`, `convert_override` treated that self-override like a child detach (`_clearStrokeStyle: true`), clearing `style_refs["stroke"]` on those `28` instances.
- **Fix ([`crates/designos-fig/src/fig.rs:2261-2307`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2261-L2307)):** In `override_entries`, when `path.len() == 1 && (self_sym == Some(path[0]) || self_guid == Some(path[0]))` and the override entry carries `fillPaints` / `strokePaints` without `styleIdForFill` / `styleIdForStrokeFill`, clone the entry and restate the instance node's own `styleIdForFill` / `styleIdForStrokeFill` before calling `convert_override`.
- **Kit impact:** Style links cleared drops from `367` (on `d4302880`) to **`339`** (on `HEAD`), restoring all `28` `Buttons/Button` `Gradient/skeuemorphic-gradient-border` stroke style links with `0` resolved-paint changes.
- **Unit tests:** `instance_self_fill_override_without_style_keeps_instance_own_fill_style_link` (self-path via `symbolID`) and `instance_self_stroke_override_without_style_keeps_instance_own_stroke_style_link` (self-path via instance `guid`).

### R2.5 `MINOR-3`, `MINOR-4`, `LOW-1` & `LOW-2` — New Tests, Mutant Kills & Comment Hygiene

1. **8 New Tests in [`crates/designos-fig/tests/fig_library.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/tests/fig_library.rs) (all 8 RED on `d4302880`, all 25 GREEN on `HEAD`):**
   - `instance_self_fill_override_without_style_keeps_instance_own_fill_style_link` (`MINOR-1`)
   - `instance_self_stroke_override_without_style_keeps_instance_own_stroke_style_link` (`MINOR-1`)
   - `instance_fill_override_style_paints_beat_stale_inline_override_paints` (`MINOR-3` — kills **`M1`**)
   - `instance_stroke_override_style_paints_beat_stale_inline_override_paints` (`MINOR-3` — kills **`M5`**)
   - `instance_fill_override_style_wins_over_stale_inline_color_var_in_override` (`MINOR-3` — kills **`M11`**)
   - `instance_stroke_override_style_wins_over_stale_inline_color_var_in_override` (`MINOR-3` — kills **`M12`**)
   - `instance_stroke_same_path_multi_entry_override_preserves_color_var_and_detaches_style` (`LOW-1`)
   - `plain_node_with_local_fill_and_stroke_styles_skips_stale_inline_color_var` (`LOW-2`)
2. **Mutant Kill Verification ([`plans/reports/agy-1464-fill-style-override-evidence/r2-mutants.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/plans/reports/agy-1464-fill-style-override-evidence/r2-mutants.log)):**
   - `M0` (null control, no mutation): `25 passed; 0 failed`
   - `M1` (`fig.rs` `convert_override` fill branch uses inline `fillPaints` before `lk.style_node`): **KILLED** (`instance_fill_override_style_paints_beat_stale_inline_override_paints` + `instance_fill_override_style_wins_over_stale_inline_color_var_in_override` fail)
   - `M5` (`fig.rs` `convert_override` stroke branch uses inline `strokePaints` before `lk.style_node`): **KILLED** (`instance_stroke_override_style_paints_beat_stale_inline_override_paints` + `instance_stroke_override_style_wins_over_stale_inline_color_var_in_override` fail)
   - `M11` (`fig.rs` `bound_variables` drops `!fill_from_style` guard): **KILLED** (`instance_fill_override_style_wins_over_stale_inline_color_var_in_override` + `plain_node_with_local_fill_and_stroke_styles_skips_stale_inline_color_var` fail)
   - `M12` (`fig.rs` `bound_variables` drops `!stroke_from_style` guard): **KILLED** (`instance_stroke_override_style_wins_over_stale_inline_color_var_in_override` + `plain_node_with_local_fill_and_stroke_styles_skips_stale_inline_color_var` fail)
3. **Comment Hygiene (`MINOR-4` & `LOW-1`):**
   - Removed the `#1464: ` issue prefix from all 7 Round-1 test doc comments in [`crates/designos-fig/tests/fig_library.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/tests/fig_library.rs).
   - Added a code comment in `convert_override` ([`crates/designos-fig/src/fig.rs:2356-2361`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2356-L2361)) documenting why a style-only detach entry (`styleIdForFill` / `styleIdForStrokeFill` = `4294967295:4294967295` with no `fillPaints` / `strokePaints`) emits `_clearFillStyle` / `_clearStrokeStyle` without touching paints or `boundVariables` collected from an earlier `symbolOverrides` entry for the same `guidPath`.

### R2.6 Acceptance Verification Table

| # | Acceptance Requirement | Command | Key Output (`plans/reports/agy-1464-fill-style-override-evidence/`) | Verdict |
|---|---|---|---|---|
| 1a | All 8 new Round-2 tests **RED** on `d4302880` | `cargo test -p designos-fig --test fig_library --locked` (on `d4302880` code) | `test result: FAILED. 17 passed; 8 failed` (`r2-red-on-d4302880.log`) | **PASS** |
| 1b | All 13 child/plain tests **RED** on PR base `64f2dd07` | `cargo test -p designos-fig --test fig_library --locked` (on `64f2dd07` code) | `test result: FAILED. 12 passed; 13 failed` (`r2-red-on-64f2dd07.log`) | **PASS** |
| 1c | All 25 `fig_library` tests **GREEN** on `HEAD` | `cargo test -p designos-fig --test fig_library --locked` | `test result: ok. 25 passed; 0 failed` (`r2-green-on-head.log`) | **PASS** |
| 1d | Mutants `M1`, `M5`, `M11`, `M12` killed (`M0` passes) | Python mutation runner over `crates/designos-fig/src/fig.rs` | `M0: 25 passed; M1: 2 failed (KILLED); M5: 2 failed (KILLED); M11: 2 failed (KILLED); M12: 2 failed (KILLED)` (`r2-mutants.log`) | **PASS** |
| 2 | Kit detector `Fills`/`Stroke` `ov=true, via_style=true` divergences = `0` | `cargo test --release -p designos-fig --test kit_detect -- --ignored --nocapture` | `Fills/Stroke ov=true via_style=true count: 0` (`119` on `64f2dd07` → `0` on `HEAD`; `r2-kit-detect.log`) | **PASS** |
| 3a | Rustfmt check | `cargo fmt --all -- --check` | `fmt_exit=0` (`r2-clippy.log`) | **PASS** |
| 3b | Clippy `-D warnings` (`designos-fig` clean; workspace has only `main`'s 2 `agent_permissions.rs` lints) | `cargo clippy -p designos-fig --all-targets --locked -- -D warnings` & `cargo clippy --workspace --all-targets --locked -- -D warnings` | `clippy_fig_exit=0`; `clippy_ws_exit=101` (only `crates/designos-app/src/agent_permissions.rs:1176` & `:1194` `needless_return`; `r2-clippy.log`) | **PASS** |
| 3c | `designos-fig` & workspace `--lib` test gates | `cargo test -p designos-fig --locked` & `cargo test --workspace --locked --lib` | `exit_code=0` for both (`143` lib + `25` `fig_library` in `designos-fig`; all workspace `--lib` suites green; `r2-test-gates.log`) | **PASS** |
| 4 | Pushed, PR #1484 body updated, `git log --oneline origin/main..HEAD`, `out/cargo` deleted | `git log --oneline origin/main..HEAD` & `ls -d out/cargo` | `8d84743d fix(fig): preserve instance own paint style link on self-overrides (#1464)`, `59b666d8 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1464-fill-style-override`, `d4302880 fix(fig): instance fill and stroke overrides keep their own style, not the main component's (#1464)`; `ls: out/cargo: No such file or directory` | **PASS** |

### R2.7 Skills Loaded & Applied

- `/es:dev` → `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Applied red-first test verification on `d4302880` and `64f2dd07` before landing the `fig.rs` fix, followed by full test and clippy gate verification.
- `/es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Preserved zero-copy borrowing on the common `override_entries` path, cloning the `KiwiVal::Object` map only on the rare self-override branch (`path.len() == 1`) that carries `fillPaints`/`strokePaints` without a style key.
- `/es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Verified CPU reference rendering parity on all 6 affected kit frame families via `kit_exp` (`designos-render`) without perturbing `SceneIndex` or GPU interactive budgets.
- `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Tracked every acceptance criterion to a reproducible log artifact under `plans/reports/agy-1464-fill-style-override-evidence/` and local crop renders under `out/evidence/1464-r2-*.png`.
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): Used falsification-first mutation testing (`M0`, `M1`, `M5`, `M11`, `M12`) and exhaustive 864,057-node census comparison across Options A, B, and C to prove why Option A is required.

### R2.8 Round-2 Epistemic Ledger, Weakest Link, Assumptions & Unresolved Questions

- **[FACT]** Across all 113 pages (`864,057` hydrated nodes) of the Untitled UI v8.0 kit, Option A replaces stale inline override paints with the named style's paints on `202` nodes across `96` top-level frames and changes resolved paints on `308` nodes across `99` top-level frames (`122` inside the 6 C1 frames, `186` across `93` frames outside C1).
- **[FACT]** Applying `minor1-fix-trial.diff` in `override_entries` preserves the `28` `Buttons/Button` self-override `Gradient/skeuemorphic-gradient-border` stroke style links (`367` → `339` style links cleared across the kit) with `0` resolved-paint regressions.
- **[FACT]** All 8 new Round-2 tests in `crates/designos-fig/tests/fig_library.rs` fail on `d4302880`, pass on `HEAD`, and kill mutants `M1`, `M5`, `M11`, and `M12`.
- **Weakest link:** Visual ground-truth verification of the `186` non-C1 nodes relies on internal kit consistency (matching printed swatch hex labels, WCAG contrast badges, and avatar row names) rather than a pixel export from Figma Desktop.
- **Assumptions:** None.
- **Unresolved questions:** None.

Status: DONE
