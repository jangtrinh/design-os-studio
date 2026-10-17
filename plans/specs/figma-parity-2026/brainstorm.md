# Figma parity 2026 — brainstorm (Wave A + Wave B)

Source: `plans/261017-figma-gap-analysis/gap-list.md` (58 gaps, 10 epics) and the owner decisions of 2026-10-17:
- all epics are in scope;
- the Figma Plugin API runtime is P1;
- Figma clipboard paste interop is P0;
- REA runs on the local Figma.app 126.9.13.

**Run mode.** The owner is asleep and pre-authorized all phases ("es dev full phase và dispatch worker ra làm"). The gates below are passed on that authorization. Every owner-visible choice is listed under **Assumptions for morning review**.

## Press release (Wave A)
> **Paste from Figma, style everything, ship a PDF.** Designers moving from Figma can copy any selection in Figma desktop and paste it straight into design:os studio with frames, auto layout, components, styles and images intact. Effects and layout grids become reusable styles next to color and text styles. Any frame or page exports to PDF from the Export panel, exactly as in Figma.

## Users and jobs
- **Solo/small-team designer migrating from Figma.** Bring existing work over one selection at a time, without re-drawing it. Clipboard interop is the most natural path.
- **Design-system maintainer.** Define shadows, blurs, glass and grids once and apply them everywhere. Effect and grid styles are a daily need.
- **Designer handing off to a client.** Export frames as PDF.

## MoSCoW
**Wave A (P0), must:**
1. Paste a Figma clipboard payload (copied from Figma.app) into the studio canvas, reusing the `.fig` kiwi importer. Preserve images and fall back gracefully for unknown nodes.
2. Effect styles and grid styles: create, apply, detach, rename, list in the style picker, and round-trip through `.fig` import/export plus native save.
3. PDF export of the selected frames/pages from the Export panel and the export settings.

**Wave B (P1, small), should:**
4. Vertical wrap in the auto-layout UI.
5. Auto spacing Space Around / Space Evenly.
6. Text wrap Balance / Pretty.
7. Variable code syntax (Web/iOS/Android) shown in Dev Mode and used by codegen.
8. Composed color + opacity variables.
9. Vue and HTML codegen targets.

**Could (later waves):**
- slots, check designs, progressive blur, grid 2.0 (more than 8 tracks, auto-position), PPTX, the Figma Plugin API runtime (P1, XL, its own epic).
- Plugin API: a design doc and spike only tonight.

**Won't (this run):**
- real-time multiplayer, Sites, code layers, AI image models, 3D.

## No-gos
- No network calls added for these features.
- No change to the agent CLI bypass flags (owner ruling R1).
- No loosening of perf budgets, gates or assertions.
- No push and no merge to main without the owner.

## Risks
- **Clipboard payload format.** It is HTML `text/html` with `(figmeta)` and `(figma)` base64 comment markers wrapping a fig-kiwi archive. R3 REA will confirm the exact markers.
  - If Figma's clipboard schema differs from the `.fig` schema, the payload embeds its own schema, so the existing decoder handles it.
  - The egui/arboard clipboard on macOS: reading `text/html` may need `NSPasteboard` via arboard's HTML support. Verify it.
- **Effect/grid styles in kiwi.** Style nodes carry `styleType` EFFECT/GRID. R1 confirms the exact field names from the schema dump.
- **PDF export.** The engine exists (`designos-export/src/pdf.rs`) with 4096 px side / 500 pages / 128 MiB bounds. Wiring it into ExportFormat touches export presets in `designos-doc/settings.rs`.

## Assumptions for morning review (owner-visible, chosen conservatively = Figma UI3 1:1)
- A1. The effect-style picker lives in the Effects section header (four-dots "Style" button), as in Figma UI3. The grid-style picker lives in the Layout guide section.
- A2. The Paste of a Figma payload uses the normal Cmd+V. The new nodes are placed centered in the viewport (Figma's behavior when the source frame is not on the page). Images in the payload embed into the document.
- A3. PDF is added to the export format dropdown as "PDF" (Figma lists PNG/JPG/SVG/PDF). Multiple selected frames become a multi-page PDF, one page per frame, as Figma does.
- A4. Wave B text wrap uses the labels "Balance" and "Pretty" in the type settings popover, matching Figma.
