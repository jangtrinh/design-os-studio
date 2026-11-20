---
id: detail-1154-nodespec-text-measured-not-estimate
type: note
title: "S Hug text built by NodeSpec, create_node and update_node is measured by the renderer's shaping, not estimated"
updated: "2026-10-25"
status: draft
---

## Nguồn
Bản sao từ nhánh `docs/specs-untitled-ui-dark` (`docs/specs/ui-revamp-untitled-dark/1154-nodespec-text-measured-not-estimated.md`), lấy ngày 25/10/2026 để pack tự đủ. Nguồn chuẩn chi tiết cho US tương ứng.


| Field | Value |
|---|---|
| Issue | #1154 (epic #1118, spec key S) |
| Size | M. Two days: a text-measure hook in `designos-doc`, a renderer-backed measurer installed by both binaries, three call sites in the spec builder, create/update text sizing in designos-mcp, eight tests. |
| Depends on | none |
| Unblocks | Every agent-built button, chip, tab and badge (designos-mcp and the CLI). Spec A's onboarding mocks. The revamp boards built with `--apply-spec`. |
| Touches | `crates/designos-doc/src/spec.rs`:<br>- `estimate_intrinsic_text_size` `:1889-1913`<br>- text-node sizing `:1328-1347`<br>- generated label sizing `:1432-1437`<br>`crates/designos-doc/src/spec_instance.rs:1257-1273` (text override on an instance)<br>New `crates/designos-doc/src/text_measure.rs`<br>`crates/designos-render/src/text.rs` (`text_geometry` `:774`; new `measure_text_box`)<br>`crates/designos-app/src/textmeasure.rs:16-32` (calls the shared function), `crates/designos-app/src/main.rs` (install at start, before `:142`)<br>`crates/designos-mcp/src/engine.rs` (install in `Engine::new`; `CreateArgs` `:1376-1390`; `create` `:1222-1249`)<br>`crates/designos-mcp/src/tools/mod.rs` (`build_created_node` `:108-146`; text update `:248-255`)<br>The `designos_create_node` description in `engine.rs` and its generated mirrors: `crates/designos-mcp/tests/snapshots/tools.json`, `docs/mcp-parity.md`, `crates/designos-mcp/generated/rules.md`. `tests/tools_snapshot.rs:35`, `:163-164` and `:167-171` fail until `cargo run -p designos-mcp --bin gen-tool-docs` is rerun. There are hand-kept copies in `README.md`, `CLAUDE.md`, `skills/design-os-studio/SKILL.md` and `skills/design-os-studio/rules.md`; no test checks them, so update them by hand. |

Code baseline: `origin/main@205c72ef`. All file:line citations were read there with `git show`. #1154 cites `spec.rs:1893`, which is the `let max_w` line inside the function; the function starts at `:1889`. It also cites `engine.rs:1143`, which is `settle_layout` in undo/redo; the relayout helper is at `:1154-1157`. `origin/main` is now `7e15ddae`, and none of the files cited here changed between the two. Re-anchor against HEAD and report drift before writing code. Scratch evidence: `$SCRATCH/agents/w5/s/`, where `$SCRATCH` = `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-AW68Nz/a6445c24-c40e-4254-a3e2-5e0bafafd0f4/scratchpad`.

## 1. Summary and user story

As an agent building UI through designos-mcp or `designos --apply-spec`, I want a hug button's label to sit centred, as it does in Figma and in Studio's own editor. Today the label's text box is sized by a character-width table, not by shaping. The box comes out wider than the glyphs, so a `Delete` button shows 13 px of padding on the left and 21.5 px on the right. Evidence: `plans/261025-1247-untitled-ui-dark-revamp/evidence/button-label-offset@3x.png`.

## 2. Current state at `205c72ef` (FACT unless labelled)

**The estimate.**
- `estimate_intrinsic_text_size` (`spec.rs:1889-1913`) sums a per-character em table: space 0.32; `i l I 1 . , : ; | !` 0.34; `W M — @ %` 0.82; other capitals 0.68; anything else 0.56.
- The total is multiplied by the font size and by 1.05 for weight ≥ 600, then 4 px is added. The result is ceiled, with a minimum of 16.
- Height is lines × font size × 1.38.
- It does not shape the text, apply letter spacing, or look at the font family.

