# Report — DEV · Styles · delete a style, keep the linked layers' values (#127, E1-1)

## 1. What could NOT be verified

- **Windows/Linux runtime rendering of `STYLE_DELETED_TOAST` (`"Style deleted · Ctrl+Z to undo"`):** `[INFERENCE]` Verified on macOS (`cfg!(target_os = "macos")` branch `"Style deleted · Cmd+Z to undo"`) via unit/sim tests and `cargo clippy --workspace --all-targets --locked -- -D warnings`; the non-macOS branch is a compile-time `if cfg!(target_os = "macos")` string constant identical in structure to existing shortcut toast constants in the crate, but was not executed on a Windows or Linux host in this macOS session.
- **Full `designos-app` binary test suite:** `[FACT]` Per task instructions ("Run the focused tests only; the controller runs the full suite"), only the focused style test filters (`effect_grid_styles` and `style_edit`) were executed in `designos-app`.
- **Conclusion:** `[FACT]` `delete_style(doc, key) -> Result<Command, StyleOpError>` is implemented in `crates/designos-doc/src/styles_ops.rs` and wired to the `"Delete style"` row in `crates/designos-app/src/panels/style_picker.rs` after `"Detach style"`, with A13 read-only library protection, single-step undo, toast notification, and clean `.fig` round-trip export.
- **Weakest link:** `[FACT]` `crates/designos-app/src/shell/tests/style_edit.rs` clicks `"Edit style"` by row index offset (`5.0 * theme::MENU_ITEM_HEIGHT`), which is coupled to the number of action rows in `style_picker::build_menu`.

---

## 2. Acceptance table (1–8), with commands and pasted output

| # | Check | Verdict | Evidence |
|---|---|---|---|
| 1 | `red.log` and `green.log` | PASS (`[FACT]`) | `plans/reports/agy-127-delete-style-evidence/red.log`, `plans/reports/agy-127-delete-style-evidence/green.log` |
| 2 | `cargo fmt --all -- --check` exits 0 | PASS (`[FACT]`) | Exit code `0` |
| 3 | `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0 | PASS (`[FACT]`) | Exit code `0` |
| 4 | `cargo test -p designos-doc --locked` and `cargo test -p designos-fig --locked` pass | PASS (`[FACT]`) | `designos-doc`: 633 + 3 + 5 passed; `designos-fig`: 121 unit + 449 integration passed |
| 5 | `cargo test -p designos-app --bin designos --locked <style test filters>` passes under the harness | PASS (`[FACT]`) | `effect_grid_styles`: 7 passed; `style_edit`: 15 passed |
| 6 | `python3 scripts/zero_fake_ui_gate.py` passes | PASS (`[FACT]`) | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| 7 | Visual evidence (`1440×900` on a scratch copy of Untitled UI kit with a local-style-linked layer selected and the style menu open) | PASS (`[FACT]`) | `plans/reports/agy-127-delete-style-evidence/style-menu-delete.png` |
| 8 | Delete `out/` at the end | PASS (`[FACT]`) | `rm -rf out` verified (`OUT_REMOVED=yes`) |

### Pasted commands and output

#### Acceptance 1 — `red.log` and `green.log`

Command (Red):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  echo "=== designos-doc ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked styles_ops
  echo "=== designos-fig ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-fig --test fig_effect_grid_styles --locked deleted_style
  echo "=== designos-app ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked effect_grid_styles
} > plans/reports/agy-127-delete-style-evidence/red.log 2>&1
```

Output (`plans/reports/agy-127-delete-style-evidence/red.log`):
```text
=== designos-doc ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-doc)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 13.91s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)

running 16 tests
test styles_ops::tests::editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step ... ok
test styles_ops::tests::grid_style_is_created_from_a_frame_with_a_chosen_name ... ok
test styles_ops::tests::applying_links_nodes_and_copies_values ... ok
test styles_ops::tests::editing_a_style_updates_every_linked_node_and_only_those ... ok
test styles_ops::tests::effect_style_is_created_from_a_node_and_undone_in_one_step ... ok
test styles_ops::tests::a_colour_style_edit_recolours_every_run_of_a_rich_text_layer ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file ... ok
test styles_ops::tests::creation_is_refused_without_values_or_on_the_wrong_node ... ok
test styles_ops::tests::editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused ... ok
test styles_ops::tests::a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides ... ok
test styles_ops::tests::renaming_changes_the_display_name_only ... ok
test styles_ops::tests::detaching_drops_the_link_but_keeps_the_values ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file ... FAILED
test styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step ... FAILED
test styles_ops::tests::effect_and_grid_styles_survive_a_native_save_and_load ... ok

failures:

---- styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file stdout ----

thread 'styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file' (47255046) panicked at crates/designos-doc/src/styles_ops.rs:879:45:
called `Result::unwrap_err()` on an `Ok` value: Batch([])
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step stdout ----

thread 'styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step' (47255051) panicked at crates/designos-doc/src/styles_ops.rs:813:9:
assertion failed: !d.styles.contains_key("FillStyle") && !d.style_meta.contains_key("FillStyle")


failures:
    styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file
    styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step

test result: FAILED. 14 passed; 2 failed; 0 ignored; 0 measured; 617 filtered out; finished in 0.01s

error: test failed, to rerun pass `-p designos-doc --lib`
=== designos-fig ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-doc)
   Compiling designos-fonts v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-fonts)
   Compiling designos-render v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-render)
   Compiling designos-svg v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-svg)
   Compiling designos-webcapture v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-webcapture)
   Compiling designos-fig v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-fig)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.63s
     Running tests/fig_effect_grid_styles.rs (out/cargo/debug/deps/fig_effect_grid_styles-46a2f99555638b59)

running 2 tests
test a_soft_deleted_style_is_ignored ... ok
test deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip ... FAILED

failures:

---- deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip stdout ----

thread 'deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip' (47257043) panicked at crates/designos-fig/tests/fig_effect_grid_styles.rs:569:5:
deleted style key must not reappear in styles
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.04s

error: test failed, to rerun pass `-p designos-fig --test fig_effect_grid_styles`
=== designos-app ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-doc)
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-mcp)
   Compiling designos-fonts v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-fonts)
   Compiling designos-render v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-render)
   Compiling designos-svg v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-svg)
   Compiling designos-export v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-export)
   Compiling designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-gpu)
   Compiling designos-webcapture v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-webcapture)
   Compiling designos-fig v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-fig)
   Compiling designos-library v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-library)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.41s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 7 tests
test shell::tests::effect_grid_styles::local_styles_list_effect_and_grid_groups ... ok
test shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu ... FAILED
test shell::tests::effect_grid_styles::creating_an_effect_style_links_the_layer_in_one_undo_step ... ok
test shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step ... FAILED
test shell::tests::effect_grid_styles::renaming_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::detaching_and_applying_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::creating_detaching_and_applying_a_grid_style ... ok

failures:

---- shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu stdout ----

thread 'shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu' (47261314) panicked at crates/designos-app/src/shell/tests/effect_grid_styles.rs:176:9:
assertion `left == right` failed: Effect: a local style can be deleted from the menu
  left: ("", false)
 right: ("Delete style", false)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step stdout ----

thread 'shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step' (47261317) panicked at crates/designos-app/src/shell/tests/effect_grid_styles.rs:143:5:
assertion `left == right` failed
  left: "Created \"Effect style 1\""
 right: "Style deleted · Cmd+Z to undo"


failures:
    shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu
    shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step

test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 2933 filtered out; finished in 0.80s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

Command (Green):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  echo "=== cargo test -p designos-doc --locked styles_ops ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked styles_ops
  echo "=== cargo test -p designos-fig --test fig_effect_grid_styles --locked deleted_style ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-fig --test fig_effect_grid_styles --locked deleted_style
  echo "=== cargo test -p designos-app --bin designos --locked effect_grid_styles ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked effect_grid_styles
} > plans/reports/agy-127-delete-style-evidence/green.log 2>&1
```

Output (`plans/reports/agy-127-delete-style-evidence/green.log`):
```text
=== cargo test -p designos-doc --locked styles_ops ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.15s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)

running 16 tests
test styles_ops::tests::creation_is_refused_without_values_or_on_the_wrong_node ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file ... ok
test styles_ops::tests::editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file ... ok
test styles_ops::tests::renaming_changes_the_display_name_only ... ok
test styles_ops::tests::grid_style_is_created_from_a_frame_with_a_chosen_name ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file ... ok
test styles_ops::tests::detaching_drops_the_link_but_keeps_the_values ... ok
test styles_ops::tests::a_colour_style_edit_recolours_every_run_of_a_rich_text_layer ... ok
test styles_ops::tests::effect_style_is_created_from_a_node_and_undone_in_one_step ... ok
test styles_ops::tests::applying_links_nodes_and_copies_values ... ok
test styles_ops::tests::a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides ... ok
test styles_ops::tests::editing_a_style_updates_every_linked_node_and_only_those ... ok
test styles_ops::tests::editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step ... ok
test styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step ... ok
test styles_ops::tests::effect_and_grid_styles_survive_a_native_save_and_load ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 617 filtered out; finished in 0.04s

     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-d19362047bc102bf)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s

     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-c7fad459a80a2bad)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s

=== cargo test -p designos-fig --test fig_effect_grid_styles --locked deleted_style ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.18s
     Running tests/fig_effect_grid_styles.rs (out/cargo/debug/deps/fig_effect_grid_styles-46a2f99555638b59)

running 2 tests
test a_soft_deleted_style_is_ignored ... ok
test deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.02s

=== cargo test -p designos-app --bin designos --locked effect_grid_styles ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.38s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 7 tests
test shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu ... ok
test shell::tests::effect_grid_styles::local_styles_list_effect_and_grid_groups ... ok
test shell::tests::effect_grid_styles::creating_an_effect_style_links_the_layer_in_one_undo_step ... ok
test shell::tests::effect_grid_styles::renaming_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::detaching_and_applying_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::creating_detaching_and_applying_a_grid_style ... ok
test shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 2933 filtered out; finished in 0.62s
```

#### Acceptance 2 & 3 — `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
echo "=== FMT CHECK ==="
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
echo "FMT_EXIT=$?"
echo "=== CLIPPY ==="
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
echo "CLIPPY_EXIT=$?"
rm -rf "$S"
```

Output:
```text
=== FMT CHECK ===
FMT_EXIT=0
=== CLIPPY ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.18s
CLIPPY_EXIT=0
```

#### Acceptance 4 — `cargo test -p designos-doc --locked` and `cargo test -p designos-fig --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
echo "=== DESIGNOS-DOC ==="
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked
echo "=== DESIGNOS-FIG ==="
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-fig --locked
rm -rf "$S"
```

Output:
```text
=== DESIGNOS-DOC ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.83s
     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-d19362047bc102bf)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-c7fad459a80a2bad)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
=== DESIGNOS-FIG ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-540f1f9206f56183)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.90s
     Running tests/clipboard.rs (out/cargo/debug/deps/clipboard-4d126ac74bee9c11)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/component_props.rs (out/cargo/debug/deps/component_props-6a6c79033748144a)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/dev_mode.rs (out/cargo/debug/deps/dev_mode-5f91afe4d1363fcb)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/fig_child_margins.rs (out/cargo/debug/deps/fig_child_margins-deb2909275dc189e)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/fig_effect_grid_styles.rs (out/cargo/debug/deps/fig_effect_grid_styles-46a2f99555638b59)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/fig_export_field_shapes.rs (out/cargo/debug/deps/fig_export_field_shapes-14b8df8b8f7ae19b)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/fig_export_grid_shapes.rs (out/cargo/debug/deps/fig_export_grid_shapes-b69708343c7c2519)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/fig_export_non_finite.rs (out/cargo/debug/deps/fig_export_non_finite-f7feffeef1a81be5)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/fig_export_real_schema.rs (out/cargo/debug/deps/fig_export_real_schema-24ffa9c18be63a4b)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/fig_export_real_tags.rs (out/cargo/debug/deps/fig_export_real_tags-2d8f4e74dbaf2a0c)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/fig_grid_fixed_row_export.rs (out/cargo/debug/deps/fig_grid_fixed_row_export-fd08bc8bc1ec3c39)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/fig_grid_override_anchors.rs (out/cargo/debug/deps/fig_grid_override_anchors-db94d009ffe66fc0)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/fig_import.rs (out/cargo/debug/deps/fig_import-c9e0797ea0b1168b)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
     Running tests/fig_import_legacy_export.rs (out/cargo/debug/deps/fig_import_legacy_export-d14e96387ebe08b8)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/fig_library.rs (out/cargo/debug/deps/fig_library-b0a7ff67511e03f7)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/fig_non_finite_derived.rs (out/cargo/debug/deps/fig_non_finite_derived-472de72c78f482b7)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s
     Running tests/fig_prop_var_value.rs (out/cargo/debug/deps/fig_prop_var_value-34118cea98741b37)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/fig_roundtrip.rs (out/cargo/debug/deps/fig_roundtrip-e9f3e8b61062eec7)
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s
     Running tests/fig_slots.rs (out/cargo/debug/deps/fig_slots-21329094591d6fe9)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/fig_unmapped_layer_edits.rs (out/cargo/debug/deps/fig_unmapped_layer_edits-48221fa3fdecb603)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/fig_upgrade.rs (out/cargo/debug/deps/fig_upgrade-d0ed35f294fb72c0)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/figjam.rs (out/cargo/debug/deps/figjam-86670bb62c35ba97)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/grid_child_align_patch.rs (out/cargo/debug/deps/grid_child_align_patch-a7b039df119da04e)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/kiwi_zero_size_struct_limit.rs (out/cargo/debug/deps/kiwi_zero_size_struct_limit-a068d29779e35311)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
     Running tests/pen_component_depth_limits.rs (out/cargo/debug/deps/pen_component_depth_limits-d1ce03893d77534b)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
     Running tests/pen_schema_2_21.rs (out/cargo/debug/deps/pen_schema_2_21-7fcb22e3beaf2421)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/proto_import.rs (out/cargo/debug/deps/proto_import-0f6b52516254a530)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/proto_vars.rs (out/cargo/debug/deps/proto_vars-6de4e2b305badc80)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/real_files.rs (out/cargo/debug/deps/real_files-dc310001cabe8a39)
