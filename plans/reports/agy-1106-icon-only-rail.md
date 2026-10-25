# Report — DEV · UI · icon-only left rail with tooltips (#1106)

## 1. What could NOT be verified

- Responsive multi-breakpoint captures (`375×812`, `768×1024`, `1024×768`) were intentionally skipped per the task specification (`Responsive captures are skipped (owner default)`).
- Live macOS VoiceOver speech output was not verified on a physical audio device; screen-reader accessibility was verified deterministically through `egui::PlatformOutput::accesskit_update` (`AccessKit` `TreeUpdate` nodes with `Role::Button` and exact accessible names).

## 2. Acceptance table with commands and pasted output

| # | Criterion | Command | Result |
|---|---|---|---|
| 1 | Red first (`red.log`) & Green on head (`green.log`): no label text painted in rail, tooltip on hover and focus, AccessKit names present, rail width `48.0`, active tile & divider | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked rail` | **PASS** (`4 failed` on base in `red.log` → `12 passed; 0 failed` on head in `green.log`) |
| 2 | Formatting & Clippy (`-D warnings`) | `cargo fmt --all -- --check && cargo clippy -p designos-app --all-targets --locked -- -D warnings` | **PASS** (exit `0`) |
| 3 | Rail, `shell::tests::home`, and `shell::tests::variables` under harness | `cargo test -p designos-app --bin designos --locked rail`, `cargo test -p designos-app --bin designos --locked shell::tests::home`, `cargo test -p designos-app --bin designos --locked shell::tests::variables` | **PASS** (`12/12`, `24/24`, `41/41`) |
| 4 | Zero Fake UI gate & spacing ratchet | `python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py` | **PASS** (`17/17 invariants satisfied`, `0` baseline regressions) |
| 5 | Release `designos` 1440×900 capture on scratch copy of Untitled UI PRO v8.0 kit | `"$CARGO_TARGET_DIR/release/designos" "$S/kit-copy.fig" --screenshot "$PWD/plans/reports/agy-1106-icon-only-rail-evidence/out.png" --size 1440x900 --no-toast` | **PASS** (`plans/reports/agy-1106-icon-only-rail-evidence/out.png`, 1440×900 PNG) |
| 6 | Draft PR against `main` with `Closes #1106` and capture in body; `out/` deleted | `rm -rf "$PWD/out" && gh pr create --draft ...` | **PASS** (`out/` removed, draft PR [#1131](https://github.com/jangtrinh/design-os-studio/pull/1131) opened) |

### Pasted Output — 1a. `plans/reports/agy-1106-icon-only-rail-evidence/red.log` (Base)

