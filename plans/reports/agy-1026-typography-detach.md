# Report: `#1026` — Detach Text Style on Style-Owned Typography Edits (Carry PR `#1113`)

## 1. What Could Not Be Verified

- **Interactive macOS Metal Window / OS Clipboard Manual QA**: Per hard workspace constraints (`Never touch the OS clipboard`, `Never open Figma or any client file`), manual window-level mouse clicking on a live macOS Metal window and OS clipboard operations were not executed. All UI widget interactions (Font Family picker, Font Weight picker, Font Style italic toggle, Font Size DragValue, Line Height DragValue, Letter Spacing DragValue, Text Align segmented buttons, Fill color swatch, and active `TextSession` canvas range edits + Undo/Redo) were verified deterministically through the `egui` headless `Sim` harness (`crates/designos-app/src/shell/tests/typography_style_detach.rs`) and unit tests (`crates/designos-app/src/canvas/textsession.rs` and `crates/designos-app/src/panels/typography.rs`).

---

## 2. Acceptance Table & Gate Outputs

| Gate / Check | Command | Exit Code | Verdict |
|---|---|---|---|
| **Red Test (on `origin/main` `411ccc2dea` before fix)** | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked typography` | `101` | **PASS (Failed as predicted: 10 failed, 9 passed)** |
| **Green Test (with fix applied)** | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked typography` | `0` | **PASS (20 passed, 0 failed)** |
| **Rustfmt Check** | `cargo fmt --all -- --check` | `0` | **PASS** |
| **Clippy (`-D warnings`)** | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings` | `0` | **PASS (0 warnings)** |
| **Workspace Library Tests** | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib` | `0` | **PASS (1,380 passed, 0 failed across 12 crates)** |
| **App Binary Tests (`designos`)** | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked` | `0` | **PASS (3,811 passed, 0 failed, 0 flakes)** |
| **Zero Fake UI Gate** | `python3 scripts/zero_fake_ui_gate.py` | `0` | **PASS (17/17 invariants satisfied)** |
| **Contributor Credit Preserved** | `git log -2` | `0` | **PASS (`198a4689` authored by `NamAIEngineer`; `1090fa72` has `Co-authored-by: NamAIEngineer <NamAIEngineer@users.noreply.github.com>`, followed by Round 2 commits `712a4cf3`, `0f176b20`, `9269c066`, `0035df42`, `94b4b75e`, `20c41790`)** |
| **Temporary Build Target Removed** | `test ! -e out/cargo` | `0` | **PASS (`out/cargo` deleted)** |

### 2.1 Red Log (`plans/reports/agy-1026-typography-detach-evidence/red.log`)

Exact command (executed against `origin/main` `411ccc2dea1d6c214030602a6b94bb00e2b542d8` with only the new tests added):

```bash
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked typography > plans/reports/agy-1026-typography-detach-evidence/red.log 2>&1
```

Exact output (`exit=101`):

```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-mcp)
   Compiling designos-app v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 53.61s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-2d905839eaba8cb2)

running 19 tests
test panels::inspect::tests::typography_and_assets ... ok
test check_designs::tests::rules::a_typography_number_match_or_binding_does_not_hide_the_text_style_rule ... ok
test panels::inspect::tests::inspect_code_connect_redlines_mcp_and_typography_style_mutate_state ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session ... FAILED
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step ... FAILED
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test panels::typography::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link ... FAILED
test panels::typography::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... FAILED
test panels::typography::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... FAILED
test panels::typography::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... FAILED
test panels::typography::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... FAILED
test panels::typography::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... FAILED
test panels::typography::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... FAILED
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test panels::typography::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches ... FAILED
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok

failures:

---- canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session stdout ----

thread 'canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session' (34616586) panicked at crates/designos-app/src/canvas/textsession.rs:868:9:
assertion failed: !n.style_refs.contains_key("text")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step stdout ----

thread 'canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step' (34616587) panicked at crates/designos-app/src/canvas/textsession.rs:843:9:
a per-layer Typography edit must detach the shared text style

---- panels::typography::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link' (34616591) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:370:5:
assertion `left == right` failed: in-session font-size edit must detach the text style
  left: Some("Body/Regular")
 right: None

---- panels::typography::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo' (34616594) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:242:5:
assertion `left == right` failed: editing font size must detach the text style
  left: Some("Body/Regular")
 right: None

---- panels::typography::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo' (34616596) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:189:5:
assertion `left == right` failed: editing font weight must detach the text style
  left: Some("Body/Regular")
 right: None

---- panels::typography::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo' (34616597) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:288:5:
assertion `left == right` failed: editing letter spacing must detach the text style
  left: Some("Body/Regular")
 right: None

---- panels::typography::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo' (34616593) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:162:5:
assertion `left == right` failed: editing font family must detach the text style
  left: Some("Body/Regular")
 right: None

---- panels::typography::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo' (34616598) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:265:5:
assertion `left == right` failed: editing line height must detach the text style
  left: Some("Body/Regular")
 right: None

---- panels::typography::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo' (34616595) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:219:5:
assertion `left == right` failed: editing font style (italic) must detach the text style
  left: Some("Body/Regular")
 right: None

---- panels::typography::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'panels::typography::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches' (34616592) panicked at crates/designos-app/src/panels/../shell/tests/typography_style_detach.rs:334:5:
assertion `left == right` failed: style-owned edit after alignment/fill must detach the link
  left: Some("Body/Regular")
 right: None


failures:
    canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session
    canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step
    panels::typography::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link
    panels::typography::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches
    panels::typography::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    panels::typography::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    panels::typography::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    panels::typography::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    panels::typography::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    panels::typography::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo

test result: FAILED. 9 passed; 10 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 8.33s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

### 2.2 Green Log (`plans/reports/agy-1026-typography-detach-evidence/green.log`)

Exact command:

```bash
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked typography > plans/reports/agy-1026-typography-detach-evidence/green.log 2>&1
```

Exact output (`exit=0`):