test result: ok. 0 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/remote_components.rs (out/cargo/debug/deps/remote_components-696774ee0fa9c983)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/rest_import.rs (out/cargo/debug/deps/rest_import-a88dbc1f21425f69)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/rest_import_grid_sizing.rs (out/cargo/debug/deps/rest_import_grid_sizing-57d8c51e4932f29a)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rest_upgrade.rs (out/cargo/debug/deps/rest_upgrade-6533412029c30c14)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/settings_import.rs (out/cargo/debug/deps/settings_import-3d88453b5901fa2a)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/stack_wrap_and_text_wrap.rs (out/cargo/debug/deps/stack_wrap_and_text_wrap-5fb1d1e7e9b58504)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/variables.rs (out/cargo/debug/deps/variables-d00ef35c307885fd)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/variables_code_syntax_opacity.rs (out/cargo/debug/deps/variables_code_syntax_opacity-87ee68eada8ecdf2)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/variables_library_collections.rs (out/cargo/debug/deps/variables_library_collections-2a4c0016260f1c7a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/vector_network.rs (out/cargo/debug/deps/vector_network-7753c657aa014d08)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

#### Acceptance 5 — `cargo test -p designos-app --bin designos --locked <style test filters>`

Commands:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked effect_grid_styles
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked style_edit
rm -rf "$S"
```

Output:
```text
running 7 tests
test shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu ... ok
test shell::tests::effect_grid_styles::local_styles_list_effect_and_grid_groups ... ok
test shell::tests::effect_grid_styles::creating_an_effect_style_links_the_layer_in_one_undo_step ... ok
test shell::tests::effect_grid_styles::renaming_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::detaching_and_applying_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::creating_detaching_and_applying_a_grid_style ... ok
test shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 2933 filtered out; finished in 0.62s

running 15 tests
test styles::instance_refit_tests::a_style_edit_on_an_instance_label_is_one_undo_step ... ok
test canvas::textsession::tests::inspector_style_edits_request_the_new_face_with_or_without_a_session ... ok
test canvas::textsession::tests::undoing_a_second_quick_style_edit_finds_the_face_of_the_first ... ok
test shell::tests::style_edit::a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu ... ok
test shell::tests::style_edit::a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row ... ok
test shell::tests::style_edit::editing_a_color_style_from_the_list_updates_every_linked_layer_in_one_undo_step ... ok
test shell::tests::style_edit::a_library_style_cannot_be_edited_from_the_list ... ok
test shell::tests::style_edit::deleted_variable_bound_on_a_style_renders_detachable_pill_and_emits_broken_style_binding_lint ... ok
test shell::tests::style_edit::a_grid_style_count_field_never_leaves_a_guide_that_draws_nothing ... ok
test shell::tests::style_edit::a_drag_is_recorded_once_when_the_pointer_is_released ... ok
test shell::tests::style_edit::editing_an_effect_style_from_its_linked_row_updates_every_linked_layer_in_one_undo_step ... ok
test shell::tests::style_edit::editing_a_text_style_from_the_list_restyles_every_linked_text_layer ... ok
test shell::tests::style_edit::editing_a_grid_style_from_the_style_menu_updates_the_linked_frame ... ok
test shell::tests::style_edit::scrubbing_text_and_grid_style_fields_updates_linked_layers_in_one_undo_step ... ok
test shell::tests::style_edit::popover_number_fields_use_distinct_non_duplicate_prefixes ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 2925 filtered out; finished in 1.29s
```

#### Acceptance 6 — `python3 scripts/zero_fake_ui_gate.py`

Command:
```sh
python3 scripts/zero_fake_ui_gate.py
```

Output:
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

#### Acceptance 7 — Visual evidence (`1440×900` capture on scratch copy of Untitled UI kit)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cp "/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$S/kit.fig"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" out/cargo/release/designos "$S/kit.fig" --select "MacBook Pro" --zoom 100 --panel grid-styles --size 1440x900 --no-toast --screenshot plans/reports/agy-127-delete-style-evidence/style-menu-delete.png
echo "EXIT=$?"
ls -lh plans/reports/agy-127-delete-style-evidence/style-menu-delete.png
rm -rf "$S"
```

Output:
```text
EXIT=0
-rw-r--r--@ 1 jangtrinh  staff   280K Oct 25 12:04 plans/reports/agy-127-delete-style-evidence/style-menu-delete.png
```

#### Acceptance 8 — Delete `out/` at the end

Command:
```sh
rm -rf out
test ! -e out && echo "OUT_REMOVED=yes"
```

Output:
```text
OUT_REMOVED=yes
```

---

## 3. Changes per file

- `crates/designos-doc/src/styles_ops.rs`:
  - Added `pub fn delete_style(doc: &Document, key: &str) -> Result<Command, StyleOpError>` which refuses library styles (`is_library_style(doc, key)`) with `StyleOpError::LibraryStyle(key.into())`, finds all layers referencing `key` via `linked_style_nodes(doc, key)`, copies each linked layer's current resolved values (fills, stroke paints, text style and run fills, effects, or layout grids) plus any variable bindings carried by `doc.style_meta[key].bindings` resolved in the layer's mode via `doc.resolve_variable(id, variable)` and `set_bound_property(&mut after, property, value)` (matching `detach_style`), clears every `style_refs` slot pointing to `key`, and removes `key` from `doc.styles` and `doc.style_meta` inside a single `Command::Batch` (one undo step).
  - Added unit tests `deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step` and `a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file`.
- `crates/designos-app/src/panels/style_edit.rs`:
  - Widened visibility of `LIBRARY_STYLE_TIP` from `pub(super)` to `pub(crate)` so sim tests and `style_picker` share the exact A13 constant (`"Edit in the library file"`).
- `crates/designos-app/src/panels/style_picker.rs`:
  - Added `STYLE_DELETED_TOAST` (`"Style deleted · Cmd+Z to undo"` on macOS, `"Style deleted · Ctrl+Z to undo"` on Windows/Linux).
  - Added `Pick::Delete`, `delete_linked_style`, and the `"Delete style"` `MenuItem` row immediately after `"Detach style"` (enabled only when a local style is linked; when a library style is linked, disabled with `style_edit::LIBRARY_STYLE_TIP` as label and tooltip).
  - Added `#[cfg(test)] pub fn delete_style_row` helper for sim tests to inspect the `"Delete style"` row's `(label, disabled)` state.
- `crates/designos-app/src/shell/tests/effect_grid_styles.rs`:
  - Added sim tests `deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step` and `a_library_style_cannot_be_deleted_from_the_style_menu`.
- `crates/designos-app/src/shell/tests/style_edit.rs`:
  - Updated the 4 menu row Y-offset clicks for `"Edit style"` from `4.0 * theme::MENU_ITEM_HEIGHT` to `5.0 * theme::MENU_ITEM_HEIGHT` to account for `"Delete style"` inserted before `"Edit style"`.
- `crates/designos-fig/tests/fig_effect_grid_styles.rs`:
  - Added `.fig` round-trip test `deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip` verifying that deleting a style and exporting/re-importing `.fig` leaves no style node for the deleted key and no dangling `style_refs` on formerly linked layers while keeping their resolved values.

---

## 4. Every user-facing string, verbatim

- `"Delete style"`
- `"Style deleted · Cmd+Z to undo"`
- `"Style deleted · Ctrl+Z to undo"`
- `"Edit in the library file"`
- `"Style not changed: {e}"`

---

## 5. Findings outside scope

- `[FACT]` In `crates/designos-app/src/shell/tests/style_edit.rs`, clicking `"Edit style"` in the style menu uses a numeric row index (`5.0 * theme::MENU_ITEM_HEIGHT`) rather than looking up the row index by `MenuItem::label`.
- `[FACT]` When running `--screenshot` without `--select` on the 207,601-node Untitled UI `.fig` file, `step_pending_relayout()` and initial full-page tessellation on Page 1 can keep `gpu.at_rest()` false across early frames; selecting a target layer (`--select "MacBook Pro" --zoom 100`) settles immediately and captures cleanly.

---

## 6. Assumptions

- `[ASSUMPTION]` **Variable bindings on a deleted style:** Following `detach_style` (`crates/designos-doc/src/styles_ops.rs:273`) and `edit_style` (`crates/designos-doc/src/styles_ops.rs:208-262`), when a style carrying `StyleMeta::bindings` is deleted, each linked layer receives the current resolved value of the style (including resolving any `StyleMeta::bindings` entry in the layer's variable mode via `doc.resolve_variable(id, variable)` and applying it with `set_bound_property(&mut after, property, value)`), and loses its `style_refs` entry without adding `node.variables.bindings` (identical to how `detach_style` preserves resolved values on the layer).
- `[ASSUMPTION]` **Library style menu row presentation:** For a linked read-only library style (`is_library_style(&app.doc, key)`), the `"Delete style"` row in `style_picker::build_menu` uses `style_edit::LIBRARY_STYLE_TIP` (`"Edit in the library file"`) for both the disabled row label (matching the adjacent `"Rename style"` and `"Edit style"` rows in `style_picker.rs`) and `.tooltip(style_edit::LIBRARY_STYLE_TIP)`.
- **Unresolved questions:** None.

---

## Round 2

### 1. What could NOT be verified

- **Live screenshot of a library style's menu on the Untitled UI kit copy:** `[FACT]` `.fig` import does not populate `doc.library.external` for styles (`is_library_style(&doc, key)` is false on `.fig` imports), and inspecting the remote-component instances on the reachable pages of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` via `--export-spec` confirmed that no selectable layer on the reachable pages is linked to a style used on `doc.library_page` (`used_on_library_page`). Consequently, `plans/reports/agy-127-delete-style-evidence/r2-style-menu-delete.png` captures a **local style's menu** (`"Grid desktop"` on `"MacBook Pro"` at `1440×900`), while the library style menu state (`"Rename style"`, `"Delete style"`, `"Edit style"` disabled with `.tooltip("Edit in the library file")`) is verified in the `designos-app` sim tests (`a_library_style_cannot_be_deleted_from_the_style_menu`, `a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu`, `a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu`, `a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row`).
- **Windows/Linux runtime execution of `STYLE_DELETED_TOAST` (`"Style deleted · Ctrl+Z to undo"`):** `[INFERENCE]` Verified on macOS (`"Style deleted · Cmd+Z to undo"`). **Correction to Round 1 report:** Round 1 inaccurately claimed that `STYLE_DELETED_TOAST` matched existing shortcut toast constants in the crate; in fact, existing undo toasts in `designos-app` (`panels/menus.rs:146`, `panels/toolbar.rs:413`, `shell/tests/board.rs:251`, `canvas/input/keyboard.rs:861`) hard-code `"Cmd+Z"`, and `STYLE_DELETED_TOAST` in `crates/designos-app/src/panels/style_picker.rs` is the only toast that branches on `cfg!(target_os = "macos")`.
- **Correction to Round 1 `red.log`:** `[FACT]` Round 1's `plans/reports/agy-127-delete-style-evidence/red.log` ran against a temporary stub (`Ok(Command::Batch(vec![]))`) added at the same time as the Round 1 tests. In Round 2, `plans/reports/agy-127-delete-style-evidence/r2-red.log` was recorded against the real `6961e62f` code before any Round 2 production changes were made.
- **Full `designos-app` binary test suite:** `[FACT]` Only the focused `designos-app` style sim tests (`effect_grid_styles` and `style_edit`) were executed under the harness, alongside the full `designos-doc` and `designos-fig` test suites.
- **Conclusion:** `[FACT]` All five Round 2 rulings (H1 preserve layer values and mixed text runs without `apply_style_values`, M2 transfer style variable bindings onto `node.variables.bindings` so layers keep their look across modes and follow mode switches, M1 refuse deleting styles used on the hidden library page `used_on_library_page` and disable them in the menu, normal labels `"Rename style"` / `"Delete style"` / `"Edit style"` with `"Edit in the library file"` hover tooltip when disabled, and `delete_linked_style` sharing `run_style_op_then`) are implemented and verified.
- **Weakest link:** `[FACT]` To let `delete_linked_style` in `crates/designos-app/src/panels/style_picker.rs` call `app.run_style_op_then(...)` per Ruling 5, `fn run_style_op_then` in `crates/designos-app/src/styles.rs:143` was widened by one token to `pub(crate) fn run_style_op_then`.

---

### 2. Acceptance table (1–6), with commands and pasted output

| # | Check | Verdict | Evidence |
|---|---|---|---|
| 1 | `r2-red.log` (every new/updated test fails on `6961e62f` against real code) and `r2-green.log` (all pass on head) | PASS (`[FACT]`) | `plans/reports/agy-127-delete-style-evidence/r2-red.log` (11 failed across 4 suites), `plans/reports/agy-127-delete-style-evidence/r2-green.log` (all passed) |
| 2 | `cargo fmt --all -- --check` exits 0 and `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0 | PASS (`[FACT]`) | `FMT_EXIT=0`, `CLIPPY_EXIT=0` |
| 3 | `cargo test -p designos-doc --locked`, `cargo test -p designos-fig --locked`, and focused `designos-app` style sim tests pass under the harness | PASS (`[FACT]`) | `designos-doc`: 637 + 3 + 5 passed; `designos-fig`: 121 unit + 451 integration passed; `designos-app`: `effect_grid_styles` (8 passed), `style_edit` (12 passed) |
| 4 | `python3 scripts/zero_fake_ui_gate.py` passes | PASS (`[FACT]`) | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| 5 | New `1440×900` capture on the kit copy (local style's menu `"Grid desktop"` on `"MacBook Pro"`, as no library style is reachable on the kit copy) | PASS (`[FACT]`) | `plans/reports/agy-127-delete-style-evidence/r2-style-menu-delete.png` (`1440×900`) |
| 6 | Delete `out/` (`rm -rf out`), commit `fix(styles): …`, push, update PR #1112 body | PASS (`[FACT]`) | `OUT_REMOVED=yes` |

#### Acceptance 1 — `r2-red.log` (against real `6961e62f` code) and `r2-green.log`

Command (Red — run with only the Round 2 test additions/updates applied on top of `6961e62f`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  echo "=== cargo test -p designos-doc --locked styles_ops ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-doc --locked styles_ops 2>&1 || true
  echo "=== cargo test -p designos-fig --test fig_effect_grid_styles --locked ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-fig --test fig_effect_grid_styles --locked 2>&1 || true
  echo "=== cargo test -p designos-app --bin designos --locked effect_grid_styles ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-app --bin designos --locked effect_grid_styles 2>&1 || true
  echo "=== cargo test -p designos-app --bin designos --locked style_edit ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-app --bin designos --locked style_edit 2>&1 || true
} > plans/reports/agy-127-delete-style-evidence/r2-red.log
rm -rf "$S"
```

Output (`plans/reports/agy-127-delete-style-evidence/r2-red.log`):
```text
=== cargo test -p designos-doc --locked styles_ops ===
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)

running 20 tests
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file ... ok
test styles_ops::tests::a_colour_style_edit_recolours_every_run_of_a_rich_text_layer ... ok
test styles_ops::tests::applying_links_nodes_and_copies_values ... ok
test styles_ops::tests::creation_is_refused_without_values_or_on_the_wrong_node ... ok
test styles_ops::tests::a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides ... ok
test styles_ops::tests::detaching_drops_the_link_but_keeps_the_values ... ok
test styles_ops::tests::editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step ... ok
test styles_ops::tests::editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused ... ok
test styles_ops::tests::editing_a_style_updates_every_linked_node_and_only_those ... ok
test styles_ops::tests::deleting_a_text_style_keeps_a_bold_run ... FAILED
test styles_ops::tests::deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches ... FAILED
test styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step ... FAILED
test styles_ops::tests::deleting_a_colour_style_on_text_keeps_a_highlighted_run ... FAILED
test styles_ops::tests::effect_style_is_created_from_a_node_and_undone_in_one_step ... ok
test styles_ops::tests::grid_style_is_created_from_a_frame_with_a_chosen_name ... ok
test styles_ops::tests::renaming_changes_the_display_name_only ... ok
test styles_ops::tests::reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref ... FAILED
test styles_ops::tests::effect_and_grid_styles_survive_a_native_save_and_load ... ok

failures:

---- styles_ops::tests::deleting_a_text_style_keeps_a_bold_run stdout ----

thread 'styles_ops::tests::deleting_a_text_style_keeps_a_bold_run' (48551417) panicked at crates/designos-doc/src/styles_ops.rs:1011:9:
assertion `left == right` failed: every run keeps its weight after deleting the text style
  left: [([Paint { kind: Solid(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 400, 16.0), ([Paint { kind: Solid(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 400, 16.0)]
 right: [([Paint { kind: Solid(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 400, 16.0), ([Paint { kind: Solid(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 700, 16.0)]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- styles_ops::tests::deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches stdout ----

thread 'styles_ops::tests::deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches' (48551416) panicked at crates/designos-doc/src/styles_ops.rs:1082:9:
assertion `left == right` failed
  left: None
 right: Some("v_surface")

---- styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step stdout ----

thread 'styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step' (48551418) panicked at crates/designos-doc/src/styles_ops.rs:860:9:
assertion `left == right` failed
  left: None
 right: Some("v_fill")

---- styles_ops::tests::deleting_a_colour_style_on_text_keeps_a_highlighted_run stdout ----

thread 'styles_ops::tests::deleting_a_colour_style_on_text_keeps_a_highlighted_run' (48551415) panicked at crates/designos-doc/src/styles_ops.rs:984:9:
assertion `left == right` failed: every run keeps its colour after deleting the fill style
  left: [([Paint { kind: Solid(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 400, 12.0), ([Paint { kind: Solid(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 400, 12.0)]
 right: [([Paint { kind: Solid(Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 400, 12.0), ([Paint { kind: Solid(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }), opacity: 1.0, visible: true, blend: Normal }], 400, 12.0)]

---- styles_ops::tests::reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref stdout ----

thread 'styles_ops::tests::reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref' (48551427) panicked at crates/designos-doc/src/styles_ops.rs:1128:41:
called `Result::unwrap_err()` on an `Ok` value: Batch([SetStyles { styles: {}, style_meta: {} }, Update { node: Node { id: NodeId(6), ... } }])

failures:
    styles_ops::tests::deleting_a_colour_style_on_text_keeps_a_highlighted_run
    styles_ops::tests::deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches
    styles_ops::tests::deleting_a_text_style_keeps_a_bold_run
    styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step
    styles_ops::tests::reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref

test result: FAILED. 15 passed; 5 failed; 0 ignored; 0 measured; 617 filtered out; finished in 0.00s

=== cargo test -p designos-fig --test fig_effect_grid_styles --locked ===
running 14 tests
test a_style_used_by_a_remote_component_on_the_library_page_cannot_be_deleted ... FAILED
test deleting_colour_and_text_styles_on_mixed_run_text_preserves_rendered_pixels ... FAILED
...
---- a_style_used_by_a_remote_component_on_the_library_page_cannot_be_deleted stdout ----
thread 'a_style_used_by_a_remote_component_on_the_library_page_cannot_be_deleted' (48551807) panicked at crates/designos-fig/tests/fig_effect_grid_styles.rs:618:41:
called `Result::unwrap_err()` on an `Ok` value: Batch([SetStyles { styles: {}, style_meta: {} }, Update { node: Node { id: NodeId(2), ... } }])

---- deleting_colour_and_text_styles_on_mixed_run_text_preserves_rendered_pixels stdout ----
thread 'deleting_colour_and_text_styles_on_mixed_run_text_preserves_rendered_pixels' (48551810) panicked at crates/designos-fig/tests/fig_effect_grid_styles.rs:671:5:
assertion `left == right` failed: rendered pixels of mixed-colour text must not change after deleting its fill style (351 bytes differed)
  left: 351
 right: 0

test result: FAILED. 12 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

=== cargo test -p designos-app --bin designos --locked effect_grid_styles ===
running 8 tests
test shell::tests::effect_grid_styles::a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu ... FAILED
test shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu ... FAILED
...
---- shell::tests::effect_grid_styles::a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu stdout ----
thread 'shell::tests::effect_grid_styles::a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu' (48553351) panicked at crates/designos-app/src/shell/tests/effect_grid_styles.rs:216:5:
Delete style row is disabled when a node on the hidden library page uses the style

---- shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu stdout ----
thread 'shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu' (48553350) panicked at crates/designos-app/src/shell/tests/effect_grid_styles.rs:189:9:
assertion `left == right` failed: Effect: the Delete style row keeps its normal label
  left: "Edit in the library file"
 right: "Delete style"

test result: FAILED. 6 passed; 2 failed; 0 ignored; 0 measured; 2933 filtered out; finished in 0.68s

=== cargo test -p designos-app --bin designos --locked style_edit ===
running 15 tests
test shell::tests::style_edit::a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row ... FAILED
test shell::tests::style_edit::a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu ... FAILED
...
---- shell::tests::style_edit::a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row stdout ----
thread 'shell::tests::style_edit::a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row' (48555312) panicked at crates/designos-app/src/shell/tests/style_edit.rs:287:9:
assertion `left == right` failed: Effect: the Rename style row keeps its normal label
  left: "Edit in the library file"
 right: "Rename style"

---- shell::tests::style_edit::a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu stdout ----
thread 'shell::tests::style_edit::a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu' (48555310) panicked at crates/designos-app/src/shell/tests/style_edit.rs:262:9:
assertion `left == right` failed: Effect: the Edit style row keeps its normal label
  left: "Edit in the library file"
 right: "Edit style"

test result: FAILED. 13 passed; 2 failed; 0 ignored; 0 measured; 2926 filtered out; finished in 1.15s
```

Command (Green — `r2-green.log`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  echo "=== designos-doc::styles_ops (r2-green) ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-doc --lib styles_ops --locked -- --nocapture 2>&1
  echo "=== designos-fig::fig_effect_grid_styles (r2-green) ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-fig --test fig_effect_grid_styles --locked -- --nocapture 2>&1
  echo "=== designos-app::shell::tests::effect_grid_styles (r2-green) ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-app --bin designos shell::tests::effect_grid_styles --locked -- --nocapture 2>&1
  echo "=== designos-app::shell::tests::style_edit (r2-green) ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
      cargo test -p designos-app --bin designos shell::tests::style_edit --locked -- --nocapture 2>&1
} > plans/reports/agy-127-delete-style-evidence/r2-green.log
rm -rf "$S"
```

Output (`plans/reports/agy-127-delete-style-evidence/r2-green.log`):
```text
=== designos-doc::styles_ops (r2-green) ===
running 20 tests
test styles_ops::tests::a_colour_style_edit_recolours_every_run_of_a_rich_text_layer ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file ... ok
test styles_ops::tests::creation_is_refused_without_values_or_on_the_wrong_node ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file ... ok
test styles_ops::tests::editing_a_style_updates_every_linked_node_and_only_those ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file ... ok
test styles_ops::tests::editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused ... ok
test styles_ops::tests::a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides ... ok
test styles_ops::tests::editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step ... ok
test styles_ops::tests::applying_links_nodes_and_copies_values ... ok
test styles_ops::tests::deleting_a_colour_style_on_text_keeps_a_highlighted_run ... ok
test styles_ops::tests::deleting_a_text_style_keeps_a_bold_run ... ok
test styles_ops::tests::deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches ... ok
test styles_ops::tests::grid_style_is_created_from_a_frame_with_a_chosen_name ... ok
test styles_ops::tests::detaching_drops_the_link_but_keeps_the_values ... ok
test styles_ops::tests::renaming_changes_the_display_name_only ... ok
test styles_ops::tests::effect_style_is_created_from_a_node_and_undone_in_one_step ... ok
test styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step ... ok
test styles_ops::tests::reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref ... ok
test styles_ops::tests::effect_and_grid_styles_survive_a_native_save_and_load ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 617 filtered out; finished in 0.02s

=== designos-fig::fig_effect_grid_styles (r2-green) ===
running 14 tests
test a_soft_deleted_style_is_ignored ... ok
test a_stroke_style_node_is_a_stroke_style ... ok
test legacy_inherit_refs_resolve_to_the_style_nodes ... ok
test fill_style_gradient_stop_binding_round_trips ... ok
test noise_texture_and_glass_effects_round_trip_through_fig_import_export_reimport ... ok
test a_style_used_by_a_remote_component_on_the_library_page_cannot_be_deleted ... ok
test effect_style_shadow_vars_round_trip ... ok
test consumers_take_effect_and_grid_values_from_the_style_node ... ok
test fig_style_colorvar_round_trips ... ok
test export_then_import_keeps_effect_and_grid_styles_and_their_links ... ok
test style_values_and_metadata_are_imported ... ok
test deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip ... ok
test text_style_paragraph_indent_and_weight_round_trip ... ok
test deleting_colour_and_text_styles_on_mixed_run_text_preserves_rendered_pixels ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

=== designos-app::shell::tests::effect_grid_styles (r2-green) ===
running 8 tests
test shell::tests::effect_grid_styles::local_styles_list_effect_and_grid_groups ... ok
test shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu ... ok
test shell::tests::effect_grid_styles::creating_an_effect_style_links_the_layer_in_one_undo_step ... ok
test shell::tests::effect_grid_styles::renaming_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu ... ok
test shell::tests::effect_grid_styles::creating_detaching_and_applying_a_grid_style ... ok
test shell::tests::effect_grid_styles::detaching_and_applying_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 2933 filtered out; finished in 0.68s

=== designos-app::shell::tests::style_edit (r2-green) ===
running 12 tests
test shell::tests::style_edit::a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu ... ok
test shell::tests::style_edit::a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row ... ok
test shell::tests::style_edit::a_library_style_cannot_be_edited_from_the_list ... ok
test shell::tests::style_edit::deleted_variable_bound_on_a_style_renders_detachable_pill_and_emits_broken_style_binding_lint ... ok
test shell::tests::style_edit::a_grid_style_count_field_never_leaves_a_guide_that_draws_nothing ... ok
test shell::tests::style_edit::editing_a_color_style_from_the_list_updates_every_linked_layer_in_one_undo_step ... ok
test shell::tests::style_edit::editing_a_text_style_from_the_list_restyles_every_linked_text_layer ... ok
test shell::tests::style_edit::a_drag_is_recorded_once_when_the_pointer_is_released ... ok
test shell::tests::style_edit::editing_a_grid_style_from_the_style_menu_updates_the_linked_frame ... ok
test shell::tests::style_edit::editing_an_effect_style_from_its_linked_row_updates_every_linked_layer_in_one_undo_step ... ok
test shell::tests::style_edit::scrubbing_text_and_grid_style_fields_updates_linked_layers_in_one_undo_step ... ok
test shell::tests::style_edit::popover_number_fields_use_distinct_non_duplicate_prefixes ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 2929 filtered out; finished in 1.16s
```

#### Acceptance 2 — `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
    GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    cargo fmt --all -- --check
echo "FMT_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
    GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    cargo clippy --workspace --all-targets --locked -- -D warnings
echo "CLIPPY_EXIT=$?"
rm -rf "$S"
```

Output:
```text
FMT_EXIT=0
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-mcp)
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-eval)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-gpu)
    Checking designos-render v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-render)
    Checking designos-fig v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-fig)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.31s
