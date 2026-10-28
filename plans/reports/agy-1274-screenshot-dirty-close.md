# Report — DEV · #1274: a `--screenshot` run never stops on "Save changes?"

| Field | Value |
|---|---|
| Issue | `#1274` (P1, capture tooling) |
| Base | `origin/release/v0.7.0` (`1b33582b8`, `release: merge v0.6.5 into v0.7.0 (#1275)`) |
| Branch | `jangtrinh/agy-1274-screenshot-dirty-close` |
| Commits | `8fe3b22b` (`test(app): cover dirty document close in screenshot and normal runs (#1274)`) → `fix(app): close screenshot runs without asking leave questions or saving (#1274)` |

---

## 1. What could NOT be verified, first

- **Real-binary GUI window run**: **real-binary run left to the controller (owner active)**.
  - Per task instruction:
    > Before it, read `ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print int($NF/1000000000); exit}'`. If the Mac has been idle less than 120 s, skip the run, write "real-binary run left to the controller (owner active)" in the report, and rely on the two `Sim` tests.
  - Measured `HIDIdleTime` before running: `74` seconds (first check) and `27` seconds (second check) — both `< 120` s because the owner was actively using the Mac. Therefore, no window was opened on Jang's screen and the verification relies on the two deterministic `Sim` tests (`screenshot_run_closes_without_leave_questions_when_dirty` and `normal_run_still_asks_before_closing_a_dirty_document`) plus the full 3,462-test `designos` binary suite and 1,342-test workspace `--lib` suite.
- **Note on CLI path for dirtying a document during `--screenshot`**:
  - Passing `--apply-spec <spec> --screenshot out.png` hits `opts.is_agent_cli() == true` (`crates/designos-app/src/cli.rs:128`), so `main()` (`crates/designos-app/src/main.rs:171-177`) runs `agent_cli::run_agent_cli(&opts)` (writing the spec directly to the file on disk and exiting `0`) and returns before opening the `eframe` window or invoking `drive_screenshot`.
  - The CLI paths that open a `--screenshot` window and mutate the in-memory document (setting `dirty = true` via `app.changed()` before `drive_screenshot` requests `ViewportCommand::Close`) are:
    1. `designos <copy.designos> --select <frame> --layout-grid columns --screenshot out.png` (`crates/designos-app/src/shell.rs:289-301`, calls `apply_layout_grid_preset` → `app.changed()`)
    2. `designos <copy.designos> --dev css --dev-demo --screenshot out.png` (`crates/designos-app/src/shell.rs:279-281`, calls `seed_dev_demo` → `app.changed()`)
    3. `designos <copy.designos> --panel check-designs-fixed --screenshot out.png` (`crates/designos-app/src/shell.rs:412-419`, calls `fix_all_design_issues` → `app.changed()`)
    4. `designos <copy.designos> --import-capture <capture.json> --screenshot out.png` (`crates/designos-app/src/cli.rs:132`, `crates/designos-app/src/shell.rs:222-224`, calls `import_capture_into_app` → `app.changed()`)

---

## 2. Drift Check (`release/v0.6.5` spec vs `origin/release/v0.7.0` HEAD `1b33582b8`)

| Symbol / Location | Spec says (`release/v0.6.5`) | HEAD `1b33582b8` says (`origin/release/v0.7.0`) | Drift |
|---|---|---|---|
| `drive_screenshot` definition | `crates/designos-app/src/shell.rs:494` | `crates/designos-app/src/shell.rs:498` | `+4` lines |
| `ViewportCommand::Close` sends in `drive_screenshot` | `shell.rs:540`, `:553`, `:569` | `crates/designos-app/src/shell.rs:544`, `:557`, `:573` | `+4` lines |
| Close request handling (`close_requested()`) | `shell.rs:1495-1497` | `crates/designos-app/src/shell.rs:1508-1521` | `+13` lines |
| `may_quit` definition | `crates/designos-app/src/home/leave.rs:84` | `crates/designos-app/src/home/leave.rs:84` | `0` lines |

---

## 3. Root Cause & Fix (`file:line` evidence)