```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-mcp)
   Compiling designos-app v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 54.48s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-2d905839eaba8cb2)

running 20 tests
test panels::inspect::tests::typography_and_assets ... ok
test check_designs::tests::rules::a_typography_number_match_or_binding_does_not_hide_the_text_style_rule ... ok
test panels::inspect::tests::inspect_code_connect_redlines_mcp_and_typography_style_mutate_state ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step ... ok
test panels::typography::tests::alignment_and_fill_edits_keep_a_linked_text_style ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test panels::typography::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link ... ok
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test panels::typography::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test panels::typography::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test panels::typography::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test panels::typography::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test panels::typography::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test panels::typography::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
test panels::typography::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 6.10s
```

### 2.3 `cargo fmt --all -- --check`

Exact command:

```bash
cargo fmt --all -- --check
```

Exact output (`exit=0`):

```text
```

### 2.4 `cargo clippy --workspace --all-targets --locked -- -D warnings`

Exact command:

```bash
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
```

Exact output (`exit=0`):

```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-doc v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-doc)
    Checking designos-diff v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-diff)
    Checking designos-fonts v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-fonts)
    Checking designos-render v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-render)
    Checking designos-svg v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-svg)
    Checking designos-export v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-export)
    Checking designos-gpu v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-gpu)
    Checking designos-webcapture v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-webcapture)
    Checking designos-fig v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-fig)
    Checking designos-library v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-library)
    Checking designos-eval v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-eval)
    Checking designos-mcp v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-mcp)
    Checking designos-app v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.08s
```

### 2.5 `cargo test --workspace --locked --lib`

Exact command:

```bash
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
```

Exact output (`exit=0`):

```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-954071a99f0355be)
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-54b030362ab79b6a)
running 672 tests
test result: ok. 672 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.50s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-b0fb06508a5f1957)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-cee0f0ff60625584)
running 154 tests
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.55s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-e6b5e6d2fb73e410)
running 143 tests
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.49s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-1cac0c57d5614e3e)
running 30 tests
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-cf69135250c23b7e)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-6a5434ac6eccfb94)
running 37 tests
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.08s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-d70a05cbf567a7e3)
running 262 tests
test result: ok. 262 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.56s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-34b07dcd084a67a8)
running 58 tests
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.82s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-d0464f00b06bb177)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-60a1e05e125e03e8)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
```

### 2.6 `cargo test -p designos-app --bin designos --locked`

Exact command:

```bash
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
```

Exact output (`exit=0`):

```text
     Running unittests src/main.rs (out/cargo/debug/deps/designos-2d905839eaba8cb2)
running 3822 tests
running 1 test
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3821 filtered out; finished in 31.11s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3821 filtered out; finished in 32.29s
test result: ok. 3809 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out; finished in 538.57s
```

### 2.7 `python3 scripts/zero_fake_ui_gate.py`

Exact command:

```bash
python3 scripts/zero_fake_ui_gate.py
```

Exact output (`exit=0`):

```text
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
```

---

## 3. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`) & Weakest Link

### Epistemic Ledger

- **FACT (`F1`)**: In `designos_doc::TextStyleDef` (`crates/designos-doc/src/styles.rs:58-76`), a shared text style owns `family`, `weight`, `style`, `size`, `line_height`, `letter_spacing`, `decoration`, `transform`, `align_v`, `auto_resize`, `max_lines`, `paragraph_spacing`, `paragraph_indent`, `list_style`, `features`, and `variable_axes`, and explicitly does **not** own horizontal `align` (`TextAlign`) or `fill` (`Color`).
- **FACT (`F2`)**: `origin/main` (`411ccc2dea1d`) already landed `textedit::Snap::style_ref` and `textsession::edit_props_linked` / `base_style_linked` (via commit `8dfaee931` for `styles/new_text_box.rs`), where `base_style_linked(self, None, |_| {})` unconditionally removes `style_refs["text"]` even when `TextProps` are unchanged.
- **FACT (`F3`)**: Contributor PR `#1113` (`f0f2985d35f93ff07b06062cedaca80da666d4ab`, authored by `NamAIEngineer <NamAIEngineer@users.noreply.github.com>`) conflicted in `crates/designos-app/src/canvas/textsession.rs` and `crates/designos-app/src/textedit.rs` because PR `#1113` was branched prior to `8dfaee931` and used `detach_style: bool` on `edit_props` (which detached only when `props_changed == true`).
- **INFERENCE (`I1`)**: Replacing the boolean/option parameter on `edit_props` with `enum LinkEdit<'a> { Keep, Set(Option<&'a str>), DetachIfChanged }` in `edit_props_with_link` preserves both invariants simultaneously: `edit_props_linked` (`LinkEdit::Set`) continues to set/clear links unconditionally for `new_text_box.rs`, while `style_selection_detaching_as` (`LinkEdit::DetachIfChanged`) detaches `style_refs["text"]` only when `t.style` or `t.runs` actually changed.
- **ASSUMPTION (`A1`)**: Including `crates/designos-app/src/shell/tests/typography_style_detach.rs` via `#[cfg(test)] #[path = "../shell/tests/typography_style_detach.rs"] mod typography_style_detach;` inside `crates/designos-app/src/panels/typography.rs` satisfies both the brief's exact test file path requirement and the strict `Files you may modify` boundary without touching `crates/designos-app/src/shell/tests.rs` (preventing merge conflicts with wave-1 sibling `agy-132-effect-stop-binds`).

### Weakest Link (One Line)

- **Weakest link**: `LinkEdit::Set(None)` (used by `base_style_linked` in `new_text_box.rs`) vs `LinkEdit::DetachIfChanged` (used by `style_selection_detaching_as` in `typography.rs`) have subtly different no-op semantics when `props_changed == false`, which we explicitly isolated in `edit_props_with_link` and verified across all 3,811 `designos` binary tests.

---

## 4. Skills Loaded & Changes Made Because of Them