CLIPPY_EXIT=0
```

#### Acceptance 3 — `cargo test -p designos-doc --locked`, `cargo test -p designos-fig --locked`, and focused `designos-app` style sim tests

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
    GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    cargo test -p designos-doc --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
    GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    cargo test -p designos-fig --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
    GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    cargo test -p designos-app --bin designos shell::tests::effect_grid_styles --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
    GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    cargo test -p designos-app --bin designos shell::tests::style_edit --locked
rm -rf "$S"
```

Output summary:
```text
Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)
test result: ok. 637 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.86s
Running tests/auto_layout_wrap_fields_compat.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/changelog_pillars.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
DOC_EXIT=0

Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-540f1f9206f56183)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.21s
Running tests/fig_effect_grid_styles.rs (out/cargo/debug/deps/fig_effect_grid_styles-46a2f99555638b59)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
(plus all remaining designos-fig integration test binaries: 0 failed)
FIG_EXIT=0

Running unittests src/main.rs (shell::tests::effect_grid_styles)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 2933 filtered out; finished in 0.68s
Running unittests src/main.rs (shell::tests::style_edit)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 2929 filtered out; finished in 1.16s
```

#### Acceptance 4 — `python3 scripts/zero_fake_ui_gate.py`

Command:
```sh
python3 scripts/zero_fake_ui_gate.py
```

Output:
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

#### Acceptance 5 — New `1440×900` capture on the kit copy (`r2-style-menu-delete.png`)

**Which menu:** Local style's menu (`"Grid desktop"` linked on `"MacBook Pro"` on the `"Getting started"` page of the Untitled UI kit copy at `1440×900`), because no library style is reachable on a selectable layer on the kit copy (`.fig` import does not populate `doc.library.external` for styles, and the remote-component instances on the reachable pages carry no effect or grid style references).

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cp "/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$S/kit.fig"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
    GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" \
    out/cargo/debug/designos "$S/kit.fig" --select "MacBook Pro" --zoom 100 --panel grid-styles --size 1440x900 --no-toast --screenshot plans/reports/agy-127-delete-style-evidence/r2-style-menu-delete.png
echo "EXIT=$?"
ls -lh plans/reports/agy-127-delete-style-evidence/r2-style-menu-delete.png
sips -g pixelWidth -g pixelHeight plans/reports/agy-127-delete-style-evidence/r2-style-menu-delete.png
rm -rf "$S"
```

Output:
```text
EXIT=0
-rw-r--r--@ 1 jangtrinh  staff   280K Oct 25 13:05 plans/reports/agy-127-delete-style-evidence/r2-style-menu-delete.png
/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/plans/reports/agy-127-delete-style-evidence/r2-style-menu-delete.png
  pixelWidth: 1440
  pixelHeight: 900
