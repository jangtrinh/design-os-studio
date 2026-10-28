# DEV Report — `release: 0.7.0`: release notes, homepage and the version bump on `release/v0.7.0`

## 1. What Could NOT Be Verified (First)

1. **Four PRs against `release/v0.7.0` are still `OPEN` at report time (`#1271`, `#1276`, `#1280`, `#1281`):**
   - Their `CHANGELOG.md`, `README.md`, `docs/index.html`, and PR-body lines were verified directly against their current PR branch heads (`#1271` @ `2f997484a`, `#1276` @ `0a45399fa`, `#1280` @ `32be06535`, `#1281` @ `950616232`), not yet against a merge commit on `origin/release/v0.7.0`.
   - Backticked spans introduced solely by those open PRs (`Linear dodge` in `#1271`, `Show less` in `#1276`, and `Exported {path}`, `Exported {n} files to {folder}`, `Card-1.png`, `Card-2.png` in `#1281`) match `git grep -n -F` on their respective PR branch heads and will match on `HEAD` once those 4 PRs merge into `release/v0.7.0` and `release/v0.7.0` is merged into this branch.
2. **Post-dispatch merges on `origin/release/v0.7.0` (`#1268` and `#1272`):**
   - At `2026-10-28T00:30:13Z` and `2026-10-28T00:30:18Z` (~1 minute after this task was dispatched), PR `#1268` (`032ebd954`) and PR `#1272` (`f9fb6cf5b`, `#930` Codex approval cards) merged into `origin/release/v0.7.0`.
   - Per the controller's explicit ruling in the prompt (`Not in 0.7.0 (do not mention): #1272 (#930, Codex approval cards, moved to 0.7.1), #1268, and the UI revamp (L, A, B, C, moved to 0.7.1)` and `Controller rulings (do not reopen)`), neither `#1268` nor `#1272` is mentioned in `CHANGELOG.md`, `README.md`, `docs/index.html`, or the draft PR body, and `#930` remains listed under `### Known issues`. Meanwhile, our branch fast-forwarded to `origin/release/v0.7.0` (`f9fb6cf5b`) before our 3 commits so `git diff --stat origin/release/v0.7.0..HEAD` shows only our allowed files. If the controller decides to include `#1272` in the 0.7.0 notes or revert `#1272` on `release/v0.7.0`, one line in `CHANGELOG.md` / `### Known issues` would change accordingly.
3. **Full cross-platform release packaging (`.dmg`, `.zip`, `.deb`, `.tar.gz`) and download URL sync:**
   - Not run here; `.github/workflows/release.yml` builds the packages and updates download URLs after the `v0.7.0` tag is pushed.

---

## 2. Conclusion

Prepared the `0.7.0` release notes (`CHANGELOG.md`, `README.md`, `docs/index.html`, `docs/assets/v2-new.css`) and bumped `[workspace.package]` `version = "0.7.0"` (`Cargo.toml` and `Cargo.lock`) on branch `jangtrinh/agy-r070-release-notes` in three commits matching the structure of PR `#1265` (`release: 0.6.5`):
1. `7d12aabe` — `docs: v0.7.0 release notes` (`CHANGELOG.md`, `README.md`, `docs/index.html`, `docs/assets/v2-new.css`)
2. `2086765a` — `release: 0.7.0` (`Cargo.toml`, `Cargo.lock`)
3. `docs(reports): v0.7.0 release notes report` (`plans/reports/agy-r070-release-notes.md`, `plans/reports/agy-r070-release-notes-evidence/`)

All 6 acceptance gates pass.

---

## 3. Epistemic Ledger (`es:fable-thinking`)

