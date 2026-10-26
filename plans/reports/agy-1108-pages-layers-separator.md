# DEV Report · Left panel · Draggable Pages/Layers separator (`#1108`) — 0.6.3 P1

## 1. What could NOT be verified

- **Full `designos-app` test suite (`2953` tests):** Not run per controller instruction (*"Run the panel and left-panel test modules, not the full suite; the controller runs the full suite"*). All `173` tests across `pages_layers_separator` (`6`), `layer_drag_fast` (`33`), `panel_keys` (`41`), `panels::` (`87`), and `comments::settings` (`6`) were verified under the isolated `HOME` test harness.
- **Responsive window-size captures:** Skipped per controller instruction (*"Responsive captures are skipped (owner default)"*).
- **Physical VoiceOver / macOS accessibility client announcement:** AccessKit tree emission (`Role::Splitter`, `Orientation::Horizontal`, `numeric_value`, `min_numeric_value`, `max_numeric_value`, `numeric_value_step`, `numeric_value_jump`, and label `"Resize Pages and Layers"`) was verified through `egui::PlatformOutput::accesskit_update` (`FACT`), while live macOS VoiceOver audio output was not exercised in the headless test harness.

**Conclusion:** Every checkbox and acceptance criterion of `#1108` is implemented and verified (`FACT`).
**Weakest link:** In `egui 0.35.0`, `PointerState::button_double_clicked(PointerButton::Primary)` transitions to `true` on the button-release (`pressed: false`) event of the second click rather than the button-press (`pressed: true`) event (`FACT`); `left_sep::prepare_height` checks `double_clicked` on both press and release so double-click resets work across multi-frame and single-frame deliveries.

### Skills Loaded & Changes Driven by Them
- `es:dev` / `es:feature-dev`: Followed the Red-first (`red.log` on unmodified base) -> Green (`green.log` on head) TDD loop without stopping for brainstorm or plan re-approval.
- `es:rust`: Used `#[derive(Clone, Copy, Debug, Default)]` value state in `egui` temp storage (`SepDrag`) with zero heap allocations per frame and atomic JSON persistence via `designos_library::write_atomic`.
- `es:rust-gpu-canvas`: Preserved idle-frame and canvas cache invariants; dragging or stepping the Pages/Layers separator mutates only `app.ui.pages_height` and never calls `app.changed()` or dirties `TileView::rev` (`FACT`).
- `es-designer`: Used existing `theme.rs` tokens exclusively (`theme::BORDER`, `theme::BG_BRAND`, `theme::BORDER_SELECTED`, `theme::RADIUS_MARK`, `theme::SPACE_INLINE`, `theme::PAGE_ROW_HEIGHT`, `theme::PANEL_TITLE_ROW_HEIGHT`, `theme::LAYER_HEADER_HEIGHT`), added only layout constants to `theme.rs` (`0` new colours, `0` literal geometry offsets outside `theme.rs` verified by `scripts/check_spacing_relationships.py`), and matched the 6 px hit zone and 2 px `theme::BG_BRAND` highlight of the left-panel width resize handle.
- `es:astra-work` & `es:fable-thinking`: Maintained an explicit epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), verified `EventFilter { vertical_arrows: true, .. }` focus-locking so `egui` does not steal focus on `ArrowUp`/`ArrowDown`, and verified visual captures on a scratch copy of the Untitled UI PRO v8.0 kit.

---

## 2. Acceptance Table (1–7)

| # | Gate | Verdict | Evidence |
|---|---|---|---|
| 1 | Red first (`red.log` fails on base, `green.log` passes on head) | **PASS** (`FACT`) | `plans/reports/agy-1108-pages-layers-separator-evidence/red.log` (`0 passed; 6 failed`), `plans/reports/agy-1108-pages-layers-separator-evidence/green.log` (`6 passed; 0 failed`) |
| 2 | `cargo fmt --all -- --check` exits 0 | **PASS** (`FACT`) | Exit code `0` |
| 3 | `cargo clippy -p designos-app --all-targets --locked -- -D warnings` exits 0 | **PASS** (`FACT`) | Exit code `0` |
| 4 | Panel & left-panel test modules under mandatory isolated-`HOME` test harness | **PASS** (`FACT`) | `pages_layers_separator` (`6 passed`), `layer_drag_fast` (`33 passed`), `panel_keys` (`41 passed`), `panels::` (`87 passed`), `comments::settings` (`6 passed`) — `173 passed; 0 failed` |
| 5 | `python3 scripts/zero_fake_ui_gate.py` (plus `scripts/check_spacing_relationships.py`) | **PASS** (`FACT`) | `17/17` invariants satisfied; `0` literal geometry regressions |
| 6 | Visual evidence at `1440×900` on Untitled UI PRO v8.0 kit copy (`--select "Input field" --no-toast`) | **PASS** (`FACT`) | `01-default-split.png` (default 5-row / 120 px Pages split) and `02-changed-split.png` (persisted 11-row / 264 px Pages split via isolated `HOME` `settings.json`) |
| 7 | Delete `out/` at the end | **PASS** (`FACT`) | `rm -rf out` executed; `out/` absent |

