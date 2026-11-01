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

## Round 2 (Corrected in Round 3 per `MINOR-2`)

### R2.1 What Could NOT Be Verified

- **Figma desktop/cloud ground-truth pixel export of the Untitled UI v8.0 kit:** No Figma export of the kit exists on disk, and opening Figma is prohibited by task scope. Visual verification uses headless `designos-render` CPU renders (`kit_exp`) before (`64f2dd07`) and after (`HEAD`) across the affected frame families, cross-checked against adjacent text labels, contrast badges, and avatar row labels inside the kit itself.
- **Live GUI window screenshot:** Not captured because all changes are in `designos-fig` import logic (`fig.rs` + `fig_library.rs`).

### R2.2 Corrections to Round-1 Report (`LOW-3` & `MINOR-2`)

1. **PR base commit (`LOW-3`):** Round 1 cited `7ce3a092` as the base commit, whereas PR #1484 branched from `64f2dd07` (3 commits after `7ce3a092`). All base comparisons use `64f2dd07` (and `d4302880` for the Round-1 head).
2. **Workspace clippy with `-D warnings` (`LOW-3`):** Running `cargo clippy --workspace --all-targets --locked -- -D warnings` exits `101` due solely to 2 pre-existing `clippy::needless_return` lints on `origin/main` at [`crates/designos-app/src/agent_permissions.rs:1176`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-app/src/agent_permissions.rs#L1176) and [`:1194`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-app/src/agent_permissions.rs#L1194) (`#1417` had already removed the 3 `panels/components.rs` lints). Running `cargo clippy -p designos-fig --all-targets --locked -- -D warnings` exits `0` with `0` warnings.
3. **Swatch `NodeId` hydration order (`LOW-3`):** The `Gradient/Neutral/600 -> 500 (90deg)` swatch (`1525:273952`) on Page 4 Frame `1137` (`Gradients`) receives `NodeId(212423)` when Page 4 is hydrated first (as in `kit_exp`'s single-frame harness), and `NodeId(215654)` when all 113 pages are hydrated in page-index order (`0..112`, 864,057 total nodes).
4. **Effect overrides (`MINOR-2`):** Round-1 Ruling 1 stated *"apply it to `styleIdForEffect` only if the override carries effects"*. Leaving `styleIdForEffect` unchanged deviates from that wording because `331` instance self-overrides in the kit carry stale `effects` without repeating `styleIdForEffect` while the outer instance node itself links an effect style (e.g. `1256:132203` `Buttons/Social button` links `7395:8771`, which is `Shadows/shadow-xs-skeuomorphic`, not `Shadows/shadow-xs` as Round 1 wrote). Those `331` nodes already render as pre-existing hybrids on `main` (`kit_detect` reports `('effect', 'ov=true', 'via_style=true', 'via_var=false') 331`). Per Round-2 `MINOR-2`, effects stay as built and the controller tracks the follow-up.

### R2.3 `MAJOR-1` — Option A (Style Wins Over Stale Override Paints) & Complete Kit Census

Per Round-2 `MAJOR-1`, **Option A replaces Round-1 Ruling 3**: when an override names a local style (`styleIdForFill` or `styleIdForStrokeFill`) that exists in the document (`lk.style_node(r)` is `Some`), the style's paints and variable bindings win over the inline paints saved in the override (unless `colorVar` is bound on the inline paint), matching `convert_node` ([`crates/designos-fig/src/fig.rs:1535-1542`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L1535-L1542)).

#### Why Option A Beats Option B and Option C

- **Option B (override raw paints win, while the style link is still relinked):** Keeps the stale inline image on `41` unbound `Avatar user square` nodes and produces **2 broken hybrid avatars** (`NodeId(415527)` in Page 43 Frame `58648` and `NodeId(417278)` in Page 43 Frame `59012`), where the override links bound style `4835:409903` (`Avatar user square/Eduard Franz (neutral background)`, neutral backdrop `#e0e0e0` + Eduard Franz's portrait) while the inline `fillPaints` carry only a single `Image` paint. Under Option B, the style's `fills/0` colour variable binding lands on the override's lone image paint, turning both avatars into flat `#e0e0e0` squares with no portrait. Under **Option A**, all `70` `Avatar user square` rows in Frame `21627` and all `8` avatar frames on Pages 42–43 take both the backdrop and portrait from the named style so faces match row labels.
- **Option C (do not relink where the style resolves differently from the saved override paints — Round-1 Ruling 3 as written):** Leaves the `14` `Gradient/Neutral/*` and `Gradient/Skies/*` swatches on Page 4 Frame `1137` (`Gradients`) at their stale pre-v8 Blue-Gray stops (`#475467 → #667085` on `212423`/`215654`), failing Acceptance #2 (`#525252 → #737373`).

#### Table 1 — All `202` Raw-Paint Replacements Across `96` Top-Level Frames (`64f2dd07` vs `HEAD`)

When an override carries both inline `fillPaints` and a valid `styleIdForFill` whose style paints differ from the inline cache, Option A replaces the stale inline paints with the named style's paints (`202` nodes across `96` top-level frames; all `202` also change resolved paints):

| Family / Named Style(s) | Style Bound? | Nodes | Frames | Inside C1 (`6` frames) | Outside C1 (`90` frames) | Raw Paint Before (`64f2dd07`) → After (`HEAD`) | Evidence Crop (`out/evidence/`) |
|---|---:|---:|---:|---:|---:|---|---|
| **`Neutral/900`** (`1:676`) on `Text` `"Label"` (`_Nav item base` / `_Footer link`) | Yes (`Colors/Neutral/900`) | `98` | `85` | `3` in `3` frames (`88899`, `89183`, `89194` on Page 84) | `95` in `82` frames (`81` mobile frames on Pages 79–90 + `14` in Page 99 Frame `97168` `Footer`) | `#101828` (stale Gray/900) → `#171717` (Neutral/900) | `1464-r2-label-87152-{before,after,crop}.png` |
| **`Avatar user square/* (color background)`** (`1794:446949`..`4833:413759`, `1794:446969`) | No (`2` paints: solid `<bg>` + `img`) | `41` | `7` | `0` | `41` in `7` frames (`35` `Rect` `"Image"` in Page 13 Frame `21627` `Avatar users` + `6` `Frame` `"Avatar"` on Pages 42–43 frames `54868`, `55017`, `55227`, `55378`, `58797`, `59162`) | `img` (1-paint stale photo) → `<bg>,img` (2-paint tinted backdrop + matching user portrait) | `1464-r2-avatars-21627-{before,after,crop}.png` |
| **`Avatar user square/* (neutral background)`** (`4835:409883`..`4835:409903`) | Yes (`fills/0` → `bg-tertiary` `#e0e0e0` + `img`) | `37` | `3` | `0` | `37` in `3` frames (`35` `Rect` `"Image"` in Page 13 Frame `21627` `Avatar users` + `2` `Frame` `"Avatar"` on Page 43 frames `58648`, `59012`) | `e6e6e6/1.00,img` (`35` in `21627`) or `img` (`2` in `58648`, `59012`) → `e0e0e0/1.00,img` (matching user portrait) | `1464-r2-avatars-21627-{before,after,crop}.png` |
| **`Gradient/Neutral/*` & `Gradient/Skies/*`** (`1272:5913`..`1272:5920`, `7942:72516`, `11255:6363`) | Yes (`stopsVar` → `_Primitives`) | `14` | `1` | `14` in `1` frame (Page 4 Frame `1137` `Gradients`) | `0` | Stale v7 Blue-Gray stops (e.g. `#475467 → #667085` on `212423`/`215654`) → true v8 Neutral/Sky stops (`#525252 → #737373`) | `1464-r2-gradients-1137-{before,after,crop}.png` |
| **`Gray/300`..`Gray/950`** (`2685:402979`..`2685:402985`, `5016:372050`) | Yes (`Colors/Gray (light mode)/*`) | `11` | `1` | `0` | `11` in `1` frame (Page 4 Frame `743` `Colors`: `8` `Frame` `"Swatch"` `215013..215055` + `3` `Text` `"WCAG contrast"` `214996..215008` with `Gray/600`) | Stale pre-v8 Gray (`#d2d6db`..`#090e15`, `#697586`) → v8 Gray (`#d1d5db`..`#030712`, `#4b5563`) | `1464-r2-colors-743-{before,after,crop}.png` (`1464-r3-colors-743-*`) |
| **`Base/Black`** (`800:25014`) on `Text` `"WCAG contrast"` | Yes (`Colors/Base/black`) | `1` | `1` | `0` | `1` in `1` frame (Page 4 Frame `743` `Colors`, `NodeId(213607)` on the `Gray/500` swatch card) | `#667085` (stale Gray/500 text on Gray/500 background) → `#000000` (`Base/Black`, readable contrast badge) | `1464-r2-colors-743-{before,after,crop}.png` (`1464-r3-colors-743-*`) |
| **Total Raw-Paint Replacements** | — | **`202`** | **`96`** | **`17` in `4` frames** | **`185` in `92` frames** | — | — |

#### Table 2 — All `308` Resolved-Paint Changes Across `99` Top-Level Frames (`64f2dd07` vs `HEAD`)

Across the entire kit, `308` nodes in `99` top-level frames change their **resolved** paints: all **`202` raw-and-resolved** nodes from Table 1 plus **`106` resolved-only** nodes (where raw paints did not change and relinking/detaching the style prevented `resolve_node` from overwriting them with the main component's style):

| Scope | Family / Component | Nodes | Frames | Resolved Paint Before (`64f2dd07`) → After (`HEAD`) | Split (`202` raw+resolved / `106` resolved-only) | Visual Evidence (`out/evidence/`) |
|---|---|---:|---:|---|---|---|
| **Inside C1 (`6` frames)** | Page 4 Frame `1137` (`Gradients`) — `Vector` `"Swatch"` (`Gradient/*`) | `110` | `1` | Main component's `Gradient/Gray/600 -> 500 (90deg)` (`#4b5563 → #6b7280`) → each swatch's own `Gradient/*` (`14` raw+resolved Neutral/Sky swatches + `94` resolved-only relinked swatches + `2` resolved-only detached custom gradients `214842`, `215622`) | `14` raw+resolved, `96` resolved-only (`94` relink + `2` detach) | `1464-r2-gradients-1137-{before,after,crop}.png` |
| **Inside C1 (`6` frames)** | Page 84 Frames `88899`, `89183`, `89194`, `89205`, `89216` (`Landing page 13`) — `4` `Frame` `"Content"` (`1272:5919`), `4` `Instance` `"Credit card mockup"` (`1272:5920`), `1` `Frame` `"Image"` (`1033:618`) | `9` | `5` | Clobbered by main component's `Gradient/Gray/*` or avatar `img` → dark Neutral gradients (`#171717 → #525252`, `#262626 → #404040`) and overridden avatar `img` | `9` resolved-only | `1464-r2-landing-89216-{before,after,crop}.png` |
| **Inside C1 (`6` frames)** | Page 84 Frames `88899`, `89183`, `89194` — `Text` `"Label"` (`Neutral/900` `1:676`) | `3` | `3` | `#101828` → `#171717` | `3` raw+resolved | `1464-r2-label-87152-{before,after,crop}.png` |
| **Subtotal Inside C1** | — | **`122`** | **`6`** | — | **`17` raw+resolved, `105` resolved-only** | — |
| **Outside C1 (`93` frames)** | Pages 79–90 (`81` mobile frames `87152..90681`) + Page 99 Frame `97168` (`Footer`, `14` nodes) — `Text` `"Label"` (`Neutral/900` `1:676`; all `98` replaced `Label` nodes across the kit change resolved `#101828 → #171717`) | `95` | `82` | `#101828` → `#171717` | `95` raw+resolved | `1464-r2-label-87152-{before,after,crop}.png` |
| **Outside C1 (`93` frames)** | Page 13 Frame `21627` (`Avatar users`, `70` `Rect` `"Image"` nodes) + Pages 42–43 (`8` frames `54868`, `55017`, `55227`, `55378`, `58648`, `58797`, `59012`, `59162`, `8` `Frame` `"Avatar"` nodes) — `Avatar user square/*` | `78` | `9` | `1`-paint `img` or `#e6e6e6,img` → `<bg>,img` / `#e0e0e0,img` with each row's own user portrait | `78` raw+resolved | `1464-r2-avatars-21627-{before,after,crop}.png` |
| **Outside C1 (`93` frames)** | Page 4 Frame `743` (`Colors`) — `8` `Frame` `"Swatch"` (`Gray/300`..`Gray/950`, `215013..215055`) + `3` `Text` `"WCAG contrast"` (`Gray/600`, `214996..215008`) + `1` `Text` `"WCAG contrast"` (`Base/Black`, `213607`) | `12` | `1` | Stale v7 Gray (`#d2d6db`..`#090e15`, `#697586`) and low-contrast `#667085` badge → v8 Gray (`#d1d5db`..`#030712`, `#4b5563`) and `#000000` badge | `12` raw+resolved | `1464-r3-colors-743-{before,after,crop}.png` (stacks `y=380..540` badge + `y=2650..2830` Gray swatches; `27,037/27,037` changed pixels) |
| **Outside C1 (`93` frames)** | Page 34 Frame `41530` (`Forgot password`, `NodeId(246271)` in page-order hydration) — `"Back to log in"` button `Text` `"Text"` relinked from `1:1030` (`Base/White`) to `1:1098` (`Brand/700`) | `1` | `1` | `resTF` changes `#ffffff → #6941c6` (`rawTF` was already `#6941c6`), but **no visible pixel change exists** in the rendered frame (`0` differing pixels between base and head renders of `41530`) | `1` resolved-only | No visible change in rendered frame (`0` pixel diff) |
| **Subtotal Outside C1** | — | **`186`** | **`93`** | — | **`185` raw+resolved, `1` resolved-only** | — |
| **Total Resolved-Paint Changes** | — | **`308`** | **`99`** | — | **`202` raw+resolved, `106` resolved-only** | — |

### R2.4 `MINOR-1` — Self-Override Style-Link Preservation (Refined in Round 3 per `MAJOR-1`)

- **Root cause:** In the Untitled UI kit, `28` `Buttons/Button` instances carry `styleIdForStrokeFill` (`7391:365097` `Gradient/skeuemorphic-gradient-border`) on the `INSTANCE` record itself and also have a self-targeted entry (`guidPath.guids.len() == 1` matching the instance's own `symbolID` or `guid`) in `symbolOverrides` that repeats `strokePaints` without repeating `styleIdForStrokeFill`. On `d4302880`, `convert_override` emitted `_clearStrokeStyle: true` on those self entries, clearing `style_refs["stroke"]` on those `28` instances.
- **Fix ([`crates/designos-fig/src/fig.rs:2624-2636`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2624-L2636)):** In `override_entries`, after `convert_override(o, lk)` builds `set`, when `!derived && path.len() == 1` and `path[0]` matches the instance's `symbolID` or `guid`, for each `(style, slot, clear_key)` in `[("styleIdForFill", "fill", "_clearFillStyle"), ("styleIdForStrokeFill", "stroke", "_clearStrokeStyle")]`, if `o.get(style).is_none() && inst_obj.contains_key(style) && set.remove(clear_key).is_some()`, call `insert_style_override_link(&mut set, inst_obj, lk, style, slot, clear_key)` to restate the instance's own style link only where `convert_override` emitted a clear for that slot.
- **Pre-existing observation (`LOW-3` of Round 2 review):** On those `28` restored `Buttons/Button` instances, the restored style `Gradient/skeuemorphic-gradient-border` (`7391:365097`) is an unbound white gradient (`ffffff/0.12 → ffffff/0.00`) whereas the instances' raw `strokePaints` store solid `#7f56d9`, so the style chip names the gradient while the canvas draws purple (identical on base `64f2dd07`, Round 1, Round 2, and Round 3; tracked with the effects self-override follow-up).

### R2.5 `MINOR-3`, `MINOR-4`, `LOW-1` & `LOW-2` — Round-2 Tests, Mutant Kills & Comment Hygiene

1. **8 Tests Added in Round 2 in [`crates/designos-fig/tests/fig_library.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/tests/fig_library.rs):**
   - `instance_self_fill_override_with_paints_keeps_instance_style_link` (`MINOR-1` — red on `d4302880`; kills `R2a`, `R2b`, `R2c`, `R2e`)
   - `instance_self_stroke_override_with_paints_keeps_instance_style_link` (`MINOR-1` — red on `d4302880`; kills `R2a`, `R2b`, `R2c`, `R2e`)
   - `instance_override_with_color_var_and_named_style_keeps_bound_paints` (`MINOR-3` — red under mutant **`M1`**)
   - `instance_override_with_named_style_skips_stale_stop_and_opacity_bindings` (`MINOR-3` — red under mutant **`M5`**)
   - `instance_text_override_with_text_and_fill_styles_keeps_both_links` (`MINOR-3` — red under mutant **`M11`**)
   - `instance_text_fill_override_marks_text_style_override_field` (`MINOR-3` — red under mutant **`M12`**)
   - `instance_same_path_multi_entry_overrides_apply_in_order` (`LOW-1` — pins list-order override application)
   - `plain_node_with_local_fill_style_skips_stale_paint_variable_bindings` (`LOW-2` — red under mutant **`M5`**)
2. **Comment Hygiene (`MINOR-4` & `LOW-1`):**
   - Removed the `#1464: ` issue prefix from all 7 Round-1 test doc comments in [`crates/designos-fig/tests/fig_library.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/tests/fig_library.rs).
   - Added a code comment at [`crates/designos-fig/src/fig.rs:2339-2340`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig/src/fig.rs#L2339-L2340) documenting that `symbolOverrides` entries apply in list order so a later paints-only entry on the same `guidPath` clears an earlier style link on that path while non-paint entries keep it.

---

## Round 3

### R3.1 What Could NOT Be Verified

- **Figma ground-truth rendering for Option A and for the 28 `Buttons/Button` stroke self-overrides:** No Figma export of the Untitled UI v8.0 kit exists on disk, and opening Figma is prohibited by task rules. Both Option A (style paints winning over stale inline override paints) and the `28` `Buttons/Button` instances whose restored `Gradient/skeuemorphic-gradient-border` style is an unbound white gradient while their raw stroke is `#7f56d9` (`LOW-3`) cannot be compared against a live Figma render.
- **Interactive GUI edit of a component main inside the Studio window:** Skipped because Round 3 changes no rendered pixels at import (`0` raw or resolved paint differences vs Round 1 / Round 2); main-to-instance propagation (`sync_instances`) for an instance with an opacity-only self entry is verified at document level in `self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main`.

### R3.2 Summary of Round-3 Changes (`MAJOR-1`, `LOW-1`, `LOW-2`, `MINOR-2`, `LOW-3`)

1. **`MAJOR-1` (`crates/designos-fig/src/fig.rs:2630`):** Applied `fix-trial-self-entry-paints-only.diff` verbatim (`if o.get(style).is_none() && inst_obj.contains_key(style) && set.remove(clear_key).is_some()`). The self-entry guard now restates the instance's own fill or stroke style link only when `convert_override` actually emitted `_clearFillStyle` / `_clearStrokeStyle` for that slot (i.e., the self entry carried that slot's paints without a style key). Self entries that only set opacity, size, or component properties no longer add bogus `OverrideField::Fills` / `OverrideField::Stroke` flags, eliminating all `577` unintended override flag additions across the kit while keeping all `28` intended `Buttons/Button` stroke style link restorations.
2. **Three New Tests Ported from `review_spec_r2_1484.rs` (`crates/designos-fig/tests/fig_library.rs:1430-1542`):**
   - `self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main` — **RED** on `4df73b31`, **GREEN** on `HEAD`; verifies an opacity-only self entry adds neither `OverrideField::Fills` nor `OverrideField::Stroke` and follows main fill/stroke edits across `sync_instances`.
   - `self_entry_with_paints_only_for_fill_adds_no_stroke_override` — **RED** on `4df73b31`, **GREEN** on `HEAD`; verifies a fill-only self entry adds `OverrideField::Fills` and preserves the fill link without adding `OverrideField::Stroke`.
   - `self_entry_that_names_its_own_style_keeps_that_style` (`LOW-1`) — **GREEN** on `4df73b31` and `HEAD`, and **kills mutant `R2d`** (both `if inst_obj.contains_key(style)` and `if inst_obj.contains_key(style) && set.remove(clear_key).is_some()`).
3. **`LOW-2` (`crates/designos-fig/tests/fig_library.rs:1228-1428`):** Dropped the extra self entry and trailing `self fill style link` assertion from the 6 Round-2 tests (`instance_override_with_color_var_and_named_style_keeps_bound_paints`, `instance_override_with_named_style_skips_stale_stop_and_opacity_bindings`, `instance_text_override_with_text_and_fill_styles_keeps_both_links`, `instance_text_fill_override_marks_text_style_override_field`, `instance_same_path_multi_entry_overrides_apply_in_order`, `plain_node_with_local_fill_style_skips_stale_paint_variable_bindings`). Their red evidence is their respective mutant kills (`M1`, `M5`, `M11`, `M12`) in `r3-mutants.log`.
4. **`MINOR-2`:** Corrected all 11 factual items in `## Round 2` above and in the PR #1484 body, re-cropped `out/evidence/1464-r2-colors-743-{before,after,crop}.png` and `out/evidence/1464-r3-colors-743-{before,after,crop}.png` to include both changed bands in Frame `743` (`27,037 / 27,037` changed pixels across the `Base/Black` badge and all `8` `Gray/300`..`Gray/950` swatches + `3` `Gray/600` WCAG contrast texts), stated plainly that Frame `41530` (`Forgot password`, `NodeId(246271)`) has no visible pixel change (`0` differing pixels), and ran our own `kit_census` + `census_join.py` and `kit_detect` + `detect_summary.py` runs.
5. **`LOW-3`:** Documented in one line (R2.4 above and R3.5 below) that the `28` restored `Buttons/Button` stroke links (`Gradient/skeuemorphic-gradient-border`) name an unbound white gradient while their raw stroke paints are `#7f56d9` (rendering purple on base, r1, r2, and r3 alike), matching the self-override pattern of the effects follow-up issue.

### R3.3 Acceptance Verification & Pasted Outputs

#### Acceptance 1 — Red on `4df73b31`, Green on `HEAD`, and Mutant `R2d` Killed

- **1a. First two new tests RED on `4df73b31` (`plans/reports/agy-1464-fill-style-override-evidence/r3-red-on-4df73b31.log`):**
  ```text
  $ git checkout 4df73b31 -- crates/designos-fig/src/fig.rs
  $ env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-fig --test fig_library --locked
  running 28 tests
  ...
  test self_entry_that_names_its_own_style_keeps_that_style ... ok
  test self_entry_with_paints_only_for_fill_adds_no_stroke_override ... FAILED
  test self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main ... FAILED
  ...
  failures:

  ---- self_entry_with_paints_only_for_fill_adds_no_stroke_override stdout ----

  thread 'self_entry_with_paints_only_for_fill_adds_no_stroke_override' (61652108) panicked at crates/designos-fig/tests/fig_library.rs:1511:5:
  {Fills, Stroke}

  ---- self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main stdout ----

  thread 'self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main' (61652109) panicked at crates/designos-fig/tests/fig_library.rs:1476:5:
  an opacity-only self entry overrides no fill: {Opacity, Fills, Stroke}

  failures:
      self_entry_with_paints_only_for_fill_adds_no_stroke_override
      self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main

  test result: FAILED. 26 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
  ```

- **1b. All 28 `fig_library` tests GREEN on `HEAD` (`plans/reports/agy-1464-fill-style-override-evidence/r3-green-on-head.log`):**
  ```text
  $ env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-fig --test fig_library --locked
  running 28 tests
  ...
  test self_entry_with_paints_only_for_fill_adds_no_stroke_override ... ok
  test self_entry_that_names_its_own_style_keeps_that_style ... ok
  test self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main ... ok
  ...
  test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
  ```

- **1c. Mutant run including `R2d`, `M1`, `M5`, `M11`, `M12` (`plans/reports/agy-1464-fill-style-override-evidence/r3-mutants.log`):**
  ```text
  M0 null control => SURVIVED passed=28 failed=0 [] (16.1s)
  M1 convert_override: style wins even when own paints bind colorVar => KILLED passed=27 failed=1 [instance_override_with_color_var_and_named_style_keeps_bound_paints] (4.4s)
  M5 bound_variables: drop the stale-binding skip => KILLED passed=26 failed=2 [instance_override_with_named_style_skips_stale_stop_and_opacity_bindings, plain_node_with_local_fill_style_skips_stale_paint_variable_bindings] (3.0s)
  M11 insert_style_override_link: overwrite styles map instead of merge => KILLED passed=27 failed=1 [instance_text_override_with_text_and_fill_styles_keeps_both_links] (2.9s)
  M12 apply_paint_style_links: text fill override flags Fills not TextStyle => KILLED passed=27 failed=1 [instance_text_fill_override_marks_text_style_override_field] (2.6s)
  R2a self-entry guard removed (round-1 behaviour) => KILLED passed=25 failed=3 [instance_self_fill_override_with_paints_keeps_instance_style_link, instance_self_stroke_override_with_paints_keeps_instance_style_link, self_entry_with_paints_only_for_fill_adds_no_stroke_override] (3.6s)
  R2b self entry matched by symbolID only => KILLED passed=26 failed=2 [instance_self_fill_override_with_paints_keeps_instance_style_link, instance_self_stroke_override_with_paints_keeps_instance_style_link] (3.3s)
  R2c self entry matched by instance guid only => KILLED passed=25 failed=3 [instance_self_fill_override_with_paints_keeps_instance_style_link, instance_self_stroke_override_with_paints_keeps_instance_style_link, self_entry_with_paints_only_for_fill_adds_no_stroke_override] (3.1s)
  R2d guard ignores a style key the self entry names itself (if inst_obj.contains_key(style)) => KILLED passed=25 failed=3 [self_entry_that_names_its_own_style_keeps_that_style, self_entry_with_paints_only_for_fill_adds_no_stroke_override, self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main] (6.2s)
  R2d_clear guard drops o.get(style).is_none() check (if inst_obj.contains_key(style) && set.remove(clear_key).is_some()) => KILLED passed=27 failed=1 [self_entry_that_names_its_own_style_keeps_that_style] (3.3s)
  R2e guard keeps the emitted _clear*Style (set.contains_key(clear_key) without remove) => KILLED passed=25 failed=3 [instance_self_fill_override_with_paints_keeps_instance_style_link, instance_self_stroke_override_with_paints_keeps_instance_style_link, self_entry_with_paints_only_for_fill_adds_no_stroke_override] (3.1s)
  R2f guard restates link even when entry carries no paints (4df73b31 regression) => KILLED passed=26 failed=2 [self_entry_with_paints_only_for_fill_adds_no_stroke_override, self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main] (7.1s)
  ```

#### Acceptance 2 — Kit Census Rerun (`census_join.py` Against Review's r1 TSV) & Detector Rerun

- **2a. `kit_census` + `census_join.py` (`plans/reports/agy-1464-fill-style-override-evidence/r3-census-join.log`):**
  ```text
  $ out/cargo/release/examples/kit_census "$S/kit.fig" out/evidence/r3-census-head.tsv out/evidence/r3-styles-head.tsv
  $ gzip -f out/evidence/r3-census-head.tsv
  $ python3 /Users/jangtrinh/Products/design-os-studio/plans/reports/code-reviewer-261101-1710-pr-1484-r2-evidence/harness/census_join.py \
      /Users/jangtrinh/Products/design-os-studio/out/evidence/code-reviewer-261101-1000-pr-1484/census-head.tsv.gz \
      out/evidence/r3-census-head.tsv.gz \
      "r1 d4302880 -> r3 HEAD" \
      out/evidence/r3-census-diff-r1-r3.tsv
  == r1 d4302880 -> r3 HEAD: 864057 vs 864057 nodes; ids equal: True
  name/kind/src mismatches: 0
  rows with any change: 28
  per column: {'sS': 28}
  resolved paint/effect changed: 0 nodes in 0 top frames
    inside C1: 0 in 0 frames; outside: 0 in 0 frames
  raw paints changed: 0 nodes in 0 frames
  override flag changed: 0
  style link changed: 28
      link 28 ('sS:-->7391:365097', 'Instance')
  ```
- **2b. `kit_detect` + `detect_summary.py` (`plans/reports/agy-1464-fill-style-override-evidence/r3-kit-detect.log`):**
  ```text
  $ out/cargo/release/examples/kit_detect "$S/kit.fig" out/evidence/r3-detect-head.tsv
  $ python3 /Users/jangtrinh/Products/design-os-studio/plans/reports/code-reviewer-261101-1710-pr-1484-r2-evidence/harness/detect_summary.py out/evidence/r3-detect-head.tsv
  == r3-detect-head.tsv (143966 rows)
     ('effect', 'ov=false', 'via_style=false', 'via_var=true') 148
     ('effect', 'ov=false', 'via_style=true', 'via_var=false') 4450
     ('effect', 'ov=false', 'via_style=true', 'via_var=true') 4641
     ('effect', 'ov=true', 'via_style=false', 'via_var=true') 7
     ('effect', 'ov=true', 'via_style=true', 'via_var=false') 331
     ('fill', 'ov=false', 'via_style=false', 'via_var=true') 28839
     ('fill', 'ov=true', 'via_style=false', 'via_var=true') 3639
     ('stroke', 'ov=false', 'via_style=false', 'via_var=true') 93733
     ('stroke', 'ov=true', 'via_style=false', 'via_var=true') 8178
     Fills/Stroke override rows diverging via style: 0 []
  ```

#### Acceptance 3 — Formatting, Clippy, and Test Gates

- **3a. `cargo fmt` & `cargo clippy` (`plans/reports/agy-1464-fill-style-override-evidence/r3-clippy.log`):**
  ```text
  === cargo fmt --all -- --check ===
  fmt_exit=0
  === cargo clippy -p designos-fig --all-targets --locked -- -D warnings ===
      Checking designos-fig v0.7.6 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1464-fill-style-override/crates/designos-fig)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.62s
  clippy_fig_exit=0
  === cargo clippy --workspace --all-targets --locked -- -D warnings ===
  error: unneeded `return` statement
      --> crates/designos-app/src/agent_permissions.rs:1176:9
  error: unneeded `return` statement
      --> crates/designos-app/src/agent_permissions.rs:1194:9
  clippy_ws_exit=101
  ```
- **3b. `cargo test -p designos-fig --locked` & `cargo test --workspace --locked --lib` (`plans/reports/agy-1464-fill-style-override-evidence/r3-test-gates.log`):**
  ```text
  === cargo test -p designos-fig --locked ===
  ...
  running 143 tests (unittests src/lib.rs) ... test result: ok. 143 passed; 0 failed
  running 28 tests (tests/fig_library.rs) ... test result: ok. 28 passed; 0 failed
  (45 test binaries total: 509 passed; 0 failed)
  test_fig_exit=0
  === cargo test --workspace --locked --lib ===
  designos-app: 0 passed; designos-diff: 13 passed; designos-doc: 708 passed; designos-eval: 1 passed;
  designos-export: 153 passed; designos-fig: 143 passed; designos-fonts: 30 passed; designos-gpu: 4 passed;
  designos-library: 37 passed; designos-mcp: 322 passed; designos-render: 56 passed; designos-svg: 1 passed;
  designos-webcapture: 8 passed (total: 1,476 passed; 0 failed)
  test_ws_lib_exit=0
  ```

#### Acceptance 4 — Pushed, PR #1484 Body Updated, `git log`, and `out/cargo` Deleted

```text
$ git log --oneline origin/main..HEAD
50a7daf7 fix(fig): restate instance self-override paint style link only when paints clear it (#1464)
4df73b31 docs(reports): add Round 2 census and verification for #1464
8d84743d fix(fig): preserve instance own paint style link on self-overrides (#1464)
59b666d8 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1464-fill-style-override
d4302880 fix(fig): instance fill and stroke overrides keep their own style, not the main component's (#1464)

$ rm -rf out/cargo && ls -d out/cargo 2>&1 || true
ls: out/cargo: No such file or directory
```

### R3.4 Skills Loaded & Changes Made Because of Them

- `/es:dev` → `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Ran the 3 new `fig_library` tests against `4df73b31` first to record red evidence (`r3-red-on-4df73b31.log`) before applying `fix-trial-self-entry-paints-only.diff`.
- `/es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Kept the `override_entries` self-entry check in-place on `set` (`set.remove(clear_key).is_some()`) with zero extra map allocations or clones.
- `/es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Checked pixel diffs on full CPU frame renders (`743` and `41530`) to verify that `1464-r3-colors-743-crop.png` captures all `27,037` changed pixels across both vertical bands in Frame `743` and that Frame `41530` has `0` changed pixels.
- `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Executed our own independent `kit_census` + `census_join.py` and `kit_detect` + `detect_summary.py` runs against the APFS kit clone rather than reusing prior logs.
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`): Added `SelfSentinel` alongside `SelfOwnKey` in `self_entry_that_names_its_own_style_keeps_that_style` after falsification analysis showed that under the new `&& set.remove(clear_key).is_some()` guard, dropping only `o.get(style).is_none() &&` (`R2d_clear`) is only reachable when `o.get(style)` is an explicit detach sentinel (`4294967295:4294967295`), killing both `R2d` and `R2d_clear`.

### R3.5 Epistemic Ledger, Weakest Link, Assumptions & Unresolved Questions

- **[FACT]** On `4df73b31`, `override_entries` (`crates/designos-fig/src/fig.rs:2629-2633`) restated the instance's own style link whenever `o.get(style).is_none() && inst_obj.contains_key(style)`, adding `577` `OverrideField::Fills` / `OverrideField::Stroke` flags (`507` `ovS` `0→1`, `70` `ovF` `0→1`) on kit instances whose self entries carried no paints for that slot.
- **[FACT]** Requiring `&& set.remove(clear_key).is_some()` (`fix-trial-self-entry-paints-only.diff`) reduces `r1 d4302880 -> r3 HEAD` census differences to `28` rows (`style link changed: 28`, `override flag changed: 0`, `raw paints changed: 0`, `resolved paint/effect changed: 0`), while `kit_detect` reports `0` `Fills`/`Stroke` override rows diverging via style across `143,966` rows (`r3-census-join.log`, `r3-kit-detect.log`).
- **[FACT]** `self_entry_without_paints_adds_no_paint_override_and_the_copy_follows_its_main` and `self_entry_with_paints_only_for_fill_adds_no_stroke_override` fail on `4df73b31` (`26 passed; 2 failed`) and pass on `HEAD` (`28 passed; 0 failed`), and `self_entry_that_names_its_own_style_keeps_that_style` kills both `R2d` and `R2d_clear` (`r3-mutants.log`).
- **[ASSUMPTION]** Under Option A, when an override entry in a `.fig` archive carries both a valid local `styleIdForFill` / `styleIdForStrokeFill` and differing inline `fillPaints` / `strokePaints` (without `colorVar`), Figma displays the named style's paints rather than the inline paints saved on the override entry (`202` kit nodes).
- **[ASSUMPTION]** On the `28` `Buttons/Button` instances where the instance record links `Gradient/skeuemorphic-gradient-border` (`ffffff/0.12 → ffffff/0.00`) and a self entry restates `#7f56d9` `strokePaints` without `styleIdForStrokeFill` (`LOW-3`), preserving the style link while keeping the raw stroke unchanged matches base `64f2dd07` until self-override style vs raw paint precedence is unified with the effects follow-up issue.
- **Weakest link:** No Figma ground-truth export exists to confirm whether Figma renders the named style or the inline paints on the `202` Option-A nodes and the `28` `Buttons/Button` stroke self-overrides.
- **Unresolved questions:** None blocking this PR; Figma truth for Option A and for the `28` gradient-vs-purple `Buttons/Button` strokes (`LOW-3`) remains unverified without a Figma export.

Status: DONE