```

#### Acceptance 6 — Delete `out/` (`rm -rf out`)

Command:
```sh
rm -rf out && test ! -e out && echo "OUT_REMOVED=yes"
```

Output:
```text
OUT_REMOVED=yes
```

---

### 3. Changes per file (Round 2)

- `crates/designos-doc/src/styles_ops.rs`:
  - **H1 (Delete never changes how a layer looks):** Removed the `apply_style_values(&mut copy, stored, key)` re-apply in `delete_style`. Linked layers already hold their stored values (and any per-run text overrides such as a highlighted word or bold word), so removing the `style_refs` entry leaves the layer's visual appearance unchanged.
  - **M2 (Variable bindings stay bound):** When the deleted style has `StyleMeta::bindings`, `delete_style` now iterates over each linked slot (`"fill"`, `"stroke"`, `"text"`, `"effect"`), translates `"fills/..."` keys to `"strokes/..."` when linked in the `"stroke"` slot, preserves any existing node-level binding on `copy.variables.bindings` (`if copy.variables.bindings.contains_key(&node_key) { continue; }`), and inserts `(node_key, variable.clone())` into `copy.variables.bindings` whenever `property_kind(&node_key).is_some()`.
    - **List of fallback cases where a binding cannot move (`property_kind(&node_key).is_none()`):** None of the valid `StyleMeta::bindings` keys accepted by `style_property_kind` (`fills/{i}`, `fills/{i}/opacity`, `fills/{i}/stops/{j}` and their `strokes/{i}` counterparts, the 8 text properties `fontSize`/`lineHeight`/`letterSpacing`/`paragraphSpacing`/`paragraphIndent`/`fontWeight`/`fontFamily`/`fontStyle`, and the 6 effect properties `effects/{i}/color`/`radius`/`spread`/`offsetX`/`offsetY`/`opacity`) lack a node binding slot (`property_kind(&node_key)` returns `Some(_)` for all of them, and `Style::Grid` has no variable bindings). The `else if let Some(val) = doc.resolve_variable(id, variable) { set_bound_property(&mut copy, &node_key, &val); }` branch is kept as a defensive fallback only for unrecognized/synthetic keys in `StyleMeta::bindings` where `property_kind(&node_key).is_none()`.
    - **Why the binding loop is not shared with `resolve_node` (`crates/designos-doc/src/variables_apply.rs:540`):** After M2, `delete_style` transfers binding references (`String -> String`) into `copy.variables.bindings` without evaluating or writing resolved literals onto the node's raw properties, whereas `resolve_node` evaluates variable values and writes resolved literals onto a temporary `Cow<Node>` at render/export time without mutating `node.variables.bindings` (and `variables_apply.rs` is outside the allowed modified-files scope).
  - **M1 (Library copies):** Added `pub fn used_on_library_page(doc: &Document, key: &str) -> bool`, which checks whether any node on the hidden library page (`is_library_node(doc, n.id)`) references `key` in `n.style_refs`. Updated `delete_style` to refuse with `StyleOpError::LibraryStyle(key.into())` when `is_library_style(doc, key) || used_on_library_page(doc, key)`.
  - **Tests in `styles_ops.rs`:** Updated `deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step` to assert `node.variables.bindings` transfer on delete and clean undo; added `deleting_a_colour_style_on_text_keeps_a_highlighted_run`, `deleting_a_text_style_keeps_a_bold_run`, `deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches` (dark/light mode look before & after delete + mode switching after delete), and `reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref`.
- `crates/designos-app/src/panels/style_edit.rs`:
  - Updated `pub(crate) fn is_read_only(app: &App, key: &str) -> bool` to return `designos_doc::styles_ops::is_library_style(&app.doc, key) || designos_doc::styles_ops::used_on_library_page(&app.doc, key)`.
- `crates/designos-app/src/panels/style_picker.rs`:
  - **Ruling 4 (Library style menu):** Updated `menu_model` so `"Rename style"`, `"Delete style"`, and `"Edit style"` always keep their normal row labels (`"Rename style"`, `"Delete style"`, `"Edit style"`) and attach `.tooltip(ro_tip)` (`"Edit in the library file"` when `read_only`, `""` otherwise) instead of replacing all three labels with `"Edit in the library file"`.
  - **Ruling 5 (`run_style_op_then` reuse):** Simplified `delete_linked_style` to delegate to `app.run_style_op_then(op, |_, _| {})` instead of duplicating the `apply` / `record_relayout` / `changed` / `toast` / error-handling block.
  - Updated `#[cfg(test)]` helpers `edit_style_row`, `rename_style_row`, and `delete_style_row` to return `MenuItem` so tests inspect `label`, `disabled`, and `tooltip`.
- `crates/designos-app/src/styles.rs`:
  - Widened `fn run_style_op_then` at line 143 to `pub(crate) fn run_style_op_then` (1-token visibility change) so `style_picker::delete_linked_style` can invoke `app.run_style_op_then(...)`.
- `crates/designos-app/src/shell/tests/effect_grid_styles.rs`:
  - Updated `a_library_style_cannot_be_deleted_from_the_style_menu` to verify `row.label == "Delete style"`, `row.disabled`, and `row.tooltip == style_edit::LIBRARY_STYLE_TIP`.
  - Added `a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu` verifying that a style used by a node on `doc.library_page` is disabled in the style menu with `"Edit in the library file"` tooltip and refused if clicked.
- `crates/designos-app/src/shell/tests/style_edit.rs`:
  - Updated `a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu` and `a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row` to assert that `"Edit style"` and `"Rename style"` keep their normal labels (`"Edit style"` and `"Rename style"`), are disabled (`row.disabled`), and carry `row.tooltip == style_edit::LIBRARY_STYLE_TIP` (`"Edit in the library file"`).
- `crates/designos-fig/tests/fig_effect_grid_styles.rs`:
  - Added `a_style_used_by_a_remote_component_on_the_library_page_cannot_be_deleted` (verifying `delete_style` returns `Err(StyleOpError::LibraryStyle)` when a remote component on the hidden library page uses the style) and `deleting_colour_and_text_styles_on_mixed_run_text_preserves_rendered_pixels` (verifying `designos_render::render_node` pixels are byte-for-byte identical before and after deleting a colour style on text with one red word and a text style on text with one bold word).

---

### 4. Every user-facing string, verbatim

- `"Rename style"`
- `"Detach style"`
- `"Delete style"`
- `"Edit style"`
- `"Edit in the library file"`
- `"Style deleted · Cmd+Z to undo"`
- `"Style deleted · Ctrl+Z to undo"`
- `"Style not changed: {e}"`

Status: DONE

---

## Round 3

### 0. Skills loaded & checks applied (`msg_b4231ee679d3`)

- **Skills loaded:** `es:dev` -> `es:feature-dev` (DEV phase only), `es:rust`, `es:rust-gpu-canvas`, `es:designer`, `es:astra-work`, `es:fable-thinking`.
- **What was checked and changed because of them:**
  1. **No allocation or full-document walk on a per-frame path (`es:rust`, `es:rust-gpu-canvas`, Ruling 1 / H-A):** Reverted `style_edit::is_read_only` (`crates/designos-app/src/panels/style_edit.rs:35-37`) to `is_library_style(&app.doc, key)` (`O(1)` lookup on `doc.style_meta`), restricted `used_on_library_page` (`crates/designos-doc/src/styles_ops.rs:321-333`) to walk only the `doc.library_page` subtree via `n.children` DFS (early-returning `false` when `doc.library_page.is_none()`), and called `used_on_library_page` only for the `"Delete style"` row in `style_picker::menu_model` (`crates/designos-app/src/panels/style_picker.rs:91`) when the style menu is open and in `delete_style` (`crates/designos-doc/src/styles_ops.rs:508`). Verified via `take_library_page_scans` (`crates/designos-doc/src/styles_ops.rs:316-318`) in `library_page_scan_never_runs_on_idle_frame_and_at_most_once_when_menu_open` (`crates/designos-app/src/shell/tests/effect_grid_styles.rs:285-306`) that `0` scans run per idle frame with the menu closed and at most `1` runs per frame with the menu open.
  2. **`egui` lock discipline (`es:rust`):** Verified zero `ctx.load_texture` calls inside `ctx.data_mut` across all touched files.
  3. **`interactive()` guards (`es:rust-gpu-canvas`):** Verified `crates/designos-gpu/src/compile.rs` was not modified and all `self.interactive()` guards remain intact.
  4. **Menu & tooltip craft (`es:designer`, Ruling 5 / L-C):** Updated the colour-style menu in `crates/designos-app/src/panels/properties.rs:261-263` so a linked library style keeps its normal `"Edit style"` label, is disabled (`linked.is_none() || read_only`), and attaches `.tooltip(ro_tip)` (`"Edit in the library file"`).

---

### 1. What could NOT be verified

- **Visual capture (b) — live window screenshot of a library style's menu showing disabled rows via Home/library flow or CLI flags:** `[FACT]` Not reachable from CLI flags in an isolated `HOME` for three concrete reasons verified in source:
  1. `crates/designos-app/src/cli.rs` provides no CLI flag to publish a document as a library or enable a library in a second consumer file.
  2. `library_styles_list` in `crates/designos-app/src/panels/properties.rs:607-609` filters `app.library_styles()` with `matches!(s.style, Style::Text(_) | Style::Fill(_))`, and `style_picker::menu_model` (`crates/designos-app/src/panels/style_picker.rs:76`) populates the effect/grid style menu only from `style_entries_of(&app.doc, kind_of(prop))` (`doc.styles`), so even in an interactive session an enabled library's effect or grid style cannot be linked onto a consumer layer from the UI.
  3. Under Ruling 1 (H-A), a style used on `doc.library_page` disables only `"Delete style"` while keeping `"Rename style"` and `"Edit style"` enabled; only a style with `is_library_style(&doc, key) == true` disables all three rows (`"Rename style"`, `"Delete style"`, `"Edit style"`) in `style_picker::menu_model` (`crates/designos-app/src/panels/style_picker.rs:89-97`) and `"Edit style"` in the colour-style menu (`crates/designos-app/src/panels/properties.rs:261-263`).
  - **Sim tests that prove the disabled rows and their hover tooltips paint:**
    - `crates/designos-app/src/shell/tests/effect_grid_styles.rs:182-232` (`a_library_style_cannot_be_deleted_from_the_style_menu`): opens `StyleProperty::Effect`, hovers rows `menu-row-0-2` (`"Rename style"`), `menu-row-0-4` (`"Delete style"`), and `menu-row-0-5` (`"Edit style"`), and asserts `painted_texts(&s)` contains `style_edit::LIBRARY_STYLE_TIP` (`"Edit in the library file"`) for each row.
    - `crates/designos-app/src/shell/tests/effect_grid_styles.rs:235-282` (`a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu`): opens `StyleProperty::Effect` for a style referenced on `doc.library_page`, hovers `menu-row-0-4` (`"Delete style"`) and asserts `"Edit in the library file"` paints, then hovers `menu-row-0-2` (`"Rename style"`) and asserts `"Edit in the library file"` does not paint.
    - `crates/designos-app/src/shell/tests/style_edit.rs:253-318` (`a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu`): opens the colour-style menu (`app.props.styles_menu = Some(false)`) on `"Hero"` with `"Brand/Primary"` marked as a library style, asserts `"Edit style"` is painted (and `"Edit in the library file"` is not painted as the row label when unhovered), then hovers the `"Edit style"` row rect and asserts `"Edit in the library file"` paints as the tooltip.
- **Residual L-A edge case when own `"fontStyle"` has no weight keyword in one mode while the deleted style's `"fontWeight"` varies across modes:** `[FACT]` When a text node has its own `"fontStyle"` variable binding AND the deleted text style has a `"fontWeight"` variable binding (`own_binding_overrides_style_key` at `crates/designos-doc/src/styles_ops.rs:344-346`), `delete_style` does not move `"fontWeight"` into `copy.variables.bindings` (because `BTreeMap` key order applies `"fontStyle"` before `"fontWeight"`, which would let `"fontWeight"` clobber the weight implied by `"fontStyle"`, e.g. `"Bold Italic"` -> `700` clobbered to `300`) and instead bakes the current mode's resolved `"fontWeight"` onto the node (`crates/designos-doc/src/styles_ops.rs:537-541`). If the layer's own `"fontStyle"` variable resolves in another mode to a string without a weight keyword (such as `"Italic"`, where `apply_font_style` leaves `ts.font_weight` untouched) while the deleted style's `"fontWeight"` variable had a different per-mode weight in that other mode, baking a single scalar `ts.font_weight` cannot vary per mode without a separate node binding key that precedes `"fontStyle"`. Whenever `"fontStyle"` carries a weight keyword in each mode (e.g. `"Bold Italic"` / `"SemiBold Italic"`, or `"Regular"` / `"Bold"`), `resolve_node` produces the exact same `Node` in every mode before and after `delete_style` (`crates/designos-doc/src/styles_ops.rs:1429-1594` and `crates/designos-fig/tests/fig_effect_grid_styles.rs:701-793`).

---

### 2. Rulings 1–6 resolution summary

1. **Ruling 1 (H-A — library-page scan never runs on an idle frame):**
   - `style_edit::is_read_only` (`crates/designos-app/src/panels/style_edit.rs:35-37`) is restored to its round-1 definition (`pub(super) fn is_read_only(app: &App, key: &str) -> bool { is_library_style(&app.doc, key) }`).
   - Only the `"Delete style"` row in `style_picker::menu_model` (`crates/designos-app/src/panels/style_picker.rs:91-96`) and `delete_style` (`crates/designos-doc/src/styles_ops.rs:508`) call `used_on_library_page`.
   - `used_on_library_page` (`crates/designos-doc/src/styles_ops.rs:321-333`) walks only the `doc.library_page` subtree (`stack` initialized with `doc.library_page`, pushing `n.children`).
   - **Recorded numbers:**
     - **Sim call counter (`library_page_scan_never_runs_on_idle_frame_and_at_most_once_when_menu_open`, `crates/designos-app/src/shell/tests/effect_grid_styles.rs:285-306`):** `0` scans per frame with a style-linked layer selected and the style menu closed (`closed_scans == 0`); `1` scan per frame with the style menu open (`open_scans == 1 <= 1`).
     - **`designos-doc` perf probe (`used_on_library_page_walks_only_the_library_page_subtree_and_probes_300k_nodes`, `crates/designos-doc/src/styles_ops.rs:1341-1426`, 300,000 main-page nodes + 2,000 library-page nodes):** `subtree = 594.212µs/call` vs `full_walk = 286.131589ms/call` (~481× faster; recorded in `plans/reports/agy-127-delete-style-evidence/r3-green.log`).
2. **Ruling 2 (L-D — model operations):**
   - `rename_style` (`crates/designos-doc/src/styles_ops.rs:146-162`), `update_style` (`crates/designos-doc/src/styles_ops.rs:170-265`), and `bind_style_property` (`crates/designos-doc/src/styles_ops.rs:274-309`) remain guarded by `is_library_style` only. Only `delete_style` (`crates/designos-doc/src/styles_ops.rs:508`) checks `is_library_style(doc, key) || used_on_library_page(doc, key)`.