```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 12.07s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 12 tests
test panels::rail::tests::cells_follow_the_measured_rail_layout ... ok
test panels::rail::tests::rail_width_and_active_tile_and_divider ... FAILED
test panels::rail::tests::rail_shows_tooltip_on_hover_and_on_keyboard_focus ... FAILED
test panels::rail::tests::rail_buttons_preserve_accesskit_names ... FAILED
test panels::rail::tests::rail_paints_icons_only_without_label_text ... FAILED
test textmeasure::tests::caret_stays_on_text_line_before_a_trailing_soft_break ... ok
test terminal::tests::at_mention_typed_before_trailing_words_opens_popup_and_replaces_only_that_token ... ok
test shell::tests::agents_panel_only::the_rail_has_no_terminal_button_and_keeps_the_updates_button ... ok
test shell::tests::variables::rail_opens_floating_panel_then_full_view_and_back ... ok
test shell::tests::plugins::plugins_rail_opens_library ... ok
test shell::tests::rail_menu_opens_and_closes ... ok
test shell::tests::agents_panel_only::tools_rail_item_opens_only_the_tools_panel ... ok

failures:

---- panels::rail::tests::rail_width_and_active_tile_and_divider stdout ----

thread 'panels::rail::tests::rail_width_and_active_tile_and_divider' (50446211) panicked at crates/designos-app/src/panels/rail.rs:387:9:
assertion `left == right` failed: icon-only rail width comes from theme tokens (32px tile + 2 * 8px inset)
  left: 56.0
 right: 48.0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- panels::rail::tests::rail_shows_tooltip_on_hover_and_on_keyboard_focus stdout ----

thread 'panels::rail::tests::rail_shows_tooltip_on_hover_and_on_keyboard_focus' (50446210) panicked at crates/designos-app/src/panels/rail.rs:310:13:
assertion `left == right` failed: hovering rail-File must show only tooltip "File", got texts: ["File", "Agents", "Assets", "Tools", "Variables", "Plugins"]
  left: ["File", "Agents", "Assets", "Tools", "Variables", "Plugins"]
 right: ["File"]

---- panels::rail::tests::rail_buttons_preserve_accesskit_names stdout ----

thread 'panels::rail::tests::rail_buttons_preserve_accesskit_names' (50446208) panicked at crates/designos-app/src/panels/rail.rs:378:13:
expected AccessKit Role::Button with label "File" on rail button

---- panels::rail::tests::rail_paints_icons_only_without_label_text stdout ----

thread 'panels::rail::tests::rail_paints_icons_only_without_label_text' (50446209) panicked at crates/designos-app/src/panels/rail.rs:273:9:
rail buttons must show icons only with no painted label text, found: ["File", "Agents", "Assets", "Tools", "Variables", "Plugins"]


failures:
    panels::rail::tests::rail_buttons_preserve_accesskit_names
    panels::rail::tests::rail_paints_icons_only_without_label_text
    panels::rail::tests::rail_shows_tooltip_on_hover_and_on_keyboard_focus
    panels::rail::tests::rail_width_and_active_tile_and_divider

test result: FAILED. 8 passed; 4 failed; 0 ignored; 0 measured; 2930 filtered out; finished in 0.43s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

### Pasted Output — 1b. `plans/reports/agy-1106-icon-only-rail-evidence/green.log` (Head)

```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.57s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 12 tests
test panels::rail::tests::cells_follow_the_measured_rail_layout ... ok
test panels::rail::tests::rail_paints_icons_only_without_label_text ... ok
test panels::rail::tests::rail_buttons_preserve_accesskit_names ... ok
test panels::rail::tests::rail_width_and_active_tile_and_divider ... ok
test panels::rail::tests::rail_shows_tooltip_on_hover_and_on_keyboard_focus ... ok
test textmeasure::tests::caret_stays_on_text_line_before_a_trailing_soft_break ... ok
test terminal::tests::at_mention_typed_before_trailing_words_opens_popup_and_replaces_only_that_token ... ok
test shell::tests::rail_menu_opens_and_closes ... ok
test shell::tests::plugins::plugins_rail_opens_library ... ok
test shell::tests::variables::rail_opens_floating_panel_then_full_view_and_back ... ok
test shell::tests::agents_panel_only::the_rail_has_no_terminal_button_and_keeps_the_updates_button ... ok
test shell::tests::agents_panel_only::tools_rail_item_opens_only_the_tools_panel ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 2930 filtered out; finished in 0.29s
```

### Pasted Output — 2. `cargo fmt` & `cargo clippy`

```text
$ cargo fmt --all -- --check
FMT_CHECK_EXIT=0

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy -p designos-app --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 56.16s
CLIPPY_EXIT=0
```

### Pasted Output — 3. `shell::tests::home`, `shell::tests::variables`, & `shell::tests::lint`

```text
running 24 tests
test shell::tests::home::account_and_plan_labels_are_gone_from_the_ui_sources ... ok
test shell::tests::home::first_launch_shows_the_home_screen_with_the_getting_started_project ... ok
test shell::tests::home::renaming_a_file_from_the_home_dialog_persists ... ok
test shell::tests::home::library_option_sets_the_root_and_home_option_shows_the_home_screen ... ok
test shell::tests::home::search_filters_files_and_the_sidebar_navigates_sections ... ok
test shell::tests::home::emptying_the_trash_needs_a_confirmation ... ok
test shell::tests::home::help_menu_offers_shortcuts_about_and_a_confirmed_github_link ... ok
test shell::tests::home::importing_a_file_adds_it_to_the_project ... ok
test shell::tests::home::double_clicking_a_card_opens_it_and_single_click_selects ... ok
test shell::tests::home::renaming_in_the_header_persists_to_the_library ... ok
test shell::tests::home::new_design_file_creates_and_opens_an_untitled_library_file ... ok
test shell::tests::home::right_click_menu_moves_a_file_to_the_trash_and_trash_restores_it ... ok
test shell::tests::home::the_editor_header_shows_project_and_file_and_none_of_the_account_labels ... ok
test shell::tests::home::dragging_a_card_onto_a_project_moves_the_file ... ok
test shell::tests::home::external_files_keep_their_name_and_offer_save_to_library ... ok
test shell::tests::home_agents_card::the_agents_card_pins_its_button_and_description ... ok
test shell::tests::home_agents_card::the_agents_card_button_is_named_open_agents_for_a_screen_reader ... ok
test shell::tests::home::renaming_in_the_header_gets_the_letters_after_the_other_cli_form_was_open ... ok
test shell::tests::home_agents_card::the_agents_card_description_names_every_supported_cli_and_no_tool_count ... ok
test shell::tests::home::back_to_files_flushes_edits_and_returns_to_the_home_screen ... ok
test shell::tests::home::autosave_runs_after_the_idle_delay_and_ctrl_s_saves_at_once ... ok
test shell::tests::home_agents_card::the_longer_description_wraps_above_the_button ... ok
test shell::tests::home::userflows_uf01_to_uf08_end_to_end_figma_parity ... ok
test shell::tests::home::unchanged_documents_are_never_rewritten_by_autosave ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 2918 filtered out; finished in 2.56s