**Where it is used:**
- `spec.rs:1328-1347`: every NodeSpec text node. Without `width`, it is a hug `WidthAndHeight` box of exactly the estimate. With `width` or `width_sizing: "fill"`, the mode is `Height` and the height comes from estimated line count × 1.45.
- `spec.rs:1432-1437`: the label that a frame or preset with `text` generates (buttons, badges).
- `spec_instance.rs:1257-1273`: text overrides on component instances. `WidthAndHeight` gets the estimate, `Height` gets the estimated line count.
- `spec_bind_reference_forms_tests.rs:141`: a test helper.

**The estimate is what the board shows.**
- Reproduced in scratch from `evidence/brainstorm-compare-board.designos` with `designos board.designos --export-spec all.json`. Every `Export` label in a hug `Button secondary` (padding `0/14/0/14`) has the estimated width:
  - 11 px/600: 45 px, from (0.68 + 5 × 0.56) × 11 × 1.05 + 4 = 44.2, ceiled.
  - 14 px/600: 56 px.
- For #1154's `Delete` at 14 px/600: (0.68 + 0.56 + 0.34 + 0.56 + 0.56 + 0.56) × 14 × 1.05 + 4 = 51.9, ceiled to 52 px. The glyphs drawn are 41.5 px, measured by the issue writer on a scale-2 render; not re-measured here.

**The real measurer.**
- `designos_render::text_geometry(props, size)` (`designos-render/src/text.rs:774`) shapes with the renderer's own fonts, runs, case and wrap.
- The editor's `textmeasure::measure(props, width)` (`designos-app/src/textmeasure.rs:16-32`) calls it and ceils width and height to whole pixels (`:31`). `layout::fit_text` (`designos-app/src/layout.rs:19-25`) uses it on text edits.
- `designos-doc` depends on neither `designos-render` nor `designos-fonts`, while `designos-render` depends on `designos-doc` (`designos-render/Cargo.toml:10`). So `designos-doc` cannot call the renderer directly.
- `designos-mcp` already depends on `designos-render` (`designos-mcp/Cargo.toml:23`).

**Other paths that size text without measuring:**
- **create_node** (`engine.rs:1222-1249` → `tools/mod.rs:108-146`) builds the node from a rect whose width and height default to 100 (`CreateArgs`, `#[serde(default = "default_size")]`, `engine.rs:1386-1389`). It sets `content` (`tools/mod.rs:142-144`). The text constructor sets `auto_resize: WidthAndHeight` (`designos-doc/src/node.rs:401-412`). So a created text claims to hug its content but is a 100×100 box.
- **update_node** replaces `content` and clears runs and lists without resizing (`tools/mod.rs:248-255`).
- **apply_batch** reaches the same builders: `create` and `insert` call `build_created_node` (`tools/batch.rs:1576`), and `apply_spec` calls `apply_spec_command` (`:1808`).
- **CLI:** `--apply-spec` calls `Document::apply_spec` (`designos-app/src/main.rs:301-307`), defined at `designos-doc/src/spec.rs:621`.

**Undo and live sessions.**
- designos-mcp builds one `Command` per tool call (`apply_spec_command`, `spec_instance.rs:2253-2257`, "returns a single undoable Command") and runs it through `Engine::exec` (`engine.rs:1180-1186`). `exec` records the live ops for that command before it runs. Those ops are the edit-log patch a live Studio session replays.
- The relayout after it (`Engine::relayout`, `engine.rs:1154-1157`, which calls `Document::relayout_subtree`) is not a recorded op. Studio lays out on its side.

**Studio never re-measures what it opens or receives.**
- `font_refit.rs:8-9`: "a text box that came from the file is never replaced by the app's own measure … A document that is only opened is never re-measured."
- No `fit_text` caller is in the live-patch path (`live_patch_apply.rs:88` onward; callers of `fit_text` at `205c72ef` are text edits, typography, styles, paste, find, board bar, comp ops and font refit).
- So an estimated box stays wrong in Studio until a user edits that text.

**Fonts in headless processes.**
- designos-mcp does not fetch fonts: there is no `designos_fonts` use in `designos-mcp/src`.
- The bundled fonts are `Inter[opsz,wght].ttf`, its italic, and Roboto Mono (`crates/designos-fonts/assets/`).
- ASSUMPTION: the renderer applies the variable `wght` axis when shaping weight 600. Closing check: test 2 compares measured widths at 400 and 600.

