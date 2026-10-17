# Figma parity 2026 — use cases and acceptance (Wave A + Wave B)

## UC-A1 Paste from Figma (P0)
- **Given** the clipboard holds a Figma selection copied from Figma.app (`text/html` containing the `(figmeta)` and `(figma)` base64 payload),
- **When** the user presses Cmd+V on the canvas,
- **Then** the copied nodes appear on the current page, centered in the viewport, as one undoable step.
- **And** frames, auto layout, text, vectors, components/instances, fills/effects and images are preserved within what the `.fig` importer already supports.
- **And** a non-Figma clipboard (plain text, SVG, PNG, studio's own format) keeps today's behavior.
- **And** a malformed or hostile payload never crashes the app. The same kiwi/inflate budgets apply, and the user sees a toast error.

Acceptance:
- AC1 a fixture payload built from a real `.fig` pastes N>0 nodes.
- AC2 the paste is undoable in one step.
- AC3 non-Figma clipboard is unchanged (existing clipboard tests pass).
- AC4 a malformed payload gives Err plus a toast, with no panic (test).
- AC5 a screenshot shows the pasted result.

## UC-A2 Effect and grid styles (P0)
- **Given** a node with effects, or a frame with layout grids,
- **When** the user creates a style from them,
- **Then** the style appears in the style picker and in the left-panel local styles.
- **And** applying it to other nodes links them (`style_refs`).
- **And** editing the style updates every linked node.
- **And** detaching keeps the values but drops the link.

Acceptance:
- AC6 `Style::Effect` and `Style::Grid` exist in designos-doc with apply/detach.
- AC7 they round-trip through native save/load.
- AC8 `.fig` import maps EFFECT/GRID style nodes to them, and export writes them back (round-trip test on a fixture).
- AC9 the UI picker lists them (screenshot).
- AC10 MCP `get_document` exposes them.

## UC-A3 PDF export (P0)
- **Given** one or more frames are selected,
- **When** the user picks PDF in Export and exports,
- **Then** a PDF is written with one page per frame, vector where possible, honoring the existing PDF bounds.

Acceptance:
- AC11 `ExportFormat::Pdf` exists and the export panel lists it.
- AC12 exporting 2 frames gives a 2-page PDF (test parses the page count).
- AC13 the bounds are enforced (oversized → error, not abort).

## Wave B (P1, small)
- UC-B1 **Vertical wrap.** The auto-layout wrap toggle works for vertical direction. AC: the engine test lays out the vertical wrap columns correctly; the UI toggle is shown for vertical.
- UC-B2 **Space Around / Space Evenly** in primary-axis alignment. AC: the engine positions items per CSS justify-content semantics (tests with literal expected positions); `.fig` import maps the Figma enum values.
- UC-B3 **Text wrap Balance / Pretty.** AC: Balance equalizes line widths (test: max−min line width shrinks against greedy); Pretty avoids a single-word last line (test); the setting is stored on TextProps and round-trips.
- UC-B4 **Variable code syntax.** AC: per-variable WEB/iOS/ANDROID strings are stored, round-trip through `.fig` (`codeSyntax`), and are shown in Dev Mode; CSS codegen uses the WEB syntax when present.
- UC-B5 **Composed color + opacity variables.** AC: a color bound to a variable with opacity bound to a number variable resolves to the composed RGBA; `.fig` mapping follows `COLOR_OPACITY`.
- UC-B6 **UIKit and Android XML codegen** (Figma's native Dev Mode set; Vue/HTML are only Code Connect labels). AC: the `generate_code` targets `uikit` and `android-xml` exist; snapshot tests on a fixture node; MCP lists them.