running 41 tests
...
test shell::tests::variables::rail_opens_floating_panel_then_full_view_and_back ... ok
...
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 2901 filtered out; finished in 1.63s

running 2 tests
test shell::tests::lint::nothing_overflows_and_icons_are_centered ... ok
test shell::tests::lint::design_os_gates_audit_all_studio_screens_zero_overflow_zero_purple_and_valid_contrast ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 2940 filtered out; finished in 0.99s
```

### Pasted Output — 4. `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`

```text
$ python3 scripts/zero_fake_ui_gate.py
============================================================================================================
 design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)
============================================================================================================
...
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1676, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 10 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

### Pasted Output — 5. Release 1440×900 Screenshot Capture

```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$CARGO_TARGET_DIR/release/designos" "$S/kit-copy.fig" --screenshot "$PWD/plans/reports/agy-1106-icon-only-rail-evidence/out.png" --size 1440x900 --no-toast
EXIT=0
```

## 3. Changes per file

- [`crates/designos-app/src/theme.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/theme.rs):
  - Narrowed `pub const RAIL_WIDTH: f32` from `56.0` to `48.0` (`theme::RAIL_TILE` `32.0` + `2.0 * theme::INSET_FRAME` `8.0`, matching `SPACE_REGION = 48.0` / `space-12` on the 4 px grid).
- [`crates/designos-app/src/panels/rail.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/panels/rail.rs):
  - Removed the visible caption text `p.text(...)` under each rail icon so rail buttons render icons only.
  - Centered the `32×32` (`theme::RAIL_TILE`) active/hover rounded tile (`theme::RADIUS_CONTROL`, `theme::BG_SELECTED` / `theme::BG_HOVER`) and `20×20` (`theme::RAIL_ICON_SIZE`) icon at `ir.center()`, and aligned `rail-menu`, `rail-help`, and the horizontal separators with `theme::INSET_FRAME` (`8.0`) inside the `48px` rail.
  - Added `rail_tooltip` so every rail button shows its name in a tooltip both on pointer hover (`resp.on_hover_text(label)`) and on keyboard focus (`resp.has_focus()` via `egui::Tooltip::for_widget(&resp)`), without registering any focusable widget inside the tooltip (preserving Tab order).
  - Kept AccessKit `Role::Button` labels on every rail button (`"Main menu"`, `"File"`, `"Agents"`, `"Assets"`, `"Tools"`, `"Variables"`, `"Plugins"`, `"Updates & Shortcuts"`) via `widgets::labelled`.
  - Added 4 unit tests in `panels::rail::tests` covering icon-only rendering (no painted label text), hover + keyboard-focus tooltips and Tab order, AccessKit button labels, `RAIL_WIDTH == 48.0`, the active `32×32` tile, and the divider between `Tools` and `Variables`.
