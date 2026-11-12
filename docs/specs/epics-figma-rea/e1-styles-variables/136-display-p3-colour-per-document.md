# #136 Store and render Display P3 colour per document (E1-7)

| Field | Value |
|---|---|
| Issue | #136 (E1-7, P2). Owner ruling R3.9 (2026-10-17): store and render Display P3 per document; sRGB files stay bit-exact |
| Still real? | Yes. No colour profile in the document model, renderer, GPU pipeline or PNG writer (evidence below) |
| Size | XL: about 24–32 builder hours in 4 slices. Slice i: S, 4 h. Slice ii-a: M, 8 h. Slice ii-b: M/L, 10 h. Slice iii: S, 3 h. Slice iv (per Q2): M, 6 h |
| Tier | Claude Sonnet for every slice. Render, GPU and CLAUDE.md §4 invariants are sensitive paths |
| Depends on | REA step 0 (E1-e); #1664 merged (slice i touches `fig.rs`/`fig_export.rs`). Unblocks #137 slice (b) |
| Ready after owner answers? | Slices i–iii are ready after step 0. Slice iv waits on Q2 |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

- A `.fig` file in Display P3 opens in Studio. On a P3 display it shows the same saturated colours Figma shows. It saves back as a P3 file.
- PNG exports of a P3 document carry a Display P3 colour tag, so viewers show them correctly.
- sRGB documents look and save exactly as before, bit for bit.

## 2. Re-anchor (origin/main)

- FACT: `Color` is documented as "Straight (non-premultiplied) sRGB" (`crates/designos-doc/src/paint.rs:4-12`). The document has no colour-profile field.
- FACT: `documentColorProfile` is in the kiwi schema as `NodeChange.documentColorProfile = 349 : DocumentColorProfile` (`crates/designos-fig/src/figma_schema.txt:1597`), with `enum DocumentColorProfile { LEGACY=0, SRGB=1, DISPLAY_P3=2 }` (`:2880-2883`). Import keeps it only as per-node metadata text (`crates/designos-fig/src/fig.rs:1200`). The DOCUMENT node itself is skipped (`fig.rs:1532`), so the document-level value may be lost today. INFERENCE; the builder proves it with a fixture in slice i.
- FACT: the GPU canvas format is `Rgba8Unorm` (`crates/designos-gpu/src/pipelines.rs:6`, `crates/designos-app/src/canvas/gpu.rs:1634`). There is no colour-space or `CGColorSpace` handling anywhere in `designos-app` or `designos-gpu` (grep on origin/main).
- FACT: PNG export is `encode_png_pixmap` (`crates/designos-export/src/raster.rs:177`), with no colour chunk.
- FACT: a P3 → sRGB conversion exists for the DTCG importer only (`display_p3_to_srgb`, `crates/designos-fig/src/variables_dtcg.rs:2049`). Slice ii-a moves it to `designos-doc::color_space` and reuses it (DRY).
- FACT (Figma RE, `plans/reports/re-figma-261031-color-modes-typography-binds.md` §A2–A3):
  - Figma converts to the target space and clamps each channel to [0,1], with no gamut mapping;
  - the file has a profile with the labels "sRGB" and "Display P3";
  - modals "Assign Display P3" and "Convert to Display P3", and the conversions `SRGB_TO_DISPLAY_P3` / `DISPLAY_P3_TO_SRGB`;
  - Dev Mode hint "This is a fallback sRGB value for browsers that do not support P3 colors."
- ASSUMPTION (to close in step 0): in a P3 document, Figma stores each `Color` as Display P3 components (a Figma "Assign" keeps the numbers and changes the meaning). `LEGACY` behaves as sRGB.

## 3. Scope and rules

0. **REA step 0 (E1-e, about 2 h, `rea`)**:
   - (a) confirm the ASSUMPTION above, using a scratch Figma file in each profile and the same `#FF0000` fill, then compare kiwi `color`;
   - (b) where `documentColorProfile` sits (DOCUMENT node or canvas);
   - (c) what Figma does with images in a P3 doc (expected: not converted);
   - (d) whether `LEGACY` and `SRGB` render the same.

   Report: `plans/reports/re-figma-<date>-display-p3-documents.md`.
1. **Slice i: model and round trip.**
   - `Document.color_profile: ColorProfile { Legacy, Srgb, DisplayP3 }`, default `Legacy`, serde default and skipped when default.
   - `.fig` import and export of `documentColorProfile`.
   - `.designos` round trip.
   - Colour values are stored unchanged (raw components). The meaning comes from the document profile.
2. **Slice ii-a: correct colour on any display.**
   - When the profile is `DisplayP3`, every document colour is converted P3 → sRGB with a per-channel clamp before rasterising: solid fills, gradient stops, strokes, effect colours, text runs and variable-resolved colours.
   - The conversion sits at one shared point: the paint/colour conversion each renderer already uses (the builder locates the CPU `designos-render` and GPU `designos-gpu` `paintconv.rs` entry points). It is not scattered across draw calls.
   - For `Legacy`/`Srgb` the code path is untouched, so the output is bit-exact.
   - The CPU and GPU outputs match (headless `budget = None` pixel-diff parity, CLAUDE.md §4.3).
