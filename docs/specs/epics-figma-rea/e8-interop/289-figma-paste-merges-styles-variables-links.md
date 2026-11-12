# #289 Paste from Figma keeps shared styles, variables, prototype links and library refs

| Field | Value |
|---|---|
| Issue | #289 (task T8.4, M / P1; epic #21; Backlog) |
| Code baseline | `origin/main@d22dad970`. Re-anchor every line against HEAD and report drift before coding. |
| Verdict | **Still real.** The paste path drops every style ref, variable binding and reaction on purpose (es-debt note). |
| Size | M–L: 10–14 h builder + 1 h step-0 capture |
| Tier | Claude Sonnet (merge semantics across styles/variables/flows, undo, a shared app path) |
| Touches | `crates/designos-app/src/clipboard.rs` (`place_figma` `:168-245`, `detach_foreign_refs` `:249-296`), new merge helper in `crates/designos-doc/src/` (e.g. `paste_assets.rs`, pure function, unit-testable), `crates/designos-doc/src/lib.rs` (`StyleMeta` `:1616-1634`: new optional `paste_source` field, see R3), `crates/designos-fig/src/clipboard.rs` only if step 0 shows the import drops payload style/variable nodes. Issue lists `fig.rs` (HOT FILE queue position 29); touch it only if step 0 requires. |
| Depends on | Step 0 capture. PRs #1664 / #1671 / #1672 if `designos-fig` is touched. Owner answers Q-289 (name-collision rule). |

## 1. Still real? (FACT unless tagged)

- `place_figma` (`app/src/clipboard.rs:168`) imports the payload into a throwaway `frag` Document (`:171-172`), captures its top-level layers (`:173-174`) and inserts copies; the doc comment says "styles, variables, prototype links and library components of the payload are not merged into the document" (`:167-169`).
- `detach_foreign_refs` clears them on every pasted node: `n.style_refs.clear()`, `n.variables = Default::default()`, `n.prototype.reactions.clear()` (`:291-293`). Component links are remapped through `map` when the main is in the payload, else dropped and counted (`:252-289`); the count reaches a toast (`:240-242`).
- Only `frag.images` are carried (`:201-206`, `Command::SetImages`). `frag.styles`, `frag.style_meta`, `frag.variables`, `frag.variable_collections`, `frag.flows` are discarded.
- Undoable commands to carry them exist: `Command::SetStyles { styles, style_meta }`, `SetVariables { variables, collections }`, `SetFlows { flows }` (`designos-doc/src/history.rs:36-68`).
- Keys for linking exist: `StyleMeta.key` "Library key (Figma style `key`)" (`designos-doc/src/lib.rs:1619-1621`), `Variable.key: Option<String>` (`designos-doc/src/variables.rs:127`). Library style refs import as `key:<k>` ids (`fig_export.rs` writer branch `Some(id) if id.starts_with("key:")`, near `:3076`).
- Reactions hold `NodeId` destinations (`designos-doc/src/prototype.rs:430-481`, `Action::* { dest: NodeId }`), so they can be remapped with the same `map`.
- UNKNOWN (format): whether a Figma.app clipboard payload carries the **style nodes** and **VARIABLE / VARIABLE_SET nodes** the copied layers use, or only the consumer refs (`styleIdForFill` with `guid` or `assetRef`, REA `plans/261017-figma-parity-wave-ab/rea-findings.md:27-38`). The existing fixtures are synthesized from a `.fig` (`crates/designos-fig/tests/clipboard.rs:47-88`), not captured from Figma.app. Closing check: step 0.

## 2. User-visible outcome

A designer copies a card from Figma and pastes it in Studio. The card keeps its colour, text and effect styles and its variable bindings (switching the variable mode recolours it), its prototype links between pasted frames still work, and pasting it twice does not create duplicate styles or variables.

## 3. Step 0 (before code; 1 h; owner-manual leg possible)

Copy, in Figma.app (installed at `/Applications/Figma.app`), from a scratch file: (a) a frame with a local fill + text style and a variable-bound fill; (b) a frame with a library style; (c) two frames with a prototype link. Save each `text/html` clipboard to `crates/designos-fig/tests/fixtures/figma-clipboard-*.html` (strip nothing; the scratch file holds no client data). Dump with `parse_figma_clipboard_html` + `decode` and record which node types and ref forms appear. Use the scratch file, never a client or kit file (video/kit rules do not apply to tests, but the kit is 336 MB and private-licensed).

