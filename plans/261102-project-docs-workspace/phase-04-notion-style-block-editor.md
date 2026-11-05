# Phase 04 — Notion-style block editor

UI: [part 2](ui-spec-part-2-editor-dialogs-and-states.md) §3, §4, §8 (banners), §14, §15. Mockups: ui-mockups/png/m03 (handle + block menu), m04, m04b, m05, m06, m10. The spike reports against these mockups.

Decisions: P5, P8. Size XL · 1 d spike + 8–10 d. Depends on 02.

## Spike first (1 d, report before building)
Prototype in egui: block list with caret movement across blocks, Enter/Backspace split/merge, `/` menu, IME (Vietnamese Telex) input, undo. Report: what works, inline formatting approach (egui `TextEdit` per block vs custom layout with cosmic-text), risks. Owner sees a capture before the build starts.

## Requirements
1. Edit in place on the reading page (no separate source view as default; "Edit source" stays in the row menu).
2. Blocks: H1–H3, paragraph, bullet/numbered list, checklist (click toggles), callout, table (add/remove row/column, Tab moves cell, horizontal scroll), frame/state embed (`/frame` opens the pin picker from 03), Mermaid (`/diagram`, source editor + preview from 05), code.
3. Inline: bold, italic, inline code, link (Cmd+B / I / E / K); Markdown shortcuts (`# `, `- `, `[] `, `> `).
4. Block handle on hover: drag to reorder, menu (turn into, duplicate, delete, Copy cho AI).
5. Undo/redo per user action; saving is debounced (500 ms idle) through `designos-docs` with conflict handling (reload prompt on conflict, never silent overwrite).
6. Vietnamese IME works in every text block.

## Files
`crates/designos-app/src/project_tab/editor/` (new). Check `crates/designos-app/src/textedit.rs`, `richtext.rs` and `agent_composer_text.rs` for reusable caret/IME/selection code before writing new.

## Acceptance
1. Interaction tests for split/merge, slash insert, list indent/outdent, table navigation, checklist toggle.
2. Every edit round-trips to Markdown identical to what phase 01 serialises.
3. IME test with Telex sequence produces correct precomposed text.
4. Captures at 1440: editing a page with a table, checklist, callout and an embed.

## Non-goals
Comments, multi-user cursors (agents only, via 06), images other than frame embeds.

## Spike result and owner decisions (2026-11-03)
Spike report: `plans/reports/spike-1526-block-editor.md` (branch `spike/1526-block-editor`). Owner chose, via AskUserQuestion:
1. **Custom surface over egui galleys**, not one `TextEdit` per block (custom passed 22/22 scripted checks; per-block `TextEdit` writes the Telex preedit into the document and cannot select across blocks). Build estimate revised to 10–12 d, including a 1–2 d AccessKit task inside this phase.
2. **v1 must have:** selection across blocks (Shift+arrows, drag, Cmd+A twice selects the page), Cmd+B with a collapsed caret (typing continues bold), double click selects a word and triple click selects the block.
3. **First task of the build:** reproduce, with a real keyboard, the lost space after Telex-composed words seen in the spike (`real-ime/real-ime-1.png`), fix the cause and add a regression test. Before merge the owner types one Vietnamese sentence by hand to confirm.
Reuse map and corrections from the spike (FACT, checked on origin/main): `agent_composer_text.rs` holds only strings (47 lines), so caret/IME code comes from `textedit.rs`, `canvas/textsession.rs`, `richtext.rs` and `textmeasure.rs`; the workspace is on egui/eframe 0.35, not 0.31.
