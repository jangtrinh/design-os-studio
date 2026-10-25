# E15-20 Show a style preset gallery rendered by our own renderer

- Issue: #608 (task E15-20, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `assets/styles/presets.json` (new), `assets/styles/samples/{web,mobile}.json` (new NodeSpec samples), `assets/styles/thumbs/*.png` and `assets/styles/thumbs/manifest.json` (new, generated and committed), `crates/designos-render/examples/style_thumbnails.rs` (new generator), `crates/designos-app/src/panels/style_gallery.rs` (new), the Agents rail panel host in `crates/designos-app/src/shell.rs`, `crates/designos-app/src/terminal.rs` (`active_style` in `sync_live_file_context`).

## 1. Summary and user story

As a user, I want to choose a style by looking at small renders of a web screen and a mobile screen, click one, and have the document take that style in one undo step, with my agent told which style is now active.

## 2. Current state

- FACT: the left rail has `RailItem::Agents` (`crates/designos-app/src/app.rs:55-63`). When it is open, the left panel is the AI panel with its own width `ui.ai_panel_width`, clamped to `[260, max]` (`crates/designos-app/src/shell.rs:1121-1124`), and hosts the embedded agent terminal (`crates/designos-app/src/terminal.rs:1-9`).
- FACT: `designos-render` renders a node headlessly with `render_node(doc, node, scale)` (`crates/designos-render/src/lib.rs:162`). The CPU path is the bit-exact reference renderer (CLAUDE.md §2, §4.3).
- FACT: there is no style picker or preset data today.
- FACT: design rules: 28 px rows and segmented tabs (D12, D116), 24×24 px minimum targets, 4/8 px rhythm, `#27272A` hairlines, concentric radii, Inter 6-step ramp, no purple chrome (`design/principles.md` P2–P5), and every control must do real work (P6, `scripts/zero_fake_ui_gate.py`).

## 3. Behaviour rules

1. **Preset = archetype + full params.** `presets.json` lists presets `{ id, name, archetype, params }` where `params` holds all eight keys of E15-18 `StyleParams`. Every archetype has at least one preset; the first preset of each archetype is its catalogue `preset`, a field this task adds to each E15-18 §5 catalogue entry (E15-18 has no preset concept).
2. **Sample screens.** `samples/web.json` (1440×900 frame: top bar, sidebar, title, a card grid, a primary button, a table row set) and `samples/mobile.json` (390×844 frame: status bar, title, list, primary button, tab bar). Both use only `var:` references to the E15-19 names, written in the collection-qualified form (E15-19 rule 11, for example `var:designos-style:color/color/primary`) and the E15-19 text and effect styles (`text_style: "<Name>"` for keys `designos-style:text/<Name>`, `effect_style: "Elevation/<n>"` for keys `designos-style:effect/elevation-<n>`). No hard-coded colour, radius, font or size.
3. **Thumbnail generation** (`cargo run -p designos-render --example style_thumbnails -- --out assets/styles/thumbs`):
   - for each preset: new document → `compile_style` (E15-19) → `apply_spec` of each sample → `render_node` on the CPU path at scale `0.25` (web, 360×225 px) and `0.5` (mobile, 195×422 px) → PNG with no metadata chunks (stable bytes);
   - writes `manifest.json`: `{ catalogue_version, renderer: "<designos-render crate version>", samples_sha1: { web, mobile }, thumbs: { "<preset>-web.png": "<sha1>", … } }`.
   - Fonts come from the `designos-fonts` cache; the example fetches missing fonts first and fails if any is unavailable (no fallback face in a committed thumbnail).
4. **Staleness.** A test recomputes the manifest inputs (catalogue version, sample hashes, renderer version) and fails when they differ from the committed manifest, naming the command to regenerate. A second test re-renders each thumbnail and compares bytes (AC1).
5. **Gallery content** is compiled into the binary with `include_bytes!`; opening it needs no network and no disk read outside the app (AC3).
6. **Picking a preset** calls the same code path as `designos_apply_style` (E15-19) with the preset's archetype and params. It is one undo step (AC2). The undo label is `Apply style "<preset name>"`.
7. **Telling the agent.** `publish_live_context` (`crates/designos-app/src/live_patch_apply.rs:28-30`, called from `shell.rs:1145`) takes no text, it republishes the document through `TerminalState::sync_live_file_context` (`terminal.rs:800`). This task adds an `active_style` string field to the `context.json` that function writes (computed from `Document.applied_style`, or omitted when it is `None`): `Active style: <preset name, or "Custom"> (archetype <id>; heading <family>, body <family>, palette <id>, radius <id>, elevation <id>). Use the qualified variables var:designos-style:color/color/<role> (and the shape, space and type collections, E15-19 rule 11) and the text styles (text_style: "<Name>", keys designos-style:text/<Name>, for example Body Large).` A pick changes the document, so the existing sync key (`doc.mutation_epoch()`) republishes it with no extra trigger, and a session started later reads the same field. E15-16's first-turn bootstrap also reads `Document.applied_style`.
8. **Current marker.** The card whose archetype and params equal `Document.applied_style` shows the selected state. If the document has a style that no preset matches, a "Custom" row at the top shows the archetype name and an Edit link that opens the variables panel on `Style / Color`.

## 4. Data model / schema