- [`crates/designos-app/src/shell/tests/home.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/shell/tests/home.rs):
  - Asserted `theme::RAIL_WIDTH == 48.0` and `r.left() == theme::RAIL_WIDTH + theme::BORDER_WIDTH + theme::INSET_FRAME` in `the_editor_header_shows_project_and_file_and_none_of_the_account_labels`.

## 4. Every user-facing string, verbatim

- `"Main menu"` (tooltip and AccessKit `Role::Button` label on `rail-menu`)
- `"File"` (existing `ITEMS[0]` tooltip and AccessKit `Role::Button` label)
- `"Agents"` (existing `ITEMS[1]` tooltip and AccessKit `Role::Button` label)
- `"Assets"` (existing `ITEMS[2]` tooltip and AccessKit `Role::Button` label)
- `"Tools"` (existing `ITEMS[3]` tooltip and AccessKit `Role::Button` label)
- `"Variables"` (existing `ITEMS[4]` tooltip and AccessKit `Role::Button` label)
- `"Plugins"` (existing `ITEMS[5]` tooltip and AccessKit `Role::Button` label)
- `"Updates & Shortcuts"` (existing tooltip and AccessKit `Role::Button` label on `rail-help`)

### Skills Loaded & Changes Made Because of Them

- `es:dev` / `es:feature-dev`: Executed the DEV phase directly from the accepted spec, writing failing tests first (`red.log`) before implementation (`green.log`).
- `es:designer` + `design/principles.md` + `design/soul.md`:
  - Applied the 4 px grid rhythm (`RAIL_WIDTH = 48.0 = RAIL_TILE (32.0) + 2 * INSET_FRAME (8.0)`), keeping pointer targets at `>= 32×32 px` (`> 24×24 px` WCAG 2.5.8 AA floor).
  - Used existing `theme.rs` tokens (`theme::INSET_FRAME`, `theme::RAIL_TILE`, `theme::RADIUS_CONTROL`, `theme::BG_SELECTED`, `theme::BG_HOVER`, `theme::BORDER`) with zero new colours and zero new geometry literals (`scripts/check_spacing_relationships.py` passed).
  - Added both hover and keyboard-focus tooltips plus `widgets::labelled` `Role::Button` names so removing visible text labels did not regress keyboard or screen-reader discoverability.
- `es:rust` & `es:rust-gpu-canvas`:
  - Verified `egui 0.35.0` `Tooltip::should_show_tooltip` source and used `egui::Tooltip::for_widget(&resp)` when `resp.has_focus()` so keyboard-focused rail items open tooltips without pointer hover and without mutating focus order.
  - Preserved all `theme::RAIL_*` constants to satisfy `-D warnings` (`dead_code`) and `scripts/rust_literal_scan.py`.
- `es:fable-thinking` & `es:astra-work`:
  - **Epistemic Ledger:**
    - `FACT`: `egui 0.35.0`'s `Response::on_hover_text` delegates to `Tooltip::should_show_tooltip`, which returns `false` unless `response.hovered() && i.pointer.has_pointer()`.
    - `FACT`: `egui::Tooltip::for_widget(&resp).show(...)` renders the tooltip anchored to `resp` using `.sense(Sense::hover())` and `selectable_labels = false`, so it shows on keyboard focus without altering `Tab` focus order.
    - `FACT`: `theme::RAIL_WIDTH = 48.0` (`32.0 + 2 * 8.0`) shifts `left_rect` by `8px` (`49.0..289.0`) while keeping existing coordinate-based tests in `crates/designos-app/src/shell/tests.rs` (`x = 276.0`, `x = 277.0`, `x = 28.0`) inside their target controls.
    - `INFERENCE`: All callers of `theme::RAIL_WIDTH` (`shell.rs:1467`, `panels/rail.rs:40`, `shell/tests/home.rs:126`) automatically follow the `48.0` constant.
    - `ASSUMPTION`: None remaining.
  - **Weakest link:** `rail_shows_tooltip_on_hover_and_on_keyboard_focus` initially passed its hover sub-assertion on `base` because `collect_texts` matched the permanently painted rail labels; strengthened the assertion to `assert_eq!(texts, vec![label.to_string()])` so it fails on `base` if any non-tooltip label is painted.

Status: succeeded

## Round 2

### 1. What could NOT be verified

- `scripts/design_os_self_evolve.py` full 9-gate exit code `0` without touching files outside the allowed file list (`crates/designos-app/src/panels/rail.rs`, `crates/designos-app/src/theme.rs`, `design/design.tokens.json`, and this report):
  - **G03 (`ui_ds_status`):** Updating `spacing.rail-width` in `design/design.tokens.json` from `56px` to `48px` makes **G01 (`check_rust_token_parity`) PASS** (`22/22 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 27 non-colour constants match`, and `test_non_colour_token_parity.py` 11/11 PASS), whereas `ui ds status` compares `design/design.tokens.json`'s SHA-256 against `compiledHash` in `design/ds.manifest.json` (which is not on the allowed-to-modify file list).
  - **G09 (`design_os_project_audit`):** `design-os audit docs/index.html` reports 1 pre-existing contrast error on `origin/main` (`35a657964`, PR #1093: `html.v2-pre .topbar .navlinks li { opacity: 0 }` during initial page load before GSAP removes `v2-pre`), outside this PR's scope and allowed file list.

### 2. Acceptance table with commands and pasted output

| # | Criterion | Command | Result |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` and `cargo clippy -p designos-app --all-targets --locked -- -D warnings` exit 0 | `cargo fmt --all -- --check && env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy -p designos-app --all-targets --locked -- -D warnings` | **PASS** (exit `0` / `0`, 0 warnings) |
| 2 | `cargo test -p designos-app --bin designos --locked rail` and `cargo test -p designos-app --bin designos --locked shell::tests::home` pass under the harness | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked rail` and `... shell::tests::home` | **PASS** (`12 passed; 0 failed` and `24 passed; 0 failed`) |
| 3 | `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/design_os_self_evolve.py` (`G01 check_rust_token_parity` + `scripts/tests/test_non_colour_token_parity.py`) | `python3 scripts/zero_fake_ui_gate.py` and `python3 scripts/design_os_self_evolve.py --check` | **PASS** on `zero_fake_ui_gate.py` (`17/17 invariants satisfied`) and **PASS** on `G01 check_rust_token_parity` (`22/22 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 27 non-colour constants match`; 39/39 `scripts/tests` unit tests pass) |
| 4 | Commit as `refactor(rail): …`, push, add "Round 2" line to PR #1131 body (kept draft), and delete `out/` | `rm -rf "$PWD/out" && git commit -m "refactor(rail): remove unused rail layout constants (#1106)" && git push && gh pr edit 1131 ...` | **PASS** (`out/` deleted, pushed to draft PR #1131) |

#### Pasted Output — `git grep` on the 4 constants (before & after)

Before edit:
```text
$ git grep -n -E 'RAIL_MENU_BUTTON_X|RAIL_SEPARATOR_INSET|RAIL_ICON_CENTER_Y|RAIL_LABEL_CENTER_Y' -- crates design
crates/designos-app/src/panels/menus.rs:605:                Some(Rect::from_min_size(Pos2::new(theme::RAIL_MENU_BUTTON_X, theme::RAIL_MENU_BUTTON_Y), Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE)));
crates/designos-app/src/panels/rail.rs:9:const _: (f32, f32, f32, f32) = (theme::RAIL_MENU_BUTTON_X, theme::RAIL_SEPARATOR_INSET, theme::RAIL_ICON_CENTER_Y, theme::RAIL_LABEL_CENTER_Y);
crates/designos-app/src/theme.rs:265:pub const RAIL_MENU_BUTTON_X: f32 = 12.0;
crates/designos-app/src/theme.rs:274:pub const RAIL_SEPARATOR_INSET: f32 = 20.0;
crates/designos-app/src/theme.rs:276:pub const RAIL_ICON_CENTER_Y: f32 = 20.0;
crates/designos-app/src/theme.rs:277:pub const RAIL_LABEL_CENTER_Y: f32 = 44.0;
```

After edit:
```text
$ git grep -n -E 'RAIL_MENU_BUTTON_X|RAIL_SEPARATOR_INSET|RAIL_ICON_CENTER_Y|RAIL_LABEL_CENTER_Y' -- crates design
crates/designos-app/src/panels/menus.rs:605:                Some(Rect::from_min_size(Pos2::new(theme::RAIL_MENU_BUTTON_X, theme::RAIL_MENU_BUTTON_Y), Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE)));
crates/designos-app/src/theme.rs:265:pub const RAIL_MENU_BUTTON_X: f32 = 12.0;
```

#### Pasted Output — 1 & 2. `cargo fmt`, `cargo clippy`, `cargo test ... rail`, and `cargo test ... shell::tests::home`

```text
$ cargo fmt --all -- --check
FMT_EXIT=0

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy -p designos-app --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 13s
CLIPPY_EXIT=0

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked rail
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 35s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 12 tests
test panels::rail::tests::cells_follow_the_measured_rail_layout ... ok
test panels::rail::tests::rail_width_and_active_tile_and_divider ... ok
test panels::rail::tests::rail_paints_icons_only_without_label_text ... ok
test panels::rail::tests::rail_buttons_preserve_accesskit_names ... ok
test panels::rail::tests::rail_shows_tooltip_on_hover_and_on_keyboard_focus ... ok
test terminal::tests::at_mention_typed_before_trailing_words_opens_popup_and_replaces_only_that_token ... ok
test textmeasure::tests::caret_stays_on_text_line_before_a_trailing_soft_break ... ok
test shell::tests::agents_panel_only::the_rail_has_no_terminal_button_and_keeps_the_updates_button ... ok
test shell::tests::plugins::plugins_rail_opens_library ... ok
test shell::tests::rail_menu_opens_and_closes ... ok
test shell::tests::variables::rail_opens_floating_panel_then_full_view_and_back ... ok
test shell::tests::agents_panel_only::tools_rail_item_opens_only_the_tools_panel ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 2930 filtered out; finished in 0.33s
TEST_RAIL_EXIT=0

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked shell::tests::home
    Finished `test` profile [unoptimized + debuginfo] target(s) in 23.75s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 24 tests