## 3. Behaviour rules

1. **One measure for every text box Studio writes.**
   - `designos-doc` gains `text_measure::TextMeasure`, a `Send + Sync` trait with `fn measure(&self, tp: &TextProps, wrap_width: Option<f32>) -> Vec2`, and `pub fn install(m: &'static dyn TextMeasure)`, held in a `OnceLock`. A second install with another measurer is ignored and logged once in debug builds.
   - `pub(crate) fn intrinsic_text_size(tp, wrap)` uses the installed measurer, or the existing estimate when none is installed. The estimate is kept unchanged for crates and tests that have no renderer.
2. **The renderer-backed measurer.**
   - `designos-render` gains `pub fn measure_text_box(tp: &TextProps, wrap_width: Option<f32>) -> Vec2`. It returns `text_geometry`'s line width and height, rounded **up to the next 0.25 px**, not to a whole pixel. Then a hug label's right padding exceeds its left by less than 0.25 px.
   - `designos-render` exposes a unit struct implementing `TextMeasure` with it.
   - `designos-app/src/main.rs` installs it first thing in `main` (before the agent CLI branch at `:142`). designos-mcp installs it in `Engine::new`.
   - Studio's editor keeps its own whole-pixel `textmeasure::measure` (`:31`) for edits (non-goal).
3. **NodeSpec text** (`spec.rs:1328-1347`, `:1432-1437`): every estimate call becomes `intrinsic_text_size`.
   - `WidthAndHeight`: size = measure(None).
   - `Height` (a given `width`, or `fill`): height = measure(Some(width)).y, replacing the line-count × 1.45 guess.
   - An explicit `width` or `height` in the spec still wins, as today.
4. **Instance text overrides** (`spec_instance.rs:1257-1273`) use the same function and the same two modes. `None` and `Truncate` keep their box, as today.
5. **create_node and apply_batch create/insert of a text layer:**
   - `CreateArgs.width` and `height` become `Option<f32>` (default `None`); other kinds still default to 100.
   - For `kind: "text"`:
     - no width: `WidthAndHeight`, size = measure(None)
     - width only: `Height`, height = measure(Some(width)).y
     - both given: the box is kept as given and `auto_resize` becomes `None`, so the file no longer claims to hug a box it does not hug
   - The tool description's "defaults to … width/height=100" is updated for text.
6. **update_node and apply_batch update of a text layer.** When `text`, a text style, font size, weight, family or letter spacing changes, and the call passes no `width` or `height`, a `WidthAndHeight` or `Height` text is re-measured as in rule 3, inside the same command.
7. **One command, one undo step, no extra live ops.** The measured sizes are part of the `Insert` or `Update` command the tool already builds. No second command is added, and the edit-log patch carries the same number of ops with the corrected `size`. Agent undo stays one step per tool call.
8. **Report.**
   - `apply_spec`, `apply_batch` and `create_node` replies gain `measured_with_fallback: [node ids]`. These are texts whose family was not available to the measurer, so they were measured with the fallback face.
   - The reply warns `TEXT_MEASURED_WITH_FALLBACK_FONT` once per call. Studio fetches the real face later but never re-measures that box (section 2), so the agent should know.

## 4. Data model / schema changes

None to the file format. `auto_resize` on a text made by `create_node` with an explicit width and height becomes `None` (rule 5). New `designos-doc` module `text_measure` (trait plus `OnceLock`). New `designos-render` function `measure_text_box`.

## 5. Contracts

- `designos_create_node`: for text, `width` and `height` are optional and mean "measure" when omitted. The reply's `node.size` is the measured box.
- `designos_apply_spec`, `designos_apply_batch`, `designos_create_node`: new reply field `measured_with_fallback` and new warning code `TEXT_MEASURED_WITH_FALLBACK_FONT`. These are additive, and existing clients ignore unknown fields.
- `designos_update_node`: a content or style change on hug text changes `size` in the reply.
- Rust: `designos_doc::text_measure::{TextMeasure, install}`, `designos_render::measure_text_box`. `estimate_intrinsic_text_size` stays `pub(crate)` as the fallback.

## 6. File-format mapping