```json
// assets/styles/presets.json
{ "schema": 1, "presets": [
  { "id": "quiet-editorial-ink", "name": "Quiet ink", "archetype": "quiet-editorial",
    "params": { "heading": "fraunces", "body": "inter", "caption": "inter", "data": "jetbrains-mono",
                "palette": "ink-on-paper", "radius": "crisp", "elevation": "flat-hairline", "imagery": "none" } } ] }
```

Rust: `pub struct Preset { id, name, archetype, params: StyleParams }` in `designos-doc/src/style_archetype/presets.rs`, validated at load: every preset must `resolve` to `Resolution::Resolved`. No document format change beyond E15-19's `applied_style`.

## 5. Contracts

No new MCP tool. The gallery uses E15-19's compile path. `designos_get_guidelines { "category": "style" }` (E15-18) adds `presets: [{ id, name, archetype }]` to the catalogue listing so agents can name a preset.

## 6. File-format mapping

None.

## 7. UI

- **Entry.** The AI panel header gets a segmented control with 28 px tabs: `Terminal`, `Styles` (and `Variants` from E15-38). `Styles` shows the gallery in the AI panel area; the terminal session keeps running.
- **Layout.** A search field (28 px tall, filters by preset name, archetype name and surface tag) above a two-column grid of preset cards that reflows to one column below 300 px panel width. Gaps 8 px, panel padding 12 px.
- **Card.** `BG_SECONDARY` (`#1E1E22`) surface, 1 px `BORDER` (`#27272A`) hairline, radius 8 with the 4 px inner thumbnail radius (concentric rule). Content: the web thumbnail full width, the mobile thumbnail overlapping its lower-right corner at 40% card width with a 1 px hairline, then the preset name in 12 px Inter title weight and a 10 px caption line `archetype · palette` in `TEXT_SECONDARY`. Hover uses `BG_HOVER`; the selected card uses the `BORDER_SELECTED` (`#E36323`) 1 px border and a check icon (Lucide glyph, `ICON_SELECTED`).
- **Action.** Clicking a card applies it at once (it is one undo step, so no confirm dialog). A toast confirms `Style applied: <name>` with an Undo action. Keyboard: arrow keys move focus, Enter applies.
- **Empty search.** Shows "No style matches '<query>'" and a Clear button.
- **States.** While applying (≤ 50 ms) no spinner is shown. No animation runs at rest (0% idle CPU invariant).
- Evidence: desktop capture at 1440 of the gallery with one selected card (responsive: SKIPPED, owner default). Load `es-designer` before writing the panel code.

## 8. Edge cases

1. Document is a library file whose `Style / *` collections are subscribed copies: cards are disabled with the caption "Styles are edited in the library file".
2. Read-only document (viewer or archived branch): cards disabled; tooltip explains why.
3. Thumbnail PNG missing from the binary list (packaging bug): the card shows the preset name and a hairline box with "Preview unavailable"; a test prevents this state in CI.
4. Board (FigJam-like) documents: the Styles tab is still available; styles apply to variables only.
5. Agent session not running: nothing is queued; `context.json` already carries `active_style`, so a session started later reads it.
6. Search with only whitespace: treated as empty.

## 9. Non-goals

- Editing archetype parameters in the UI (the agent or the variables panel does that).
- User-created presets (later pack).
- Generating thumbnails at runtime.

## 10. Test plan

Seams: `cargo run -p designos-render --example style_thumbnails -- --check` (bytes and manifest; `cargo test --example` builds it under the libtest harness, so its `main` never runs and `--check` goes to libtest), `cargo test -p designos-render` for the staleness tests that call the same check function, the app shell test harness (a new module under `crates/designos-app/src/shell/tests/`, which exists beside `shell.rs`) for UI behaviour, and a `--screenshot` capture.

Fail-before: `every_preset_has_two_thumbnails` fails on `main`.

| Test | Input | Expected |
|---|---|---|
| `every_preset_has_two_thumbnails` (AC1) | presets.json | `<id>-web.png`, `<id>-mobile.png` present and listed in the manifest |
| `thumbnails_match_fresh_render` (AC1) | re-render each preset | bytes equal to committed PNG |
| `manifest_is_current` | catalogue version, samples, renderer version | equal to manifest; failure message names the regenerate command |
| `pick_applies_in_one_undo_step` (AC2) | shell test: open fixture, click card 1 | `applied_style` set; one undo restores the original document hash |
| `gallery_needs_no_network` (AC3) | shell test that counts requests made by the `designos-fonts` fetcher and any other HTTP client during gallery open (test-build counter) | all cards render; request count 0 |
| `selected_card_matches_applied_style` | apply preset 3 via MCP, open gallery | card 3 selected |
| `samples_use_only_variables` | walk both samples | no literal colour, radius, font family or font size |

## 11. Performance budgets

- Gallery open ≤ 16 ms on first frame after textures are created; thumbnails become textures once, outside `ctx.data_mut` (CLAUDE.md §4.2).
- Scrolling the gallery must not call `app.changed()` or bump `TileView::rev` (§4.1).
- Thumbnail generation ≤ 60 s for the whole catalogue in CI (release build).

## 12. Dependencies and build order

- Needs E15-19 (#606) and, through it, E15-18, E15-3 and E15-4. The sample screens bind with `var:` and `text_style` through `apply_spec`, so E15-3 (#588) must have merged. Uses E15-16 (#601) for the bootstrap line.
- E15-38 adds the `Variants` tab to the same segmented control; merge E15-20 first.

## 13. Owner rulings that apply

- Full scope: every preset has both thumbnails.
- Thumbnails come from our own renderer, never a third-party service.
- Zero fake UI: every card applies a real style.