1. **`/es:dev` & `es:feature-dev`** (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`):
   - **What changed**: Enforced strict red-green-refactor sequencing — wrote the 8 `Sim` tests in `crates/designos-app/src/shell/tests/typography_style_detach.rs` first, ran them against `origin/main` (`411ccc2dea`) to capture `red.log` (`10 failed`), and only then applied the contributor cherry-pick and follow-up refinements.
2. **`es:rust`** (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`):
   - **What changed**: Applied Law 1 (Zero-Copy Borrowing & Lazy Cloning) in `crates/designos-app/src/panels/typography.rs::set_style`: instead of PR `#1113`'s eager `let before = t.clone();` on every selected text node (which cloned `t.text: String`, `t.style`, and `t.runs` even when the text node had no linked style), we now check `n.style_refs.contains_key(text_ref)` first and only snapshot `(t.style.clone(), t.runs.clone())` for style-linked nodes, avoiding unnecessary heap allocations on unlinked text layers. Also replaced the raw `"text"` string literal with `crate::styles::StyleKind::Text.ref_key()`.
3. **`es:rust-gpu-canvas`** (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`):
   - **What changed**: Verified that `set_style` and `style_selection_detaching_as` mutate `style_refs` inside the single `commit_coalesced` / `TextSession::edit_props_with_link` transaction so `SceneIndex` and `TileView::rev` bump exactly once per edit with no extra frame invalidations.
4. **`es:fable-thinking` & `es:astra-work`** (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`):
   - **What changed**: Diagnosed the `f0f2985d` vs `8dfaee931` conflict in `textsession.rs` before editing, noticed during initial `red.log` capture that combining `font-weight` and `italic` into one `#[test]` hid the second assertion on panic, split them into independent `#[test]` functions so every style-owned field failed independently in `red.log`, and verified all acceptance gates on the final committed state.
5. **`es:designer`** (`/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md`):
   - **What changed**: Verified that the Typography inspector header transitions cleanly from the linked style pill (`Body/Regular` + unlink button) back to the unlinked `+` ("Create text style") action immediately after any style-owned Typography edit, while keeping the linked style pill intact across horizontal alignment and text fill edits.

---

## Round 2

### Root cause

1. **`canvas/textsession.rs::text_command` bypassed style detach (`MAJOR-1`)**:
   - Keyboard shortcuts (`Cmd+B`, `Cmd+I`, `Cmd+U` in `crates/designos-app/src/canvas/textsession.rs:802-804` (`key_event`) and `crates/designos-app/src/shell.rs:937-941` (`Shell::key`), plus `Cmd+Shift+.` / `Cmd+Shift+,` in `crates/designos-app/src/canvas/textsession.rs:805-806` (`key_event`) and `crates/designos-app/src/shell.rs:838-844` (`Shell::try_typography_shortcut`)) and the top-bar **Text** menu (`Bold`, `Italic`, `Underline`, `Increase font size`, `Decrease font size` in `crates/designos-app/src/panels/menus.rs:216-220` (`menus::run`)) dispatch through `canvas::textsession::text_command`.
   - In an active text session, `text_command` called `style_selection(app, f)` (`detach_style: false` / `LinkEdit::Keep`) instead of `style_selection_detaching_as(app, f, EditKind::Other)`.
   - Outside a text session, `text_command` called `app.edit_nodes(&[id], |n| { richtext::apply_all(t, &f); ... })` directly without removing `n.style_refs[text_ref()]`.
2. **Paragraph fields in `panels/typography.rs` bypassed style detach (`MINOR-2`)**:
   - In the Typography **Type settings -> Details** popover (`crates/designos-app/src/panels/typography.rs:806-838`), `Paragraph spacing` (`:813`) goes through `set_base`, and `Paragraph indent` (`:808`), `List spacing` (`:818`), and `Vertical trim` (`:836`) go through `set_text_props`.
   - Both `set_base` and `set_text_props` mutated `NodeKind::Text(t)` (outside a session) or `ts::base_style` / `ts::edit_text_props` (inside a session) without detaching `n.style_refs[text_ref()]`, even though `designos_doc::lint::token_match::same_text_style` and `styles/text_style_ops.rs` treat `paragraph_spacing`, `paragraph.indent`, `paragraph.list_spacing`, and `paragraph.leading_trim` as shared-text-style-owned fields (while `align_h`, `fills`, `hanging_punctuation` (`:824`), `hanging_list` (`:829`), `direction` (`:848`), and `text_wrap` (`:860`) are not style-owned).
3. **Unified shared helper**:
   - Extracted `style_owned_changed` / `style_owned_props_changed` and `pub fn edit_node_text_detaching(n: &mut Node, f: impl FnOnce(&mut TextProps))` in `crates/designos-app/src/canvas/textsession.rs:333-362`, comparing style-owned fields before and after via `designos_doc::lint::token_match::same_text_style` plus `(paragraph.indent, paragraph.list_spacing, paragraph.leading_trim)` and per-run styles, then removing `n.style_refs[text_ref()]` when any style-owned field changed.
   - Routed `panels/typography.rs::set_style`, `panels/typography.rs::set_base`, `panels/typography.rs::set_text_props`, and `canvas/textsession.rs::text_command` (outside a session) through `ts::edit_node_text_detaching`, and routed `canvas/textsession.rs::text_command` (in a session) through `style_selection_detaching_as(app, f, EditKind::Other)` and `base_style` / `edit_text_props` through `LinkEdit::DetachIfStyleOwnedChanged`. Because `edit_node_text_detaching` uses `n.kind.any_text_mut()`, `set_style`, `set_base`, and `set_text_props` also edit `Sticky`, `ShapeWithText`, and `Connector`-label text in a mixed selection outside a session (matching `text_command`).

### Files changed