3. **Ruling 3 (L-A — look unchanged in every mode, including mixed own and style bindings):**
   - Added `own_binding_overrides_style_key` (`crates/designos-doc/src/styles_ops.rs:342-375`), which detects when moving a style binding `node_key` into `copy.variables.bindings` (`BTreeMap<String, String>`) would sort *after* a layer's own binding that overrides the same visible field:
     1. Style `"fontWeight"` vs layer's own `"fontStyle"` (`"fontStyle"` < `"fontWeight"` in `BTreeMap` key order, and `set_bound_property` for `"fontStyle"` sets `ts.font_weight` via `apply_font_style`).
     2. Style `"fills/{i}/opacity"` vs layer's own `"fills/{i}"` when the `"fills/{i}"` color variable has a translucent mode (`c.a < 1.0`, checked by `color_var_has_translucent_mode` at `crates/designos-doc/src/styles_ops.rs:335-337`), because `set_paint_color` sets `paint.opacity = c.a` when `c.a < 1.0`.
     3. Style `"strokes/{i}/opacity"` vs layer's own `"strokes/{i}"` when the `"strokes/{i}"` color variable has a translucent mode (`c.a < 1.0`).
     4. Style `"effects/{i}/opacity"` vs layer's own `"effects/{i}/color"` (`"effects/{i}/color"` < `"effects/{i}/opacity"` in `BTreeMap` key order, and `set_effect_field(_, "color", _)` overwrites `color.a`).
     5. Style `"fills/{i}/stops/{j}"` vs layer's own `"fills/{i}"` (and `"strokes/{i}/stops/{j}"` vs `"strokes/{i}"`), where `"fills/{i}"` replaces `paint.kind = PaintKind::Solid(...)`.
   - When `own_binding_overrides_style_key(doc, n, &node_key)` is true (`crates/designos-doc/src/styles_ops.rs:537-542`), `delete_style` does not insert `node_key` into `copy.variables.bindings` and instead bakes `set_bound_property(&mut copy, &node_key, &val)` on `copy` before the layer's own binding runs.
   - Tested in `deleting_a_style_with_mixed_own_and_style_bindings_preserves_resolved_look_in_all_modes` (`crates/designos-doc/src/styles_ops.rs:1429-1594`) and `own_font_style_binding_and_style_font_weight_binding_preserve_resolved_node_across_modes` (`crates/designos-fig/tests/fig_effect_grid_styles.rs:701-793`).
4. **Ruling 4 (Nothing vanishes silently):**
   - Added `bake_unrecognized_style_binding` (`crates/designos-doc/src/styles_ops.rs:379-498`) called from the fallback branch of `delete_style` (`crates/designos-doc/src/styles_ops.rs:545-547`) when `property_kind(&node_key).is_none()`, normalizing legacy/aliased keys (`fontSize/0`, `fills/0/color`, `strokes/0/color`, `fillOpacities/0`, `strokeOpacities/0`, `fillStops/0/1`, `strokeStops/0/1`, `cornerRadius`, `topLeftRadius`/`topRightRadius`/`bottomRightRadius`/`bottomLeftRadius`, `paddingLeft`/`paddingRight`/`paddingTop`/`paddingBottom`, `itemSpacing`, `counterAxisSpacing`, `strokeWeight`/`strokeTopWeight`, `width`/`height`/`minWidth`/`maxWidth`/`minHeight`/`maxHeight`, `opacity`, `visible`, `characters`, and bare effect fields `color`/`radius`/`spread`/`offsetX`/`offsetY`) and baking their resolved value onto `copy`. Tested in `deleting_a_style_bakes_unrecognized_binding_keys_instead_of_dropping_them` (`crates/designos-doc/src/styles_ops.rs:1597-1707`).
   - Replaced `let _stored = doc.styles.get(key).ok_or_else(...)` with `if !doc.styles.contains_key(key) { return Err(StyleOpError::UnknownStyle(key.into())); }` (`crates/designos-doc/src/styles_ops.rs:505-507`).
   - Replaced `app.run_style_op_then(op, |_, _| {})` in `delete_linked_style` (`crates/designos-app/src/panels/style_picker.rs:351`) with `app.run_style_op(op)` (`crates/designos-app/src/styles.rs:138-140`), keeping `fn run_style_op_then` private (`crates/designos-app/src/styles.rs:144`).
   - Updated `deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip` (`crates/designos-fig/tests/fig_effect_grid_styles.rs:559-610`) to decode the raw exported `.fig` message via `exported_style_node_names` (`container::open` + `container::read_archive_chunks` + `Schema::decode` filtering `nodeChanges` entries where `nc.get("styleType").is_some()`) and assert `!style_names.iter().any(|n| n == "Shadow/Card" || n == "2:100")`. Demonstrated red in `plans/reports/agy-127-delete-style-evidence/r3-red.log` when `"2:100"` is kept (`exported .fig still has a style node for deleted Shadow/Card: ["Shadow/Card", "Layout/12 col", "Border/Strong"]`).
5. **Ruling 5 (L-C — colour-style menu in `crates/designos-app/src/panels/properties.rs:261-263`):**
   - Replaced the label swap (`if read_only { "Edit in the library file" } else { "Edit style" }`) with `let ro_tip = if read_only { super::style_edit::LIBRARY_STYLE_TIP } else { "" };` and `MenuItem::new("Edit style").disabled(linked.is_none() || read_only).tooltip(ro_tip)` (`crates/designos-app/src/panels/properties.rs:261-263`). Verified in `a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu` (`crates/designos-app/src/shell/tests/style_edit.rs:271-318`).
6. **Ruling 6 (L-B — report accuracy & citation fixes):**
   - **Cmd+Z toast citations:** The existing `"· Cmd+Z to undo"` toasts in `designos-app` are at `crates/designos-app/src/cmdk.rs:1072-1077` (`done_toast`) and `crates/designos-app/src/panels/check_designs.rs:272` and `299` (`check_designs_fix` / `check_designs_fix_all`), NOT `toolbar.rs:424` (which is a hover tooltip `"Remove background (Cmd+Z to undo)"`, not a toast).
   - **Test row helpers in `style_picker.rs`:** `edit_style_row` (`crates/designos-app/src/panels/style_picker.rs:104-106`), `rename_style_row` (`crates/designos-app/src/panels/style_picker.rs:110-113`), and `delete_style_row` (`crates/designos-app/src/panels/style_picker.rs:117-120`) already returned `MenuItem` in Round 1 (`6961e62f`); Round 2 did not change their return type.
   - **`is_read_only` visibility (`crates/designos-app/src/panels/style_edit.rs:35-37`):** Restored to `pub(super) fn is_read_only(app: &App, key: &str) -> bool` calling unqualified `is_library_style(&app.doc, key)` (imported via `use designos_doc::styles_ops::*;` at `crates/designos-app/src/panels/style_edit.rs:17`).
   - **Visual capture:** Captured a brand-new `1440×900` screenshot from HEAD at `plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png` (`sha256: c7815b196261aaf74625abd208c515c8f22f97701d490dfec04ab0055ed39de6`, distinct from Round 1/2's `88d076ae1e47bec7a08edaec0fdef7d62ab2b4742999afdaf1699db4f71ea3ea`).

---

### 3. Acceptance table (Round 3), with commands and pasted output

| # | Check | Verdict | Evidence |
|---|---|---|---|
| 1 | `r3-red.log` and `r3-green.log` | PASS (`[FACT]`) | `plans/reports/agy-127-delete-style-evidence/r3-red.log`, `plans/reports/agy-127-delete-style-evidence/r3-green.log` |
| 2 | `cargo fmt --all -- --check` exits 0 | PASS (`[FACT]`) | Exit code `0` |
| 3 | `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0 | PASS (`[FACT]`) | Exit code `0` |
| 4 | `cargo test -p designos-doc --locked` and `cargo test -p designos-fig --locked` pass | PASS (`[FACT]`) | `designos-doc`: 640 + 3 + 5 passed; `designos-fig`: 121 unit + 450 integration passed |
| 5 | `cargo test -p designos-app --bin designos --locked style` passes under the isolated harness | PASS (`[FACT]`) | `89 passed; 0 failed` (including `effect_grid_styles`: 9 passed, `style_edit`: 12 passed) |
| 6 | `python3 scripts/zero_fake_ui_gate.py` passes | PASS (`[FACT]`) | `Gate Verdict: PASS (17/17 invariants satisfied)` |
| 7 | New `1440×900` visual evidence on kit copy in isolated `HOME` (`r3-local-style-menu.png`, verified `!= style-menu-delete.png`) | PASS (`[FACT]`) | `plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png` (`1440×900`, `IDENTICAL_TO_R1=no`) |
| 8 | Delete `out/` before finishing (`rm -rf out && test ! -e out`) | PASS (`[FACT]`) | `OUT_REMOVED=yes` |

#### Pasted commands and output (Round 3)

##### Check 1 — `r3-red.log` and `r3-green.log`

Command (Green, in isolated `HOME`):
```sh
export PATH="/Users/jangtrinh/.cargo/bin:$PATH"
export CARGO_HOME="/Users/jangtrinh/.cargo"
export RUSTUP_HOME="/Users/jangtrinh/.rustup"
S=$(mktemp -d /tmp/ds-sub.XXXXXX)
trap "rm -rf $S" EXIT
export HOME=$S/h TMPDIR=$S/t CARGO_TARGET_DIR=$PWD/out/cargo
export XDG_CONFIG_HOME=$S/c XDG_DATA_HOME=$S/d XDG_CACHE_HOME=$S/k
export GIT_AUTHOR_NAME=ds-worker GIT_AUTHOR_EMAIL=ds-worker@local
export GIT_COMMITTER_NAME=ds-worker GIT_COMMITTER_EMAIL=ds-worker@local
mkdir -p $HOME $TMPDIR $CARGO_TARGET_DIR $XDG_CONFIG_HOME $XDG_DATA_HOME $XDG_CACHE_HOME

{
  echo "=== GREEN 1: cargo test -p designos-doc --locked styles_ops -- --nocapture ==="
  cargo test -p designos-doc --locked styles_ops -- --nocapture 2>&1
  echo ""
  echo "=== GREEN 2: cargo test -p designos-fig --test fig_effect_grid_styles --locked ==="
  cargo test -p designos-fig --test fig_effect_grid_styles --locked 2>&1
  echo ""
  echo "=== GREEN 3: cargo test -p designos-app --bin designos --locked style ==="
  cargo test -p designos-app --bin designos --locked style 2>&1
} | tee plans/reports/agy-127-delete-style-evidence/r3-green.log
```

Output summary (`r3-red.log` -> `r3-green.log`):
```text
--- r3-red.log (against 52d5541f) ---
designos-doc::styles_ops:
  FAILED: styles_ops::tests::deleting_a_style_bakes_unrecognized_binding_keys_instead_of_dropping_them
  FAILED: styles_ops::tests::deleting_a_style_with_mixed_own_and_style_bindings_preserves_resolved_look_in_all_modes
  FAILED: styles_ops::tests::used_on_library_page_walks_only_the_library_page_subtree_and_probes_300k_nodes
  test result: FAILED. 20 passed; 3 failed

designos-fig --test fig_effect_grid_styles:
  FAILED: deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip
    (exported .fig still has a style node for deleted Shadow/Card: ["Shadow/Card", "Layout/12 col", "Border/Strong"])
  FAILED: own_font_style_binding_and_style_font_weight_binding_preserve_resolved_node_across_modes
    (resolve_node in mode m1 must match before and after delete_style: font_weight 300 != 700)
  test result: FAILED. 13 passed; 2 failed

designos-app --bin designos style:
  FAILED: shell::tests::effect_grid_styles::a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu
    (Rename style stays enabled for library-page style)
  FAILED: shell::tests::effect_grid_styles::library_page_scan_never_runs_on_idle_frame_and_at_most_once_when_menu_open
    (expected 0 library-page scans per frame with the style menu closed, got 4)
  FAILED: shell::tests::style_edit::a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu
    (colour-style menu should paint 'Edit style' label, got [..., "Edit in the library file", ...])
  test result: FAILED. 86 passed; 3 failed

--- r3-green.log (HEAD) ---
=== GREEN 1: cargo test -p designos-doc --locked styles_ops -- --nocapture ===
running 23 tests
...
[perf-probe] 300k main nodes + 2k library_page nodes: subtree=594.212µs/call vs full_walk=286.131589ms/call
test styles_ops::tests::used_on_library_page_walks_only_the_library_page_subtree_and_probes_300k_nodes ... ok
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 617 filtered out; finished in 11.97s

=== GREEN 2: cargo test -p designos-fig --test fig_effect_grid_styles --locked ===
running 15 tests
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

=== GREEN 3: cargo test -p designos-app --bin designos --locked style ===
running 89 tests
test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 2853 filtered out; finished in 1.84s
```

##### Check 2 & 3 — `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`

Output:
```text
=== FMT CHECK ===
fmt exit: 0
=== CLIPPY ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.05s
clippy exit: 0
```

##### Check 4 & 5 — `cargo test -p designos-doc --locked`, `cargo test -p designos-fig --locked`, and `cargo test -p designos-app --bin designos --locked style`

Output:
```text
=== FULL DESIGNOS-DOC TESTS ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)
test result: ok. 640 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.86s
     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-d19362047bc102bf)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-c7fad459a80a2bad)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

=== FULL DESIGNOS-FIG TESTS ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-540f1f9206f56183)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.90s
     Running tests/fig_effect_grid_styles.rs (out/cargo/debug/deps/fig_effect_grid_styles-46a2f99555638b59)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
(all 34 integration test binaries in designos-fig: 450 passed; 0 failed)

=== DESIGNOS-APP STYLE TESTS ===
test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 2853 filtered out; finished in 1.84s
```

##### Check 6 — `python3 scripts/zero_fake_ui_gate.py`

Output:
```text
Gate Verdict: PASS (17/17 invariants satisfied)
```

##### Check 7 — Visual evidence (`plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png`)

Command:
```sh
cp "/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$S/kit.fig"
out/cargo/debug/designos "$S/kit.fig" --select "MacBook Pro" --zoom 85 --panel grid-styles --size 1440x900 --no-toast --screenshot plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png
sips -g pixelWidth -g pixelHeight plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png
shasum -a 256 plans/reports/agy-127-delete-style-evidence/style-menu-delete.png plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png
```

Output:
```text
SCREENSHOT_EXIT=0
-rw-r--r--@ 1 jangtrinh  staff   340K Oct 25 14:53 plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png
  pixelWidth: 1440
  pixelHeight: 900
