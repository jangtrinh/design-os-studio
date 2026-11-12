# #941 .fig font weight round trip: real style names, stale `wght` on open, `axisName` "Weight"

| Field | Value |
|---|---|
| Issue | #941 (epic #21, Backlog). Also absorbs the two leftover bullets of #891 (stale-axis files, `axisName` preference) — see `891-font-weight-writers-close-out.md`. |
| Code baseline | `origin/main@d22dad970`. Re-anchor every line against HEAD and report drift before coding. |
| Verdict | **Still real, all three items.** Item 3's open question is now answered from the real kit (below). |
| Size | M: 6–8 h builder |
| Tier | Claude Sonnet (touches the shared `TextStyle` model, `.fig` import and export, load migration; judgment on the name registry) |
| Touches | `crates/designos-doc/src/paint.rs` (`TextStyle` `:715-755`, `set_font_weight` `:760-765`), `crates/designos-doc/src/lib.rs` (`migrate_text` `:1570`), `crates/designos-fig/src/fig_export.rs` (`style_name_from_weight` `:2897-2918`, `append_text_style_fields` `:3087-3090`, run style writer near `:3197`), `crates/designos-fig/src/fig.rs` (`weight_from_style_name` `:2991` call sites), `crates/designos-fig/src/rest_import.rs` (`fontVariations` `:547-560`), `crates/designos-fig/src/export.rs:301` (REST writer, same axis fields). HOT FILES `fig.rs` / `fig_export.rs` / `rest_import.rs`. |
| Depends on | PRs #1664, #1671, #1672 (all touch `designos-fig`; land first, then rebase). No owner answer needed. |

## 1. Still real? (FACT unless tagged)

Measured on the Untitled UI PRO v8.0 kit copy (`~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`), raw kiwi dump via `cargo run -p designos-fig --example fig2json` (2026-11-06):

- **Item 1, style names.** `fontName.style` values the kit uses (count of nodes): Inter "Medium" 5,505, "Regular" 4,835, "Semi Bold" 2,770, "Bold" 216, "Italic" 65, "Medium Italic" 46, "Semi Bold Italic" 4, "Extra Bold" 4, "Bold Italic" 3, "Extra Light" 1, "Thin" 1, "Light" 1, "Black" 1; Euclid Circular A "SemiBold" 22 (no space); Suisse Intl "Book" 8; Sharp Grotesk "Medium 20" 1.
- Studio's writer builds the name from the weight only: `style_name_from_weight` (`fig_export.rs:2897`) returns "SemiBold", "ExtraBold", "ExtraLight" (no space) and never "Book" or "Medium 20"; `append_text_style_fields` writes it with an empty `postscript` (`:3087-3089`). `TextStyle` has no field that keeps the imported name (`paint.rs:715-755`: family, weight, italic only).
- INFERENCE: every Inter "Semi Bold" text (2,770 in the kit) is written back as "Inter SemiBold", a face name Inter does not have in Figma's font list, so Figma shows a missing font. **Not verified in Figma.app** (owner-manual step, AC-6).
- **Item 3, `axisName`.** The kit has `fontVariations` entries `{"axisName": "Weight", "axisTag": 2003265652, "value": 500.0}` and `600.0` (2 each), and `{"axisName": "Slant", "axisTag": 1936486004}` (2). 2003265652 = 0x77676874 = `wght`; 1936486004 = 0x736c6e74 = `slnt`. So real Figma files **do** put the display name in `axisName`.
- The importer prefers `axisName` (`rest_import.rs:551`: `fstr(fv, "axisName")...or_else(axisTag)`), and `set_axis` lowercases (`paint.rs:771-776`), so the axis is stored as `weight` / `slant`. Every reader asks for `wght` (`get_axis("wght")`: `lint/rules/contrast.rs:147`, `richtext.rs:114`, `panels/typography.rs:1060`), so these axes are never read. → Real bug.
- Studio's own writers put the tag in both fields (`fig_export.rs:3061` `axisName: tag`; `export.rs:301`), which is why `fig_roundtrip` tests pass.
- **Item 2, stale `wght`.** No load-time pass reconciles `font_weight` with a `wght` axis: `migrate_text` (`lib.rs:1570`) only migrates lists and grid alignment. A v0.6.0 file with `font_weight 700` and `wght 400` still draws regular (issue text; the renderer reads the axis first per `textsession.rs:1166` test comment).

## 2. User-visible outcome

1. A Figma file opened in Studio and saved as `.fig` opens in Figma with the same fonts: "Inter Semi Bold" stays "Inter Semi Bold", not a missing "Inter SemiBold".
2. Text using a variable font with a weight axis from Figma draws at the weight Figma shows.
3. A file saved by Studio 0.6.0 with a bold text that draws regular draws bold on open; one Ctrl+B toggles it.

## 3. Behaviour rules (decided; technical)

