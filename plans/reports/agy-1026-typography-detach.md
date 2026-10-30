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
| **Contributor Credit Preserved** | `git log -2` | `0` | **PASS (`198a4689` authored by `NamAIEngineer`; `9dd8f21c` has `Co-authored-by: NamAIEngineer <NamAIEngineer@users.noreply.github.com>`)** |
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

Status: DONE
