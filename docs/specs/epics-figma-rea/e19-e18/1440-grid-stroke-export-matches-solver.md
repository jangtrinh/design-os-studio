# #1440 Exported strokes inset grid content the way the layout solver does (CSS and Tailwind agree)

| Field | Value |
|---|---|
| Issue | #1440 (epic #1486) |
| Code baseline | `origin/main@d22dad970`. Re-anchor before coding. |
| Verdict | **Partly fixed.** Tailwind per-side weights (scene C2) now emit per-side classes (PR #1619 carried #1570). Still real: Center stroke (C1, 1 px, CSS and Tailwind disagree), strokes that take no layout space (N6d, 2 px), harness traceback on `outline-offset-[…]`. |
| Size | M: 5–7 h |
| Tier | Argon (Gemini 4 Argon): reversible, Chrome harness gives a numeric oracle (0.00 px). Only after owner question 8 fixes the stroke model. |
| Touches | `crates/designos-export/src/css.rs` (`border()` and the grid `box-sizing` gate), `crates/designos-export/src/tailwind.rs` (stroke block, `box-border` gate), `scripts/qa/grid_chrome/tailwind_css_map.py`, `scripts/qa/grid_chrome/probe_page.py`, `crates/designos-export/tests/grid_chrome_fixtures.rs` (new scenes) |
| Depends on | #1442 lands first (same `css.rs`/`tailwind.rs`/fixtures files). #1455 lands before both (touches `css.rs:136`, `tailwind.rs:370`). Open PR #1578 edits `tailwind.rs` (toolchain bump; trivial rebase). Owner question 8. |

## 1. Still real? (FACT on `d22dad970`)

- Solver stroke model: `AutoLayout::effective_padding` (`crates/designos-doc/src/node.rs:991-999`): only when `layout_version == 0 && stroke_included`, padding grows by `Stroke::layout_inset` (`paint.rs:522-533`): Inside = w, Center = w/2, Outside = 0, per side from `side_weights`. Version ≥ 1 or `stroke_included == false`: strokes never move children.
- CSS `border()` (`css.rs:356-399`): Inside with side weights → `border-width` per side (`:377-381`); Inside uniform → `border` (`:384`); Center/Outside → `outline` + `outline-offset: -w/2` for Center (`:385-391`). So a v0 included Center stroke insets 0 in CSS vs w/2 in the solver (C1: 1 px); any v1/not-included Inside stroke insets w in CSS vs 0 in the solver (N6d: 2 px).
- Tailwind stroke (`tailwind.rs:476-522`): Inside per side → `border-t-[…]` etc. (`:496-501`, C2 fixed); otherwise `border-{w}` plus, for Center/Outside, `outline-offset-[…]` (`:513-521`) — a border with an outline offset and no outline: disagrees with CSS.
- `box-sizing` gates: CSS `css.rs:125` and Tailwind `tailwind.rs:389-391` use `effective_padding`.
- Harness: `tailwind_token_to_decls` raises `ValueError("Unsupported Tailwind token …")` (`scripts/qa/grid_chrome/tailwind_css_map.py:156`), which aborts the whole run.

## 2. User-visible outcome

A developer pastes exported CSS or Tailwind for a stroked grid frame: children sit exactly where Studio draws them (0.00 px in Chrome) for Inside, Center and Outside strokes, uniform and per side, in both v0 (borders take space) and v1 frames. CSS and Tailwind describe the same stroke the same way.

## 3. Behaviour rules

- R1 (owner question 8 picks the model; recommended option A): content inset = solver inset. A stroke that takes layout space in the solver is emitted so CSS gives the same inset; a stroke that takes none is emitted so CSS gives none.
  - Option A (recommended): Inside/Center stroke that takes no space → `outline` with negative `outline-offset` (Inside: −w, Center: −w/2); per-side weights with no space → `box-shadow: inset` per side. v0 included Center → `border: w/2` + `outline: w/2` offset 0 (inner half takes space, outer half does not). Inside included keeps `border`.
  - Option B: keep `border` and subtract the stroke inset from `padding` where padding ≥ w; fall back to A when it is not.
- R2 Tailwind emits the same model as CSS for the same node (one shared decision helper in `codegen.rs` or `css.rs`, used by both; DRY).
- R3 Scope: auto-layout frames (grid and flex) — both use the same `effective_padding`. Non-auto-layout frames with absolute children: non-goal (see §7).
- R4 Harness: an unknown Tailwind token makes that scene a FAIL line (`FAIL <scene> unsupported token <tok>`), run continues, exit 1. Teach the map `outline`, `outline-[…]`, `outline-offset-[…]`, `shadow-[inset_…]` if R1 uses them.

## 4. Acceptance criteria (each new fixture red on `d22dad970`)

- AC-1 Add committed scenes to `grid_chrome_fixtures.rs`: C1 (v0 included, Center 2 px), C2 (v0 included, Inside sides 4/1/2/3), N6d (v1, Inside 2 px), plus Outside 2 px (v0 and v1) and Center per-side v1. Each scene runs in CSS and Tailwind mode.
- AC-2 `python3 scripts/qa/grid-chrome-compare.py --json-out <path>` reports every scene ≤ 0.5 px (target 0.00) in both modes, exit 0; log + `--screenshot` PNG saved under `../e19-e18/evidence/1440/`.
- AC-3 For every stroke scene, CSS and Tailwind (mapped through `tailwind_css_map.py`) produce the same computed border/outline/box-shadow widths (asserted in a Rust test comparing the two exporters' stroke decision for the same node).
- AC-4 Harness unknown-token case: a Python unit test feeds `foo-[1px]` → that scene FAILs, the other scenes still run, exit 1, no traceback.
- AC-5 Existing goldens updated only where R1 changes output; each changed golden listed in the PR with the reason. `cargo test -p designos-export` green.
- AC-6 Visual: one Chrome screenshot per stroke scene (harness PNG) attached.

## 5. Test steps

1. Add scenes (AC-1); run harness on `d22dad970`: expect C1 1 px, N6d 2 px fails, plus a traceback on C1's Tailwind (current behaviour) — first land AC-4 so the run completes.
2. Implement R1–R2; rerun; green.
3. `cargo test -p designos-export`; `cargo clippy -p designos-export --all-targets -D warnings`.

## 6. Edge cases

- Dashed strokes: `outline-style: dashed` works; `box-shadow` cannot dash → per-side dashed with no space falls back to `border` + padding compensation, or a comment; builder records which and measures.
- Gradient stroke Inside (`border-image`, `css.rs:395-398`): `border-image` needs `border`; keep `border` + padding compensation for gradients.
- Bound stroke weight variables (`css.rs:389`, `tailwind.rs:516`): offsets use `calc(-1 * var(…))` / `calc(-0.5 * var(…))`.
- Line and Text nodes keep their special cases (`css.rs:369-376`).
- `border-radius` with outline: Chrome ≥ 94 follows radius; fine for target browsers.

## 7. Non-goals

Absolute children inside a bordered non-auto-layout frame (CSS `border` also shifts absolutely positioned children); file a follow-up if the owner wants it. Native exporters' strokes.

## 8. Figma-parity facts

- FACT (code): Studio's solver follows Figma's `bordersTakeSpace` (`stroke_included`) for layout version 0 and never insets in version 1 (`node.rs:986-990` doc comment).
- Figma Dev Mode's own CSS for strokes: not checked; `[UNVERIFIED]`. If the owner wants Dev-Mode-identical text, check a stroked auto-layout frame in Figma Dev Mode (inspect panel CSS) for Inside/Center/Outside.

## 9. Undecided user-visible details

- Question 8: exported stroke model (outline/box-shadow vs border + padding compensation).

## 10. Builder tier

Argon after question 8. Machine oracle: harness 0.00 px + goldens.

## Epistemic ledger

- FACT: C2 fixed by #1619 (`tailwind.rs:493-501`); C1/N6d logic unchanged (`css.rs:383-391`).
- INFERENCE: C1 and N6d drifts still measure 1 px and 2 px (code path unchanged since the r3 review; not re-measured here — AC-1 step 1 re-measures).
- ASSUMPTION (medium): stroke model applies to flex frames too (same solver rule). Flips if the owner limits question 8 to grids.
