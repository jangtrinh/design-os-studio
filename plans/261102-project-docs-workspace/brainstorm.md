# Brainstorm — Project docs workspace (replaces the Spec Reader)

Date: 2026-11-02 · Status: brainstorm done, plan not written · Owner rounds: R1–R3 (this file)

## Outcome
A designer, PM or agent opens one **Project** tab and reads the whole story of a feature in one place: overview, spec, the real screens and their states, and the discovery behind them. Documents are plain Markdown next to the design files; frames inside them are live renders of the real canvas. This replaces the current Spec Reader, which is switched off on `main` (`crates/designos-app/src/shell.rs:1863`, `let specs_reader_open = false;`, since 7d4e911, 2026-10-16) and contains a fake "LIVE CANVAS EMBED" (`specs.rs:3288-3316`, five hard-coded labels, no render).

Source of the idea: itspatmorgan/design-studio (MIT) document management — artifact tree grouped by purpose, centred reading page with "On this page" contents, live embeds of real screens, an "About this prototype" entry page. We port structure and behaviour; visuals use our own Operate tokens (D2). No TypeScript code is copied.
Evidence: `plans/reports/design-studio-doc-ui-evidence/` (ds-*.jpg = their demo at 1440, studio-*.png = ours), ruling `plans/reports/review-261102-design-studio-adoption-ruling.md`.

## Owner decisions (2026-11-02)
| # | Decision | Round |
|---|---|---|
| P1 | Take design-studio's document management as a whole and merge it with spec-driven ("UI và structure bên đó chỉnh chu hơn spec driven của chúng ta") | R0 |
| P2 | Lives in its own **Project tab** (opened from Home), full width: doc tree · reading page · contents | R1 |
| P3 | Docs are **`.md` files next to the design files** in the library project (`docs/`); Studio watches disk | R1 |
| P4 | Tree grouped **by purpose**: Overview · Spec (PRD, US, AC) · Screens · States · Discovery | R1 |
| P5 | Editing is a **Notion-style block editor** in place | R1 |
| P6 | One Project = one library project holding **many design files**, flexible | R2 |
| P7 | Frame embeds are **real renders**, update when the frame changes; click opens that file's tab and zooms to the frame | R2 |
| P8 | v1 blocks besides heading/paragraph/list: **frame/state embed, table, checklist + callout, Mermaid diagram** | R2 |
| P9 | **MCP tools so AI writes docs directly** | R2 |
| P10 | Old D3 KPI strip (4 metrics) moves to the top of **Overview**; **role views (PO/Dev/QA) dropped**; D4 "Copy cho AI" kept | R2 — amends D3 placement and the role-view parts of D20/S02 |
| P11 | Screens/States are **pinned on purpose** (by user or AI via MCP), with a "Suggest from canvas" action; no auto-listing | R3 |
| P12 | AI and human edit the same page **live, like the canvas**: AI cursor + flash, one Cmd+Z step per AI tool call, AI changes held while the user is typing | R3 |
| P13 | Existing spec v2 packages are **read in place and sorted into groups**, files unchanged: intake → Overview, prd/us → Spec, ux → Screens/States, decisions/review → Discovery | R3 |
| kept | D2 Operate tokens, no cards-in-cards, zero emoji in chrome · D4 Copy cho AI · Zero Fake UI · Purple Ban · ≥24 px targets · 4 px rhythm · tables scroll horizontally | earlier rulings |

## Controller technical decisions (invisible; recorded, not asked)
- Embed reference in Markdown: `![Cart](designos:<file-studio-id>#<node-id>)`, node id + last known name for repair; NodeId is stable across save/load (designos-doc `lib.rs:88-92`), not across copy/paste → a broken ref shows a "Frame not found — relink" card, never a blank.
- Pins and group order live in a declared manifest `docs/docs-manifest.json` (explicit, validated; no default by omission). Not `project.json`: the library already uses that name for the project manifest (`crates/designos-library/src/store.rs:25`).
- UI is ported 1:1 from design-studio (owner, 2026-11-02: "UI thì cứ bế từ bên design studio sang"): measured layout, type scale, radii and spacing in `ui-spec-ported-from-design-studio.md`. Colours map almost 1:1 because both use the same neutral scale; the font stays Inter (app chrome font).
- Embed renders come from the existing renderer off the UI thread, cached by frame content revision; no `doc.to_json()` on the UI thread; idle Project tab = 0% CPU.
- Markdown round-trip must be lossless for the v1 block set; unknown Markdown is preserved as a raw block.
- MCP: doc read/list/write-block/insert-embed/pin tools on the existing server, same live-patch + undo semantics as canvas tools; doc text is untrusted data (#1289).

## Non-goals (v1)
- Interactive prototype playback inside embeds (option rejected in R2).
- Comments/threads in docs, multi-user sync, publishing a static site.
- Converting spec v2 files to a new format (P13: read in place).
- Role-based views (P10).

## Draft acceptance (to become issue AC)
1. Home → project → "Open project" opens a Project tab with tree, page, contents; screenshot at 1440.
2. Creating/editing a page writes `docs/*.md`; an external edit to the file shows up without reopening.
3. `/frame` inserts a pinned frame; changing the frame on canvas updates the embed; clicking opens the right file tab zoomed to it.
4. A spec v2 package appears sorted into the five groups with files byte-identical.
5. An MCP call writes a block while the page is open: AI cursor + flash, one Cmd+Z undoes it, held while the user types.
6. Overview shows the 4-metric KPI strip from one spec model; "Copy cho AI" works on any block.
7. Zero Fake UI gate and token parity gates pass; old fake embed removed.

## Open items for the plan
- Size: epic, rough 3–5 weeks split into ~6 slices (tab + tree + reader; md storage + watcher; block editor; frame embed render; MCP doc tools + live edit; spec v2 mapping + KPI). INFERENCE, to confirm in planning.
- Block editor in egui is the largest risk; spike first.