| Field | Before | After |
|---|---|---|
| hug text `size.x` (spec, create, instance override) | em-table estimate + 4, ceiled | shaped width, rounded up to 0.25 px |
| `Height` text `size.y` | estimated lines × font size × 1.45 | shaped height at that width |
| `create_node` text with explicit width and height | `WidthAndHeight`, 100×100 by default | `auto_resize: None`, box as given |

`.fig` and `.designos` readers are unchanged. Files written before this spec keep their estimated boxes until a user edits the text (section 2: Studio does not re-measure on open).

## 7. UI

None in Studio's chrome. Agent-built content changes: labels are centred in hug buttons. `es-designer` is not needed. No colour is involved, so the token-contract C-1 pairs (`specs/token-contract.md` section (f)) do not apply.

## 8. Edge cases

- **Fallback font** (a family not bundled and not in the disk cache): measured and rendered with the same fallback in that process, so headless renders are consistent. In Studio, after the real face loads, the box is the fallback's width (rule 8 warns). Re-measuring it is a follow-up (non-goal).
- **Multi-line content** (`\n`): the widest shaped line, as in `text_geometry`.
- **Letter spacing, text case, mixed runs:** shaped as the renderer draws them. The estimate ignored all three.
- **Empty text:** `text_geometry` gives width 0. Keep the estimate's minimum of 16 px wide and one line high, so an empty label stays selectable.
- **Determinism:** the same binary, bundled fonts and input give the same numbers. Tests use only bundled Inter. `measured_with_fallback` makes a host-dependent measure visible.
- **Studio editing a measured text later:** `fit_text` re-fits to the whole-pixel editor measure. That is a change of under 1 px, and the editor already does it on every edit.
- **No measurer installed** (`designos-doc` unit tests, other crates): the estimate, exactly as today. Existing `designos-doc` tests are unchanged.
- **Test binaries never run `main()`.** The measurer is process-wide, so in the `designos-app` and `designos-mcp` test binaries it is installed by `Sim` construction (`shell/tests.rs`) and by `Engine::new`. Every test in those binaries then measures; none relies on install order. Spec L's headless gate therefore sees measured text, as the real binary does.
- **Expected test churn.** Existing `designos-mcp` and `designos-app` tests that assert estimated text sizes (for example 45 px for `Export` at 11/600) get new expected values in this PR. That churn is expected, not a regression; the PR lists each changed assertion.

## 9. Non-goals