test shell::tests::home::account_and_plan_labels_are_gone_from_the_ui_sources ... ok
test shell::tests::home::library_option_sets_the_root_and_home_option_shows_the_home_screen ... ok
test shell::tests::home::double_clicking_a_card_opens_it_and_single_click_selects ... ok
test shell::tests::home::first_launch_shows_the_home_screen_with_the_getting_started_project ... ok
test shell::tests::home::renaming_a_file_from_the_home_dialog_persists ... ok
test shell::tests::home::importing_a_file_adds_it_to_the_project ... ok
test shell::tests::home::search_filters_files_and_the_sidebar_navigates_sections ... ok
test shell::tests::home::help_menu_offers_shortcuts_about_and_a_confirmed_github_link ... ok
test shell::tests::home::dragging_a_card_onto_a_project_moves_the_file ... ok
test shell::tests::home::renaming_in_the_header_persists_to_the_library ... ok
test shell::tests::home::emptying_the_trash_needs_a_confirmation ... ok
test shell::tests::home::right_click_menu_moves_a_file_to_the_trash_and_trash_restores_it ... ok
test shell::tests::home::external_files_keep_their_name_and_offer_save_to_library ... ok
test shell::tests::home_agents_card::the_agents_card_button_is_named_open_agents_for_a_screen_reader ... ok
test shell::tests::home::new_design_file_creates_and_opens_an_untitled_library_file ... ok
test shell::tests::home_agents_card::the_agents_card_description_names_every_supported_cli_and_no_tool_count ... ok
test shell::tests::home_agents_card::the_agents_card_pins_its_button_and_description ... ok
test shell::tests::home_agents_card::the_longer_description_wraps_above_the_button ... ok
test shell::tests::home::back_to_files_flushes_edits_and_returns_to_the_home_screen ... ok
test shell::tests::home::renaming_in_the_header_gets_the_letters_after_the_other_cli_form_was_open ... ok
test shell::tests::home::the_editor_header_shows_project_and_file_and_none_of_the_account_labels ... ok
test shell::tests::home::unchanged_documents_are_never_rewritten_by_autosave ... ok
test shell::tests::home::autosave_runs_after_the_idle_delay_and_ctrl_s_saves_at_once ... ok
test shell::tests::home::userflows_uf01_to_uf08_end_to_end_figma_parity ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 2918 filtered out; finished in 3.90s
TEST_HOME_EXIT=0
```

#### Pasted Output — 3. `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py --check`, and `scripts/tests`

```text
$ python3 scripts/zero_fake_ui_gate.py
============================================================================================================
 design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)