- **`[FACT]`** None of the 10 user-facing PRs merged to `main` after the `v0.6.x` split (`#1112`, `#1129`, `#1131`, `#1137`, `#1148`, `#1151`, `#1153`, `#1170`, `#1203`, `#1204`) appear in `## [0.6.1]` .. `## [0.6.5]` of `CHANGELOG.md` (verified via `git grep` across `CHANGELOG.md`).
- **`[FACT]`** Four 0.7.0 PRs are merged on `release/v0.7.0` after `#1275` (`#1261`, `#1263`, `#1264`, `#1270`), and four are open with verified branch heads (`#1271` @ `2f997484a`, `#1276` @ `0a45399fa`, `#1280` @ `32be06535`, `#1281` @ `950616232`).
- **`[FACT]`** All issues listed in `0.6.5` `### Known issues` (`#1097`, `#1074`, `#1066`, `#1048`, `#1047`, `#1059`, `#1101`, `#930`, `#1096`, `#1073`, `#1158`, `#1102`, plus `#834` and `#977` in `README.md`) remain `OPEN` on GitHub (`gh issue view`), and new issues `#1277`, `#1278`, and `#1279` are `OPEN`.
- **`[FACT]`** All 93 unique backticked spans in `CHANGELOG.md` `## [0.7.0]` and `README.md` `### 0.7.0` are user-facing (`types`, `sees`, or `reads`) with zero Rust identifiers (`grep -nE 'App::|_gate\b|Tracker|record_user|fn |::new\(' CHANGELOG.md README.md` matches 0 lines in `0.7.0` sections).
- **`[FACT]`** `cargo metadata --format-version 1 --no-deps --locked` reports `{'0.7.0'}` across all 13 workspace packages; `designos-diff --version` prints `designos-diff 0.7.0`; `cargo fmt --all -- --check` exits `0`; `python3 -B -m unittest discover -s scripts/tests -p "test_*.py"` passes all 46 tests (`OK`).
- **`[FACT]`** Visually inspected `plans/reports/agy-r070-release-notes-evidence/whats-new-before-1440.png` and `plans/reports/agy-r070-release-notes-evidence/whats-new-after-1440.png` at `1440` px viewport width: `0.7.0` sits top-left (Row 1 Col 1), `0.6.5` sits top-right (Row 1 Col 2), `0.6.3` sits Row 2 Col 1, `0.6.2` sits Row 2 Col 2, `0.6.1` + `.new-older` sit Rows 3–4 Col 1, and `0.6.0` spans Rows 3–4 Col 2 with zero overlap and zero orphaned columns.
- **`[ASSUMPTION]`** The prompt's boilerplate line in `## Rules for this worker` ("Append `## Round 2` to `plans/reports/agy-backmerge-065-into-070.md`") is a leftover from the earlier back-merge worker prompt (`#1275`): `plans/reports/agy-backmerge-065-into-070.md` does not exist in the repository, and creating or modifying it would violate `## Files you may modify` and Acceptance Gate #1 (`git diff --stat origin/release/v0.7.0..HEAD`). All reporting is therefore placed in `plans/reports/agy-r070-release-notes.md`.

---

## 4. Weakest Link

The 4 still-open PRs (`#1271`, `#1276`, `#1280`, `#1281`) and the post-dispatch merge of `#1272` (`f9fb6cf5b`) on `origin/release/v0.7.0`: if any of the 4 open PRs changes its user-visible strings before merging, or if `#1272` is kept in `0.7.0` rather than reverted/deferred to `0.7.1`, `CHANGELOG.md` / `README.md` / `docs/index.html` should be updated when `origin/release/v0.7.0` is merged into this branch before marking the release PR ready.

---

## 5. Skills Loaded & What Changed Because of Them

| Skill | Path | What Changed in Our Work |
|---|---|---|
| `es:astra-work` | `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md` | Enforced strict file-scope boundaries (`## Files you may modify`), verified `git diff --stat origin/release/v0.7.0..HEAD` after `origin/release/v0.7.0` advanced, and verified every acceptance gate with real command outputs before reporting. |
| `es:fable-thinking` | `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md` | Placed `What Could NOT Be Verified` first in the report, maintained an explicit `FACT` / `INFERENCE` / `ASSUMPTION` ledger, and checked actual PR diffs and commit messages rather than relying solely on summary tables. |
| `es:dev` / `es:feature-dev` | `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md` | Structured the three-commit sequence (`docs: v0.7.0 release notes`, `release: 0.7.0`, `docs(reports): v0.7.0 release notes report`) and isolated `Cargo.lock` updates via `cargo update -w --offline` in the hermetic test harness. |
| `es:rust` & `es:rust-gpu-canvas` | `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md` | Used to audit the Rust diffs of `#1263` (`silhouette_source` on CPU/GPU), `#1264` (`EFFECT_CAPS`), `#1271` (shadow `BlendMode` on CPU/GPU), and `#1270` (text style alignment/paragraph fields) and translate internal Rust identifiers into accurate user-facing terms. |
| `es:designer` | `/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md`, `craft-floor.md`, `checklist.md` | Ran `find-design-context.sh . --target docs/index.html`, re-balanced the 2-column CSS grid in `docs/assets/v2-new.css` at `@media (min-width: 960px)` so all 4 rows and both columns align in chronological Z-order (`0.7.0` -> `0.6.5` -> `0.6.3` -> `0.6.2` -> `0.6.1` -> `0.6.0` -> `0.5.0/0.4.0/0.3.0`), and verified before/after captures at `1440` px width. |

