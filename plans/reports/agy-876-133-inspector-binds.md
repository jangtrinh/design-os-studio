# Report: Always-Visible Variable Bind Icon & Typography Bind Entry Points (`#876`, `#133`)

- **Branch:** `jangtrinh/agy-876-133-inspector-binds`
- **Base:** `origin/main` (`0c894123`, includes `#1425`)
- **Issues:** Closes `#876` · Part of `#133` (inspector half)

## 1. Summary of Changes

1. **R1 — Always-Visible Variable Bind Icon When Matching Variables Exist (`#876`):**
   - Added per-frame `CachedLibVars` + `has_matching_var(app, ctx, key0, kind, target_kind)` in [`crates/designos-app/src/panels/var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs) checking both `app.doc.variable_collections` and enabled subscribed library variables (`app.library_variables()`, cached once per egui pass) filtered by `scope_allows(&v.scopes, key0, target_kind)`.
   - Added `value_rect` and `style_value_rect` (and updated `numeric` / `style_numeric` closures to receive `(ui, val_r)`) so unbound fields reserve `theme::TARGET_MIN + theme::BORDER_WIDTH` (`25.0 px`) on the right when `has_matching_var(...)` is true, and keep full width when no matching variable exists.
   - Updated `draw_hover_button` so `hover_button` and `style_hover_button` render the `24x24` (`theme::TARGET_MIN`) hexagonal bind icon flush with `rect.right()` (`rect.right() - val_r.right() == 25.0 px`, `hit_r.left() - val_r.right() == 1.0 px`) at rest in `theme::ICON_QUATERNARY` with no background fill, and highlight with `theme::BG_TERTIARY` + `theme::ICON` on hover or when the picker is open. Kept `picker_stop_hover_button` hover-or-open-only inside the color picker stop list.
2. **R2 — Complete `#876` Coverage on Auto-Layout & Per-Side Stroke Weight Fields:**
   - Wrapped `al-counter-gap` in [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/properties.rs) (`wrap_lines_ui`) in `var_bind::numeric(app, ui, pane, id, r, &["counterAxisSpacing"], ...)`.
   - Wrapped individual per-side stroke weight fields in [`crates/designos-app/src/panels/stroke.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/stroke.rs) in `var_bind::numeric` with `&["strokeTopWeight"]`, `&["strokeRightWeight"]`, `&["strokeBottomWeight"]`, and `&["strokeLeftWeight"]`.
3. **R3 — Typography Variable Bind Entry Points (`#133` Inspector Half):**
   - Wired all 8 typography properties in [`crates/designos-app/src/panels/typography.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/typography.rs) when `!editing`:
     - `fontFamily` (`VariableKind::String`, `var_bind::pill` + `var_bind::value_rect` + `var_bind::hover_button`)
     - `fontStyle` (`VariableKind::String` primary bind key, with read-only pill fallback for legacy `fontWeight` `VariableKind::Number` bindings)
     - `fontSize` (`VariableKind::Number`, `var_bind::numeric`)
     - `lineHeight` (`VariableKind::Number`, `var_bind::numeric` when `LineHeight::Px` or `Auto`)
     - `letterSpacing` (`VariableKind::Number`, `var_bind::numeric` when `LetterSpacing::Px`)
     - `fills/0` (`VariableKind::Color` on the text color row, narrowing `text-hex` via `var_bind::value_rect`)
     - `paragraphIndent` and `paragraphSpacing` (`VariableKind::Number`, `var_bind::numeric` inside `details_popover`)
4. **R4 — Scope Filtering & Subscribed Library Variables:**
   - Verified `scope_allows` mapping across all typography, stroke, layout, corner, opacity, fill, and effect keys, including subscribed library variables via `CachedLibVars`.
5. **R5 — Linked Text Style Detach on Typography Variable Bind:**
   - In [`crates/designos-app/src/panels/var_bind.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/crates/designos-app/src/panels/var_bind.rs) (`bind_picker` `PickResult::Picked` for `BindTarget::Node`), when `is_text_style_key(&key0)` (`fontFamily`, `fontStyle`, `fontWeight`, `fontSize`, `lineHeight`, `letterSpacing`, `paragraphSpacing`, `paragraphIndent`) is bound on a node with a linked `StyleKind::Text` ref, `n.style_refs.remove(StyleKind::Text.ref_key())` is executed inside the same `Tx` (`"Bind Variable"` / `"Bind Library Variable"`), so one Undo restores both the text style ref and the previous binding state.
6. **R6 — Font Loading on `fontFamily` Variable Bind:**
   - Wrapped `PickResult::Picked` in `textsession::requesting_new_faces` and triggered `app.fonts.retry_family(&fam)` + `textsession::load_for_with(&app.doc, &app.fonts, new_faces)` when `key0 == "fontFamily"`.

## 2. Verification & Evidence Logs

All logs are archived under [`plans/reports/agy-876-133-inspector-binds-evidence/`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/):

| Gate | Command | Result | Evidence Log |
|---|---|---|---|
| RED Sim Tests (on `origin/main`) | `cargo test -p designos-app --bin designos --locked -- inspector_var_binds` | `8 failed; 0 passed` (expected RED) | [`00-red-inspector_var_binds.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/00-red-inspector_var_binds.log) |
| GREEN Sim Tests | `cargo test -p designos-app --bin designos --locked -- inspector_var_binds` | `8 passed; 0 failed` | [`01-green-inspector_var_binds.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/01-green-inspector_var_binds.log) |
| Existing Suites | `variables`, `effect_stop_binds`, `style_edit`, `typography_style_detach`, `typography` | `212 passed; 0 failed` | [`test-variables.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/test-variables.log), [`test-effect_stop_binds.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/test-effect_stop_binds.log), [`test-style_edit.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/test-style_edit.log), [`test-typography_style_detach.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/test-typography_style_detach.log), [`test-typography.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/test-typography.log) |
| Layout Gate | `cargo test -p designos-app --bin designos --locked -- layout_unchanged` | `20 passed; 0 failed` | [`test-layout_unchanged.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/test-layout_unchanged.log) |
| Rustfmt | `cargo fmt --all -- --check` | `PASS` (exit 0) | [`06-cargo-fmt.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/06-cargo-fmt.log) |
| Clippy | `cargo clippy --workspace --all-targets --locked` | Zero warnings in all touched files (5 pre-existing lints on `origin/main` in untouched `agent_permissions.rs` / `components.rs`) | [`07-cargo-clippy.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/07-cargo-clippy.log) |
| Workspace Lib Tests | `cargo test --workspace --locked --lib` | `1,264 passed; 0 failed` across all workspace crates | [`08-cargo-test-lib.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/08-cargo-test-lib.log) |
| Chrome Colour Literals | `python3 scripts/check_chrome_colour_literals.py` | `PASSED` (`199/199` baseline sites) | [`02-check_chrome_colour_literals.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/02-check_chrome_colour_literals.log) |
| Spacing Relationships | `python3 scripts/check_spacing_relationships.py` | `PASS` (no count rose above baseline) | [`03-check_spacing_relationships.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/03-check_spacing_relationships.log) |
| Python Script Tests | `python3 -m unittest discover -s scripts/tests` | `Ran 207 tests — OK` | [`04-scripts-tests.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/04-scripts-tests.log) |
| Zero Fake UI Gate | `python3 scripts/zero_fake_ui_gate.py` | `PASS (17/17 invariants satisfied)` | [`05-zero_fake_ui_gate.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-876-133-inspector-binds/plans/reports/agy-876-133-inspector-binds-evidence/05-zero_fake_ui_gate.log) |

## 3. Visual Verification (`out/evidence/*.png`, Uncommitted)

All 4 `1600x1000` screenshots were captured from an APFS clone (`cp -c`) of the Untitled UI PRO VARIABLES v8.0 kit (`$S/untitled.fig`) and a zero-variable starter document (`$S/no-vars.designos`) into `out/evidence/` (untracked, not committed) and visually inspected:

1. **`out/evidence/frame-selected-inspector.png` (`--select id:36`, `Label` Frame):**
   - Always-visible `24x24` hexagonal bind icons appear at rest (`theme::ICON_QUATERNARY`) to the right of Auto-layout `gap` (`14`), `Dimensions` (`W 137.2`, `H 24.6`), `Padding` (`5.6`, `2.8`), and `Appearance` (`100%`, `R 0`) with `1px` spacing and zero text/unit clipping.
2. **`out/evidence/text-selected-inspector.png` (`--select id:37`, `Change modes` Text):**
   - Typography section displays always-visible bind icons at rest on `fontFamily` (`Inter`), `fontStyle` (`Me...`), `fontSize` (`S 15.4`), `lineHeight` (`15.4`), `letterSpacing` (`0.08`), `Appearance` `opacity` (`100%`), and `Fill` hex (`FFFFFF`).
3. **`out/evidence/text-details-popover.png` (`--select id:37 --panel type-details`):**
   - `Type settings` popover shows always-visible bind icons on `Paragraph indent` (`0`) and `Paragraph spacing` (`0`), while non-bindable `List spacing` (`0`) retains full width.
4. **`out/evidence/no-variables-inspector.png` (zero-variable document, `03 Console - All...` Frame in Design mode):**
   - Right inspector shows zero bind icons and full-width (`64px`) value boxes on `gap`, `W`, `H`, `Padding`, `opacity`, and `R`.

## 4. Files Modified

- `crates/designos-app/src/panels/var_bind.rs`
- `crates/designos-app/src/panels/typography.rs`
- `crates/designos-app/src/panels/stroke.rs`
- `crates/designos-app/src/panels/properties.rs`
- `crates/designos-app/src/panels/effects.rs`
- `crates/designos-app/src/panels/style_edit.rs`
- `crates/designos-app/src/shell/tests.rs`
- `crates/designos-app/src/shell/tests/inspector_var_binds.rs`
- `crates/designos-app/tests/layout-baseline/*.layout.json` (inspector / style-edit / type-details regions only)
- `plans/reports/agy-876-133-inspector-binds.md` & `plans/reports/agy-876-133-inspector-binds-evidence/*.log`