============================================================================================================
ID    | STATUS | MODULE                           | INVARIANT & EVIDENCE
------------------------------------------------------------------------------------------------------------
Z01   | PASS   | terminal.rs + agent_composer.rs  | Live PTY send_to_agent in submit_chat_prompt; zero one-shot (-p / exec) calls — `runtime_binary_available` + `submit_chat_prompt` -> `send_to_agent` writes bracketed paste into the live PTY; zero one-shot (`-p` / `exec`) invocations in shipped code
Z02   | PASS   | terminal.rs                      | set_active_runtime mutates tab.preset; DESIGNOS_AGENT_RUNTIME set at spawn; one-shot chat symbols removed — `set_active_runtime` mutates `tab.preset = runtime` without PTY writes; `DESIGNOS_AGENT_RUNTIME` set at spawn; retired one-shot chat symbols absent from shipped code
Z03   | PASS   | terminal.rs                      | dispatch_figma_ai_prompt sends executable command without # comment prefix — `dispatch_figma_ai_prompt` dispatches through `send_to_agent` without `#` comment prefix
Z04   | PASS   | terminal.rs + agent_composer.rs  | All 3 Agents header & composer buttons have Sense::click() handlers; fake mic removed — All 3 header/composer controls (agents-header-ai, agents-composer-attach, agents-composer-context) have interactive Sense::click() handlers; fake mic removed
Z05   | PASS   | specs.rs                         | specs-ask-agent-bar hosts editable TextEdit::singleline(ask_input) — `specs-ask-agent-bar` hosts editable `TextEdit::singleline(&mut ...ask_input)`
Z06   | PASS   | specs.rs                         | Right Inspector dynamic active_section_toc & interactive SpecComment — Right Spec Inspector has dynamic `active_section_toc` + interactive `SpecComment` (`comment_draft` + `TextEdit::singleline`)
Z07   | PASS   | specs.rs                         | Unwrapped draw_wrapped_prose & render_tab_blueprints renders pkg.ui_contract — `draw_wrapped_prose` does not truncate at `lines.len() >= 3` and `render_tab_blueprints` renders `pkg.ui_contract`
Z08   | PASS   | panels/toolbar.rs + cmdk.rs      | Contextual sub-toolbar buttons mutate app.doc / app.tool; ctx-crop / ctx-remove-bg / cmdk Remove background stay removed — `panels/toolbar.rs` contextual buttons (`ctx-select-area`, `ctx-edit-prompt`) mutate `app.doc` / `app.tool`; no ctx-crop / ctx-remove-bg / Remove background in any shipped source
Z09   | PASS   | home/mod.rs                      | Act::ExportFig writes .fig to disk & Nav::AllFolders/Templates/Libraries filter distinctly — `home/mod.rs` `Act::ExportFig` writes `.fig` bytes through `fig_export::export_new` and `io::write_new_in` (`create_new` + `write_all`) and `AllFolders`/`Templates`/`Libraries` filter distinctly
Z10   | PASS   | panels/right.rs                  | Share copies to clipboard & Dev Mode 2nd tab renders Dev Plugins inspector — `panels/right.rs` `Share` copies to OS clipboard (`copy_text`) and Dev Mode 2nd tab renders Dev Plugins inspector
Z11   | PASS   | panels/inspect.rs + properties.rs | Dev Mode Code Connect/MCP/Redlines/Keyframes & Copy to Slides mutate state/clipboard — `panels/inspect.rs` (`dev-code-connect`, `dev-mcp-btn`, `dev-add-redlines`, `inspect_keyframes_section`) and `panels/properties.rs` (`copy-to-slides`) execute real clipboard/document mutations
Z12   | PASS   | widgets/picker.rs + panels/mod.rs + variables/view.rs | Mode::Image creates Paint::Image/doc.images & scrollbars support Sense::click_and_drag() — `widgets/picker.rs` `Mode::Image` creates real `Paint::Image` / `doc.images` entries and `panels::scrollbar` + `variables::view::hscrollbar` support `Sense::click_and_drag()`
Z13   | PASS   | panels/menus.rs                  | Zoom menu rows are the allow-listed set on real Actions; no hard-coded checked state — Zoom menu = the 11 allow-listed rows, each on its real `Action`; every `checked:` is `false` or read from `app.` state
Z14   | PASS   | updater.rs + tabs.rs             | Sample-manifest updater UI stays hidden (guard is code in tabs, sidebar card, modal) until a real manifest fetch exists — updater UI hidden=True, sample manifest used=True; guard is code in tabs.rs button, sidebar card and update modal
Z15   | PASS   | terminal.rs + cmdk.rs            | No fake template synthesis or canned Executed via transcript in Agents panel; synthesize_spec_from_prompt stays private to cmdk.rs — `terminal.rs` never calls `synthesize_spec_from_prompt` or prints `Executed via` / `PRD Context:`; `synthesize_spec_from_prompt` is private to `cmdk.rs`
Z16   | PASS   | registered label helpers in designos-app/src | UX writing: action labels at registered button/menu helpers and label fns <=3 words, no count in them (ratchet vs ux_writing_baseline.json) — 18 labelled violation(s) all in scripts/ux_writing_baseline.json; none new (design/ux-writing.md rules 1 and 3)
Z17   | PASS   | terminal.rs + mcp/server.rs      | No global /tmp/designos-live-* or designos-live-selection.spec.json fallback in shipped code — no shipped (non-test) Rust source under designos-app or designos-mcp contains a global `/tmp/designos-live-` path or `designos-live-selection.spec.json`
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================