3. **Slice ii-b: wide gamut on P3 displays (macOS).**
   - When the document is P3 and the window's screen is wide-gamut, the canvas presents Display P3 values: the surface or layer colour space is Display P3, and egui chrome is converted so its look is unchanged.
   - Interactive and idle-refine paths keep their caps and the 0% idle CPU rule (CLAUDE.md §4.4).
   - Non-macOS platforms keep ii-a.
   - **AC-3 is owner-manual** (a human compares Studio and Figma side by side on a P3 display).
4. **Slice iii: export tags.**
   - PNG of a P3 document is written in P3 values with a Display P3 `iCCP` (or `cICP`) chunk. JPEG gets an ICC profile. SVG and PDF get `color(display-p3 …)` / an ICC output intent.
   - sRGB exports are byte-identical to main.
4b. **Dev Mode / CSS colour output**: in #137 slice (b), not here.
5. **Slice iv (per Q2): file colour profile setting.**
   - The File menu has "Color profile" → sRGB / Display P3, with Figma's two actions: "Assign" (keep numbers, change the meaning) and "Convert" (change the numbers, keep the look). Each is one undo step.
   - Copy per `design/ux-writing.md` (action ≤ 3 words).

## 4. Files likely touched

- `crates/designos-doc/src/lib.rs` (`Document`), `paint.rs` (doc comment), `color_space.rs`
- `crates/designos-fig/src/fig.rs`, `fig_export.rs`
- `crates/designos-render/src/` (paint conversion), `crates/designos-gpu/src/paintconv.rs`, `pipelines.rs`
- `crates/designos-app/src/canvas/gpu.rs` (surface, ii-b)
- `crates/designos-export/src/raster.rs` and the SVG/PDF writers
- For slice iv: `crates/designos-app/src/` (File menu)

## 5. Acceptance criteria

- **AC-1** (i) A `.fig` fixture with `documentColorProfile = DISPLAY_P3` opens with `color_profile == DisplayP3`, and after save and reopen the value is the same. Every `Color` component is bit-identical through the round trip.
- **AC-2** (i, ii-a, iii) sRGB and Legacy documents are bit-exact:
  - existing `designos-render` and `designos-gpu` pixel tests unchanged;
  - `.designos` and `.fig` bytes identical to main for the existing fixtures;
  - PNG bytes identical.
- **AC-3** (ii-b, OWNER-MANUAL) On a P3 Mac display, a P3 fixture with `color(display-p3 1 0 0)` looks the same in Studio and in Figma, judged in a side-by-side screenshot.
- **AC-4** (ii-a) On an sRGB render, a P3 red `(1,0,0)` renders as sRGB `(255, 0, 0)` after clamp. A P3 `(0.5,0.5,0.5)` grey renders within ±1/255 of the converted value. CPU equals GPU.
- **AC-5** (iii) A P3 document's PNG has a Display P3 colour chunk, and its pixel values are the P3 values. An sRGB document's PNG has no new chunk.
- **AC-6** (iv, if in scope) Assign keeps the numbers and Convert keeps the look; each is one undo step. 1440 capture.

## 6. Test steps

1. Slice i: `cargo test -p designos-fig` (new `fig_document_color_profile_round_trip.rs`) and `cargo test -p designos-doc`.
2. Slice ii-a: `cargo test -p designos-render`, `cargo test -p designos-gpu -- --skip large_file_compile_benchmarks`, plus a new CPU-equals-GPU test for a P3 fixture.
3. Slice ii-b: a GUI check by the controller when the Mac is idle (HIDIdleTime ≥ 300), plus the AC-3 owner check.
4. Slice iii: `cargo test -p designos-export`.

## 7. Edge cases

- Images inside a P3 document: per step 0 (c). Expected: image pixels are untouched and tagged by their own profile.
- Colours from a library file in a different profile: convert when the library value is copied in. The builder records the rule from step 0.
- Variables resolve to raw colour values and the document profile applies. DTCG import of `display-p3` values into a P3 doc keeps P3 numbers, not the sRGB conversion done today.
- The Studio UI chrome is never converted on sRGB displays.
- The OKLCH picker in a P3 document: the picker shows document-space values. #137 handles the display-p3 notation.

## 8. Non-goals

- CMYK, HLG and BT.2020 (the conversion enum lists HLG but no requirement asks for it).
- Gamut warnings: Figma has none (RE §A2).
- CSS `color(display-p3 …)` output (#137 b).

## 9. Figma parity

Checked: schema enums and RE strings (above). Needs a live check: step 0 (a)–(d), plus AC-3 on a P3 display.

## 10. Builder rules

Four PRs. Slice i is on the hot-file queue (after #135). Slices ii-a and ii-b touch no hot file and may run after slice i merges. `Refs #136` until the last slice. Visual evidence:
- ii-a: CPU and GPU renders of the P3 fixture next to main's;
- ii-b: the AC-3 photo or screenshot pair;
- iii: `pngcheck`/`exiftool` output.
