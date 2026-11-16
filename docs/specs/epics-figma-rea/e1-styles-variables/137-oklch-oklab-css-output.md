# #137 Add OKLCH/OKLAB picker modes and emit oklch() and color(display-p3) CSS (E1-8)

| Field | Value |
|---|---|
| Issue | #137 (E1-8, P2). Owner ruling R3.9. The 2026-10-31 controller re-ruling (wave-1 README) puts the CSS half after #136, with authored colour per paint and Figma notation |
| Still real? | Partly. Picker modes, conversions and arrow stepping shipped (#1451 `bc555a68a`, #1459 via #1554 `481b0e7ea`). Open: CSS `oklch()` / `oklab()` for authored colours, and `color(display-p3 …)` |
| Size | Slice (a): M, about 8 h. Slice (b): S, about 2 h |
| Tier | Slice (a): Argon. It has a machine-checkable round trip and CSS strings, and Argon built #1451 and #1459. Slice (b): Argon |
| Depends on | (a): #136 slice i merged (2026-10-31 ruling: the CSS half comes after #136), #1664 merged (hot files) and Q4. (b): #136 slice i merged |
| Ready after owner answers? | (a) after Q4 and #136 slice i. (b) after #136 slice i |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

A designer picks a colour in OKLCH (or OKLAB) mode. Studio remembers that the colour was authored in OKLCH, and CSS / Dev Mode output prints `oklch(62.8% 0.26 29.23deg)` for it. The authored values survive a `.fig` round trip, so Figma also sees an OKLCH-authored colour. In a Display P3 document, other colours print as `color(display-p3 r g b)`. sRGB documents without authored colours print the same CSS as today.

## 2. Re-anchor (origin/main)

- FACT: `Paint { kind, opacity, visible, blend }` has no authored colour (`crates/designos-doc/src/paint.rs:443-448`). `GradientStop` is at `paint.rs:180`. There are 33 `Paint { kind` literal sites in 24 files and 79 `GradientStop {` sites in 36 files (grep count on origin/main), which is the blast radius of a new field.
- FACT (schema): `Paint.authoredColor = 44 : AuthoredColor` and `authoredStops = 45 : AuthoredColorStop[]` (`crates/designos-fig/src/figma_schema.txt:881-882`); `AuthoredColor { space, c0..c4 }` (`:495-501`); `ColorSpace { DEFAULT, SRGB, DISPLAY_P3, HSL, HSB, OKLAB, OKLCH, CMYK }` (`:486-494`); `Effect.authoredColor = 56` (`:1084`). Neither import nor export reads these fields: the only grep hits for `authoredColor` are in the schema.
- FACT: CSS colours come from one function, `css_color` (`crates/designos-export/src/util.rs:70-82`), which prints hex or rgba. Callers: `css.rs` (8), `tailwind.rs` (2), `html.rs` (2), `codegen.rs` and `design_context.rs` (1 each).
- FACT (Figma RE, `plans/reports/re-figma-261031-color-modes-typography-binds.md:44-50`). CSS rule order:
  1. a resolved variable gives its own value;
  2. an authored colour prints in its space: `oklch(<L*100>% <C> <hue*360>deg[ / a])`, `oklab(…)`, `hsl(…)`, `color(srgb …)`, `color(display-p3 …)`, with 4 decimals, alpha at 2 decimals and omitted when 1. HSB, CMYK and DEFAULT fall through to rule 3;
  3. a P3 document prints `color(display-p3 r g b[ / a])`;
  4. otherwise rgba or hex.

  The user cannot pick OKLCH as an output format.
- FACT: OKLCH/OKLAB conversions exist (`crates/designos-doc/src/color_space.rs:45-149`).

## 3. Scope and rules

**Slice (a): authored colour**
1. **Model.**
   - `Paint.authored: Option<AuthoredColor>` and `GradientStop.authored: Option<AuthoredColor>`, with `AuthoredColor { space: AuthoredSpace, c: [f32; 4], alpha: f32 }`. The serde default is `None` and is skipped. Builder: field layout must match the kiwi `c0..c4` order from step (a0).
   - Add `Paint::with_kind`-style constructors where they cut churn. Old files are unchanged.
2. **Which edits set it (scope per Q4; recommended answer: OKLCH and OKLAB only).**
   - An edit in the picker's OKLCH or OKLAB mode (fields, sliders, arrow steps) sets `authored` to that space with the typed values.
   - An edit in any other mode (hex, RGB, HSB, HSL, eyedropper, variable bind) clears `authored`. The displayed RGB is the clamp-converted value, as today.
3. **.fig round trip.** Import and export `authoredColor` / `authoredStops` for the spaces in scope, plus verbatim preservation of any other space (HSL, HSB, CMYK…). An imported Figma colour authored in HSL is then not lost (law 1, "nothing vanishes silently").
4. **CSS output.** In `css_color` and its callers, a paint or stop with `authored` in OKLCH/OKLAB prints `oklch(…)` / `oklab(…)` in Figma notation (rule 2). Everything else is unchanged. A resolved variable still wins (rule 1).
5. **Picker reopen.** A paint authored in OKLCH opens the picker in OKLCH mode with the authored numbers, not numbers re-derived from the clamped RGB.

a0. Before coding, read one Figma-written fixture that has an OKLCH-authored fill (owner scratch file) and record the `c0..c4` mapping for OKLCH, OKLAB and HSL. Per the RE, L, C and hue are stored as fractions; the fixture confirms it.

**Slice (b): Display P3 CSS (after #136 slice i)**
6. In a document with `color_profile == DisplayP3`, a colour without authored space prints `color(display-p3 r g b[ / a])` with 4 decimals (rule 3). An authored `DISPLAY_P3` colour prints the same. sRGB documents are unchanged.

## 4. Files likely touched

- `crates/designos-doc/src/paint.rs`, `color_space.rs`
- `crates/designos-app/src/widgets/picker.rs` (set/clear authored)
- `crates/designos-fig/src/fig.rs`, `fig_export.rs`, `rest_import.rs` (REST `authoredColor`, if the REST shape has it; the builder checks)
- `crates/designos-export/src/util.rs`, `css.rs`, `tailwind.rs`, `html.rs`, `codegen.rs`, `design_context.rs`
- The 24 + 36 files with `Paint {` / `GradientStop {` literals (mechanical)

## 5. Acceptance criteria

- **AC-1** (a) Pick `oklch(62.8% 0.2577 29.23deg)` in the picker's OKLCH mode, and the CSS for the layer prints `background: oklch(62.8% 0.2577 29.23deg);`. Red on main, which prints hex.
- **AC-2** (a) Change the same fill in HEX mode, and the CSS prints hex again. `authored` is `None`.
- **AC-3** (a) `.fig` round trip of an OKLCH-authored fill and an OKLCH-authored gradient stop: the space and values come back within 1e-6, and an HSL-authored fill from a Figma fixture is preserved verbatim.
- **AC-4** (a) sRGB documents with no authored colours: the CSS, Tailwind, HTML and codegen output of every existing export test is byte-identical.
- **AC-5** (a) Reopening the picker on an OKLCH-authored paint shows the authored L, C and H, not the clamp round-tripped values. Desktop capture at 1440.
- **AC-6** (b) A P3 document fill `(1, 0, 0)` with no authored colour prints `color(display-p3 1 0 0)`. With alpha 0.5 it prints `color(display-p3 1 0 0 / 0.5)`.
- **AC-7** Conversions stay within 1/255 (already covered by #1451 tests; keep them green).

## 6. Test steps

`cargo test -p designos-doc color_space`, `cargo test -p designos-export`, `cargo test -p designos-fig` (new `fig_authored_color_round_trip.rs`), `cargo test -p designos-app --bin designos widgets::picker`, then the workspace gates and the 1440 capture.

## 7. Edge cases

- An out-of-gamut OKLCH value: the stored authored value is unchanged, and the rendered colour is clamped per channel (Figma rule, RE §A2). No warning.
- A colour bound to a variable: the variable's value wins in CSS (rule 1). Binding clears `authored`.
- Gradients: each stop has its own authored colour. CSS gradients print per-stop `oklch()`.
- Effects: `Effect.authoredColor = 56` is round-tripped verbatim in slice (a). CSS shadow colours follow the same rule if the effect colour has `authored`. The builder records whether the effect colour picker sets it.
- Copy/paste and duplicate keep `authored`, because it is plain data on the paint.

## 8. Non-goals

- An OKLCH output-format toggle in Dev Mode (dropped in the 2026-10-31 ruling; Figma has none).
- Parsing `oklch()` text typed into the CSS row (wave-2 follow-up).
- Gamut mapping.

## 9. Figma parity

Checked: RE §A1–A3 (rule order, notation, clamp). Needs a live check: step a0 (`c0..c4` mapping).

## 10. Builder rules

Two PRs. (a) goes on the hot-file queue after #136 slice i, as the 2026-10-31 ruling requires. Its PR uses `Refs #137`. (b) uses `Closes #137`.
