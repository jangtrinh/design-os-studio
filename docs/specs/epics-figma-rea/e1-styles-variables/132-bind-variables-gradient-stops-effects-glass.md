# #132 Bind variables to gradient stops, effect parameters and glass parameters (E1-5a)

| Field | Value |
|---|---|
| Issue | #132 (E1-5a, P1). Milestone on GitHub: Backlog; body says v0.8 Variables everywhere |
| Still real? | Partly. Stop + effect binds shipped (#1425, `0c8941235`). Three items open, all verified below |
| Size | S for the two .fig items (about 3 builder hours). The glass item is blocked by #38 and is sized there |
| Tier | Argon (reversible; a `.fig` round-trip test can check it) |
| Depends on | #1664 merged (hot file `fig_export.rs`, see README §Hot files). The glass item also depends on #38 (open) |
| Ready after owner answers? | Yes. No owner question; the glass hold is the 2026-10-30 wave-1 ruling |

Code baseline: `origin/main@d22dad970`. Line numbers move; each one is cited with its symbol.

## 1. Outcome

Suppose a designer binds a variable to a shadow's X, Y, blur or spread on an instance that overrides its main's effects. Those bindings now survive a `.fig` save and reopen, like the colour binding already does. Bound gradient stops on an instance's overridden fill also come back after a reopen.

## 2. Re-anchor (origin/main)

- FACT: `.fig` export of an instance with overridden effects calls `encode_effect(e, cv, opacity)`, passing the colour and opacity bindings only. It is in the `OverrideField::Effects` arm, `crates/designos-fig/src/fig_export.rs:2246-2257`. `encode_effect` passes `None` for radius, spread, offsetX and offsetY (`fig_export.rs:2730-2732`). The non-override paths use `encode_effect_with_bindings` (`fig_export.rs:880`, `:1111`, `:2734-2741`). So the gap the 2026-11-02 status comment names is still on main.
- FACT: binding keys and scopes for effect radius, spread and offsets exist: `property_kind` and `scope_allows` in `crates/designos-doc/src/variables_apply.rs:30-34` and `:64-68`.
- FACT: override paints are written with bindings when a fill variable is present: `has_fill_var`, `fig_export.rs:2201`. `encode_paint_with_bindings` writes `stopsVar` (`fig_export.rs:2595`, `:2715`). No test covers re-importing bound stops on an overridden instance paint. Per the 2026-11-02 status comment, the builder confirms this by grep in `crates/designos-fig/tests/`.
- FACT: the glass effect has no binding keys. `effect_field_key` accepts only `color|radius|spread|offsetX|offsetY` (`variables_apply.rs:134-140`). `GlassEffect` is at `crates/designos-doc/src/paint.rs:640`. #38 (glass parameter parity) is OPEN.
- INFERENCE: the override arm reads opacity through `cx.opacity_var_data(...)` (`fig_export.rs:468`), not through the plain alias that `encode_effect_with_bindings` uses. The fix must keep that opacity path, so a composed opacity binding does not regress.

## 3. Scope

1. In the `OverrideField::Effects` arm, write the radius, spread, offsetX and offsetY bindings of each effect, keeping the current colour and opacity encoding. Preferred implementation: one helper used by both the override and non-override paths. It takes an opacity source argument, so the override path keeps `opacity_var_data`.
2. Add a round-trip test for gradient stops bound on an instance that overrides its fill.
3. Glass parameters (light angle, intensity, refraction, depth, dispersion, frost): out of this slice. The issue stays open for this item until #38 lands; then it gets its own spec, built on #38's field names.

## 4. Files likely touched

- `crates/designos-fig/src/fig_export.rs` (`OverrideField::Effects` arm `:2246-2257`; `encode_effect*` `:2730-2741`)
- New test file `crates/designos-fig/tests/fig_instance_override_effect_and_stop_bindings.rs` (name describes behaviour, no issue number)

## 5. Acceptance criteria (red on main, green after)

- **AC-1** An instance overrides its main's effects with a drop shadow whose `effects/0/radius`, `/spread`, `/offsetX` and `/offsetY` are bound to four number variables. After `.fig` export and re-import, the instance carries the same four bindings. Each one resolves per mode: switch the mode, and the resolved radius equals that mode's value. This is red on main.
- **AC-2** The same instance keeps its `effects/0/color` and `effects/0/opacity` bindings byte-identical to main's output. This guards the opacity path.
- **AC-3** An instance overrides its fill with a linear gradient whose `fills/0/stops/1` is bound to a colour variable. After `.fig` export and re-import, the stop binding is present and resolves per mode.
- **AC-4** A document with no effect bindings exports byte-identical to main. Compare the kiwi bytes of a fixture.
- **AC-5** Mutation check: revert the fix and AC-1 fails.

## 6. Test steps

1. `cargo test -p designos-fig --test fig_instance_override_effect_and_stop_bindings`: red on base, green on head.
2. `cargo test -p designos-fig` (all), `cargo clippy -p designos-fig -- -D warnings`, `cargo fmt --check`.
3. Visual evidence: terminal capture of the red-then-green runs. This is not a UI change.

## 7. Edge cases

- An effect type without spread (blur effects): only the bound keys that exist are written. No empty `VariableData`.
- A binding to a variable that is missing from the document: skip it, the same as `encode_effect_with_bindings` (`resolve_var_guid` returns `None`).
- An instance with effects overridden and no bindings: unchanged output (AC-4).
- A library variable: the alias resolves through `cx.resolve_var_guid`, as today.

## 8. Non-goals

- Glass parameters (wait for #38).
- Picker or UI changes (shipped in #1425).
- The REST JSON exporter. File a follow-up if the builder finds the same gap there; do not fix it here.

## 9. Figma parity

- FACT: Figma scopes `EFFECT_FLOAT` and `EFFECT_COLOR` are in the schema enum and already enforced (`scope_allows`).
- No live Figma check is needed. The kiwi `Effect` fields that carry variables are the same ones the non-override path writes today.

## 10. Builder rules

PR body `Refs #132` (not `Closes`: glass stays open). Hot-file queue: one worker at a time in `fig.rs`/`fig_export.rs`/`kiwi.rs`/`rest_import.rs`, so start only after #1664 merges. Every figure in the report needs a citation (shared brief rule). Test names describe behaviour; no issue numbers.