- **Root Cause**:
  - In [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1274-screenshot-dirty-close/crates/designos-app/src/shell.rs#L498-L575), `Shell::drive_screenshot` writes the captured PNG (or hits the 240-frame timeout) and issues `ctx.send_viewport_cmd(egui::ViewportCommand::Close)` at lines `544`, `557`, and `573`.
  - In [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1274-screenshot-dirty-close/crates/designos-app/src/shell.rs#L1508-L1521), `Shell::draw_window` handled `ctx.input(|i| i.viewport().close_requested())` unconditionally with:
    ```rust
    if !self.app.may_quit(&ctx) {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
    } else {
        crate::decks::ui::flush_notes(&mut self.app);
        self.app.save_close_version();
        self.app.flush_autosave();
        self.app.terminal.shutdown();
        for tab in &mut self.app.tabs.entries {
            tab.terminal.shutdown();
        }
    }
    ```
  - When a `--screenshot` run (`self.shot.is_some()`) had a dirty external document, an open inline/node edit, or a pending agent document, `self.app.may_quit(&ctx)` ([`crates/designos-app/src/home/leave.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1274-screenshot-dirty-close/crates/designos-app/src/home/leave.rs#L84-L106)) queued `"Save changes to …?"` / `"Apply the agent's new document before closing?"` in `self.leave`, returned `false`, and caused `Shell::draw_window` to send `ViewportCommand::CancelClose` — hanging the `--screenshot` process indefinitely on the modal dialog. Additionally, if the dirty document belonged to a library file (`active_file_id.is_some()`), `may_quit` returned `true` and the `else` branch called `flush_notes`, `save_close_version`, and `flush_autosave`, writing unwanted state to the library/autosave on screenshot exit.
- **Fix**:
  - At the single close decision in [`crates/designos-app/src/shell.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1274-screenshot-dirty-close/crates/designos-app/src/shell.rs#L1508-L1526), branch on `self.shot.is_some()` first:
    ```rust
    if ctx.input(|i| i.viewport().close_requested()) {
        if self.shot.is_some() {
            self.app.terminal.shutdown();
            for tab in &mut self.app.tabs.entries {
                tab.terminal.shutdown();
            }
        } else if !self.app.may_quit(&ctx) {
            // Edits no save has written are being asked about: the window stays open, and the close comes round again.
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else {
            crate::decks::ui::flush_notes(&mut self.app);
            self.app.save_close_version();
            self.app.flush_autosave();
            self.app.terminal.shutdown();
            for tab in &mut self.app.tabs.entries {
                tab.terminal.shutdown();
            }
        }
    }
    ```
  - This leaves `self.app.dirty` untouched, never calls `may_quit` or fakes a `"Don't Save"` answer, never runs `flush_notes` / `save_close_version` / `flush_autosave`, never cancels the close request, and preserves all exit codes in `main.rs` (`545-564`).

---

## 4. Skills Loaded & What Changed Because of Them

- `es:dev` & `es:feature-dev`: Enforced the red-first test commit (`8fe3b22b`) before the implementation commit (`fix(app)`), minimal single-site fix at the close decision, and multi-gate verification.
- `es:rust`: Guided zero-side-effect branching at `shell.rs:1509` without mutating `dirty` or triggering `Leave` answer side effects, and ensured `Sim` tests use RAII temp dirs (`Dir`) with deterministic byte/mtime/autosave assertions.
- `es:rust-gpu-canvas`: Confirmed `drive_screenshot` lifecycle (`Shot` state machine in `shell.rs:25-33, 498-575`) and verified no GPU/compile paths were altered.
- `es-astra-work`: Enforced strict adherence to the `HIDIdleTime < 120s` skip rule, isolated `CARGO_TARGET_DIR` cleanup (`out/cargo` removed), and evidence capture in `plans/reports/agy-1274-evidence/`.
- `es:fable-thinking`: Used to re-anchor line numbers against `origin/release/v0.7.0` (`1b33582b8`) before editing, verify the library-file autosave edge case (`active_file_id.is_some()`), and structure the epistemic ledger.

---

## 5. Acceptance Criteria & Verification Gates

| # | Acceptance Check | Verdict | Evidence |
|---|---|---|---|
| 1 | A process started with `--screenshot` closes without asking any leave question, whatever the document state (dirty, agent pending doc, open edits, or dirty library file), and never writes the document, library, or autosave. | **PASS** | `shell::tests::leave_prompt::screenshot_run_closes_without_leave_questions_when_dirty` (`crates/designos-app/src/shell/tests/leave_prompt.rs:616-683`) — see `plans/reports/agy-1274-evidence/red.log` (failed before fix) and `plans/reports/agy-1274-evidence/green.log` (passed after fix). |
| 2 | Exit code stays 0 when PNG was written, and existing non-zero codes/stderr lines for failure paths (`#1003`, `#1127`, `#1141`) are unchanged. | **PASS** | `crates/designos-app/src/main.rs:545-564` and `drive_screenshot` (`shell.rs:498-575`) untouched; all 3,462 `designos` binary tests pass. |
| 3 | Normal runs (no `--screenshot`) still ask before closing a dirty document and cancel the close request. | **PASS** | `shell::tests::leave_prompt::normal_run_still_asks_before_closing_a_dirty_document` (`crates/designos-app/src/shell/tests/leave_prompt.rs:686-699`) and all existing `shell::tests::leave_prompt::*` tests pass. |
| 4 | Fixed in one place, at the close decision (`shell.rs:1508-1526`), keyed on `self.shot.is_some()`, without clearing `dirty` or faking a `"Don't Save"` answer. | **PASS** | `git diff 8fe3b22b..HEAD -- crates/designos-app/src/shell.rs` touches only `shell.rs:1508-1526`; `screenshot_run_closes_without_leave_questions_when_dirty` asserts `sim.shell.app.dirty == true` after close. |

### Exact Gate Outputs

#### Red Run (`plans/reports/agy-1274-evidence/red.log`, on `8fe3b22b` before fix)
```text
=== [RED] normal_run_still_asks_before_closing_a_dirty_document ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.06s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-cfe862bd484a2ffd)

running 1 test
test shell::tests::leave_prompt::normal_run_still_asks_before_closing_a_dirty_document ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3473 filtered out; finished in 0.53s

=== [RED] screenshot_run_closes_without_leave_questions_when_dirty ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.80s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-cfe862bd484a2ffd)

running 1 test

thread 'shell::tests::leave_prompt::screenshot_run_closes_without_leave_questions_when_dirty' (4881161) panicked at crates/designos-app/src/shell/tests/leave_prompt.rs:655:5:
close request must not be cancelled
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test shell::tests::leave_prompt::screenshot_run_closes_without_leave_questions_when_dirty ... FAILED

failures:

failures:
    shell::tests::leave_prompt::screenshot_run_closes_without_leave_questions_when_dirty

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3473 filtered out; finished in 0.39s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

#### Green Run (`plans/reports/agy-1274-evidence/green.log`, after fix)
```text
=== [GREEN] normal_run_still_asks_before_closing_a_dirty_document ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 22.75s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-cfe862bd484a2ffd)

running 1 test
test shell::tests::leave_prompt::normal_run_still_asks_before_closing_a_dirty_document ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3473 filtered out; finished in 0.64s

=== [GREEN] screenshot_run_closes_without_leave_questions_when_dirty ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.68s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-cfe862bd484a2ffd)

running 1 test
test shell::tests::leave_prompt::screenshot_run_closes_without_leave_questions_when_dirty ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3473 filtered out; finished in 0.94s
```

#### `cargo fmt --all -- --check`
```text
fmt_exit=0
```

#### `cargo clippy --workspace --all-targets --locked -- -D warnings`
```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.51s
clippy_exit=0
```

#### `cargo test --workspace --locked --lib`
```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 29.29s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-51415aaf0783fca2)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-21021c15afbeea5c)
test result: ok. 657 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.54s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-2f35801bb4299e90)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-4e6c0b410eb3c094)
test result: ok. 137 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.50s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-f7ecb916621c3d2e)
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.03s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-a6e55f0efc587c4a)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-e5cada0febd69e06)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-6ab96157344a6ea9)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.45s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-8aadb59911d39ecb)
test result: ok. 255 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.55s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-eff1fa1add62ac18)
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.10s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-1ce0b9de7651ca2c)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-36e3f4b9d6d1695d)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
workspace_lib_exit=0
```

#### `cargo test -p designos-app --bin designos --locked`
```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.89s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-cfe862bd484a2ffd)
test result: ok. 3462 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 304.25s
bin_designos_exit=0
```

#### `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`
```text
========================================================================
RESULT: 21/21 Zero-Fake-UI invariants PASSED. Gate status: GREEN.
========================================================================
PASS: spacing relationships hold across theme.rs and design.tokens.json
```

---

## 6. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **FACT**: `Shell::drive_screenshot` (`crates/designos-app/src/shell.rs:498-575`) runs only when `self.shot.is_some()` (initialized in `Shell::new` when `opts.screenshot.is_some()`, `shell.rs:254-261`) and never clears `self.shot` (`s.done = true` is set instead), so `self.shot.is_some()` is invariant for the entire lifetime of a `--screenshot` run.
- **FACT**: In `Shell::draw_window` (`crates/designos-app/src/shell.rs:1508-1526`), branching on `if self.shot.is_some()` when `close_requested()` is true skips `self.app.may_quit(&ctx)`, `ViewportCommand::CancelClose`, `flush_notes`, `save_close_version`, and `flush_autosave`, while still shutting down terminals cleanly.
- **FACT**: `screenshot_run_closes_without_leave_questions_when_dirty` failed before the fix (`plans/reports/agy-1274-evidence/red.log`) and passes after the fix (`plans/reports/agy-1274-evidence/green.log`), verifying dirty external files, open inline edits, pending agent documents, and dirty library files.
- **INFERENCE**: Because `eframe` closes the native viewport on the frame where `ViewportCommand::Close` is processed and `ViewportCommand::CancelClose` is not emitted, `eframe::run_native` returns to `main.rs:544`, which evaluates `shot_wrote` and exits with code `0` (when the PNG was written) or `1` (on timeout/write error).
- **ASSUMPTION**: The prompt line mentioning `plans/reports/agy-backmerge-065-into-070.md` was a carry-over from an earlier template (`plans/reports/agy-backmerge-065-into-070.md` does not exist in the repository); including `## Round 2` below in `plans/reports/agy-1274-screenshot-dirty-close.md` satisfies the structural requirement without creating an unrelated report file outside scope.

### Weakest Link
- The real-binary macOS window close was not executed in this worker session because `HIDIdleTime` was `< 120` s (`74` s / `27` s, owner active); `Sim` tests exercise `Shell::draw_window` directly with `raw.viewport.commands` inspection.

### Unresolved Questions
- None.

---

## Round 2

- Verified `out/cargo` removed, working tree clean, commits structured as `test(app)` followed by `fix(app)`, and all gates green (`fmt`, `clippy`, `--workspace --lib`, `--bin designos`, `zero_fake_ui_gate.py`, `check_spacing_relationships.py`).

Status: DONE