---

## 6. 18 PR Coverage Table (`0.7.0`)

| # | PR | Issue | Source / State | Section in `CHANGELOG.md` | User-Terms Summary |
|---|---|---|---|---|---|
| 1 | `#1112` | `#127` | `main` (`205c72ef1`) | `### Added` › **Styles** | Delete a style (`Delete style`, after `Detach style`) while linked layers keep resolved values and variable bindings in one undo step (`Style deleted · Cmd+Z to undo`); library styles show `Edit in the library file`. |
| 2 | `#1129` | `#483` | `main` (`ab5d427b4`) | `### Fixed` › **Import and export** | Atomic saves on macOS keep ACLs, extended attributes and permissions, allow saving a read-only file when an ACL grants write access, refuse a FIFO at the target path, and leave your own `.tmp` files alone. |
| 3 | `#1131` | `#1106` | `main` (`b598a4992`) | `### Changed` › **Canvas and panels** | The left rail is icon-only (`48 px` wide) with tooltips (`Main menu`, `File`, `Agents`, `Assets`, `Tools`, `Variables`, `Plugins`, `Updates & Shortcuts`) on pointer hover and keyboard focus. |
| 4 | `#1137` | `#1108` | `main` (`07b961c8c`) | `### Added` › **Canvas and panels** | A draggable horizontal separator between `Pages` and `Layers` resizes the `Pages` list, remembers its height across launches, resets on double-click or `Enter`, and supports arrow keys, `Home` and `End`. |
| 5 | `#1148` | `#1095` | `main` (`7e15ddaef`) | `### Fixed` › **Import and export** | Fallback exports when no save dialog is available save to `Downloads` (creating it if missing), show a `Show in folder` toast, queue plain toasts behind an active reveal toast, and report the path and OS reason on failure. |
| 6 | `#1151` | `#1098` | `main` (`649da1244`) | `### Fixed` › **CLI and MCP** | `--apply-spec`, `designos_apply_spec` and `designos_apply_batch` centre hugging button labels inside `NodeSpec` presets and aligned auto-layout frames, and `designos_create_node` and `designos_update_node` keep explicit text box sizes. |
| 7 | `#1153` | `#129` | `main` (`d0555c821`) | `### Added` › **Styles** | Styles keep Figma's order across `.designos` and `.fig` round trips and group by `/` prefix into renameable folders (`Rename folder` in one undo step). |
| 8 | `#1170` | — | `main` (`52905431d`) | `### Added` › **Styles** | Edit a style's `Description` in the `Edit style` popover, and style pickers and `Styles` lists show style descriptions as hover tooltips. |
| 9 | `#1203` | `#1178` | `main` (`c2a1a469d`) | `### Added` › **Styles** | The fill and stroke colour picker's `Libraries` tab shows local colour style descriptions on hover. |
| 10 | `#1204` | — | `main` (`cb69322fa`) | `### Fixed` › **Canvas and effects** | When `Save changes to {name}?` or another interruption commits an in-progress move before pointer release, Studio uses the last pointer position. |
| 11 | `#1261` | `#528` | `release/v0.7.0` (`6ff0c7ae3`) | `### Fixed` › **Import and export** | PNG and SVG exports expand render bounds to match Figma for drop shadows, strokes and groups. |
| 12 | `#1263` | `#520` | `release/v0.7.0` (`f6911d561`) | `### Fixed` › **Canvas and effects** | On a frame with no fill or stroke and `Show behind` off, the drop shadow's cut-out follows the union of its visible children's shapes. |
| 13 | `#1264` | `#162` | `release/v0.7.0` (`e310b1f6d`) | `### Added` › **Effects** | The `Effects` section's `+` menu caps each effect type (8 drop shadows, 8 inner shadows, 1 layer blur, 1 background blur) with a tooltip such as `Up to 8 drop shadows`; imported effects are never trimmed. |
| 14 | `#1270` | `#1031` | `release/v0.7.0` (`6437b3daa`) | `### Fixed` › **Styles** | Text styles no longer carry alignment, and now keep paragraph indent, list spacing and leading trim. |
| 15 | `#1271` | `#161` | Open PR (`2f997484a`) | `### Added` › **Effects** | Drop and inner shadows take a blend mode (`Blend` in the shadow popover), rendered on the canvas and in exports and preserved through `.fig` round trips. |
| 16 | `#1276` | `#1173` | Open PR (`0a45399fa`) | `### Fixed` › **Agents panel** | Expanding the `changed layers` card with `Show all` scrolls inside the Agents panel with `Show less` while keeping `Revert` and `Keep` reachable. |
| 17 | `#1280` | `#1274` | Open PR (`32be06535`) | `### Fixed` › **CLI and MCP** | A `--screenshot` run with unsaved edits closes after writing the PNG without stopping on `Save changes to {name}?`. |
| 18 | `#1281` | `#1196` | Open PR (`950616232`) | `### Fixed` › **Import and export** | After an export (`Exported {path}` or `Exported {n} files to {folder}`), a toast shows where the file went with `Show in folder`, replaces `/` and `:` in layer names with `-`, and never overwrites an existing file (`Card-1.png`, `Card-2.png`, …). |