IDENTICAL_TO_R1=no
88d076ae1e47bec7a08edaec0fdef7d62ab2b4742999afdaf1699db4f71ea3ea  plans/reports/agy-127-delete-style-evidence/style-menu-delete.png
c7815b196261aaf74625abd208c515c8f22f97701d490dfec04ab0055ed39de6  plans/reports/agy-127-delete-style-evidence/r3-local-style-menu.png
```

##### Check 8 — Delete `out/` (`rm -rf out && test ! -e out`)

Output:
```text
OUT_REMOVED=yes
```

---

### 4. Changes per file (Round 3, `path:line-range` checked with `sed -n`)

- `crates/designos-doc/src/styles_ops.rs:311-333`: Added thread-local scan counter `LIBRARY_PAGE_SCANS` and `pub fn take_library_page_scans() -> usize` (`316-318`), and rewrote `pub fn used_on_library_page(doc: &Document, key: &str) -> bool` (`321-333`) to walk only the `doc.library_page` subtree via `n.children` DFS instead of `doc.nodes.values()`.
- `crates/designos-doc/src/styles_ops.rs:335-375`: Added `color_var_has_translucent_mode` (`335-337`) and `own_binding_overrides_style_key` (`342-375`) to detect when moving a style binding onto `node.variables.bindings` would flip precedence against an existing node-level binding.
- `crates/designos-doc/src/styles_ops.rs:377-498`: Added `bake_unrecognized_style_binding` (`379-498`) so style binding keys not recognized by `property_kind` are baked onto the node instead of vanishing silently.
- `crates/designos-doc/src/styles_ops.rs:504-555`: Updated `delete_style` to use `if !doc.styles.contains_key(key)` (`505-507`), bake overridden style bindings when `own_binding_overrides_style_key` is true (`537-542`), and call `bake_unrecognized_style_binding` in the fallback branch (`545-547`).
- `crates/designos-doc/src/styles_ops.rs:1340-1707`: Added unit tests `used_on_library_page_walks_only_the_library_page_subtree_and_probes_300k_nodes` (`1341-1426`), `deleting_a_style_with_mixed_own_and_style_bindings_preserves_resolved_look_in_all_modes` (`1429-1594`), and `deleting_a_style_bakes_unrecognized_binding_keys_instead_of_dropping_them` (`1597-1707`).
- `crates/designos-app/src/panels/style_edit.rs:35-37`: Restored `pub(super) fn is_read_only(app: &App, key: &str) -> bool` to call `is_library_style(&app.doc, key)` only.
- `crates/designos-app/src/panels/style_picker.rs:17,89-97,348-352`: Imported `used_on_library_page` (`17`), computed `delete_ro = read_only || current.as_deref().is_some_and(|k| used_on_library_page(&app.doc, k))` (`91`) for `"Delete style"` (`96`) while keeping `"Rename style"` (`94`) and `"Edit style"` (`97`) on `read_only`, and simplified `delete_linked_style` (`348-352`) to call `app.run_style_op(op)`.
- `crates/designos-app/src/styles.rs:138-145`: Added `pub(crate) fn run_style_op` (`138-140`) and restored `fn run_style_op_then` (`144`) to private module visibility.
- `crates/designos-app/src/panels/properties.rs:261-263`: Updated the colour-style menu's `"Edit style"` row to keep the `"Edit style"` label when `read_only` and attach `.tooltip(ro_tip)` (`LIBRARY_STYLE_TIP`).
- `crates/designos-fig/tests/fig_effect_grid_styles.rs:559-610,700-793`: Added `exported_style_node_names` (`559-576`) and asserted against exported `.fig` style nodes in `deleted_style_leaves_no_orphan_style_node_or_dangling_ref_on_fig_round_trip` (`579-610`); added `own_font_style_binding_and_style_font_weight_binding_preserve_resolved_node_across_modes` (`701-793`).
- `crates/designos-app/src/shell/tests/effect_grid_styles.rs:175-306`: Added `painted_texts` helper (`175-179`), hover-tooltip paint assertions in `a_library_style_cannot_be_deleted_from_the_style_menu` (`215-232`) and `a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu` (`235-282`), assertions that `"Rename style"` and `"Edit style"` stay enabled for a library-page style (`250-254`), and `library_page_scan_never_runs_on_idle_frame_and_at_most_once_when_menu_open` (`285-306`).
- `crates/designos-app/src/shell/tests/style_edit.rs:271-318`: Extended `a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu` to open the colour-style menu (`styles_menu = Some(false)`) on a library fill style and verify `"Edit style"` is painted as the row label and `"Edit in the library file"` paints on hover.

---

### 5. Every user-facing string, verbatim

- `"Rename style"`
- `"Detach style"`
- `"Delete style"`
- `"Edit style"`
- `"Edit in the library file"`
- `"Style deleted · Cmd+Z to undo"`
- `"Style deleted · Ctrl+Z to undo"`
- `"Style not changed: {e}"`

Status: DONE


---

## Round 4

### 1. What could NOT be verified

- **Independent cross-runtime verifier pass for Round 4**: Not run inside this DEV worker session (`Role: DEV`; controller runs the independent verifier after `worker_done`).
- **Multi-mode divergence across 3+ modes when 1 mode is overridden by a layer's own binding and 2+ non-overridden modes hold distinct style variable values**: When a layer's own binding (`fontStyle`, `fills/N`, or `strokes/N`) overwrites the style binding (`fontWeight`, `fills/N/opacity`, or `strokes/N/opacity`) in at least one mode, keeping the style binding on `node.variables.bindings` would reverse precedence in the overridden mode (because `"fontStyle" < "fontWeight"`, `"fills/N" < "fills/N/opacity"`, and `"strokes/N" < "strokes/N/opacity"` in `BTreeMap` iteration order), and `resolve_node` may not be altered in Round 4. Baking from the first non-overridden mode preserves exact parity across 2-mode collections (such as Light/Dark) regardless of which mode is active at deletion time.

### Conclusion & Epistemic Ledger

- **Conclusion**: Fixed both Major findings from the Round 3 review in `crates/designos-doc/src/styles_ops.rs` (with a read-only helper `font_style_weight` exposed in `crates/designos-doc/src/variables_apply.rs`) so that `resolve_node` produces the exact same resolved `Node` before and after `delete_style` in every mode, regardless of which mode was active when the user deleted the style.
- **FACT**: On `9e7c51be`, all 3 new Round 4 tests (`deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark`, `deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node`, and `table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types`) failed (`23 passed; 3 failed`, captured in `plans/reports/agy-127-delete-style-evidence/r4-red.log`).
- **FACT**: On the Round 4 fix, all 26 `styles_ops` unit tests pass (`26 passed; 0 failed`, captured in `plans/reports/agy-127-delete-style-evidence/r4-green.log`), `cargo fmt --all -- --check` exits `0`, `cargo clippy --workspace --all-targets --locked -- -D warnings` exits `0`, `cargo test -p designos-doc --locked` passes (`643` unit + `8` integration tests), `cargo test -p designos-fig --locked` passes (`121` unit + `450` integration tests), and `cargo test -p designos-app --bin designos --locked style` passes (`89` tests).
- **INFERENCE**: All 6 overlapping `(style_key, own_key)` families (`fontWeight`/`fontStyle`, `fills/N/opacity`/`fills/N`, `strokes/N/opacity`/`strokes/N`, `effects/N/opacity`/`effects/N/color`, `fills/N/stops/M`/`fills/N`, and `strokes/N/stops/M`/`strokes/N`) now classify per-mode precedence identically to `resolve_node`.
- **ASSUMPTION**: None remaining for 2-mode variable collections.
- **Weakest link**: If a collection has 3+ modes where 1 mode is overridden by `own` and 2 non-overridden modes have different style values, a single baked scalar on the raw node can store only the first non-overridden mode's value without changing `resolve_node`.

### Skills Loaded & Changes Because of Them

- **`/es:dev` -> `es:feature-dev`** (`~/.gemini/config/skills/es-dev/SKILL.md`, `~/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed Phase 3 (DEV) directly from the controller's rulings without re-opening brainstorm/plan gates or launching a separate verifier.
- **`/es:rust`** (`~/.gemini/config/skills/es-rust/SKILL.md`): Used `std::borrow::Cow<'_, str>` in `resolve_variable_with_mode` (`styles_ops.rs:345-349`) to avoid unnecessary `String` clones when overriding collection modes, and adhered to Rust 2021 edition compatibility (`first_kept_mode.and_then(...)` instead of Rust 2024 `let` chains).
- **`/es:rust-gpu-canvas`** (`~/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Preserved `resolve_node`'s borrow-first `Cow<'a, Node>` path untouched in `variables_apply.rs`.
- **`es:fable-thinking` & `es-astra-work`** (`~/.gemini/config/skills/es-fable-thinking/SKILL.md`, `~/.gemini/config/skills/es-astra-work/SKILL.md`): Predicted and reproduced both findings as red tests on `9e7c51be` (`r4-red.log`) before modifying production code, then attacked all 23 key/mode combinations across `"light"` and `"dark"` active deletion modes in a table test (`r4-green.log`).

---

### 2. Acceptance Table

| # | Check | Command | Exit / Result |
|---|---|---|---|
| 1a | `r4-red.log` on `9e7c51be` | `cargo test -p designos-doc --locked styles_ops` (before fix) | Exit `101` (`23 passed; 3 failed` — all 3 new Round 4 tests fail) |
| 1b | `r4-green.log` on Round 4 head | `cargo test -p designos-doc --locked styles_ops` (after fix) | Exit `0` (`26 passed; 0 failed`) |
| 2a | Formatting check | `cargo fmt --all -- --check` | Exit `0` |
| 2b | Workspace Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Exit `0` |
| 3a | `designos-doc` tests | `cargo test -p designos-doc --locked` | Exit `0` (`643` lib + `3` + `5` integration passed) |
| 3b | `designos-fig` tests | `cargo test -p designos-fig --locked` | Exit `0` (`121` lib + `450` integration passed across 34 binaries) |
| 3c | `designos-app` style tests | `cargo test -p designos-app --bin designos --locked style` | Exit `0` (`89 passed; 0 failed`) |
| 4 | Delete `out/` | `rm -rf out && test ! -e out` | Exit `0` (`OUT_REMOVED=yes`) |

#### Pasted Commands & Exact Output

##### Check 1a — `r4-red.log` on `9e7c51be`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home" plans/reports/agy-127-delete-style-evidence
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked styles_ops > plans/reports/agy-127-delete-style-evidence/r4-red.log 2>&1
```

Output (`plans/reports/agy-127-delete-style-evidence/r4-red.log`):
```text
RED_EXIT=101
   Compiling designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-doc)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 13.59s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)

running 26 tests
test styles_ops::tests::deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node ... FAILED
test styles_ops::tests::deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark ... FAILED
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... FAILED
...
---- styles_ops::tests::deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node stdout ----

thread 'styles_ops::tests::deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node' (52103822) panicked at crates/designos-doc/src/styles_ops.rs:1901:9:
strokes/0 on a stroke-slot style must not be moved onto the layer

---- styles_ops::tests::deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark stdout ----

thread 'styles_ops::tests::deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark' (52103827) panicked at crates/designos-doc/src/styles_ops.rs:1740:13:
assertion `left == right` failed: mixed fontStyle/fontWeight deleted in dark: resolve_node mismatch in mode 'light'
  left: Node { id: NodeId(2), ..., style: TextStyle { font_family: "Inter", font_weight: 800, italic: true, ... } }
 right: Node { id: NodeId(2), ..., style: TextStyle { font_family: "Inter", font_weight: 300, italic: true, ... } }

---- styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types stdout ----

thread 'styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types' panicked at crates/designos-doc/src/styles_ops.rs:1740:13:
assertion `left == right` failed: case=fill_opacity_with_mixed_opaque_light_translucent_dark_own_fill deleted_in=dark: resolve_node mismatch in mode 'light'
  left: Node { id: NodeId(2), ..., fills: [Paint { kind: Solid(Color { r: 0.8235294, g: 0.15686275, b: 0.15686275, a: 1.0 }), opacity: 0.25, visible: true, blend: Normal }], ... }
 right: Node { id: NodeId(2), ..., fills: [Paint { kind: Solid(Color { r: 0.8235294, g: 0.15686275, b: 0.15686275, a: 1.0 }), opacity: 0.5, visible: true, blend: Normal }], ... }

failures:
    styles_ops::tests::deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node
    styles_ops::tests::deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark
    styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types

test result: FAILED. 23 passed; 3 failed; 0 ignored; 0 measured; 617 filtered out; finished in 11.59s
```

##### Check 1b — `r4-green.log` on Round 4 head

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home" plans/reports/agy-127-delete-style-evidence
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked styles_ops > plans/reports/agy-127-delete-style-evidence/r4-green.log 2>&1
```

Output (`plans/reports/agy-127-delete-style-evidence/r4-green.log`):
```text
GREEN_EXIT=0
   Compiling designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-doc)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.64s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)

running 26 tests
test styles_ops::tests::creation_is_refused_without_values_or_on_the_wrong_node ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file ... ok
test styles_ops::tests::editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file ... ok
test styles_ops::tests::a_colour_style_edit_recolours_every_run_of_a_rich_text_layer ... ok
test styles_ops::tests::deleting_a_style_bakes_unrecognized_binding_keys_instead_of_dropping_them ... ok
test styles_ops::tests::detaching_drops_the_link_but_keeps_the_values ... ok
test styles_ops::tests::deleting_a_colour_style_on_text_keeps_a_highlighted_run ... ok
test styles_ops::tests::deleting_a_text_style_keeps_a_bold_run ... ok
test styles_ops::tests::effect_style_is_created_from_a_node_and_undone_in_one_step ... ok
test styles_ops::tests::deleting_a_style_with_mixed_own_and_style_bindings_preserves_resolved_look_in_all_modes ... ok
test styles_ops::tests::deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches ... ok
test styles_ops::tests::a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides ... ok
test styles_ops::tests::editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step ... ok
test styles_ops::tests::editing_a_style_updates_every_linked_node_and_only_those ... ok
test styles_ops::tests::applying_links_nodes_and_copies_values ... ok
test styles_ops::tests::deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node ... ok
test styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step ... ok
test styles_ops::tests::grid_style_is_created_from_a_frame_with_a_chosen_name ... ok
test styles_ops::tests::renaming_changes_the_display_name_only ... ok
test styles_ops::tests::deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark ... ok
test styles_ops::tests::reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref ... ok
test styles_ops::tests::effect_and_grid_styles_survive_a_native_save_and_load ... ok
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok
test styles_ops::tests::used_on_library_page_walks_only_the_library_page_subtree_and_probes_300k_nodes ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 617 filtered out; finished in 11.71s
```