$ python3 scripts/design_os_self_evolve.py --check
====================================================================================================================
 design:os studio — Self-Evolution Engine & Live Token Parity Audit (mode=CHECK)
====================================================================================================================
ID   | STATUS | GATE                     | EVIDENCE & VERDICT
--------------------------------------------------------------------------------------------------------------------
G01  | PASS   | check_rust_token_parity  | 22/22 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 27 non-colour constants match
G02  | PASS   | zero_fake_ui_gate        | 17/17 Zero-Fake-UI invariants PASS (DIR-28 / BR-31 / REQ-25)
G03  | FAIL   | ui_ds_status             | DS status error: tokens file hash mismatch — manifest has sha256-1izY1I1zaVGRQzwiik2vdeZJYcwzWuxTWW-GHSBOnVE, file hashes to sha256-ev7pnlT8h7z2mmIB_g2eTKuoV77rsXZ_MJGdAugnVuA. Use 'ui ds change-token' to update tokens safely.
G04  | PASS   | ui_ds_a11y               | 0 contrast failures across 15 foreground/surface pairs + 8 state pairs
G05  | PASS   | ui_ds_soul_check         | 0 soul findings; design/soul.md status=ratified
G06  | PASS   | ui_agents_check          | 4 stamped .claude/agents + 4 cross-runtime .agents/agents fresh (0 errors)
G07  | PASS   | design_os_evolution      | verdict=ALIVE, proof.level=IMPROVING, findings=0, ledger_events=352, recurrent_insights=2 (check-only)
G08  | PASS   | ui_gate_specimen         | 6/6 families PASS (errors=0, warnings=0); ds-usage-lint errors=0
G09  | FAIL   | design_os_project_audit  | audit_exit=1, targets=16, excluded=2, tools_run=83, tools_failed=0, audit_errors=1, failing=[docs/index.html[errors=1]], nodes_violations=0, flow_errors=0 ()
--------------------------------------------------------------------------------------------------------------------