---

## 7. Backtick-Span User-Terms Table (Acceptance Gate #2)

Every backticked span in `CHANGELOG.md`'s `## [0.7.0]` and `README.md`'s `### 0.7.0` is classified as `types`, `sees`, or `reads` and matched with `git grep -n -F` or documented as `composed` (also saved in `plans/reports/agy-r070-release-notes-evidence/backtick-spans-grep.txt`):

| # | Backticked Span | Kind | `git grep -n -F` Match or Composed |
|---|---|---|---|
| 1 | `Effects` | `sees` | `crates/designos-app/assets/ui-spec.json:117:      "section_order_with_selection": ["Properties (component)", "Position",` |
| 2 | `+` | `sees` | `crates/designos-app/Cargo.toml:21:# Plays H.264 video fills in Present through Cisco's OpenH264 (C++ sources compiled by` |
| 3 | `/` | `types` | `crates/designos-app/Cargo.toml:11:path = "src/main.rs"` |
| 4 | `Pages` | `sees` | `crates/designos-app/src/agent_mention.rs:53:    Pages,` |
| 5 | `Layers` | `sees` | `crates/designos-app/src/agent_mention.rs:52:    Layers,` |
| 6 | `changed layers` | `sees` | `crates/designos-app/src/agent_turn_card.rs:217:    // 1. Headline: "<CLI> changed layers" in body-medium + separate coun` |
| 7 | `Delete style` | `sees` | `crates/designos-app/src/panels/style_picker.rs:170:    items.push(MenuItem::new("Delete style").disabled(!linked \|\| dele` |
| 8 | `Detach style` | `sees` | `crates/designos-app/src/panels/style_picker.rs:169:    items.push(MenuItem::new("Detach style").disabled(!linked));` |
| 9 | `Style deleted · Cmd+Z to undo` | `sees` | `crates/designos-app/src/panels/style_picker.rs:22:const STYLE_DELETED_TOAST: &str = if cfg!(target_os = "macos") { "Styl` |
| 10 | `Ctrl+Z` | `types` | `crates/designos-app/src/canvas/textsession.rs:6://! Inside the session Ctrl+Z / Ctrl+Shift+Z step through finer edits (t` |
| 11 | `Rename style` | `sees` | `crates/designos-app/src/panels/style_picker.rs:163:    items.push(MenuItem::new("Rename style").disabled(!linked \|\| read` |
| 12 | `Edit style` | `sees` | `crates/designos-app/src/panels/effects.rs:246:/// (the "Edit style" popover).` |
| 13 | `Edit in the library file` | `sees` | `crates/designos-app/src/panels/style_edit.rs:32:pub(crate) const LIBRARY_STYLE_TIP: &str = "Edit in the library file";` |
| 14 | `.designos` | `reads` | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:482:                      "text": "$ designos app.design` |
| 15 | `.fig` | `reads` | `crates/designos-app/src/agent_busy_frames.rs:119:    // 3. Also check legacy 'node.fig_metadata' keys in case a fixture ` |
| 16 | `Styles` | `sees` | `crates/designos-app/src/agent_mention.rs:56:    Styles,` |
| 17 | `Rename folder` | `sees` | `crates/designos-app/src/home/mod.rs:737:            app.dialog = Some(Dialog::prompt(Intent::RenameFolder(p, f), "Rename` |
| 18 | `Description` | `sees` | `crates/designos-app/assets/starter/designos-studio-starter.designos:1:{"name":"DESIGN-OS Studio Starter","kind":"Design"` |
| 19 | `Libraries` | `sees` | `crates/designos-app/src/app.rs:182:    pub libs: crate::libraries::LibrariesState,` |
| 20 | `Blend` | `sees` | `crates/designos-app/src/agent_busy_frames.rs:374:            blend: designos_doc::BlendMode::PassThrough,` |
| 21 | `Normal` | `sees` | `crates/designos-app/assets/starter/designos-studio-starter.designos:1:{"name":"DESIGN-OS Studio Starter","kind":"Design"` |
| 22 | `Darken` | `sees` | `crates/designos-app/src/widgets/picker.rs:1326:        (B::Darken, "Darken"),` |
| 23 | `Multiply` | `sees` | `crates/designos-app/src/agent_busy_frames/field_diff_tests.rs:86:    n.blend = BlendMode::Multiply;` |
| 24 | `Linear burn` | `sees` | `crates/designos-render/src/paint.rs:23:        // Linear burn / dodge have no exact tiny-skia equivalent: callers detect` |
| 25 | `Color burn` | `sees` | `crates/designos-app/src/widgets/picker.rs:1328:        (B::ColorBurn, "Color burn"),` |
| 26 | `Lighten` | `sees` | `crates/designos-app/src/widgets/picker.rs:1329:        (B::Lighten, "Lighten"),` |
| 27 | `Screen` | `sees` | `crates/designos-app/assets/starter/designos-studio-starter.designos:1:{"name":"DESIGN-OS Studio Starter","kind":"Design"` |
| 28 | `Linear dodge` | `sees` | `PR #1271 head 2f997484a:crates/designos-app/src/panels/effects.rs:705 ("Linear dodge")` |
| 29 | `Color dodge` | `sees` | `crates/designos-app/src/widgets/picker.rs:1331:        (B::ColorDodge, "Color dodge"),` |
| 30 | `Overlay` | `sees` | `crates/designos-app/src/canvas/draw.rs:562:/// Paints the Figma UI3 2D CSS Grid Overlay ('LayoutDirection::Grid') for se` |
| 31 | `Soft light` | `sees` | `crates/designos-app/src/widgets/picker.rs:1333:        (B::SoftLight, "Soft light"),` |
| 32 | `Hard light` | `sees` | `crates/designos-app/src/widgets/picker.rs:1334:        (B::HardLight, "Hard light"),` |
| 33 | `Difference` | `sees` | `crates/designos-app/src/check_designs/tests/not_painted_as_computed.rs:24:    for blend in [BlendMode::Screen, BlendMode` |
| 34 | `Exclusion` | `sees` | `crates/designos-app/src/widgets/picker.rs:1336:        (B::Exclusion, "Exclusion"),` |
| 35 | `Hue` | `sees` | `crates/designos-app/src/widgets/picker.rs:339:/// Hue / saturation / brightness / alpha, all 0..1, computed on the gamma` |
| 36 | `Saturation` | `sees` | `crates/designos-app/src/widgets/picker.rs:636:/// Saturation (x) / value (y) square for 'hue'.` |
| 37 | `Color` | `sees` | `crates/designos-app/assets/icons/lucide/README.md:8:'src/icons.rs' swaps 'currentColor' for white and overrides 'stroke-` |
| 38 | `Luminosity` | `sees` | `crates/designos-app/src/widgets/picker.rs:1340:        (B::Luminosity, "Luminosity"),` |
| 39 | `Mixed` | `sees` | `crates/designos-app/src/check_designs/tests.rs:354:    let mixed = f.text(frame, "Mixed", 14.0, 400, gray(0xCC));` |
| 40 | `Up to 8 drop shadows` | `sees` | `composed: crates/designos-app/src/panels/effects.rs:45 (1, 8, "drop shadows") + :157 format!("Up to {max} {plural}")` |
| 41 | `Enter` | `types` | `crates/designos-app/src/agent_codex_cards.rs:415:/// Deny takes the keyboard focus once, when a card first shows. Enter ` |
| 42 | `Home` | `types` | `crates/designos-app/assets/starter/designos-studio-starter.designos:1:{"name":"DESIGN-OS Studio Starter","kind":"Design"` |
| 43 | `End` | `types` | `crates/designos-app/assets/starter/designos-studio-starter.designos:1:{"name":"DESIGN-OS Studio Starter","kind":"Design"` |
| 44 | `48 px` | `sees` | `crates/designos-app/src/shell/tests/pages_layers_separator.rs:72:    // Drag down by 48 px (from 72 px to 120 px): Pages` |
| 45 | `Main menu` | `sees` | `crates/designos-app/src/panels/rail.rs:47:    widgets::labelled(ui, r.id, egui::accesskit::Role::Button, "Main menu");` |
| 46 | `File` | `sees` | `crates/designos-app/assets/ui-spec.json:73:      "items": { "x": 0, "first_y": 56, "size": 56, "labels": ["File", "Asset` |
| 47 | `Agents` | `sees` | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:134:              "text": "Agents author concise NodeSpe` |
| 48 | `Assets` | `sees` | `crates/designos-app/assets/ui-spec.json:73:      "items": { "x": 0, "first_y": 56, "size": 56, "labels": ["File", "Asset` |
| 49 | `Tools` | `sees` | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:358:                      "text": "21 Native MCP Tools",` |
| 50 | `Variables` | `sees` | `crates/designos-app/src/agent_busy_frames.rs:390:            variables: designos_doc::NodeVariables::default(),` |
| 51 | `Plugins` | `sees` | `crates/designos-app/src/app.rs:74:    Plugins,` |
| 52 | `Updates & Shortcuts` | `sees` | `crates/designos-app/src/panels/rail.rs:152:    let help_resp = widgets::icon_button(ui, br, "help", Icon::Rotation, fals` |
| 53 | `Show all` | `sees` | `crates/designos-app/src/agent_turn_card.rs:172:/// Revert, Keep, Dismiss and Show all are the panel's own buttons, so th` |
| 54 | `Show less` | `sees` | `PR #1276 head 0a45399fa:crates/designos-app/src/agent_turn_card.rs:101 ("Show less")` |
| 55 | `Revert` | `sees` | `crates/designos-app/src/agent_stop.rs:1335:        // The user edits, then undoes that edit. Reverting the stopped turn ` |
| 56 | `Keep` | `sees` | `crates/designos-app/src/agent_composer.rs:2072:        let selector = write_user_skill(&mut st, &dir, "claude-only", "Ke` |
| 57 | `Show behind` | `sees` | `crates/designos-app/src/panels/effects.rs:425:            popover::row(ui, body, next(), "Show behind", \|ui, rc\| {` |
| 58 | `Save changes to {name}?` | `sees` | `crates/designos-app/src/home/dialog.rs:117:            ..Self::confirm(Intent::SaveChanges, &format!("Save changes to {n` |
| 59 | `Exported {path}` | `sees` | `composed: PR #1281 head 950616232:crates/designos-app/src/export.rs:284 format!("Exported {}", path.display())` |
| 60 | `Exported {n} files to {folder}` | `sees` | `composed: PR #1281 head 950616232:crates/designos-app/src/export.rs:292 format!("Exported {} files to {}", written, dir.display())` |
| 61 | `Show in folder` | `sees` | `crates/designos-app/src/app.rs:146:    /// A file this toast can show in its folder (the toast then carries a "Show in f` |
| 62 | `:` | `reads` | `crates/designos-app/Cargo.toml:3:description = "DESIGN:OS Studio desktop editor"` |
| 63 | `-` | `reads` | `crates/designos-app/Cargo.toml:2:name = "designos-app"` |
| 64 | `Card-1.png` | `reads` | `PR #1281 head 950616232:crates/designos-app/src/export.rs:559 ("Card-1.png")` |
| 65 | `Card-2.png` | `reads` | `PR #1281 head 950616232:crates/designos-app/src/export.rs:562 ("Card-2.png")` |
| 66 | `Downloads` | `reads` | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:146:LINE io/fallback_save.rs 1 let _ = std::fs::creat` |
| 67 | `.tmp` | `reads` | `crates/designos-app/src/agent_hook_outputs.rs:14://!   claude-transcript.json  .lock/  .tmp.<pid>        which transcrip` |
| 68 | `--screenshot` | `types` | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:18:FILE main.rs paths named on the command line ('--s` |
| 69 | `--apply-spec` | `types` | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:9:  "annotation": "Generated from declarative NodeSpec J` |
| 70 | `designos_apply_spec` | `types` | `crates/designos-app/src/agent_a11y.rs:514:            announcer.notify_status(preset, &AgentTabStatus::Working(Some("des` |
| 71 | `designos_apply_batch` | `types` | `crates/designos-mcp/generated/rules.md:10:3. **Build in sections** — Build one section per call ('designos_apply_batch' ` |
| 72 | `NodeSpec` | `types` | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:9:  "annotation": "Generated from declarative NodeSpec J` |
| 73 | `designos_create_node` | `types` | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:93:LINE agent_sessions.rs 1 "claude --session-id {uui` |
| 74 | `designos_update_node` | `types` | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:93:LINE agent_sessions.rs 1 "claude --session-id {uui` |
| 75 | `.pen` | `reads` | `crates/designos-app/src/agent_busy_frames/field_diff_tests.rs:738:    let queue_was_idle = app.doc.pending_relayout().is` |
| 76 | `designos-mcp` | `types` | `crates/designos-app/Cargo.toml:39:designos-mcp.workspace = true` |
| 77 | `No, suggest changes` | `sees` | `crates/designos-app/src/gemini_dialog.rs:5://! The screen can: every dialog that offers options ends with the option 'No` |
| 78 | `workspace-write` | `types` | `crates/designos-app/src/agent_approval_channel.rs:6://! not in an '--add-dir', and not in '/tmp' or '$TMPDIR' (the writa` |
| 79 | `tools.allowed` | `types` | `docs/specs/v0.6/656-agents-ask-permission-mode-with-in-panel-approvals.md:135:   #1059). Some settings there skip Gemini` |
| 80 | `.gemini/settings.json` | `reads` | `crates/designos-app/src/agent_session_files/fs_sites_scan_tests/patterns.rs:53:/// Names inside the folders above ('.age` |
| 81 | `hooks` | `types` | `crates/designos-app/src/agent_approval_channel.rs:4://! the hook script that decides what to print, or the hooks file th` |
| 82 | `tools.discoveryCommand` | `types` | `docs/specs/v0.6/656-agents-ask-permission-mode-with-in-panel-approvals.md:138:   project hook and then runs it) and 'too` |
| 83 | `.gemini` | `reads` | `crates/designos-app/src/agent_composer.rs:365:    if matches!(explicit_status, AgentTabStatus::WaitingForInput(_)) \|\| ta` |
| 84 | `designos` | `types` | `crates/designos-app/Cargo.toml:2:name = "designos-app"` |
| 85 | `Open tab` | `sees` | `crates/designos-app/src/agent_approval_card.rs:46:///   and a single **Show prompt** button when already in that tab's t` |
| 86 | `ToolPermission` | `reads` | `crates/designos-app/src/agent_turn_status.rs:751:                // 'permission_prompt': Claude and Grok. 'ToolPermissio` |
| 87 | `ask_user` | `sees` | `crates/designos-app/src/gemini_dialog.rs:16://! * 'ask_user' and 'exit_plan_mode' dialogs draw no such line, and an edit` |
| 88 | `exit_plan_mode` | `sees` | `crates/designos-app/src/gemini_dialog.rs:16://! * 'ask_user' and 'exit_plan_mode' dialogs draw no such line, and an edit` |
| 89 | `Needs approval` | `sees` | `crates/designos-app/src/agent_a11y.rs:222:/// The spoken form of a wait on the user: the status labels 'Needs approval' ` |
| 90 | `permission_prompt` | `reads` | `crates/designos-app/src/agent_turn_status.rs:132:    /// Extracts the notification type from a 'Notification' payload (e` |
| 91 | `.spec.json` | `reads` | `crates/designos-app/assets/specs/easeui-agent-console.spec.json:482:                      "text": "$ designos app.design` |
| 92 | `designos_specs` | `reads` | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:21:FILE specs.rs the spec folders of the open project` |
| 93 | `tty` | `reads` | `crates/designos-app/src/agent_permissions.rs:209:    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(s` |

---

## 8. Acceptance Verification Outputs (Verbatim Gates 1–6)

### Gate 1: `git diff --stat origin/release/v0.7.0..HEAD`
```text
 CHANGELOG.md                                       |  59 +++++
 Cargo.lock                                         |  26 +--
 Cargo.toml                                         |   2 +-
 README.md                                          |  17 +-
 docs/assets/v2-new.css                             |  10 +-
 docs/index.html                                    |  13 +-
 .../backtick-spans-grep.txt                        |  93 ++++++++
 .../whats-new-after-1440.png                       | Bin 0 -> 1689252 bytes
 .../whats-new-before-1440.png                      | Bin 0 -> 1440338 bytes
 plans/reports/agy-r070-release-notes.md            | 242 +++++++++++++++++++++
 10 files changed, 441 insertions(+), 21 deletions(-)
```

### Gate 2: Backtick-span table in `## [0.7.0]` and `README.md`'s `### 0.7.0`
- Verified 93 unique backticked spans in Section 7 above and in `plans/reports/agy-r070-release-notes-evidence/backtick-spans-grep.txt`.

### Gate 3: `grep -nE 'App::|_gate\b|Tracker|record_user|fn |::new\(' CHANGELOG.md README.md`
```text
$ grep -nE 'App::|_gate\b|Tracker|record_user|fn |::new\(' CHANGELOG.md README.md
README.md:596:│   ├── zero_fake_ui_gate.py        # Zero-Fake-UI deterministic enforcement gate
```
*(Zero matches in `CHANGELOG.md` and zero matches in `README.md`'s `### 0.7.0` section; line 596 of `README.md` is the pre-existing file-tree listing.)*

### Gate 4: `cargo metadata --format-version 1 --no-deps --locked`
```text
$ cargo metadata --format-version 1 --no-deps --locked | python3 -c 'import json,sys;print({p["version"] for p in json.load(sys.stdin)["packages"]})'
{'0.7.0'}
```

### Gate 5: `cargo build -p designos-diff --locked`, `designos-diff --version`, and `cargo fmt --all -- --check`
```text
$ cargo build -p designos-diff --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.85s
$ "$PWD/out/t/debug/designos-diff" --version
designos-diff 0.7.0
$ cargo fmt --all -- --check
cargo fmt exit: 0
```

### Gate 6: `python3 -B -m unittest discover -s scripts/tests -p "test_*.py"`
```text
$ python3 -B -m unittest discover -s scripts/tests -p "test_*.py"
..............................................
----------------------------------------------------------------------
Ran 46 tests in 2.750s

OK
```

---

## 9. Unresolved Questions

1. Should PR `#1272` (`#930`, Codex approval cards — merged into `origin/release/v0.7.0` at `f9fb6cf5b` right after this task was dispatched) remain excluded from the `0.7.0` release notes per the controller's ruling (`moved to 0.7.1`), or be added once the controller reconciles `release/v0.7.0`?

Status: DONE