- Re-measuring texts in files that are opened or received (Studio's rule, `font_refit.rs:8-9`).
- Fetching fonts in designos-mcp.
- Changing the editor's whole-pixel measure or `fit_text`.
- Moving text shaping into `designos-doc`.

## 10. Test plan (red first)

| # | Case | Where | Expected | Fails before |
|---|---|---|---|---|
| 1 | `delete_button_label_has_equal_side_padding` | `designos-mcp` tests (has `designos-render`) | `designos_apply_spec` builds a hug frame, row, padding `[0,12,0,12]`, counter-align centre, with a text child `Delete` at 14 px/600 Inter. Let `w = measure_text_box(label).x` and the button's x be 0. Left padding = `text.x` = 12. Right padding = `button.width − (text.x + glyph advance width from text_geometry)`. Assert `|left − right| ≤ 0.5`. | yes: 12 vs 22.5 (box 52, glyphs about 41.5) |
| 2 | `semibold_measures_wider_than_regular` | `designos-render` | `measure_text_box("Delete", 14, 600).x > measure_text_box("Delete", 14, 400).x` (wght axis applied) | closing check for the section 2 ASSUMPTION |
| 3 | `create_node_text_without_size_hugs_its_content` | `designos-mcp` | `designos_create_node {kind:"text", text:"Delete"}`: size equals `measure_text_box`; `auto_resize` is `WidthAndHeight` | yes: 100×100 |
| 4 | `create_node_text_with_size_is_fixed` | `designos-mcp` | width 80 and height 20 given: size 80×20, `auto_resize: None` | yes |
| 5 | `update_node_text_change_remeasures_hug_text` | `designos-mcp` | `"Delete"` → `"Delete all"`: width grows to the measured width in the same undo step (one `designos_undo` restores both) | yes |
| 6 | `instance_text_override_is_measured` | `designos-mcp` | an instance of a hug button component with label override `"Remove"`: label width equals `measure_text_box` | yes |
| 7 | `apply_spec_is_one_undo_step_and_one_patch` | `designos-mcp` | with live ops on, `apply_spec` of the test-1 button records the same op count as before this change and one history entry | guard (passes before and after) |
| 8 | `fallback_font_is_reported` | `designos-mcp` | a label in family `"NoSuchFamily"`: reply has `measured_with_fallback` with its id and the warning code | yes |
| 9 | `no_measurer_keeps_the_estimate` | `designos-doc` | without `install`, `intrinsic_text_size` equals `estimate_intrinsic_text_size` | guard |

Dogfood evidence (PR): rebuild the board's `Delete`, `Export` and `Share` buttons with `designos --apply-spec` on the PR build. Render at scale 3 with `--screenshot`, or with MCP `designos_render_png` to avoid #1141. Attach the image next to `evidence/button-label-offset@3x.png`, with the measured left and right gaps per button.

Run:
- `cargo test -p designos-doc text_measure`
- `cargo test -p designos-render measure_text_box`
- `cargo test -p designos-mcp`
- `cargo test --workspace --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## 11. Performance budgets

- One `text_geometry` call per text node created or changed by a tool call, replacing a character loop. INFERENCE: well under 1 ms per short label. Closing check: time `apply_spec` of a 156-node board before and after in the PR, with a budget of at most 2× the current time.
- `text_geometry` returns `Rc<TextGeometry>`; only the `Vec2` is kept, and nothing is cached across calls.
- No cost in Studio's frame loop: the measurer runs only in spec, create and update paths.

## 12. Dependencies and build order

Independent of the revamp specs. Order:
1. `text_measure` hook in `designos-doc` (test 9).
2. `measure_text_box` in `designos-render` (test 2).
3. Install it in both binaries.
4. Spec, label and instance paths (tests 1, 6, 7).
5. create and update paths (3, 4, 5).
6. Fallback report (8).
7. Dogfood render.
8. PR (squash).

## Acceptance

1. Tests 1, 3, 4, 5, 6 and 8 were seen failing before the fix and pass after it. Tests 2, 7 and 9 pass. All run in `cargo test --workspace --locked`.
2. The dogfood render of `Delete`, `Export` and `Share` shows left and right label gaps within 0.5 px per button (measured values pasted next to the image).
3. `apply_spec` of a 156-node board takes no more than 2× its current time (both timings pasted).
4. No colour is involved, so the token-contract C-1 pairs (`specs/token-contract.md` section (f)) do not apply.

## 13. Owner rulings that apply

- 2026-10-25 R3 and the dogfood rule: Studio builds its own boards through its CLI and MCP, so their renders must be true.
- CLAUDE.md marketing rule: demos must be true to the released build. Off-centre labels show up in every agent-built demo.
- Agent undo is one step per tool call (CLAUDE.md marketing section); rule 7 keeps it.
- 2026-09-22: visual evidence in every PR.
- Standing: `main` is protected.

## Decisions on the open questions (writer, 2026-10-25)

1. **Fix direction: (a) a measurer used inside `apply_spec`, or (b) re-measure after apply in designos-mcp and the CLI, then relayout?** **(a)**, as a measurer installed once per process rather than threaded through every signature.
   - It fixes the size inside the one command each tool builds. Undo stays one step, the live patch carries no extra ops (rule 7), and the relayout that follows already sees the right sizes.
   - (b) needs a second `Update` command per hug text: a second undo step, plus one extra whole-node op per text in the live patch (`Command::Update { node: Box<Node> }`).
   - (b) also misses the paths inside `designos-doc`: instance overrides at `spec_instance.rs:1263`, and batch trial documents at `batch.rs:1808`.
2. **Why installed, not passed as a parameter?** The estimate is reached three calls deep from `apply_spec_command`, from instance override code and from batch trial documents. One `OnceLock` covers them all and leaves every `designos-doc` signature and test unchanged.
3. **Round to whole pixels like the editor?** No, to 0.25 px. Whole-pixel ceiling can leave up to 1 px of extra right padding. The issue's acceptance is equal padding within 0.5 px.
4. **Should Studio re-measure on open?** No. Studio's own rule keeps file boxes untouched (`font_refit.rs:8-9`). Old agent-built files can be refreshed by re-running their spec.

## Open questions for the owner

None. Labels being centred is the expected behaviour; nothing here is a design choice.
