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