| File | Line Ranges | Commits | Summary of Changes |
|---|---|---|---|
| `crates/designos-app/src/canvas/textsession.rs` | `L16`, `L331-L363`, `L369`, `L408-L411`, `L570-L571`, `L607`, `L619`, `L644`, `L669` | `9269c066`, `94b4b75e`, `20c41790` | Added `style_owned_changed`, `style_owned_props_changed`, and `pub fn edit_node_text_detaching`; added `LinkEdit::DetachIfStyleOwnedChanged` for `base_style` and `edit_text_props`; routed `text_command` through `style_selection_detaching_as` (in session) and `edit_node_text_detaching` (outside session); removed unused `style_selection` wrapper. |
| `crates/designos-app/src/panels/typography.rs` | `L78`, `L114-L120`, `L568-L574`, `L943` | `712a4cf3`, `9269c066`, `94b4b75e`, `20c41790` | Routed `set_style`, `set_base` (temporarily `pub(crate)` in Round 2; restored to private `fn set_base` in Round 3 `03017f0b`), and `set_text_props` through `ts::edit_node_text_detaching` (which also widens mixed-selection edits to `Sticky`, `ShapeWithText`, and `Connector` labels via `any_text_mut`); removed duplicate `#[path = "../shell/tests/typography_style_detach.rs"]` module mount. |
| `crates/designos-app/src/shell/tests.rs` | `L4-L5`, `L1235` | `712a4cf3`, `20c41790` | Registered `mod typography_style_detach;` next to `mod typography;` so it shares `shell::tests::Sim` (temporary `#[allow(unused_imports)]` re-export at `L4-L5` removed in Round 3 `03017f0b`). |
| `crates/designos-app/src/shell/tests/typography_style_detach.rs` | `L1-L611` | `712a4cf3`, `0f176b20`, `0035df42`, `20c41790` | Removed the copied `Sim` struct to use shared `super::Sim`; added 8 `Sim` tests for `Cmd+B`, `Cmd+I`, `Cmd+U`, and menu `Increase font size` (outside and inside a session); added 4 `Sim` tests for `paragraph_spacing`, `paragraph.indent`, `paragraph.list_spacing`, and `paragraph.leading_trim` (`Vertical trim`); added 2 `Sim` tests for no-op same-family re-pick (`pick_family` / `"Inter"`) outside and inside a session. |
| `plans/reports/agy-1026-typography-detach-evidence/r2-*` | `r2-red.log`, `r2-green.log`, `r2-probes.log`, `r2-mutations.log` | — | Round 2 red/green logs, reviewer probe output, and 5-mutation (`M1`, `M2`, `M2b`, `M4a`, `M4b`) verification log. |

**Branch commit log (`git log --oneline 198a4689~1..HEAD`)**:

- `198a4689` — `fix(app): detach text styles on typography edits` (`NamAIEngineer`)
- `1090fa72` — `test(app): add sim tests for typography style detach and avoid unlinked clone`
- `712a4cf3` — `test(app): mount typography_style_detach in shell::tests with shared Sim`
- `0f176b20` — `test(app): add sim tests for text_command style detach` (red before `9269c066`)
- `9269c066` — `fix(app): detach linked text style in text_command via shared helper`
- `0035df42` — `test(app): add sim tests for paragraph field style detach` (red before `94b4b75e`)
- `94b4b75e` — `fix(app): detach linked text style on style-owned paragraph edits`
- `20c41790` — `test(app): add no-op family repick tests that keep text style link`

### Red -> Green

- **`plans/reports/agy-1026-typography-detach-evidence/r2-red.log`** (running the full 34-test `typography` suite against pre-Round-2 `textsession.rs` + `typography.rs`, `exit=101`):

```text
failures:
    shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_b_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_i_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_u_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::list_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::menu_increase_font_size_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_indent_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::vertical_trim_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo

test result: FAILED. 22 passed; 12 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 5.94s
```

- **`plans/reports/agy-1026-typography-detach-evidence/r2-green.log`** (running on Round 2 `HEAD` `20c41790`, `exit=0`):

```text
running 34 tests
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step ... ok
test check_designs::tests::rules::a_typography_number_match_or_binding_does_not_hide_the_text_style_rule ... ok
test panels::inspect::tests::typography_and_assets ... ok
test panels::inspect::tests::inspect_code_connect_redlines_mcp_and_typography_style_mutate_state ... ok
test panels::typography::tests::alignment_and_fill_edits_keep_a_linked_text_style ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test shell::tests::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link ... ok
test shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches ... ok
test shell::tests::typography_style_detach::cmd_b_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_u_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::list_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::paragraph_indent_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::paragraph_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_in_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_outside_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::vertical_trim_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 6.53s
```

### Verification

#### 1. Six Mandatory Acceptance Commands