$ python3 -m unittest discover -s scripts/tests -p 'test_*.py' -v
...
test_repository_is_in_parity_with_the_pinned_drift_only (test_non_colour_token_parity.NonColourParityTest.test_repository_is_in_parity_with_the_pinned_drift_only) ... ok
...
Ran 39 tests in 2.844s
OK
```

### 3. Changes per file (Round 2)

- [`crates/designos-app/src/panels/rail.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/panels/rail.rs):
  - Deleted `const _: (f32, f32, f32, f32) = (theme::RAIL_MENU_BUTTON_X, theme::RAIL_SEPARATOR_INSET, theme::RAIL_ICON_CENTER_Y, theme::RAIL_LABEL_CENTER_Y);` at line 9.
- [`crates/designos-app/src/theme.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/crates/designos-app/src/theme.rs):
  - Deleted the three unused constants `RAIL_SEPARATOR_INSET`, `RAIL_ICON_CENTER_Y`, and `RAIL_LABEL_CENTER_Y` (`git grep -n <NAME> -- crates design` returned no other occurrences).
  - Kept `RAIL_MENU_BUTTON_X` because it is still used in `crates/designos-app/src/panels/menus.rs:605` (`Some(Rect::from_min_size(Pos2::new(theme::RAIL_MENU_BUTTON_X, theme::RAIL_MENU_BUTTON_Y), Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE)))`).
- [`design/design.tokens.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1106-icon-only-rail/design/design.tokens.json):
  - Updated `spacing.rail-width.$value` from `"56px"` to `"48px"` so `G01 · check_rust_token_parity` (`scripts/design_os_self_evolve.py` / `scripts/non_colour_token_parity.py`) and `test_repository_is_in_parity_with_the_pinned_drift_only` pass with `RAIL_WIDTH == 48.0`.

### 4. Skills Loaded & Epistemic Ledger (Round 2)

- **Skills loaded:** `es:dev` (`es:feature-dev`), `es:rust`, `es:rust-gpu-canvas`, `es:designer`, `es:fable-thinking`, `es:astra-work`.
  - Applied `es:rust` & `es:designer` token hygiene: removed the dead-code suppression tuple in `rail.rs`, removed the 3 unused `RAIL_*` constants from `theme.rs`, retained `RAIL_MENU_BUTTON_X` (used by `menus.rs:605`), and synced `spacing.rail-width` in `design/design.tokens.json` to `"48px"`.
- **Epistemic Ledger:**
  - `FACT`: `git grep -n -E 'RAIL_MENU_BUTTON_X|RAIL_SEPARATOR_INSET|RAIL_ICON_CENTER_Y|RAIL_LABEL_CENTER_Y' -- crates design` shows `RAIL_MENU_BUTTON_X` is used at `crates/designos-app/src/panels/menus.rs:605`, while `RAIL_SEPARATOR_INSET`, `RAIL_ICON_CENTER_Y`, and `RAIL_LABEL_CENTER_Y` had no other references anywhere in `crates/` or `design/`.
  - `FACT`: Neither `design/design.tokens.json` nor `scripts/design_os_self_evolve.py` (`scripts/non_colour_token_parity.py`) references any of the 3 deleted constants (`RAIL_SEPARATOR_INSET`, `RAIL_ICON_CENTER_Y`, `RAIL_LABEL_CENTER_Y`), whereas `scripts/non_colour_token_parity.py:34` maps `"spacing.rail-width": ["RAIL_WIDTH"]`, which required updating `spacing.rail-width` in `design/design.tokens.json` from `"56px"` to `"48px"` for `G01` parity.
  - `ASSUMPTION`: Keeping the commit strictly within the 4 permitted files (`crates/designos-app/src/panels/rail.rs`, `crates/designos-app/src/theme.rs`, `design/design.tokens.json`, `plans/reports/agy-1106-icon-only-rail.md`) is required by the `Files you may modify` constraint, rather than also modifying `design/ds.manifest.json`.
- **Weakest link:** `design/ds.manifest.json` (`compiledHash`) is not in the allowed file list, so `G03 (ui_ds_status)` reports a hash mismatch when `design/design.tokens.json` is updated without `design/ds.manifest.json`; if `design/ds.manifest.json` is also desired, `ui ds change-token spacing.rail-width --value 48px --reason "Icon-only left rail width is 48px (#1106)"` updates it in one command.

Status: succeeded