## 4. Behaviour rules

Decided (technical):
- R1 Styles. For each style referenced by a pasted node: if the target document has a style with the same `StyleMeta.key` (non-empty), relink to it (AC 1 "same-key style links"). Else, if the payload carries the style definition, add it as a local style under a fresh id. Else (ref only, no definition), keep the resolved values on the layer (today's behaviour) and count it as "unlinked style".
- R2 Variables. Same rule by `Variable.key`; a new local variable brings its collection (match an existing collection by key, else create it with the payload's modes). Bindings are rewritten to the target ids.
- R3 Dedupe on repeat paste. The match in R1/R2 also accepts a local style/variable that an **earlier paste** created from the same source. Source id = `"<fileKey>:<payload guid>"`; `fileKey` comes from figmeta (shape is INFERENCE, `rea-findings.md:24`; step 0 confirms) or the Message's `pasteFileKey`; when neither exists, fall back to `"content:<hash of the style's values + name>"` so identical content still dedupes. Stored on variables in `Variable.extensions["pasteSource"]` (`variables.rs:133`); `StyleMeta` has no extensions map (`lib.rs:1616-1634`), so add `StyleMeta.paste_source: Option<String>` with `serde(default, skip_serializing_if = "Option::is_none")`. Match on it before creating. Second paste of the same payload creates 0 new styles/variables (AC 2).
- R4 Prototype links. Remap `dest` through `map`; a reaction whose `dest` is outside the payload is dropped and counted. Flows (starting points) of the payload are added only when their `start` frame is pasted.
- R5 Library refs. Instance whose main is in an enabled library of this file (same key): link to that library component (`library_ops::link_remote_component`, `designos-doc/src/library_ops.rs:1300`) instead of dropping. Otherwise unchanged (dropped + counted).
- R6 One undo step: `SetStyles`, `SetVariables`, `SetFlows`, `SetImages`, `Insert` in one `Tx` (the existing `tx` in `place_figma`).
- R7 Report: the existing toast lists counts only when something was not kept: "Pasted from Figma (3 styles not linked, 1 link dropped)". Copy follows the PCP ux-writing rule (≤ ~8 words of microcopy).

Needs owner (Q-289): a **name collision** — the payload has local style "Brand/Primary" (no key match) and the file already has a different "Brand/Primary".

## 5. Acceptance criteria (red then green)

- **AC-1** Fixture (a): pasted frame's fill and text keep `style_refs` pointing at a style in the target doc; red on main (`style_refs` empty).
- **AC-2** Pasting fixture (a) twice adds styles/variables once (count before/after).
- **AC-3** Target already has a style with the payload style's key → no new style; the ref points at the existing one.
- **AC-4** Variable-bound fill survives; switching the collection's mode in the target recolours it.
- **AC-5** Fixture (c): reaction between the two pasted frames points at the new ids and Present follows it; a reaction to a frame not pasted is dropped and counted.
- **AC-6** One Cmd+Z removes layers, styles, variables and flows added by the paste; redo restores them.
- **AC-7** Existing `crates/designos-fig/tests/clipboard.rs` and app clipboard tests stay green.

## 6. Test steps

1. Unit tests on the pure merge helper (doc crate) with hand-built `Document`s for R1–R4.
2. App test driving `paste_figma` with the step-0 fixtures.
3. Visual: Studio window after pasting fixture (a) with the Inspector showing the linked style name (desktop 1440 capture, responsive SKIPPED by owner default).

## 7. Edge cases

- Payload variable alias chain to a variable not in the payload → bind to the resolved literal, count "unlinked variable".
- Payload collection has modes the target collection (same key) lacks → do not add modes; bind and let the default mode resolve.
- Soft-deleted payload variables (`isSoftDeleted`, see #1647) → never created as live variables.
- Paste into a Board document or a Deck: same rules.
- Pasting into an instance slot: unchanged slot checks (`slot_edit_error`, `:197`).

## 8. Non-goals

- Studio → Figma (#316). Studio → Studio cross-file asset carry (same gap, separate issue if wanted).
- Fetching library styles from Figma's servers.

## 9. User-visible details not settled by the issue

- Name collision rule → **Q-289** in README.
- Toast wording: decided per R7.

## 10. Rollback

Single revert; the helper is additive and `detach_foreign_refs` gets back its clears.