```text
=== 1. cargo fmt --all -- --check ===
fmt exit=0

=== 2. cargo clippy --workspace --all-targets --locked -- -D warnings ===
    Checking designos-doc v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-doc)
    Checking designos-diff v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-diff)
    Checking designos-fonts v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-fonts)
    Checking designos-render v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-render)
    Checking designos-svg v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-svg)
    Checking designos-export v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-export)
    Checking designos-gpu v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-gpu)
    Checking designos-webcapture v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-webcapture)
    Checking designos-fig v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-fig)
    Checking designos-library v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-library)
    Checking designos-eval v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-eval)
    Checking designos-mcp v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-mcp)
    Checking designos-app v0.7.5 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1026-typography-detach/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 174.12s
clippy exit=0

=== 3. cargo test -p designos-app --bin designos --locked typography ===
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 6.62s

=== 4. cargo test -p designos-app --bin designos --locked textsession ===
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 3794 filtered out; finished in 0.84s

=== 5. cargo test -p designos-app --bin designos --locked text_style ===
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 3759 filtered out; finished in 13.41s

=== 6. cargo test --workspace --locked --lib ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-84b9f79dcf53b1e4)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-815df7cac2f24cbf)
test result: ok. 672 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.87s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-875c5e9aa7515b1d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-ab132beff113d839)
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.35s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-b26ed997a061f36b)
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.16s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-45d3e160fa7875ce)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-071f7214381262c2)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-7d0983952c42b7d7)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.27s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-440ebce98f1a928a)
test result: ok. 262 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.74s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-25f4198b0cded696)
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 50.65s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-82f009e569d124ef)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-a99dd1a1e2689cdb)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

#### 2. Reviewer Probes (`plans/reports/agy-1026-typography-detach-evidence/r2-probes.log`)

All 5 `text_command` probes (`cmdB-out`, `cmdB-in`, `cmdI-out`, `cmdU-out`, `menuSizeUp-out`) plus the paragraph probes (`pspacing`, `indent`) now report `link=None`, `repick-same-family` reports `link=Some("Body/Regular")`, and `cmdShiftDot` (`cmdShiftDot-in` `size=18`, `cmdShiftDot-out` `size=17`) is unchanged:

```text
PROBE cmdB-in before: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=true
PROBE cmdB-in after: content="Continue" size=16 weight=700 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=1 session=true
PROBE cmdB-out before: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=false
PROBE cmdB-out after: content="Continue" size=16 weight=700 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
PROBE cmdI-out after: content="Continue" size=16 weight=400 italic=true deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
PROBE cmdU-out after: content="Continue" size=16 weight=400 italic=true deco=Underline pspacing=0 indent=0 runs=[] link=None depth=3 session=false
PROBE menuSizeUp-out after: content="Continue" size=17 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
PROBE pspacing after: content="Continue" size=16 weight=400 italic=false deco=None pspacing=12 indent=0 runs=[] link=None depth=2 session=false
PROBE indent after: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=8 runs=[] link=None depth=2 session=false
PROBE repick-same-family after: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=false
PROBE cmdShiftDot-in before: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=true
PROBE cmdShiftDot-in after: content="Continue" size=18 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=1 session=true
PROBE cmdShiftDot-out after: content="Continue" size=17 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
```

#### 3. Mutation Testing (`plans/reports/agy-1026-typography-detach-evidence/r2-mutations.log`)

All 5 reviewer mutations (`M1`, `M2`, `M2b`, `M4a`, `M4b`) fail at least one test in `cargo test -p designos-app --bin designos --locked typography`:

| Mutation | Description | Result | Failing Tests |
|---|---|---|---|
| **`M1`** | Remove `n.style_refs.remove(text_ref())` in `edit_node_text_detaching` | **KILLED (`18 passed; 16 failed`)** | 16 out-of-session style/command/paragraph detach tests fail |
| **`M2`** | Call `style_selection_as` instead of `style_selection_detaching_as` in `set_style` | **KILLED (`32 passed; 2 failed`)** | `active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link`, `canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session` |
| **`M2b`** | Make `LinkEdit::DetachIfChanged` arm a no-op in `edit_props_with_link` (Round-1 `M2b`) / flip `set_style_preserving_link` (`r2-mutations.log`) | **KILLED (`28 passed; 6 failed` on Round-1 `M2b`; `33 passed; 1 failed` in `r2-mutations.log`)** | Round-1 `M2b` fails all 6 in-session style/command detach tests (`typography_edit_detaches_a_linked_text_style_in_an_active_text_session`, `active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link`, `cmd_b_in_session_*`, `cmd_i_in_session_*`, `cmd_u_in_session_*`, `menu_increase_font_size_in_session_*`) |
| **`M4a`** | Remove `style_owned_changed` guard in `edit_node_text_detaching` (unconditional detach) | **KILLED (`31 passed; 3 failed`)** | `panels::typography::tests::alignment_and_fill_edits_keep_a_linked_text_style`, `alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches`, `repicking_same_font_family_outside_session_keeps_linked_text_style` |
| **`M4b`** | Remove `if props_changed` guard on `LinkEdit::DetachIfChanged` in `edit_props_with_link` | **KILLED (`33 passed; 1 failed`)** | `repicking_same_font_family_in_session_keeps_linked_text_style` |

```text
=== M1 ===
test result: FAILED. 18 passed; 16 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 5.10s
=== M2 ===
test result: FAILED. 32 passed; 2 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 53.14s
=== M2b ===
test result: FAILED. 33 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.04s
=== M4a ===
test result: FAILED. 31 passed; 3 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 6.62s
=== M4b ===
test result: FAILED. 33 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.55s
```

### Residual risks

The following three behaviors noted in the Round 1 review remain intentionally out of scope for `#1026`:

1. **Collapsed-caret pending typing style + subsequent typing (`MINOR-1`)**: When a text session has a collapsed caret on non-empty text (`range.is_empty() && !empty_text`), `style_selection_impl` (`crates/designos-app/src/canvas/textsession.rs:581-590`) only updates the transient `sess.typing: Option<TextStyle>` without mutating `TextProps` or recording an undo entry; subsequent character insertion (`insert_text`, `crates/designos-app/src/canvas/textsession.rs:451-468`) splices `sess.typing` into `t.runs` via `edit_props` (`LinkEdit::Keep`). Changing `insert_text` to detach on typing-style divergence is a separate typing-pipeline policy change outside `#1026`'s Typography/shortcut style-edit scope.
2. **Double font-size step on `Cmd+Shift+.` / `Cmd+Shift+,` inside an active text session**: Both `crates/designos-app/src/canvas/textsession.rs:805-806` (`key_event`, called from `text_keys` at `canvas/textsession.rs:774`) and `crates/designos-app/src/shell.rs:838-844` (`Shell::try_typography_shortcut`, called from `Shell::handle_keys` at `shell.rs:776`) match `Cmd+Shift+.` / `Cmd+Shift+,` in the same frame when a text session is open, stepping font size by `+2.0` (`16 -> 18`) in-session vs `+1.0` (`16 -> 17`) outside a session. Both paths now detach the linked text style; deduplicating the key routing across `textsession::key_event` and `Shell::try_typography_shortcut` is orthogonal to `#1026`.
3. **In-session undo coalescing of consecutive `EditKind::Style` edits (`fill` + `size`)**: `UndoLog::record` (`crates/designos-app/src/textedit.rs:129-139`) coalesces consecutive `EditKind::Style` steps within `BURST_SECONDS: f64 = 1.0` (`crates/designos-app/src/textedit.rs:124`) by retaining the earlier `Snap` (`props` + `style_ref`), so an in-session fill edit followed within `1.0s` by an in-session size edit shares one session undo step. Adjusting `EditKind` coalescing boundaries in `textedit.rs` is outside `#1026`'s scope and file boundary.

---

## Round 3

### What could NOT be verified

- Live interactive desktop window manipulation (`designos-studio` GUI manual clicking) was not run in a human window session; all UI, popover, shortcut, and text-session behaviors were verified headlessly through `shell::tests::Sim` in `cargo test -p designos-app --bin designos`.