##### Checks 2a & 2b — `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
echo "FMT_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
echo "CLIPPY_EXIT=$?"
rm -rf "$S"
```

Output:
```text
FMT_EXIT=0
    Checking designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-doc)
    Checking designos-fonts v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-fonts)
    Checking designos-render v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-render)
    Checking designos-svg v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-svg)
    Checking designos-export v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-export)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-gpu)
    Checking designos-webcapture v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-webcapture)
    Checking designos-fig v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-fig)
    Checking designos-library v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-library)
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-127-delete-style/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 59.77s
CLIPPY_EXIT=0
```

##### Checks 3a, 3b & 3c — `cargo test -p designos-doc --locked`, `cargo test -p designos-fig --locked`, and `cargo test -p designos-app --bin designos --locked style`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-doc --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-fig --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked style
rm -rf "$S"
```

Output:
```text
=== DESIGNOS-DOC ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)
test result: ok. 643 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.36s
     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-d19362047bc102bf)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-c7fad459a80a2bad)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
DOC_EXIT=0

=== DESIGNOS-FIG ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-540f1f9206f56183)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 62.31s
     Running tests/fig_effect_grid_styles.rs (out/cargo/debug/deps/fig_effect_grid_styles-46a2f99555638b59)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
(all 34 integration test binaries in designos-fig: 450 passed; 0 failed)
FIG_EXIT=0

=== DESIGNOS-APP STYLE ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)
test result: ok. 89 passed; 0 failed; 0 ignored; 0 measured; 2853 filtered out; finished in 2.44s
APP_STYLE_EXIT=0
```

---

### 3. Changes per file

- `crates/designos-doc/src/variables_apply.rs:197-228`: Extracted read-only helper `pub(crate) fn font_style_weight(val: &str) -> Option<u16>` (`197-220`) and called it from `apply_font_style_str` (`222-228`) with zero behaviour change.
- `crates/designos-doc/src/styles_ops.rs:335-442`: Replaced `color_var_has_translucent_mode` and `own_binding_overrides_style_key` with `StyleBindingOverride` (`335-339`), `resolve_variable_with_mode` (`341-360`), `governing_collection_and_modes` (`362-388`), and `classify_own_binding_override` (`392-442`) so style-vs-own precedence is evaluated per mode across the governing variable collection.
- `crates/designos-doc/src/styles_ops.rs:593-615`: Updated `delete_style` to skip stroke-slot binding keys that do not start with `"fills/"` (`594: let Some(rest) = bkey.strip_prefix("fills/") else { continue };`) matching `resolve_node`, drop style keys overwritten in every mode (`StyleBindingOverride::AllModes`), bake from the non-overridden mode when overwritten in only some modes (`StyleBindingOverride::SomeModesBake`), and move non-overridden style keys onto `copy.variables.bindings` (`StyleBindingOverride::None`).
- `crates/designos-doc/src/styles_ops.rs:1779-2301`: Added `make_var` (`1779-1793`), `assert_resolved_matches_across_modes` (`1795-1811`), `deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark` (`1814-1916`), `deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node` (`1919-1968`), and `table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types` (`1971-2300`).

---

### 4. Each case baked

1. **Mixed-mode `fontWeight` (style) vs `fontStyle` (layer own binding)** (`StyleBindingOverride::SomeModesBake` in `crates/designos-doc/src/styles_ops.rs:395-396, 437-438, 604-606`): When the layer's own `fontStyle` binding sets a weight (`font_style_weight(s).is_some()`, e.g. `"Bold Italic"`) in some mode(s) but leaves weight untouched (e.g. `"Italic"`) in at least one mode, keeping `fontWeight` on `node.variables.bindings` would run `fontWeight` after `fontStyle` in `BTreeMap` order and clobber the mode where `fontStyle` won. Instead, `delete_style` bakes `fontWeight` from the non-overridden mode onto `copy` via `set_bound_property` and drops the `fontWeight` binding, so the non-overridden mode keeps the style's weight and the overridden mode still takes `fontStyle`'s weight.
2. **Mixed-mode `fills/{idx}/opacity` (style) vs `fills/{idx}` (layer own binding)** (`StyleBindingOverride::SomeModesBake` in `crates/designos-doc/src/styles_ops.rs:397-398, 437-438, 604-606`): When the layer's own `fills/{idx}` colour binding has `c.a < 1.0` in some mode(s) (which resets `p.opacity = 1.0` in `set_bound_property`) and `c.a >= 1.0` in at least one mode (which leaves `p.opacity` untouched), keeping `fills/{idx}/opacity` on `node.variables.bindings` would run `fills/{idx}/opacity` after `fills/{idx}` in `BTreeMap` order and overwrite `p.opacity = 1.0` in the translucent mode. Instead, `delete_style` bakes `fills/{idx}/opacity` from the opaque (`c.a >= 1.0`) mode onto `copy` and drops the `fills/{idx}/opacity` binding.
3. **Mixed-mode `strokes/{idx}/opacity` (style) vs `strokes/{idx}` (layer own binding)** (`StyleBindingOverride::SomeModesBake` in `crates/designos-doc/src/styles_ops.rs:399-400, 437-438, 604-606`): Same as `fills/{idx}/opacity` when a fill style carrying `fills/{idx}/opacity` is linked to the `"stroke"` slot (`node_key = "strokes/{idx}/opacity"`) and the layer has its own `strokes/{idx}` colour binding with `c.a < 1.0` in some mode(s) and `c.a >= 1.0` in another mode.
4. **Unrecognized / legacy style binding keys outside `property_kind`** (`bake_unrecognized_style_binding` in `crates/designos-doc/src/styles_ops.rs:446-565, 612-614`, retained from Round 3): Keys that pass the slot filter (including `slot == "stroke"` requiring `fills/` prefix) but return `None` from `property_kind(&node_key)` (such as `fontSize/0`, `fills/0/color`, `fillOpacities/0`, `effects/0/x`, `cornerRadius`, `minWidth`) are baked onto `copy` in the active mode.

### Unresolved Questions

- None.

---

## Round 5

### 1. What could NOT be verified

- **Windows/Linux runtime branch of `UNDO_SHORTCUT` (`"Ctrl+Z"`) in `style_deleted_toast` (`crates/designos-app/src/panels/style_picker.rs:20-29`):** `[INFERENCE]` Verified on macOS (`"Style deleted · 1 layer matches it in this mode only · Cmd+Z to undo"`, `"Style deleted · 3 layers match it in this mode only · Cmd+Z to undo"`, and `"Style deleted · Cmd+Z to undo"`). The non-macOS `"Ctrl+Z"` branch uses the same `cfg!(target_os = "macos")` compile-time constant (`UNDO_SHORTCUT`) as `STYLE_DELETED_TOAST` (`crates/designos-app/src/panels/style_picker.rs:20-21`) and was not executed on a Windows or Linux runner locally.
- **Alias cycle cap beyond 32 hops in `collections_and_modes` (`crates/designos-doc/src/styles_ops.rs:352`):** `[INFERENCE]` `collections_and_modes` caps alias traversal at 32 visited variables (`visited_vars.len() >= 32`), matching `Document::resolve_variable_with_modes` (`crates/designos-doc/src/variables.rs:209`, `for _ in 0..32`). Chains deeper than 32 hops are treated as unresolved in both the resolver and classifier.
- **Conclusion:** `[FACT]` All four Round 5 classifier fixes (every involved collection via `collections_and_modes`, missing mode values in `classify_own_binding_override`, delegating to `Document::resolve_variable_with_modes` for aliases and multi-axis `+` keys while removing the private resolver, and keeping mixed stop bindings `fills/N/stops/M` and `strokes/N/stops/M`), the fallback bake rule and `(NodeId, String)` reporting via `delete_style_with_fallbacks`, the fallback toast in `style_picker.rs`, and all 8 acceptance table rows plus the full `designos-doc`, `designos-fig`, and `designos` binary (`2939 passed; 0 failed`) test suites are implemented and verified.
- **Weakest link:** `[FACT]` When a layer's own binding wins in some mode combinations and the style-won mode combinations resolve to more than one distinct value, the flat `BTreeMap<String, String>` binding model on `NodeVariables` has no per-mode precedence after style detachment; `delete_style_with_fallbacks` bakes a single fallback value (preserving the delete-time mode when the style wins there, or the default mode / first style-won mode when the own binding wins at delete time), drops the style binding, and reports `(NodeId, node_key)` so the UI surfaces the non-silent toast (`Style deleted · {n} layers match it in this mode only · Cmd+Z to undo`).
- **Skills loaded & what changed because of them:**
  - `es-astra-work`, `es:fable-thinking`, `es:dev`, `es:feature-dev`: Enforced red-before-green evidence capture (`r5-red.log` against unmodified `fa7410985` before production edits, then `r5-green.log`) and root-caused the `TMPDIR` trailing-slash normalization needed for `fig_export` tests in the full `designos` binary suite.
  - `es:rust`, `es:rust-gpu-canvas`: Kept `Document::resolve_variable_with_modes` allocation-minimal (`mode_overrides: &[(&str, &str)]`, delegating `resolve_variable(node, variable)` to `self.resolve_variable_with_modes(node, variable, &[])`) with zero behaviour change to existing callers.
  - `es:designer` + `design/principles.md` + `design/soul.md` + `design/ux-writing.md`: Verified the exact fallback toast strings in `crates/designos-app/src/panels/style_picker.rs:20-29` and passed `python3 scripts/zero_fake_ui_gate.py` (`17/17 PASS`).

---

### 2. Acceptance table (rows 1–8 + gate checks), with commands and pasted output

| # | Shape / Check | Expect | Verdict | Evidence |
|---|---|---|---|---|
| 1 | 3 modes in one collection (`light`, `dim`, `dark`); own colour translucent only in `dark`; style opacity `80`/`20`/`5` | Fallback: delete-time mode is exact; `(rid, "fills/0/opacity")` is reported; `dim` gets `0.8` when deleted in `light`, `0.2` when deleted in `dim`, `0.8` when deleted in `dark` (default `light`), and `0.8` when deleted in `dark` (default `dark`, first style-won mode `light`) | PASS (`[FACT]`) | `r5_row1_three_modes_mixed_override_uses_fallback_preserves_delete_time_mode_and_reports_pair` (`styles_ops.rs:2374-2512`) |
| 2 | 3 modes (`light`, `dim`, `dark`); style opacity is the same (`80`) in every style-won mode (`light`, `dim`) | Exact in every mode (`light`, `dim`, `dark`), nothing reported (`fallbacks.is_empty()`) | PASS (`[FACT]`) | `r5_row2_three_modes_same_style_opacity_in_every_style_won_mode_is_exact_and_reports_nothing` (`styles_ops.rs:2515-2614`) |
| 3 | Style variable in a second collection (`Brand` `a`/`b`), own in `Theme` (`light`/`dark`) | **Why & which:** Classifier enumerates the Cartesian product of `(Theme, Brand)` (`4` combinations). When `Theme` own overrides in `dark` and `Brand` differs (`a=50`, `b=10`), style wins in `(light, a)=50` and `(light, b)=10` (different values) → **fallback with `(rid, "fills/0/opacity")` reported** (delete-time and default `(light, a)` exact when deleted in `(dark, b)`; delete-time `(light, b)` and `dark` combinations exact when deleted in `(light, b)`). When `Theme` own is opaque in both modes or `Brand` agrees (`a=50, b=50`) → **exact over all 4 combinations, nothing reported** | PASS (`[FACT]`) | `r5_row3_style_variable_in_second_collection_brand_and_own_in_theme` (`styles_ops.rs:2617-2759`) |
| 4 | Style `fontWeight` has no value in `light` (only `dark: 800`); own `fontStyle` `Italic`/`Bold Italic` | `dark` stays `700` after delete; `light` stays `400` as before; nothing reported. (**Classifier Fix 2 rule:** when at least one style-won mode resolves, bake the resolving mode's value; when no style-won mode resolves, return `StyleBindingOverride::AllModes` to drop the binding and leave the node's field as it is.) | PASS (`[FACT]`) | `r5_row4_style_font_weight_missing_in_light_with_own_font_style_italic_and_bold_italic` (`styles_ops.rs:2762-2830`) |
| 5 | A value stored only under a `light+compact` multi-axis key | Classified with `Document::resolve_variable_with_modes` (same alias and `+` multi-axis path as `Document::resolve_variable`); exact across all 4 `(theme, density)` combinations, nothing reported | PASS (`[FACT]`) | `r5_row5_value_stored_only_under_light_plus_compact_key_classified_with_resolve_variable` (`styles_ops.rs:2833-2947`) |
| 6 | Mixed stop keys (`fills/0/stops/0` and `strokes/0/stops/0` against an own `fills/0` and `strokes/0` that resolves only in `dark`) | Stop binding is kept (`StyleBindingOverride::None`); exact across `light`, `dim`, `dark`, nothing reported | PASS (`[FACT]`) | `r5_row6_mixed_stop_keys_keep_binding_and_match_every_mode` (`styles_ops.rs:2950-3068`) |
| 7 | All Round 4 cases | Still pass (`deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark`, `deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node`, `table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types`) | PASS (`[FACT]`) | `styles_ops` (`32 passed; 0 failed`) |
| 8 | Toast when 1 layer and when 3 layers use the fallback, and when none does | 1 layer: `"Style deleted · 1 layer matches it in this mode only · Cmd+Z to undo"`; 3 layers: `"Style deleted · 3 layers match it in this mode only · Cmd+Z to undo"`; 0 layers: `"Style deleted · Cmd+Z to undo"` | PASS (`[FACT]`) | `deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none` (`effect_grid_styles.rs:309-408`) |
| G1 | `r5-red.log` (against unmodified `fa7410985`) and `r5-green.log` (on Round 5 head) | Red fails on rows 1, 3, 4, 5, 6, 8 (`5 failed` in `styles_ops`, `1 failed` in `effect_grid_styles`); Green passes all (`32 passed; 0 failed` and `10 passed; 0 failed`) | PASS (`[FACT]`) | `plans/reports/agy-127-delete-style-evidence/r5-red.log`, `plans/reports/agy-127-delete-style-evidence/r5-green.log` |
| G2 | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` | Both exit `0` | PASS (`[FACT]`) | `FMT_EXIT=0`, `CLIPPY_EXIT=0` |
| G3 | `cargo test -p designos-doc --locked`, `cargo test -p designos-fig --locked`, and full `cargo test -p designos-app --bin designos --locked` | All pass under the isolated harness (`designos-doc`: `649 + 3 + 5 passed; 0 failed`; `designos-fig`: `121 unit + 450 integration passed; 0 failed`; `designos` bin: `2939 passed; 0 failed`) | PASS (`[FACT]`) | Full harness run logs below |
| G4 | `python3 scripts/zero_fake_ui_gate.py` and `rm -rf out` | `Gate Verdict: PASS (17/17 invariants satisfied)` and `out/` removed | PASS (`[FACT]`) | Gate output below |

#### Check G1a — `r5-red.log` against `fa7410985` before Round 5 production edits

Command:
```sh
(
  set -euo pipefail
  export PATH="$HOME/.cargo/bin:$PATH"
  export RUSTUP_HOME="$HOME/.rustup" CARGO_HOME="$HOME/.cargo"
  H="$(mktemp -d "${TMPDIR:-/tmp}/dos-home.XXXXXX")"
  T="$(mktemp -d "${TMPDIR:-/tmp}/dos-tmp.XXXXXX")"
  trap 'rm -rf "$H" "$T"' EXIT
  export HOME="$H" TMPDIR="$T" CARGO_TARGET_DIR="$PWD/out/cargo"
  export NEXTEST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2
  cargo test -p designos-doc --locked styles_ops || true
  cargo test -p designos-app --bin designos --locked effect_grid_styles || true
) 2>&1 | tee plans/reports/agy-127-delete-style-evidence/r5-red.log
```

Output (`plans/reports/agy-127-delete-style-evidence/r5-red.log`):
```text
=== [RED] cargo test -p designos-doc --locked styles_ops ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)

