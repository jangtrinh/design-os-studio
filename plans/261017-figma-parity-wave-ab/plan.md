# Plan — Figma parity Wave A (P0) + Wave B (P1 small)

- Status: in progress (autonomous overnight run; owner pre-authorized all phases)
- Domain: COMPLEX (EPIC split into waves)
- Base branch: `figma-parity-wave-ab`, from `review-fixes-261016` @ f5f02bc (PR #1 still open)
- Spec: `plans/specs/figma-parity-2026/{brainstorm,usecases}.md`
- Evidence: `plans/261017-figma-gap-analysis/` and the REA reports (`rea-findings.md` in this folder)

## Seams under test (one per slice, highest existing boundary)
| Slice | Seam |
|---|---|
| S1 clipboard | `designos_fig::clipboard::parse_figma_clipboard_html(&str) -> Result<FigClipboard>` + `import_figma_clipboard(&FigClipboard) -> Result<Document>`; app: `clipboard::paste_figma(app, html) -> bool` |
| S2 styles | `designos_doc::Document` styles API (create/apply/detach effect & grid styles); `designos_fig::import_fig` / `export_fig` round-trip; app `styles::style_entries` |
| S3 PDF | `designos_export::export(... ExportFormat::Pdf ...)`; app export panel action |
| S4 layout/text | `designos_app::layout` (`layout_frame`) with literal expected positions; `textmeasure` line breaking; fig import/export enum mapping |
| S5 variables/codegen | `designos_doc::variables_apply` resolution; fig variables import; `designos_export::codegen` targets; MCP `generate_code` |

## Slices (vertical; each worker owns its files; parallel in isolated worktrees)
| # | Slice | Delivers (end to end) | Owner files | Blocked by |
|---|---|---|---|---|
| S1 | Paste from Figma (P0) | A Cmd+V of a Figma payload adds nodes to the page, centered, as one undo step; a malformed payload gives a toast and no panic | `designos-fig/src/clipboard.rs` (new), `designos-fig/src/lib.rs` (mod + re-export), `designos-app/src/clipboard.rs`, the paste call site in `designos-app/src/ops.rs` / `shell.rs` (minimal) | — |
| S2 | Effect & grid styles (P0) | Create / apply / detach / rename effect and grid styles from the Effects and Layout-guide sections; listed with local styles; round-trip through native and .fig formats; the fig import reads values from the style node (`styleIdForEffect` 335 / `styleIdForGrid` 336, legacy `inheritEffectStyleID` 169 / `inheritGridStyleID` 170, `isSoftDeletedStyle` 176) | `designos-app/src/styles.rs`, `panels/effects.rs`, `panels/layoutguide.rs`, the style section in `panels/left.rs`; `designos-doc/src/lib.rs` (style API only); `designos-fig/src/fig.rs` (style mapping fns only) | — |
| S3 | PDF export (P0) | PDF is listed in the export format; selected frames export as a multi-page PDF; bounds enforced | `designos-export/src/lib.rs`, `pdf.rs`; `designos-doc/src/settings.rs`; `designos-app/src/panels/export.rs` | — |
| S4 | Auto layout + text wrap (P1) | Vertical wrap in the UI and engine; `LayoutAlign::{SpaceAround, SpaceEvenly}` with CSS semantics; `stackCounterAlignContent`; `stackLayoutVersion` kept; text wrap Auto/Balance/Pretty | `designos-doc/src/node.rs` (AutoLayout, LayoutAlign, TextProps), `designos-app/src/layout.rs`, `textmeasure.rs`, `panels/properties.rs` (auto-layout block), `panels/typography.rs`; `designos-fig/src/fig.rs` (stack and text mapping fns only), `fig_export.rs` (same), `rest_import.rs` (align map) | — |
| S5 | Variable code syntax, composed opacity, UIKit + Android XML codegen (P1) | `codeSyntax` (358, WEB=0 ANDROID=1 iOS=2) stored, round-tripped, shown in Dev Mode, printed per Figma's rule (codeSyntax string wins, else `var(--sanitize(name), value)`); paint/effect `opacityVar` (38/53) bound and resolved; `generate_code` gains `uikit` and `android-xml` (Figma's native set; Vue/HTML are only Code Connect labels) | `designos-doc/src/variables*.rs`, `designos-fig/src/variables.rs`, `fig.rs` (opacityVar fn only), `designos-export/src/{codegen,css}.rs` + new `uikit.rs`, `android_xml.rs`; `designos-mcp/src/engine.rs` (language list); `designos-app/src/dev/` (code syntax display) | — |

There is a `fig.rs` overlap between S2, S4 and S5, in disjoint functions. The controller merges sequentially S3 → S1 → S2 → S5 → S4 and resolves conflicts.

## Global acceptance (all slices)
- Test-first at the seam. Each new test fails before the fix (show evidence) and passes after.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p <touched crates>` all green after the last edit.
- No `#[allow]`, no loosened assertions, no new network dependencies, no change to the R1 agent flags or the perf budgets.
- UI slices load es-designer and capture before/after screenshots with `designos --screenshot` / `--panel` into `plans/261017-figma-parity-wave-ab/screens/<slice>/`. Desktop 1440 only (responsive SKIPPED by owner default).
- Commit on the worker branch with conventional messages and no AI references. No push.

## Then
Controller merge plus full gates, an independent Opus verifier (read-only), and a fix round. The final report lists the morning assumptions (A1–A4 in brainstorm.md).

Later waves: slots (E3), check designs, grid 2.0, progressive blur, Draw/E4, Motion/E6, Plugin API runtime (P1 XL, needs its own design doc from REA R3/R4), PPTX, FigJam/Slides depth.