- R1 Keep the source name. Add `TextStyle.font_style_name: Option<String>` (`serde(default, skip_serializing_if = "Option::is_none")`). The `.fig` and REST importers fill it from `fontName.style` for styles and every run.
- R2 Invalidate on change. `set_font_weight` (`paint.rs:760`) and every italic toggle clear `font_style_name` when the weight or italic actually changes. Grep every `.italic =` writer outside tests and route it the same way (enumerate in the PR).
- R3 Writer order for `fontName.style`: (a) `font_style_name` when set; (b) else the name the same document already uses for that `(family, weight, italic)` (first match over styles and runs, built once per export); (c) else `style_name_from_weight` as today, except for family "Inter" (the only family with measured names: kit FACT above; also Studio's bundled default), which uses Figma's spaced names "Semi Bold", "Extra Bold", "Extra Light". Other families keep the unspaced table (Euclid Circular A measured "SemiBold"); their real names are reached through (a)/(b). A per-family table grows only from measured files.
- R4 Axis tag wins. Import reads `axisTag` first (4-byte big-endian → ASCII, trailing spaces trimmed) and uses `axisName` only when `axisTag` is absent or zero. Applies to `rest_import.rs:547-560` and any other `fontVariations` reader (grep `fontVariations` in `designos-fig/src`).
- R5 Repair on open. A load pass (in `migrate_text` or beside it, idempotent) renames legacy axis keys `weight`→`wght`, `slant`→`slnt`, `width`→`wdth` when no tag-named axis exists, then, for any style or run whose `wght` axis differs from `font_weight` by > 0.5 and whose `font_weight` was written by Studio ≤ 0.6.x, sets the axis to `font_weight`. INFERENCE to confirm in the PR: there is no file-version field that says "written by 0.6.0"; if none exists, the rule is "`font_weight` wins over a stale axis when the axis value is one of the 9 CSS steps and the two disagree" (a Figma-imported fractional axis such as 549.6 is never touched). Record the chosen rule.
- R6 Writers keep `axisName` = tag (unchanged; Figma reads the tag).

## 4. Acceptance criteria (red then green)

- **AC-1** Import → export of a synthetic `.fig` with Inter "Semi Bold" and Euclid "SemiBold" writes those exact strings back (unit test in `crates/designos-fig/tests/`). Red on main (writes "SemiBold" for both).
- **AC-2** Changing the weight of that Inter text to 700 in Studio writes "Bold"; to 600 writes "Semi Bold" (rule R3c). A Roboto text created in Studio at 600 (no source name, no other Roboto 600 in the file) writes "SemiBold" exactly as today (no regression; Roboto's real Figma name for 600 is UNKNOWN until read from a real file).
- **AC-3** Import of `fontVariations [{axisName:"Weight", axisTag:2003265652, value:600}]` yields `get_axis("wght") == Some(600.0)` and no `weight` key. Red on main.
- **AC-4** A v0.6.0-shaped native JSON fixture (`font_weight 700`, `variation_axes [["wght",400]]`) opens with `wght == 700`; one Ctrl+B (existing `TextCmd::Bold`, `textsession.rs:706`) sets weight 400 and axis 400. A fractional axis (549.6 with weight 550) is left unchanged.
- **AC-5** Kit: after a save and reopen, the multiset of `(family, style)` pairs in the raw dump equals the original's (script compares the two `fig2json` dumps; attach counts).
- **AC-6** Owner-manual: one saved kit frame with Inter "Semi Bold" opened in Figma.app shows no missing-font warning (screenshot). Flag as owner-manual, never claimed.

## 5. Test steps

1. `cargo test -p designos-fig` and `-p designos-doc` for the new tests; run each red on main first.
2. `cargo run --release -p designos-fig --example fig2json -- <kit>` before and after a `designos <kit> --export-fig out.fig`; compare `fontName` pairs and `fontVariations` (delete the multi-GB dumps after).
3. Owner-manual Figma check (AC-6).

## 6. Edge cases

- Mixed runs: each run keeps its own `font_style_name`; a range edit invalidates only the runs it changes.
- Shared text styles: the style definition keeps its name; a layer linked to it inherits it.
- Family swap (Inter → Roboto) clears `font_style_name` (the old name may not exist in the new family).
- REST JSON import (`fontPostScriptName` / `fontStyle` fields, if present) fills the same field; if absent, unchanged behaviour.
- `.pen` and SVG importers: leave `None` (no name source).

## 7. Non-goals

- Resolving real face names from installed font files (designos-fonts); R3b/R3c cover the measured cases.
- Writing `postscript` names (stays empty as today).
- Any UI change (the Weight picker keeps CSS weights).

## 8. Format facts checked

- `fontName {family, style, postscript}` and `fontVariations [{axisName, axisTag, value}]` read from the real kit dump (above). Tag encoding = big-endian ASCII, matches Studio's writer `u32::from_be_bytes` (`fig_export.rs:3061`).

## 9. User-visible details not settled by the issue

None left for the owner. File-format and migration choices recorded in §3.

## 10. Rollback

Field is optional and skipped when `None`; reverting the commit leaves files readable by older builds (unknown optional field ignored by serde default). The R5 repair writes back only on the next save.