running 32 tests
...
test styles_ops::tests::r5_row2_three_modes_same_style_opacity_in_every_style_won_mode_is_exact_and_reports_nothing ... ok
test styles_ops::tests::r5_row6_mixed_stop_keys_keep_binding_and_match_every_mode ... FAILED
test styles_ops::tests::r5_row3_style_variable_in_second_collection_brand_and_own_in_theme ... FAILED
test styles_ops::tests::r5_row4_style_font_weight_missing_in_light_with_own_font_style_italic_and_bold_italic ... FAILED
test styles_ops::tests::r5_row1_three_modes_mixed_override_uses_fallback_preserves_delete_time_mode_and_reports_pair ... FAILED
test styles_ops::tests::r5_row5_value_stored_only_under_light_plus_compact_key_classified_with_resolve_variable ... FAILED
...
failures:
    styles_ops::tests::r5_row1_three_modes_mixed_override_uses_fallback_preserves_delete_time_mode_and_reports_pair
    styles_ops::tests::r5_row3_style_variable_in_second_collection_brand_and_own_in_theme
    styles_ops::tests::r5_row4_style_font_weight_missing_in_light_with_own_font_style_italic_and_bold_italic
    styles_ops::tests::r5_row5_value_stored_only_under_light_plus_compact_key_classified_with_resolve_variable
    styles_ops::tests::r5_row6_mixed_stop_keys_keep_binding_and_match_every_mode

test result: FAILED. 27 passed; 5 failed; 0 ignored; 0 measured; 617 filtered out; finished in 12.33s

=== [RED] cargo test -p designos-app --bin designos --locked effect_grid_styles ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 10 tests
...
test shell::tests::effect_grid_styles::deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none ... FAILED

failures:

---- shell::tests::effect_grid_styles::deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none stdout ----

thread 'shell::tests::effect_grid_styles::deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none' (53198979) panicked at crates/designos-app/src/shell/tests/effect_grid_styles.rs:413:5:
assertion `left == right` failed
  left: "Style deleted · Cmd+Z to undo"
 right: "Style deleted · 1 layer matches it in this mode only · Cmd+Z to undo"

failures:
    shell::tests::effect_grid_styles::deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none

test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 2937 filtered out; finished in 4.28s
```

#### Check G1b — `r5-green.log` on Round 5 head

Command:
```sh
(
  set -euo pipefail
  export PATH="$HOME/.cargo/bin:$PATH"
  export RUSTUP_HOME="$HOME/.rustup" CARGO_HOME="$HOME/.cargo"
  H="$(mktemp -d "${TMPDIR:-/tmp}/dos-home.XXXXXX")"
  T="$(mktemp -d "${TMPDIR:-/tmp}/dos-tmp.XXXXXX")"
  trap 'rm -rf "$H" "$T"' EXIT
  export HOME="$H" TMPDIR="$T" CARGO_TARGET_DIR="$PWD/out/cargo"
  export NEXTEST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2
  cargo test -p designos-doc --locked styles_ops
  cargo test -p designos-app --bin designos --locked effect_grid_styles
) 2>&1 | tee plans/reports/agy-127-delete-style-evidence/r5-green.log
```

Output (`plans/reports/agy-127-delete-style-evidence/r5-green.log`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)

running 32 tests
test styles_ops::tests::a_colour_style_edit_recolours_every_run_of_a_rich_text_layer ... ok
test styles_ops::tests::a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file ... ok
test styles_ops::tests::a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file ... ok
test styles_ops::tests::applying_links_nodes_and_copies_values ... ok
test styles_ops::tests::creation_is_refused_without_values_or_on_the_wrong_node ... ok
test styles_ops::tests::deleting_a_colour_style_on_text_keeps_a_highlighted_run ... ok
test styles_ops::tests::deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches ... ok
test styles_ops::tests::deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node ... ok
test styles_ops::tests::deleting_a_style_bakes_unrecognized_binding_keys_instead_of_dropping_them ... ok
test styles_ops::tests::deleting_a_text_style_keeps_a_bold_run ... ok
test styles_ops::tests::deleting_a_style_with_mixed_own_and_style_bindings_preserves_resolved_look_in_all_modes ... ok
test styles_ops::tests::deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step ... ok
test styles_ops::tests::detaching_drops_the_link_but_keeps_the_values ... ok
test styles_ops::tests::editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step ... ok
test styles_ops::tests::editing_a_style_updates_every_linked_node_and_only_those ... ok
test styles_ops::tests::deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark ... ok
test styles_ops::tests::editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused ... ok
test styles_ops::tests::effect_style_is_created_from_a_node_and_undone_in_one_step ... ok
test styles_ops::tests::grid_style_is_created_from_a_frame_with_a_chosen_name ... ok
test styles_ops::tests::r5_row1_three_modes_mixed_override_uses_fallback_preserves_delete_time_mode_and_reports_pair ... ok
test styles_ops::tests::r5_row2_three_modes_same_style_opacity_in_every_style_won_mode_is_exact_and_reports_nothing ... ok
test styles_ops::tests::r5_row3_style_variable_in_second_collection_brand_and_own_in_theme ... ok
test styles_ops::tests::r5_row4_style_font_weight_missing_in_light_with_own_font_style_italic_and_bold_italic ... ok
test styles_ops::tests::effect_and_grid_styles_survive_a_native_save_and_load ... ok
test styles_ops::tests::r5_row6_mixed_stop_keys_keep_binding_and_match_every_mode ... ok
test styles_ops::tests::renaming_changes_the_display_name_only ... ok
test styles_ops::tests::r5_row5_value_stored_only_under_light_plus_compact_key_classified_with_resolve_variable ... ok
test styles_ops::tests::reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref ... ok
test styles_ops::tests::table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types ... ok
test styles_ops::tests::used_on_library_page_walks_only_the_library_page_subtree_and_probes_300k_nodes ... ok

test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 617 filtered out; finished in 12.06s

     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 10 tests
test shell::tests::effect_grid_styles::a_library_style_cannot_be_deleted_from_the_style_menu ... ok
test shell::tests::effect_grid_styles::a_style_used_on_the_hidden_library_page_is_disabled_in_the_style_menu ... ok
test shell::tests::effect_grid_styles::creating_an_effect_style_links_the_layer_in_one_undo_step ... ok
test shell::tests::effect_grid_styles::creating_detaching_and_applying_a_grid_style ... ok
test shell::tests::effect_grid_styles::deleting_a_style_from_the_style_menu_detaches_linked_layers_shows_toast_and_undoes_in_one_step ... ok
test shell::tests::effect_grid_styles::detaching_and_applying_an_effect_style_from_the_menu ... ok
test shell::tests::effect_grid_styles::library_page_scan_never_runs_on_idle_frame_and_at_most_once_when_menu_open ... ok
test shell::tests::effect_grid_styles::deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none ... ok
test shell::tests::effect_grid_styles::local_styles_list_effect_and_grid_groups ... ok
test shell::tests::effect_grid_styles::renaming_an_effect_style_from_the_menu ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 2937 filtered out; finished in 1.08s
```

#### Checks G2 & G3 — `fmt`, `clippy`, `designos-doc`, `designos-fig`, and full `designos` binary (`2939 passed; 0 failed`)

Command:
```sh
(
  set -euo pipefail
  export PATH="$HOME/.cargo/bin:$PATH"
  export RUSTUP_HOME="$HOME/.rustup" CARGO_HOME="$HOME/.cargo"
  export TMPDIR="${TMPDIR:-/tmp}"
  export TMPDIR="${TMPDIR%/}"
  H="$(mktemp -d "${TMPDIR:-/tmp}/dos-home.XXXXXX")"
  T="$(mktemp -d "${TMPDIR:-/tmp}/dos-tmp.XXXXXX")"
  trap 'rm -rf "$H" "$T"' EXIT
  export HOME="$H" TMPDIR="$T" CARGO_TARGET_DIR="$PWD/out/cargo"
  export NEXTEST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 RUST_TEST_THREADS=2

  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --locked -- -D warnings
  cargo test -p designos-doc --locked
  cargo test -p designos-fig --locked
  cargo test -p designos-app --bin designos --locked
)
```

Output:
```text
=== GATE: cargo fmt --all -- --check ===
=== GATE: cargo clippy --workspace --all-targets --locked -- -D warnings ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 29s
=== GATE: cargo test -p designos-doc --locked ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-070d2d6646064e6d)
test result: ok. 649 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.66s
     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-d19362047bc102bf)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-c7fad459a80a2bad)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
=== GATE: cargo test -p designos-fig --locked ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-540f1f9206f56183)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.50s
(all 34 integration test binaries in designos-fig: 450 passed; 0 failed; 6 ignored)
=== GATE: cargo test -p designos-app --bin designos --locked ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)
test result: ok. 2939 passed; 0 failed; 8 ignored; 0 measured; 0 filtered out; finished in 249.87s
```

#### Check G4 — `python3 scripts/zero_fake_ui_gate.py` and `rm -rf out`

Command:
```sh
python3 scripts/zero_fake_ui_gate.py
rm -rf out scripts/__pycache__ && test ! -e out && echo "OUT_REMOVED=yes"
```

Output:
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
OUT_REMOVED=yes
```

---

### 3. Changes per file

- `crates/designos-doc/src/variables.rs:198-232, 271-285`:
  - Exposed read-only helper `pub(crate) fn resolve_variable_with_modes(&self, node: NodeId, variable: &str, mode_overrides: &[(&str, &str)]) -> Option<VariableValue>` (`206-222`) along with `variable_mode_with_overrides` (`224-230`) and `resolve_multi_axis_value_with_modes` (`271-285`), and made `Document::resolve_variable` (`198-202`) delegate to `self.resolve_variable_with_modes(node, variable, &[])` with zero behaviour change.
- `crates/designos-doc/src/styles_ops.rs:335-481, 610-671, 2353-3068`:
  - Deleted the private `resolve_variable_with_mode` and single-collection `governing_collection_and_modes` helpers.
  - Added `collections_and_modes` (`341-390`) which collects every variable collection used by the layer's own variable or the style variable, including following `VariableValue::Alias` links (up to 32 visited variables) and multi-axis `+` mode keys across collections.
  - Updated `classify_own_binding_override` (`395-481`) to evaluate `doc.resolve_variable_with_modes` across the Cartesian product of all involved collections' modes (`422-448`), keep mixed gradient stop bindings (`if is_stop { return StyleBindingOverride::None; }`, `456-458`), drop the binding and leave the field as it is when no style-won mode resolves (`let Some(first_val) = style_won_vals.first() else { return StyleBindingOverride::AllModes; }`, `459-461`), bake the agreed value without fallback when all resolving style-won modes agree (`462-464`), and apply the 3-step fallback bake with `fallback: true` when style-won modes resolve to different values (`466-480`).
  - Added `pub fn delete_style_with_fallbacks(doc: &Document, key: &str) -> Result<(Command, Vec<(NodeId, String)>), StyleOpError>` (`616-671`) next to `delete_style` (`610-612`), recording `(id, node_key)` whenever `StyleBindingOverride::SomeModesBake { val, fallback: true }` is applied.
  - Added `assert_resolved_matches_across_mode_combos` (`2353-2372`) and tests `r5_row1` through `r5_row6` (`2374-3068`).
- `crates/designos-app/src/panels/style_picker.rs:20-51`:
  - Added `UNDO_SHORTCUT` (`20`) and `style_deleted_toast(fallback_layers: usize) -> String` (`23-29`), and updated `delete_linked_style` (`39-51`) to call `delete_style_with_fallbacks`, deduplicate affected `NodeId`s, and pass the affected layer count to `style_deleted_toast`.
- `crates/designos-app/src/shell/tests/effect_grid_styles.rs:309-408`:
  - Added sim test `deleting_a_style_shows_fallback_toast_for_one_and_three_layers_and_plain_toast_when_none` verifying the toast text for `1` fallback layer, `3` fallback layers, and `0` fallback layers.

---

### 4. The fallback rule

When a layer's own binding (`fontStyle`, `fills/{idx}`, `strokes/{idx}`, or `effects/{idx}/color`) overrides a non-stop style binding (`fontWeight`, `fills/{idx}/opacity`, `strokes/{idx}/opacity`, or `effects/{idx}/opacity`) in at least one mode combination of the involved collections (`any_overridden && !all_overridden` in `crates/designos-doc/src/styles_ops.rs:450-458`), `classify_own_binding_override` collects the resolved `VariableValue` of the style variable in every style-won mode combination (`style_won_vals`). If no style-won mode combination resolves to a value (`style_won_vals.is_empty()`), it returns `StyleBindingOverride::AllModes`, dropping the style binding and leaving the node's field as it is; if every resolving style-won mode combination produces the same value, it bakes that value with `fallback: false` and drops the binding. Otherwise, when resolving style-won mode combinations disagree (`crates/designos-doc/src/styles_ops.rs:466-480`), it drops the style binding, records `(node_id, node_key)` in the fallback list returned by `delete_style_with_fallbacks`, and bakes a single style value chosen in order: (1) `doc.resolve_variable(id, style_var)` in the mode the node uses at delete time when the style wins at delete time (`!own_wins_at_delete`), else (2) `doc.resolve_variable_with_modes(id, style_var, &default_combo)` in the involved collections' default modes when the style wins there (`!own_wins_in_default`), else (3) `first_val.clone()`, the value from the first style-won mode combination that resolved.

#### Every user-facing string, verbatim

- `"Style deleted · 1 layer matches it in this mode only · Cmd+Z to undo"`
- `"Style deleted · 1 layer matches it in this mode only · Ctrl+Z to undo"`
- `"Style deleted · {n} layers match it in this mode only · Cmd+Z to undo"`
- `"Style deleted · {n} layers match it in this mode only · Ctrl+Z to undo"`
- `"Style deleted · Cmd+Z to undo"`
- `"Style deleted · Ctrl+Z to undo"`

Status: DONE