### Command 1 — Red (`red.log`) and Green (`green.log`)

#### Red Command & Output (`plans/reports/agy-1108-pages-layers-separator-evidence/red.log`)
```text
$ export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked pages_layers_separator
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 26.48s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 6 tests
test shell::tests::pages_layers_separator::separator_height_persists_across_restart ... FAILED
test shell::tests::pages_layers_separator::double_clicking_the_separator_resets_to_default_height ... FAILED
test shell::tests::pages_layers_separator::dragging_the_separator_clamps_at_min_and_max_bounds ... FAILED
test shell::tests::pages_layers_separator::slow_frame_drag_ends_at_the_release_point ... FAILED
test shell::tests::pages_layers_separator::dragging_the_separator_changes_the_pages_and_layers_split ... FAILED
test shell::tests::pages_layers_separator::keyboard_navigation_steps_clamps_and_resets_with_accesskit_and_focus_ring ... FAILED

failures:

---- shell::tests::pages_layers_separator::separator_height_persists_across_restart stdout ----

thread 'shell::tests::pages_layers_separator::separator_height_persists_across_restart' (54030235) panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for pages-layers-separator

---- shell::tests::pages_layers_separator::double_clicking_the_separator_resets_to_default_height stdout ----

thread 'shell::tests::pages_layers_separator::double_clicking_the_separator_resets_to_default_height' (54030231) panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for pages-list
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- shell::tests::pages_layers_separator::dragging_the_separator_clamps_at_min_and_max_bounds stdout ----

thread 'shell::tests::pages_layers_separator::dragging_the_separator_clamps_at_min_and_max_bounds' (54030233) panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for pages-layers-separator

---- shell::tests::pages_layers_separator::slow_frame_drag_ends_at_the_release_point stdout ----

thread 'shell::tests::pages_layers_separator::slow_frame_drag_ends_at_the_release_point' (54030236) panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for pages-layers-separator

---- shell::tests::pages_layers_separator::dragging_the_separator_changes_the_pages_and_layers_split stdout ----

thread 'shell::tests::pages_layers_separator::dragging_the_separator_changes_the_pages_and_layers_split' (54030232) panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for pages-layers-separator

---- shell::tests::pages_layers_separator::keyboard_navigation_steps_clamps_and_resets_with_accesskit_and_focus_ring stdout ----

thread 'shell::tests::pages_layers_separator::keyboard_navigation_steps_clamps_and_resets_with_accesskit_and_focus_ring' (54030234) panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for pages-layers-separator


failures:
    shell::tests::pages_layers_separator::double_clicking_the_separator_resets_to_default_height
    shell::tests::pages_layers_separator::dragging_the_separator_changes_the_pages_and_layers_split
    shell::tests::pages_layers_separator::dragging_the_separator_clamps_at_min_and_max_bounds
    shell::tests::pages_layers_separator::keyboard_navigation_steps_clamps_and_resets_with_accesskit_and_focus_ring
    shell::tests::pages_layers_separator::separator_height_persists_across_restart
    shell::tests::pages_layers_separator::slow_frame_drag_ends_at_the_release_point

test result: FAILED. 0 passed; 6 failed; 0 ignored; 0 measured; 2946 filtered out; finished in 0.38s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

#### Green Command & Output (`plans/reports/agy-1108-pages-layers-separator-evidence/green.log`)
```text
$ export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked pages_layers_separator
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 12.74s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 6 tests
test shell::tests::pages_layers_separator::dragging_the_separator_clamps_at_min_and_max_bounds ... ok
test shell::tests::pages_layers_separator::dragging_the_separator_changes_the_pages_and_layers_split ... ok
test shell::tests::pages_layers_separator::keyboard_navigation_steps_clamps_and_resets_with_accesskit_and_focus_ring ... ok
test shell::tests::pages_layers_separator::separator_height_persists_across_restart ... ok
test shell::tests::pages_layers_separator::double_clicking_the_separator_resets_to_default_height ... ok
test shell::tests::pages_layers_separator::slow_frame_drag_ends_at_the_release_point ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 2947 filtered out; finished in 0.48s
```

### Command 2 — `cargo fmt --all -- --check`
```text
$ cargo fmt --all -- --check
(exit 0)
```

### Command 3 — `cargo clippy -p designos-app --all-targets --locked -- -D warnings`
```text
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ cargo clippy -p designos-app --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.77s
(exit 0)
```

### Command 4 — Panel & Left-Panel Test Modules under Mandatory Harness
```text
$ export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
$ export CARGO_TARGET_DIR="$PWD/out/cargo"
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked layer_drag_fast
running 33 tests
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 2920 filtered out; finished in 0.56s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked panel_keys
running 41 tests
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 2912 filtered out; finished in 1.24s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked panels::
running 87 tests
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 2866 filtered out; finished in 0.19s

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked comments::settings
running 6 tests
test comments::settings::tests::os_user_is_never_empty ... ok
test comments::settings::tests::broken_settings_file_is_ignored ... ok
test comments::settings::tests::a_v060_settings_file_still_loads_after_the_dock_was_removed ... ok
test comments::settings::tests::pages_height_roundtrip_and_clear ... ok
test comments::settings::tests::author_roundtrip_keeps_other_keys ... ok
test comments::settings::tests::in_progress_stale_minutes_roundtrip_and_clamp ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 2947 filtered out; finished in 0.10s
```

### Command 5 — `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`
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
literal geometry outside theme.rs: offset_literals=1675, inset_literals=45, radius_literals=260, font_literals=25, nonconcentric_pairs=2
note: 11 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

### Command 6 — Visual Evidence (`1440×900` on Untitled UI PRO v8.0 Kit Copy)
```text
$ export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home-a/library" "$S/home-b/library"
$ cp "/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$S/untitled-copy.fig"
$ printf '{"pages_height": 264.0}\n' > "$S/home-b/library/settings.json"
$ env HOME="$S/home-a" TMPDIR="$S/" DESIGNOS_LIBRARY="$S/home-a/library" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home-a" CODEX_HOME="$S/home-a/.codex" CLAUDE_CONFIG_DIR="$S/home-a/.claude" ./out/cargo/release/designos "$S/untitled-copy.fig" --screenshot plans/reports/agy-1108-pages-layers-separator-evidence/01-default-split.png --size 1440x900 --select "Input field" --no-toast
$ env HOME="$S/home-b" TMPDIR="$S/" DESIGNOS_LIBRARY="$S/home-b/library" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home-b" CODEX_HOME="$S/home-b/.codex" CLAUDE_CONFIG_DIR="$S/home-b/.claude" ./out/cargo/release/designos "$S/untitled-copy.fig" --screenshot plans/reports/agy-1108-pages-layers-separator-evidence/02-changed-split.png --size 1440x900 --select "Input field" --no-toast
$ rm -rf "$S"
-rw-r--r--@ 1 jangtrinh  staff   183K Oct 25 18:53 plans/reports/agy-1108-pages-layers-separator-evidence/01-default-split.png
-rw-r--r--@ 1 jangtrinh  staff   193K Oct 25 18:54 plans/reports/agy-1108-pages-layers-separator-evidence/02-changed-split.png
```
- **`01-default-split.png`:** Shows the default Pages/Layers split (`5` rows = `120 px`: `Getting started`, `Variables`, divider, `FOUNDATIONS`, `↳ Colors`) with the `1 px` separator line above `Layers` and `"Input field"` selected on the canvas and in the Layers tree.
- **`02-changed-split.png`:** Shows the changed Pages/Layers split (`11` rows = `264 px`: `Getting started` through `↳ Spacing, radius & grids`) loaded from `settings.json` in the isolated `HOME`, with `Layers` pushed cleanly below the separator and `"Input field"` selected.

### Command 7 — Delete `out/`
```text
$ rm -rf out
(exit 0)
```

---

## 3. Changes per File

- [`crates/designos-app/src/theme.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/theme.rs): Added layout constants only (`PAGES_SEPARATOR_HIT = 6.0`, `PAGES_SEPARATOR_HIT_HALF = 3.0`, `PAGES_SEPARATOR_HIGHLIGHT_STROKE = 2.0`, `PAGES_SEPARATOR_STEP = 8.0`, `PAGES_SEPARATOR_SHIFT_MULT = 5.0`, `LAYERS_MIN_HEIGHT = 120.0`, `LEFT_DEV_READY_SECTION_STEP = 54.0`, `LAYER_FILTER_ROW_HEIGHT = CONTROL_TOOL`). No new colours and no colour changes.
- [`crates/designos-app/src/panels/left_sep.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/panels/left_sep.rs): New sibling helper module implementing `compute_bounds`, `hit_rect_for`, `prepare_height` (pre-layout slow-frame pointer drag, clamping, double-click reset, keyboard `ArrowUp`/`ArrowDown`/`Shift+Arrow`/`Home`/`End`/`Enter`, `EventFilter { vertical_arrows: true, .. }` focus lock, and persistence), and `paint_and_interact` (`1 px` `theme::BORDER` line, `6 px` hit zone, `CursorIcon::ResizeVertical` + `2 px` `theme::BG_BRAND` highlight, `1 px` `theme::BORDER_SELECTED` focus ring, and AccessKit `Role::Splitter` + `Orientation::Horizontal` + `now`/`min`/`max` numeric values).
- [`crates/designos-app/src/panels/left.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/panels/left.rs): Wired `left_sep::prepare_height` before `pages_list` and `left_sep::paint_and_interact` after `pages_list`, and recorded test rects `"pages-toggle"`, `"pages-list"`, `"pages-layers-separator"`, `"dev-ready-toggle"`, and `"layers-list"`.
- [`crates/designos-app/src/app.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/app.rs): Added `pub pages_height: Option<f32>` to `UiState`.
- [`crates/designos-app/src/comments/settings.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/comments/settings.rs): Added `load_pages_height` and `save_pages_height` (storing `"pages_height"` in `<library_root>/settings.json` while preserving all other keys) plus unit test `pages_height_roundtrip_and_clear`.
- [`crates/designos-app/src/home/session.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/home/session.rs): Loaded `self.ui.pages_height = crate::comments::settings::load_pages_height(&root)` inside `App::attach_library`.
- [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/shell.rs): Loaded `shell.app.ui.pages_height` from `library.unwrap_or_else(Library::default_root)` in `Shell::new` when `opts.library` was not attached during `Shell::build` (such as CLI `--screenshot` runs).
- [`crates/designos-app/src/shell/tests.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/shell/tests.rs) & [`crates/designos-app/src/shell/tests/pages_layers_separator.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1108-separator/crates/designos-app/src/shell/tests/pages_layers_separator.rs): Added end-to-end `Sim` tests covering drag split resize, min/max clamping (1 row + header min, content height max, Layers `>= 120 px` min, and Dev Mode coexistence), persistence across restart, double-click reset, keyboard navigation + AccessKit metadata + focus ring, and slow-frame drag release accuracy (`OneFrame`, `ReleaseFrame`, `PointerElsewhere`).

---

## 4. Every User-Facing String, Verbatim

- AccessKit node label (`crates/designos-app/src/panels/left_sep.rs`): `"Resize Pages and Layers"` (no visible UI text was added or changed).

---

## 5. Findings Outside Scope

- None.

---

## 6. Assumptions

- **Persistence store choice (`FACT` / `ASSUMPTION`):** Per Objective #4 (*"Use the same store as the left-panel width (app-wide). If per-file view state already exists and is the simpler fit, use it and say which one you chose"*), `left_panel_width` lives in app-wide `UiState` (`crates/designos-app/src/app.rs`) and app-wide preferences on disk live in `<library_root>/settings.json` (`crates/designos-app/src/comments/settings.rs`), whereas `.designos` files do not persist per-file panel split view state. We stored `pages_height: Option<f32>` in `UiState` and persisted `"pages_height"` in app-wide `<library_root>/settings.json`.
- **AccessKit role mapping (`FACT`):** In `accesskit 0.24.1` (re-exported by `egui 0.35.0`), the enum variant for ARIA `separator` / platform splitter is `egui::accesskit::Role::Splitter`, paired with `egui::accesskit::Orientation::Horizontal` and `numeric_value` / `min_numeric_value` / `max_numeric_value`.

**Unresolved questions:** None.

Status: DONE