### Acceptance Table

| Check | Status | Evidence |
|---|---|---|
| **MINOR-A (2 in-session paragraph tests + 2 run tests; kill mutants `N3`, `N4`, `N6`, `N8`, `N9`)** | **PASS** | Added `paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`, `paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`, `run_merge_into_base_on_cmd_i_detaches_linked_text_style`, and `run_only_style_change_on_cmd_i_detaches_linked_text_style` in [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L613-L735). All 5 mutants (`N3`, `N4`, `N6`, `N8`, `N9`) fail with `exit=101` in `plans/reports/agy-1026-typography-detach-evidence/r3-mutants.log`. |
| **MINOR-B (`[Text, Sticky]` mixed-selection test + wider-reach disclosure)** | **PASS** | Added `mixed_text_and_sticky_selection_shortcuts_edit_both_and_detach_linked_text_style` in [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L737-L773) verifying `Cmd+Shift+.` (`Text` `16 -> 17`, `Sticky` `32 -> 33`, `Text` detached) and `Alt+ArrowRight` (`Text` and `Sticky` `letter_spacing` `0.0 -> 0.5`). Disclosed wider reach in Round 2 and Round 3 report sections. |
| **MINOR-C (remove unused `shell/tests.rs` re-export; restore `fn set_base` visibility)** | **PASS** | Removed `#[allow(unused_imports)] use crate::panels::typography::{pick_family, set_base, set_style, set_text_props};` from [`crates/designos-app/src/shell/tests.rs`](../../crates/designos-app/src/shell/tests.rs#L1-L6) and restored private `fn set_base` at [`crates/designos-app/src/panels/typography.rs`](../../crates/designos-app/src/panels/typography.rs#L114). |
| **MINOR-D (fix Round 2 report citations)** | **PASS** | Corrected Round 2 citations in [`plans/reports/agy-1026-typography-detach.md`](../../plans/reports/agy-1026-typography-detach.md#L393-L601) to reference `canvas/textsession.rs:802-806`, `shell.rs:838-844` / `:937-941`, `panels/menus.rs:216-220`, `panels/typography.rs:806-862` (`hanging_list`, `text_wrap`), `UndoLog::record` (`textedit.rs:129-139`), `BURST_SECONDS = 1.0` (`textedit.rs:124`), and Round-1 `M2b`. |
| **MINOR-E (one rule `LinkEdit::DetachIfStyleOwnedChanged` + in/out-of-session fill tests)** | **PASS** | Removed `LinkEdit::DetachIfChanged` from [`crates/designos-app/src/canvas/textsession.rs`](../../crates/designos-app/src/canvas/textsession.rs#L365-L408) and routed `style_selection_impl` (`:591-602`) through `LinkEdit::DetachIfStyleOwnedChanged`. Added `set_style_fill_change_outside_session_keeps_linked_text_style` and `set_style_fill_change_in_session_keeps_linked_text_style` in [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L775-L803) (`r3-red.log` -> `r3-green.log`). |
| **All 6 mandatory gates green on final commit** | **PASS** | `fmt` (`exit=0`), `clippy` (`exit=0`), `typography` (`41 passed; 0 failed`), `textsession` (`42 passed; 0 failed`), `text_style` (`84 passed; 0 failed`), `lib` (`1,380 passed; 0 failed`) in `plans/reports/agy-1026-typography-detach-evidence/r3-gates.log`. |

### Files Changed in Round 3

| File | Line Ranges | Commits | Summary of Changes |
|---|---|---|---|
| [`crates/designos-app/src/canvas/textsession.rs`](../../crates/designos-app/src/canvas/textsession.rs#L365-L604) | `L365-L369`, `L404-L408`, `L591-L602` | `78e81b72` | Removed `LinkEdit::DetachIfChanged` and used `LinkEdit::DetachIfStyleOwnedChanged` in `style_selection_impl` so in-session and out-of-session edits share the exact same style-owned detach rule. |
| [`crates/designos-app/src/panels/typography.rs`](../../crates/designos-app/src/panels/typography.rs#L114-L120) | `L114` | `03017f0b` | Restored `fn set_base` from `pub(crate)` to private module visibility. |
| [`crates/designos-app/src/shell/tests.rs`](../../crates/designos-app/src/shell/tests.rs#L1-L6) | `L4-L5` | `03017f0b` | Removed unused `#[allow(unused_imports)] use crate::panels::typography::{pick_family, set_base, set_style, set_text_props};`. |
| [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L613-L803) | `L613-L803` | `efc9eb76`, `3a86cbf4`, `7aecd293` | Added 7 new tests: 2 in-session paragraph tests (`paragraph_spacing`, `paragraph.indent` via Type Details popover), 2 run tests (`N3` run merge into base on `Cmd+I`, `N4` single-run style change on `Cmd+I`), 1 `[Text, Sticky]` mixed-selection test (`Cmd+Shift+.` and `Alt+ArrowRight`), and 2 `set_style` fill tests (outside and inside a session). |
| [`plans/reports/agy-1026-typography-detach.md`](../../plans/reports/agy-1026-typography-detach.md) | `L393-L880` | — | Fixed Round 2 citations (`MINOR-D`), disclosed mixed-selection wider reach (`MINOR-B`), and added Round 3 report + evidence (`r3-red.log`, `r3-green.log`, `r3-probes.log`, `r3-mutants.log`, `r3-gates.log`). |

### Verbatim Verification Output

#### 1. Reviewer `probe-r2.rs` Results (`plans/reports/agy-1026-typography-detach-evidence/r3-probes.log`)

Not recorded: how `probe-r2.rs` was compiled once `set_base` went back to private (MINOR-C); this log cannot be reproduced from this report alone. All 17 `rv1424_*` probes from `probe-r2.rs` pass (`exit=0`) and match `probes-head.log` except `PROBE setstyle-fill-in after`, which now keeps `link=Some("Body/Regular")`:

```text
running 17 tests
test shell::tests::typography_style_detach::rv1424_in_session_list_spacing ... ok
test shell::tests::typography_style_detach::rv1424_in_session_paragraph_indent ... ok
test shell::tests::typography_style_detach::rv1424_in_session_paragraph_spacing ... ok
test shell::tests::typography_style_detach::rv1424_in_session_paragraph_spacing_through_the_popover ... ok
test shell::tests::typography_style_detach::rv1424_in_session_vertical_trim ... ok
test shell::tests::typography_style_detach::rv1424_mixed_text_and_sticky_selection ... ok
test shell::tests::typography_style_detach::rv1424_non_owned_fields_keep_link ... ok
test shell::tests::typography_style_detach::rv1424_noop_on_redundant_run ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_b_i_u_in_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_b_outside_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_i_u_outside_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_shift_period_in_and_out_of_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_menu_text_commands_in_and_out_of_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_same_family_repick_keeps_link ... ok
test shell::tests::typography_style_detach::rv1424_run_merge_change_detaches ... ok
test shell::tests::typography_style_detach::rv1424_run_only_change_detaches ... ok
test shell::tests::typography_style_detach::rv1424_set_style_fill_only_in_vs_out_of_session ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 3843 filtered out; finished in 7.53s

PROBE lspacing-in after-edit: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=6 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE indent-in after-edit: size=16 weight=400 italic=false deco=None pspacing=0 indent=8 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE pspacing-in after-edit: size=16 weight=400 italic=false deco=None pspacing=12 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE pspacing-ui-in after-edit: size=16 weight=400 italic=false deco=None pspacing=12 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE trim-in after-edit: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=CapHeight hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE mixed before: sticky(size,lh,ls)=(32.0, Auto, 0.0) text_size=16 depth=1
PROBE mixed after Cmd+Shift+.: sticky(size,lh,ls)=(33.0, Auto, 0.0) text_size=17 depth=2
PROBE mixed after Alt+Right: sticky(size,lh,ls)=(33.0, Auto, 0.5) depth=3
PROBE mixed after set_style(30): sticky(size,lh,ls)=(30.0, Auto, 0.5) text_size=30
PROBE nonowned-out after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=true hangl=true align=Center runs=[] link=Some("Body/Regular") depth=6 session=false log(can_undo)=None
PROBE nonowned-in after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=true hangl=true align=Center runs=[] link=Some("Body/Regular") depth=1 session=true log(can_undo)=Some(true)
PROBE runmerge before: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[(0, 3, 16.0, 400, false)] link=Some("Body/Regular") depth=2 session=false log(can_undo)=None
PROBE runmerge after Cmd+I: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=3 session=false log(can_undo)=None
PROBE runonly before: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[(0, 3, 16.0, 700, false)] link=Some("Body/Regular") depth=2 session=false log(can_undo)=None
PROBE runonly after Cmd+I: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[(0, 3, 16.0, 700, true)] link=None depth=3 session=false log(can_undo)=None
PROBE setstyle-fill-out after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=Some("Body/Regular") depth=2 session=false log(can_undo)=None
PROBE setstyle-fill-in after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=Some("Body/Regular") depth=1 session=true log(can_undo)=Some(true)
```

#### 2. Red -> Green (`plans/reports/agy-1026-typography-detach-evidence/r3-red.log` and `r3-green.log`)

- **`r3-red.log`** (on `7aecd293` before `78e81b72`, `exit=101`):

```text
---- shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style' (40070581) panicked at crates/designos-app/src/shell/tests/typography_style_detach.rs:801:5:
assertion `left == right` failed: fill change via set_style in a session must keep the text style link
  left: None
 right: Some("Body/Regular")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style

test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 8.13s
```

- **`r3-green.log`** (on `78e81b72`, `exit=0`):

```text
running 41 tests
test panels::inspect::tests::typography_and_assets ... ok
test check_designs::tests::rules::a_typography_number_match_or_binding_does_not_hide_the_text_style_rule ... ok
test panels::inspect::tests::inspect_code_connect_redlines_mcp_and_typography_style_mutate_state ... ok
test panels::typography::tests::alignment_and_fill_edits_keep_a_linked_text_style ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step ... ok
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link ... ok
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_b_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_u_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
test shell::tests::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_in_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::list_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_outside_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::mixed_text_and_sticky_selection_shortcuts_edit_both_and_detach_linked_text_style ... ok
test shell::tests::typography_style_detach::paragraph_indent_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::set_style_fill_change_outside_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::paragraph_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::run_only_style_change_on_cmd_i_detaches_linked_text_style ... ok
test shell::tests::typography_style_detach::run_merge_into_base_on_cmd_i_detaches_linked_text_style ... ok
test shell::tests::typography_style_detach::vertical_trim_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 8.70s
```

#### 3. Mutants `N3`, `N4`, `N6`, `N8`, `N9` (`plans/reports/agy-1026-typography-detach-evidence/r3-mutants.log`)

All 5 mutants from `mutate-r2.py` are killed (`exit=101`):

| Mutant | Mutation in `crates/designos-app/src/canvas/textsession.rs` | Result | Failing Test(s) |
|---|---|---|---|
| **`N3_drop_run_count_compare`** | Drop `\|\| runs.len() != after.runs.len()` in `style_owned_changed` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::run_merge_into_base_on_cmd_i_detaches_linked_text_style` |
| **`N4_drop_per_run_compare`** | Drop `\|\| runs.iter().zip(&after.runs).any(...)` in `style_owned_changed` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::run_only_style_change_on_cmd_i_detaches_linked_text_style` |
| **`N6_style_owned_arm_noop`** | Make `LinkEdit::DetachIfStyleOwnedChanged` arm a no-op in `edit_props_with_link` | **KILLED (`33 passed; 8 failed`, `exit=101`)** | `canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session`, `active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link`, `cmd_b_in_session_*`, `cmd_i_in_session_*`, `cmd_u_in_session_*`, `menu_increase_font_size_in_session_*`, `paragraph_indent_in_session_*`, `paragraph_spacing_in_session_*` |
| **`N8_base_style_keeps`** | Replace `LinkEdit::DetachIfStyleOwnedChanged` with `LinkEdit::Keep` in `base_style` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo` |
| **`N9_edit_text_props_keeps`** | Replace `LinkEdit::DetachIfStyleOwnedChanged` with `LinkEdit::Keep` in `edit_text_props` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo` |

```text
=== N3_drop_run_count_compare (exit=101) ===
failures:
    shell::tests::typography_style_detach::run_merge_into_base_on_cmd_i_detaches_linked_text_style
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.44s

=== N4_drop_per_run_compare (exit=101) ===
failures:
    shell::tests::typography_style_detach::run_only_style_change_on_cmd_i_detaches_linked_text_style
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.29s

=== N6_style_owned_arm_noop (exit=101) ===
failures:
    canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session
    shell::tests::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link
    shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
test result: FAILED. 33 passed; 8 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.18s

=== N8_base_style_keeps (exit=101) ===
failures:
    shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.26s

=== N9_edit_text_props_keeps (exit=101) ===
failures:
    shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 9.97s
```

#### 4. Six Mandatory Acceptance Gates (`plans/reports/agy-1026-typography-detach-evidence/r3-gates.log`)

```text
=== 1. cargo fmt --all -- --check ===
fmt exit=0
=== 2. cargo clippy --workspace --all-targets --locked -- -D warnings ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.85s
clippy exit=0
=== 3. cargo test -p designos-app --bin designos --locked typography ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 10.35s
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 8.70s
typography exit=0
=== 4. cargo test -p designos-app --bin designos --locked textsession ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 7.45s
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 3801 filtered out; finished in 0.46s
textsession exit=0
=== 5. cargo test -p designos-app --bin designos --locked text_style ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.12s
test result: ok. 84 passed; 0 failed; 0 ignored; 0 measured; 3759 filtered out; finished in 12.73s
text_style exit=0
=== 6. cargo test --workspace --locked --lib ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.72s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 672 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.38s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.18s
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.59s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.22s
test result: ok. 262 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.10s
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 34.88s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
lib exit=0
```

### Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **`FACT`**: Removing `LinkEdit::DetachIfChanged` and using `LinkEdit::DetachIfStyleOwnedChanged` in `style_selection_impl` (`crates/designos-app/src/canvas/textsession.rs:591-602`) makes `set_style` fill-only edits keep `style_refs[text_ref()]` both outside and inside an active text session (`set_style_fill_change_outside_session_keeps_linked_text_style` and `set_style_fill_change_in_session_keeps_linked_text_style`, `r3-red.log` -> `r3-green.log`), while `set_style_preserving_link` (`style_selection_as` -> `LinkEdit::Keep`) keeps the link for dedicated colour-picker edits. Correction: it does not avoid cloning; `edit_props_with_link` clones the props into `snap` on every edit (`crates/designos-app/src/canvas/textsession.rs:394`).
- **`FACT`**: Opening the Type Details popover inside an active text session and editing `"det-pspacing"` (`set_base` -> `ts::base_style`) or `"det-indent"` (`set_text_props` -> `ts::edit_text_props`) detaches `style_refs[text_ref()]` in one session undo step and one committed document undo step (`paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`, `paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`), killing mutants `N6`, `N8`, and `N9` without exposing `set_base` as `pub(crate)`.
- **`FACT`**: In `style_owned_changed` (`crates/designos-app/src/canvas/textsession.rs:333-339`), both `runs.len() != after.runs.len()` (`N3`) and `runs.iter().zip(&after.runs).any(...)` (`N4`) are necessary and exercised by `run_merge_into_base_on_cmd_i_detaches_linked_text_style` and `run_only_style_change_on_cmd_i_detaches_linked_text_style`.
- **`FACT` (MINOR-B wider-reach disclosure)**: Routing `panels/typography.rs::{set_style, set_base, set_text_props}` through `ts::edit_node_text_detaching` (`n.kind.any_text_mut()`) widens their reach in a mixed selection (`[Text, Sticky]`, `ShapeWithText`, or `Connector` label) so that `Alt+Arrow` kerning/line-height shortcuts (`Shell::try_typography_shortcut`, `shell.rs:830` -> `set_style`) and Typography inspector controls edit non-`Text` text-bearing nodes alongside `Text` nodes, matching `text_command` (`Cmd+B/I/U`, `Cmd+Shift+./,`). Locked in by `mixed_text_and_sticky_selection_shortcuts_edit_both_and_detach_linked_text_style`.
- **`INFERENCE`**: Because `style_owned_changed` compares the exact same tuple (`same_text_style` on base + `(paragraph.indent, paragraph.list_spacing, paragraph.leading_trim)` + run count and per-run `(start, end)` + `same_text_style`) in both `edit_node_text_detaching` and `LinkEdit::DetachIfStyleOwnedChanged`, no style-owned or non-style-owned field can diverge in detach behavior between in-session and out-of-session edits.
- **`ASSUMPTION`**: None.

- **Weakest link**: Collapsed-caret pending typing style (`sess.typing`) in an active text session remains a transient cursor state until characters are typed via `insert_text`, which is outside `#1026`'s Typography/shortcut style-edit scope.

### Skills Loaded & What Changed Because of Them

1. **`es-astra-work`**: Enforced the strict red-before-green verification loop (`r3-red.log` on `7aecd293` before `78e81b72`) and post-mutation verification across all 17 reviewer probes, 5 mutants, and 6 cargo gates.
2. **`es-fable-thinking`**: Guided the falsification test design for `N3` (run merge into base where base style is unchanged) vs `N4` (single-run style change where run count and base style are unchanged) and verified every file/line citation in the report against the actual source files.
3. **`es-rust`**: Restored minimal module visibility (`fn set_base` private in `panels/typography.rs`, no unused re-exports in `shell/tests.rs`) while testing `base_style` and `edit_text_props` through the Type Details popover UI in `Sim`.
4. **`es-rust-gpu-canvas`** & **`es-dev` / `es-feature-dev`**: Preserved unlinked fast-path invariants (`before` snapshot only when `n.style_refs.contains_key(text_ref())`) and atomic commit hygiene.

### Unresolved Questions

None.

Status: DONE
