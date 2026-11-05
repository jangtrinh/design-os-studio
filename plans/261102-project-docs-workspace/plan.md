# Plan — Project docs workspace (replaces the Spec Reader)

Status: planned, not started · Epic #1522 · Owner decisions: [brainstorm.md](brainstorm.md) P1–P13 · UI: [ui-spec-ported-from-design-studio.md](ui-spec-ported-from-design-studio.md) (copy design-studio 1:1, owner 2026-11-02)
Base: origin/main 8f267b3 (2026-11-02). Executors re-anchor every phase against HEAD before code.

## Outcome
One Project tab per library project: doc tree (Overview · Spec · Screens · States · Discovery), a reading page with "On this page", a Notion-style block editor, live frame embeds, Mermaid diagrams, MCP tools for agents to write docs live, and existing spec v2 packages sorted in place. The disabled Spec Reader (`shell.rs:1863`, `specs.rs`) and its fake canvas embed are removed.

## Phases
| # | Phase | Size | Depends on | Issue |
|---|---|---|---|---|
| 01 | [Docs model + storage crate](phase-01-docs-model-and-storage.md) | M · 3–4 d | — | #1523 |
| 02 | [Project tab: tree, reader, contents](phase-02-project-tab-tree-and-reader.md) | L · 4–5 d | 01 | #1524 |
| 03 | [Live frame embeds + pins](phase-03-live-frame-embeds-and-pins.md) | L · 3–4 d | 02 | #1525 |
| 04 | [Notion-style block editor](phase-04-notion-style-block-editor.md) | XL · 1 d spike + 8–10 d | 02 | #1526 |
| 05 | [Mermaid diagrams](phase-05-mermaid-diagrams.md) | M · 1 d spike + 3 d | 02 | #1527 |
| 06 | [MCP doc tools + live co-editing](phase-06-mcp-doc-tools-and-live-co-editing.md) | L · 4–5 d | 01 (read tools), 04 (write) | #1528 |
| 07 | [Spec v2 mapping, Overview KPI, retire Spec Reader](phase-07-spec-v2-mapping-kpi-and-retire-spec-reader.md) | M · 3 d | 02 | #1529 |

Total ≈ 29–35 dev-days (INFERENCE). After 02 lands, 03 / 04 / 05 / 07 can run in parallel worktrees (disjoint files listed per phase); 06 write tools wait for 04.

## Cross-phase rules
- Docs are plain `.md` under `<library>/projects/<id>/docs/`; manifest `docs/docs-manifest.json` (explicit, validated).
- Never parse or serialise docs or `doc.to_json()` on the UI thread; idle Project tab = 0 % CPU (`ps -o time=` over 20 s).
- Zero Fake UI gate, token parity gate and `cargo test --workspace --lib` green per PR; Clippy + Rustfmt; PR-only to `main`, squash merge.
- Every UI PR: 1440 capture next to the matching `plans/reports/design-studio-doc-ui-evidence/ds-*.jpg`; Figma update per owner rule before gates.
- Doc text returned over MCP is untrusted data (#1289).

## Acceptance (epic)
1. Home → project → Open project shows the Project tab matching the UI spec (capture).
2. Pages are `.md` files; external edits appear without reopening; round-trip lossless for v1 blocks.
3. `/frame` embeds render the real frame, update on change, click opens the file zoomed to it.
4. Agents write docs via MCP live: cursor + flash, one Cmd+Z per call, held while typing.
5. Spec v2 packages appear in the five groups with files byte-identical; KPI strip on Overview; Copy cho AI works.
6. `specs.rs` reader paths and the fake embed are gone; no dead `specs_reader_open`.

## Risks
- egui block editor (04) — spike decides scope of inline formatting.
- No Rust Mermaid renderer in the tree (05) — spike decides native subset vs alternative.
- Embed render cost on big files (03) — offscreen, cached by content revision, capped concurrent renders.
