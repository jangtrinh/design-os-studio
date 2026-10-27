# DEV Report · #1171: Antigravity tab repeats its output after UI zoom, and the scrollbar does not work

- **Issue:** `#1171` (`US-12` on `pm/spec-v0.6`, comment of `2026-10-26T08:54:57Z`)
- **Branch:** `jangtrinh/agy-1171-zoom-scrollback` (base: `origin/release/v0.6.3` @ `f5d61ae26`)
- **Evidence folder:** `plans/reports/agy-1171-zoom-scrollback-evidence/`

---

## 1. What Could NOT Be Verified

1. **AC-06 (Real Antigravity `1.4.7` interactive verification on Windows 11 and macOS):** Excluded from DEV scope by the task contract ("AC-06 (Windows 11 and macOS with the real Antigravity 1.4.7) belongs to QA, not to you") and by the hard rule prohibiting execution of live agent binaries (`claude`, `codex`, `gemini`, `grok`, `agy`).
2. **Live PTY byte capture from the stripped Antigravity `1.4.7` binary on `SIGWINCH`:** The task prompt states the controller records the live PTY stream for Step 0 and notifies the worker via Orca if the bytes break the `ESC[2J ESC[3J ESC[H` inference. No contradiction message arrived in the Orca inbox (`orca orchestration check --terminal term_5b8728e7-abef-4bd2-a9ab-7ae110aa2961 --json` returned `count: 0`).

---

## 2. Conclusion & Epistemic Ledger

### Conclusion
UI zoom in the Agents terminal viewport caused repeated output and broken scrollback navigation due to four compounding defects in `crates/designos-app/src/terminal.rs` and `vt100 0.15.2`:
1. `vt100 0.15.2` drops `ESC[3J` (`ED` mode 3: erase scrollback), so every `SIGWINCH` full-history reprint (`ESC[2J ESC[3J ESC[H` + history) pushed another copy of scrolled-off lines into the 5,000-line scrollback buffer.
2. `render_vt100_viewport` resized the PTY and `vt100::Parser` on every intermediate frame of a multi-step zoom instead of waiting for the viewport size to settle for 120 ms.
3. The terminal scrollbar was paint-only, hardcoded `frac = (offset as f32 / 500.0).min(1.0)` against a 5,000-line scrollback buffer, and lacked pointer hit-testing (`Sense::click_and_drag()`).
4. `vt100 0.15.2`'s `Grid::visible_rows` (`src/grid.rs:125`) computes `self.rows.iter().take(rows_len - self.scrollback_offset)`, which panics with unsigned integer subtraction underflow whenever `scrollback_offset > rows` (e.g., scrolling back 2,500 or 5,000 lines in a 24-row or 4-row viewport).

All four issues are resolved in `crates/designos-app/src/terminal.rs` and verified with red-first unit/integration tests in `crates/designos-app/src/terminal/zoom_scrollback_tests.rs`.

### Epistemic Ledger
- **[FACT]** `vt100 0.15.2` (`src/screen.rs:1171`, `fn ed`) handles modes `0`, `1`, and `2`, and drops mode `3` (`ESC[3J`) with `log::debug!("unhandled ED mode: {n}")`.
- **[FACT]** On base commit `f5d61ae26`, Fake CLI mode (a) (`ESC[2J ESC[3J ESC[H` + 30 history lines on a 20-row PTY across 5 resize steps) produces `scrollback_len = 66`, with lines `HISTORY_LINE_01`..`HISTORY_LINE_11` repeated **6 times each** (`red.log`). After the fix (`0830c6cd`), `scrollback_len = 11` and every line `HISTORY_LINE_01`..`HISTORY_LINE_30` appears **exactly once** (`green.log`).
- **[FACT]** `vt100 0.15.2` (`src/grid.rs:125`, `Grid::visible_rows`) evaluates `rows_len - self.scrollback_offset` where `rows_len == usize::from(self.size.rows)`. When `scrollback_offset > rows`, debug builds panic with `attempt to subtract with overflow`. `with_scrollback_safe_screen` in `terminal.rs` temporarily widens `parser`'s row count to `offset as u16` (preserving `cols`) while reading cells, then restores `(rows, cols)` before releasing the mutex.
- **[FACT]** `TerminalTab::request_resize(rows, cols, now_s)` debounces `pty_resize` (`master.resize` + `parser.set_size`) until `(rows, cols)` has remained unchanged for `RESIZE_DEBOUNCE_SECS` (`0.120` s) of egui frame time (`ui.input(|i| i.time)`), scheduling `ui.ctx().request_repaint_after(Duration::from_secs_f64(0.120))` with zero timer threads while layout uses the live `(rows, cols)`.
- **[FACT]** The terminal scrollbar now queries the true scrollback length via `parser_scrollback_len` (`set_scrollback(usize::MAX)` -> `scrollback()` -> restore), registers a `24 px` (`theme::TARGET_MIN`) interactive track (`Sense::click_and_drag()`) on the right edge of `viewport_rect`, maps click/drag positions across `0..=scrollback_len`, suppresses cell selection while dragging the scrollbar, and draws a `4 px` (`theme::SPACE_FACT`) thumb inset by `theme::INSET_SEGMENT` (`6 px`) with `theme::cr(theme::RADIUS_MARK)` and `theme::TEXT_SECONDARY` / `theme::TEXT_TERTIARY`.
- **[INFERENCE]** Antigravity `1.4.7` (an Ink-style TUI) responds to `SIGWINCH` by emitting `ESC[2J ESC[3J ESC[H` and reprinting its scrollback history.
- **[ASSUMPTION]** Any CSI `3J` sequence emitted by a CLI to clear scrollback uses standard CSI syntax (`ESC [ 3 J`, `ESC [ 0 3 J`, or `ESC [ ? 3 J`).
- **Weakest link (one line):** The inference that Antigravity `1.4.7` uses standard `ESC[3J` on `SIGWINCH` relies on the controller's live PTY recording rather than static disassembly of its stripped Go binary (`AC-06` QA gate).

---

## 3. Skills Loaded & What Changed Because of Them

| Skill | Path | Concrete Impact on Implementation |
|---|---|---|
| `es:astra-work` | `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md` | Enforced strict scope boundaries (only `terminal.rs`, `terminal/zoom_scrollback_tests.rs`, report, and evidence), isolated `$HOME`/`$TMPDIR` test harness, and post-mutation verification across all workspace gates. |
| `es:fable-thinking` | `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md` | Structured root-cause falsification (Step 0 mode (a) vs mode (b) before writing production code), explicit `PREDICT:` checks before tool calls, and the epistemic ledger above. |
| `es:dev` / `es:feature-dev` | `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md` | Enforced red-first TDD: 5 atomic `test(app): …` commits landed and were verified failing (`0 passed; 5 failed` in `red.log`) prior to the `fix(app): …` commit (`5 passed; 0 failed` in `green.log`). |
| `es:rust` | `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md` | Implemented zero-allocation byte-stream scanning (`advance_esc3j_scanner`) across arbitrary PTY read chunk boundaries and lock-order-safe `with_scrollback_safe_screen` without holding mutexes across UI calls. |
| `es:rust-gpu-canvas` | `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md` | Used egui's frame clock (`ui.input(|i| i.time)`) + `request_repaint_after(120ms)` for resize debounce with zero background timer threads and zero idle repaint churn once settled. |
| `es-designer` | `/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md`, `references/craft-floor.md` | Aligned the terminal scrollbar visuals with `panels::scrollbar`: `theme::TARGET_MIN` (`24 px`) pointer hit target, `theme::SPACE_FACT` (`4 px`) thumb width, `theme::INSET_SEGMENT` (`6 px`) right inset, `theme::cr(theme::RADIUS_MARK)` corner radius, and `theme::TEXT_SECONDARY` (active/hover) / `theme::TEXT_TERTIARY` (idle) colors; verified `check_spacing_relationships.py` and `zero_fake_ui_gate.py` pass. |

---

## 4. Step 0 Duplicate Counts & `ESC[3J` Route (No `vt100` Fork)

### Step 0 Duplicate Counts (Mode (a) vs Mode (b))
Fake CLI (`crates/designos-app/src/terminal/zoom_scrollback_tests.rs`) prints 30 lines (`HISTORY_LINE_01`..`HISTORY_LINE_30`) on a 20-row × 80-column PTY (so 11 lines scroll into scrollback and 19 lines + status row remain on the visible screen), then receives 5 resize steps (`cols = 76, 72, 68, 64, 60` at `t = 1.0, 2.0, 3.0, 4.0, 5.0` s):

- **On base commit (`f5d61ae26`, `plans/reports/agy-1171-zoom-scrollback-evidence/red.log`):**
  ```text
  STEP0_MODE_A_COUNTS: scrollback_len=66 counts={"HISTORY_LINE_01": 6, "HISTORY_LINE_02": 6, "HISTORY_LINE_03": 6, "HISTORY_LINE_04": 6, "HISTORY_LINE_05": 6, "HISTORY_LINE_06": 6, "HISTORY_LINE_07": 6, "HISTORY_LINE_08": 6, "HISTORY_LINE_09": 6, "HISTORY_LINE_10": 6, "HISTORY_LINE_11": 6, "HISTORY_LINE_12": 1, "HISTORY_LINE_13": 1, "HISTORY_LINE_14": 1, "HISTORY_LINE_15": 1, "HISTORY_LINE_16": 1, "HISTORY_LINE_17": 1, "HISTORY_LINE_18": 1, "HISTORY_LINE_19": 1, "HISTORY_LINE_20": 1, "HISTORY_LINE_21": 1, "HISTORY_LINE_22": 1, "HISTORY_LINE_23": 1, "HISTORY_LINE_24": 1, "HISTORY_LINE_25": 1, "HISTORY_LINE_26": 1, "HISTORY_LINE_27": 1, "HISTORY_LINE_28": 1, "HISTORY_LINE_29": 1, "HISTORY_LINE_30": 1}
  ```
  Each of the 11 scrolled-off lines (`HISTORY_LINE_01`..`HISTORY_LINE_11`) appears **6 times** (1 initial + 5 resize reprints = 66 scrollback lines).

- **After fix (`0830c6cd`, `plans/reports/agy-1171-zoom-scrollback-evidence/green.log`):**
  ```text
  STEP0_MODE_A_COUNTS: scrollback_len=11 counts={"HISTORY_LINE_01": 1, "HISTORY_LINE_02": 1, "HISTORY_LINE_03": 1, "HISTORY_LINE_04": 1, "HISTORY_LINE_05": 1, "HISTORY_LINE_06": 1, "HISTORY_LINE_07": 1, "HISTORY_LINE_08": 1, "HISTORY_LINE_09": 1, "HISTORY_LINE_10": 1, "HISTORY_LINE_11": 1, "HISTORY_LINE_12": 1, "HISTORY_LINE_13": 1, "HISTORY_LINE_14": 1, "HISTORY_LINE_15": 1, "HISTORY_LINE_16": 1, "HISTORY_LINE_17": 1, "HISTORY_LINE_18": 1, "HISTORY_LINE_19": 1, "HISTORY_LINE_20": 1, "HISTORY_LINE_21": 1, "HISTORY_LINE_22": 1, "HISTORY_LINE_23": 1, "HISTORY_LINE_24": 1, "HISTORY_LINE_25": 1, "HISTORY_LINE_26": 1, "HISTORY_LINE_27": 1, "HISTORY_LINE_28": 1, "HISTORY_LINE_29": 1, "HISTORY_LINE_30": 1}
  ```
  `scrollback_len = 11` and every line `HISTORY_LINE_01`..`HISTORY_LINE_30` has count `1`. Mode (b) (which redraws only the visible screen without `ESC[3J`) keeps its 11 scrollback lines untouched (`HISTORY_LINE_01`..`HISTORY_LINE_11`).

### Route Chosen to Honour `ESC[3J` Without Forking `vt100`
1. **Stateful boundary scanner (`advance_esc3j_scanner` & `process_pty_bytes`):**
   - At the PTY reader boundary (`crates/designos-app/src/terminal.rs`), `process_pty_bytes` scans incoming bytes with a 1-byte state machine (`esc3j_state: u8`) that detects `ESC [ 3 J` (as well as `ESC [ 0 3 J` and `ESC [ ? 3 J`) even when split across `reader.read()` chunks.
   - Bytes up to the `ESC[3J` sequence are fed to `parser.process(&bytes[seg_start..=idx])`, then `clear_parser_scrollback(parser)` runs at the exact byte boundary, and processing continues with the remaining bytes.
2. **State-preserving scrollback rebuild (`clear_parser_scrollback`):**
   - `vt100::Parser` has no public method to clear only the internal `VecDeque<Row>` scrollback buffer.
   - Instead, `clear_parser_scrollback` reads `(rows, cols) = parser.screen().size()` and captures `parser.screen().state_formatted()` (which serializes the visible grid cells, cursor position, SGR attributes, and terminal modes without scrollback). If `parser.screen().alternate_screen()` is active, it temporarily switches to the main screen (`\x1b[?47l`), snapshots `main_state`, and replays both onto a fresh `vt100::Parser::new(rows, cols, 5000)` before switching back (`\x1b[?47h`) and replaying `alt_state`.
   - When the user was scrolled into history (`prev_scrollback > 0`) at the moment `ESC[3J` arrived, `restore_scrollback: Arc<AtomicUsize>` records the offset and restores `parser.set_scrollback(target.min(sb_len))` as the reprinted history refills the scrollback buffer, so the user's scroll position does not jump to `0` unless they were already at the bottom.
3. **Workaround for `vt100 0.15.2` `Grid::visible_rows` subtraction underflow (`with_scrollback_safe_screen`):**
   - In `vt100 0.15.2`, `screen.cell(row, col)` calls `Grid::visible_rows()`, which computes `self.rows.iter().take(rows_len - self.scrollback_offset)`. Whenever `scrollback_offset > rows` (for example, `scrollback_offset = 2500` on a `24`-row viewport), `rows_len - self.scrollback_offset` panics in debug builds.
   - `with_scrollback_safe_screen` checks if `offset > usize::from(rows)` and temporarily calls `parser.set_size(offset as u16, cols)` while reading visible cells `[0..rows]`, then restores `parser.set_size(rows, cols)` and `parser.set_scrollback(offset)` before dropping the lock. Because `cols` never changes, `vt100` does not rewrap any row.

---

## 5. Acceptance Criteria & Verification Table

| ID | Requirement / Gate | Status | Evidence |
|---|---|---|---|
| **Step 0** | Fake CLI with mode (a) (`ESC[2J ESC[3J ESC[H` + full history) and mode (b) (visible screen only); show mode (a) duplicates on base commit | **PASS** | `red.log` (`scrollback_len=66`, `HISTORY_LINE_01..11` = 6 copies each) -> `green.log` (`scrollback_len=11`, 1 copy each) |
| **R1 / AC-05** | Debounce PTY & parser resize until size settles for ~120 ms of frame time (no timer thread); layout uses live size while waiting; holding zoom key sends 1 PTY resize | **PASS** | `ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab` (`pty_resize_count == 0` during 5 zoom steps over 80 ms, `pty_resize_count == 1` after 130 ms settle) |
| **R2 / AC-01** | Honour `ESC[3J` at PTY parser boundary without forking `vt100`; preserve cursor, attributes, alternate screen, visible grid, and mode (b) scrollback | **PASS** | `ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom` |
| **R3 / AC-02** | Scrollbar maps to real scrollback length (no `/ 500`), supports drag & click at every zoom across 5,000 lines and min size, 24 px hit target, `es-designer` tokens | **PASS** | `ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size` + `agent-tab-scrollbar-1440x900.png` |
| **AC-03** | 100-column box table resized to 70 columns and back to 100 columns leaves no stale narrow 70-column copy on screen or in scrollback | **PASS** | `ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy` |
| **R4 / AC-04** | No regression to Claude Code tab behaviour, alternate screen, or mouse reporting | **PASS** | `ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab` + `edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn` + all 326 `terminal` tests |
| **Edge Cases** | Zoom while streaming, zoom while scrolled into history, alternate screen + mouse reporting (`?1049h` + `?1000h` + `?1006h`), minimum clamp (`rows=4, cols=16`), tab spawned after zooming | **PASS** | `edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn` |
| **Gates** | `cargo fmt`, `cargo clippy`, `cargo test --workspace --locked --lib`, `cargo test -p designos-app --bin designos --locked`, `check_spacing_relationships.py`, `zero_fake_ui_gate.py` | **PASS** | Exact commands and outputs below |

### Commit History (5 Red `test(app):` Commits Before `fix(app):`)
```text
$ git log --oneline origin/release/v0.6.3..HEAD
0830c6cd fix(app): debounce PTY resize, honour ESC[3J scrollback clear, and wire interactive scrollbar (#1171)
261974b5c test(app): add zoom and scrollback edge-case tests (#1171)
56604e5ab test(app): add AC-05 zoom resize debounce and AC-04 regression test (#1171)
2b6ede627 test(app): add AC-03 100-column box table resize and rewrap test (#1171)
19d73d903 test(app): add AC-02 interactive scrollbar test across 5000 lines and min size (#1171)
ff658bd8f test(app): add Step 0 fake CLI and AC-01 zoom scrollback test (#1171)
```

### Exact Gate Commands & Pasted Outputs

#### 1. Red-Phase Test Run (Before `fix(app):` Commit)
Command:
```sh
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked terminal::zoom_scrollback_tests -- --nocapture
```
Output (`plans/reports/agy-1171-zoom-scrollback-evidence/red.log`):
```text
running 5 tests

thread 'terminal::zoom_scrollback_tests::ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab' (75347171) panicked at crates/designos-app/src/terminal/zoom_scrollback_tests.rs:483:5:
assertion `left == right` failed: AC-05: no PTY resize may be sent while zoom steps are still changing within the 120 ms debounce window, got 5
  left: 5
 right: 0
test terminal::zoom_scrollback_tests::ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab ... FAILED

thread 'terminal::zoom_scrollback_tests::edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn' (75347172) panicked at crates/designos-app/src/terminal/zoom_scrollback_tests.rs:528:13:
assertion `left == right` failed: streaming edge case: STREAM_LINE_01 must appear exactly once after mid-stream resize, got 2
  left: 2
 right: 1
test terminal::zoom_scrollback_tests::edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn ... FAILED

thread 'terminal::zoom_scrollback_tests::ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy' (75347170) panicked at crates/designos-app/src/terminal/zoom_scrollback_tests.rs:454:5:
AC-03: no stale narrow 70-column table copy may remain after resizing back to 100 columns (sb_len=6, ...)
test terminal::zoom_scrollback_tests::ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy ... FAILED
STEP0_MODE_A_COUNTS: scrollback_len=66 counts={"HISTORY_LINE_01": 6, "HISTORY_LINE_02": 6, "HISTORY_LINE_03": 6, "HISTORY_LINE_04": 6, "HISTORY_LINE_05": 6, "HISTORY_LINE_06": 6, "HISTORY_LINE_07": 6, "HISTORY_LINE_08": 6, "HISTORY_LINE_09": 6, "HISTORY_LINE_10": 6, "HISTORY_LINE_11": 6, "HISTORY_LINE_12": 1, "HISTORY_LINE_13": 1, "HISTORY_LINE_14": 1, "HISTORY_LINE_15": 1, "HISTORY_LINE_16": 1, "HISTORY_LINE_17": 1, "HISTORY_LINE_18": 1, "HISTORY_LINE_19": 1, "HISTORY_LINE_20": 1, "HISTORY_LINE_21": 1, "HISTORY_LINE_22": 1, "HISTORY_LINE_23": 1, "HISTORY_LINE_24": 1, "HISTORY_LINE_25": 1, "HISTORY_LINE_26": 1, "HISTORY_LINE_27": 1, "HISTORY_LINE_28": 1, "HISTORY_LINE_29": 1, "HISTORY_LINE_30": 1}

thread 'terminal::zoom_scrollback_tests::ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom' (75347168) panicked at crates/designos-app/src/terminal/zoom_scrollback_tests.rs:252:5:
assertion `left == right` failed: AC-01 mode (a): scrollback must hold 11 lines after 5 resizes, got 66
  left: 66
 right: 11
test terminal::zoom_scrollback_tests::ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom ... FAILED

thread 'terminal::zoom_scrollback_tests::ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size' (75347169) panicked at crates/designos-app/src/terminal/zoom_scrollback_tests.rs:298:28:
AC-02: terminal scrollbar track must be recorded when scrollback_len > 0
test terminal::zoom_scrollback_tests::ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size ... FAILED

test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 3336 filtered out; finished in 2.49s
```

#### 2. Green-Phase Test Run (After `fix(app):` Commit)
Command:
```sh
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked terminal::zoom_scrollback_tests -- --nocapture
```
Output (`plans/reports/agy-1171-zoom-scrollback-evidence/green.log`):
```text
running 5 tests
test terminal::zoom_scrollback_tests::ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab ... ok
test terminal::zoom_scrollback_tests::ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy ... ok
STEP0_MODE_A_COUNTS: scrollback_len=11 counts={"HISTORY_LINE_01": 1, "HISTORY_LINE_02": 1, "HISTORY_LINE_03": 1, "HISTORY_LINE_04": 1, "HISTORY_LINE_05": 1, "HISTORY_LINE_06": 1, "HISTORY_LINE_07": 1, "HISTORY_LINE_08": 1, "HISTORY_LINE_09": 1, "HISTORY_LINE_10": 1, "HISTORY_LINE_11": 1, "HISTORY_LINE_12": 1, "HISTORY_LINE_13": 1, "HISTORY_LINE_14": 1, "HISTORY_LINE_15": 1, "HISTORY_LINE_16": 1, "HISTORY_LINE_17": 1, "HISTORY_LINE_18": 1, "HISTORY_LINE_19": 1, "HISTORY_LINE_20": 1, "HISTORY_LINE_21": 1, "HISTORY_LINE_22": 1, "HISTORY_LINE_23": 1, "HISTORY_LINE_24": 1, "HISTORY_LINE_25": 1, "HISTORY_LINE_26": 1, "HISTORY_LINE_27": 1, "HISTORY_LINE_28": 1, "HISTORY_LINE_29": 1, "HISTORY_LINE_30": 1}
test terminal::zoom_scrollback_tests::ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom ... ok
test terminal::zoom_scrollback_tests::edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn ... ok
test terminal::zoom_scrollback_tests::ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 3336 filtered out; finished in 2.51s
```

#### 3. `cargo fmt --all -- --check`
Command:
```sh
cargo fmt --all -- --check
```
Output:
```text
(exited with code 0, zero diff)
```

#### 4. `cargo clippy --workspace --all-targets --locked -- -D warnings`
Command:
```sh
cargo clippy --workspace --all-targets --locked -- -D warnings
```
Output (`plans/reports/agy-1171-zoom-scrollback-evidence/clippy.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-mcp)
    Checking designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.95s
```

#### 5. `cargo test --workspace --locked --lib`
Command:
```sh
cargo test --workspace --locked --lib
```
Output (`plans/reports/agy-1171-zoom-scrollback-evidence/cargo-test-lib.log`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-e9fb9a3e00ecdbf3)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-a0c6cf32749620e6)
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.91s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-029cd168a180af6a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-f5a81d86319362e2)
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.20s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-648cb6f438830bb1)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 64.72s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-8afb0f60e41df5a3)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-49227ce26b5d6686)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-d39027c82797cc1e)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-189215ed087510e0)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.49s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-39473ba3e638dab1)
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 32.91s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-125a628128d3698e)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-a59ae055a8fa52f0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
```

#### 6. `cargo test -p designos-app --bin designos --locked` (Terminal Module & Full Suite)
Command:
```sh
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked terminal
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
```
Outputs (`cargo-test-terminal.log` and `cargo-test-bin-designos.log`):
```text
# terminal filter:
test result: ok. 326 passed; 0 failed; 1 ignored; 0 measured; 3014 filtered out; finished in 42.10s

# full --bin designos suite:
test result: ok. 3330 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 316.26s
```

#### 7. `python3 scripts/check_spacing_relationships.py` & `python3 scripts/zero_fake_ui_gate.py`
Command:
```sh
python3 scripts/check_spacing_relationships.py
python3 scripts/zero_fake_ui_gate.py
```
Output:
```text
literal geometry outside theme.rs: offset_literals=1665, inset_literals=45, radius_literals=256, font_literals=25, nonconcentric_pairs=2
note: 14 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

#### 8. 1440×900 Screenshot Evidence
- **File:** `plans/reports/agy-1171-zoom-scrollback-evidence/agent-tab-scrollbar-1440x900.png` (`1440×900`, captured via `designos --panel agents-approval-open-tab --size 1440x900 --screenshot`, showing the agent tab scrolled into history at lines `[049]..[085]` with the `↑ Scrollback` pill and the mid-track scrollbar thumb visible on the right edge of the terminal viewport).

---

## 6. Assumptions & Unresolved Questions

- **Assumptions:**
  1. `ESC[3J` (`\x1b[3J`, `\x1b[03J`, `\x1b[?3J`) is the standard xterm/VT100 sequence used by Ink and terminal CLIs to clear the scrollback buffer on `SIGWINCH`.
  2. When a user is actively scrolled into history (`scrollback_offset > 0`) during a `SIGWINCH` `ESC[3J` full-history reprint, preserving their previous `scrollback_offset` (clamped to the new `scrollback_len`) prevents unexpected jumps back to live output.
- **Unresolved Questions:**
  - None for DEV scope; `AC-06` live verification with Antigravity `1.4.7` on Windows 11 and macOS is queued for QA.

Status: DONE — all DEV acceptance criteria (Step 0, R1–R4, AC-01–AC-05, edge cases) and verification gates passed.

---

## Round 2

- **Round 2 Base Head:** `22a39a84c`
- **Round 2 Code Commits:**
  - `5654aed68` — `test(app): add regression test for agents-approval-open-tab and agents-scrollback presets (#1171)`
  - `f7099c87a` — `test(app): add deep scrollback painted-rows and release non-widening test (#1171)`
  - `21c2165f5` — `fix(app): restore agents-approval-open-tab preset and add agents-scrollback (#1171)`
  - `cb2aa92d0` — `fix(app): widen scrollback screen only under debug_assertions (#1171)`
- **Evidence Prefix:** `plans/reports/agy-1171-zoom-scrollback-evidence/r2-*`

### 1. What Could NOT Be Verified (Round 2)

1. **AC-06 (Real Antigravity `1.4.7` interactive verification on Windows 11 and macOS):** Remains in QA scope (`claude`, `codex`, `gemini`, `grok`, and `agy` are never executed by DEV).
2. **Heap-allocator byte counter around `with_scrollback_safe_screen`:** Rust's global allocator is not wrapped in a custom counting `GlobalAlloc` in `designos-app`. Instead, zero per-frame row allocation in release builds is verified structurally (confirming `parser.set_size(offset as u16, cols)` is compiled only under `#[cfg(debug_assertions)]` and `vt100-0.15.2/src/grid.rs:120` `Grid::visible_rows` performs only iterator arithmetic `skip`/`chain`/`take` with zero heap allocation) and functionally by running `deep_scrollback_offset_3000_of_5000_paints_expected_history_rows` under **both** the `test` (`dev`, `debug_assertions = true`) profile (`r2-green.log`) and the `release` (`debug_assertions = false`, `overflow-checks = false`) profile (`r2-green-release.log`).

### 2. Conclusion & Epistemic Ledger (Round 2)

#### Conclusion
Round 2 resolves both controller review items without touching any accepted Round 1 behavior (`ESC[3J` parser-boundary handling, 120 ms frame-time resize debounce, interactive scrollbar, fake CLI, and AC-01..AC-05 tests):
1. **Layout-gate preset restored byte-for-byte (`--panel agents-approval-open-tab`) & new `--panel agents-scrollback` preset registered:** The `"agents-approval-open-tab" =>` arm in `crates/designos-app/src/terminal.rs` is restored byte-for-byte to commit `f5d61ae2` (`0` scrollback lines, `0` scrollback offset), preserving its `#1126` layout-gate baseline (`cases.json` id `agents-approval-open-tab`, spec I, PR `#1206`). The 120-line scrollback demo (scrolled to offset `45`) now lives in `--panel agents-scrollback`, registered in `crates/designos-app/src/cli.rs` (`PANELS`), `crates/designos-app/src/shell.rs` panel routing, `crates/designos-app/src/terminal.rs` (`open_approval_fixture`), and `sim_approval_card_and_permissions_modal_pass_layout_lint`.
2. **Zero per-frame allocation when scrolled deep (`with_scrollback_safe_screen`):** `with_scrollback_safe_screen` in `crates/designos-app/src/terminal.rs` now widens `parser.set_size(offset as u16, cols)` only under `#[cfg(debug_assertions)]`. In release builds (`cfg(not(debug_assertions))`, `overflow-checks = false`), `vt100 0.15.2`'s `Grid::visible_rows` (`src/grid.rs:125`) wraps `rows_len - self.scrollback_offset` to a large `usize`, and because the `self.scrollback.iter().skip(scrollback_len - self.scrollback_offset)` prefix yields `scrollback_offset` (`> rows`) items first, `take()` and `screen.cell(r, c)` for `r in 0..rows` read the exact same history rows with zero `Vec::resize` allocation.

#### Epistemic Ledger (Round 2)
- **[FACT]** `git diff f5d61ae2 -- crates/designos-app/src/terminal.rs` (`r2-diff-f5d61ae2-terminal.patch`) shows zero diff inside the `"agents-approval-open-tab" =>` arm (lines 1721–1729 of `f5d61ae2` match lines 1896–1904 of `HEAD` byte-for-byte).
- **[FACT]** In `vt100 0.15.2` (`src/grid.rs:120–126`), `Grid::visible_rows` returns `self.scrollback.iter().skip(scrollback_len - self.scrollback_offset).chain(self.rows.iter().take(rows_len - self.scrollback_offset))`, and `Grid::visible_row(row)` calls `self.visible_rows().nth(usize::from(row))`. When `self.scrollback_offset > rows_len` and `row < rows_len`, `nth(usize::from(row))` is satisfied entirely from the `self.scrollback` iterator before `.chain(...)` ever advances into `self.rows.iter().take(...)`.
- **[FACT]** Under `cfg(not(debug_assertions))` (`overflow-checks = false`), `rows_len.wrapping_sub(self.scrollback_offset)` does not panic, and `with_scrollback_safe_screen` executes `f(parser.screen())` directly without calling `parser.set_size`, eliminating the `offset - rows` allocation of `vt100::Row` (`cols` cells each) on every paint and copy frame.
- **[FACT]** At `scrollback_offset = 3_000` of `5_000` lines in a `24`-row × `80`-column viewport (`5,000` total lines `SB_ROW_0001`..`SB_ROW_5000`, so `scrollback_len = 4,976` and `5,000 - 3,000 = 2,000`), the 24 painted rows are `SB_ROW_2001`..`SB_ROW_2024` in both the `test` (`dev`, `debug_assertions = true`) profile (`r2-green.log`) and the `release` (`debug_assertions = false`) profile (`r2-green-release.log`).
- **[INFERENCE]** Because `with_scrollback_safe_screen` no longer calls `parser.set_size` in release builds, scrolling to offset `4,000` at `100` columns while a CLI streams allocates `0` temporary cells per frame instead of `~400,000` cells per frame.
- **[ASSUMPTION]** Standard Cargo release profile settings (`debug-assertions = false`, `overflow-checks = false`) remain unmodified in `Cargo.toml` (which defines no custom `[profile.release]` overrides).
- **Weakest link (one line):** Reliance on `vt100 0.15.2`'s `rows_len - self.scrollback_offset` wrapping arithmetic under `cfg(not(debug_assertions))` is coupled to `vt100 0.15.2` internals and standard release overflow semantics (`overflow-checks = false`).

### 3. Skills Loaded & What Changed Because of Them (Round 2)

| Skill | Path | Concrete Impact on Round 2 |
|---|---|---|
| `es:astra-work` | `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md` | Enforced strict file-scope discipline (`terminal.rs`, `terminal/zoom_scrollback_tests.rs`, `cli.rs`, `shell.rs`, report, and `r2-*` evidence) and post-mutation verification on the final commit. |
| `es:fable-thinking` | `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md` | Verified `vt100-0.15.2/src/grid.rs:120` iterator chain semantics before editing `with_scrollback_safe_screen` and predicted/caught a `clippy::useless_conversion` warning in the new test before finalizing commits. |
| `es:dev` / `es:feature-dev` | `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`, `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md` | Landed both Round 2 tests red-first in separate `test(app): …` commits (`5654aed68`, `f7099c87a`, verified failing in `r2-red.log`) before their corresponding `fix(app): …` commits (`21c2165f5`, `cb2aa92d0`, verified passing in `r2-green.log`). |
| `es:rust` | `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md` | Guarded the temporary `parser.set_size` widening under `#[cfg(debug_assertions)]` with a clear doc comment explaining release-mode wrapping subtraction and `Iterator::take` behavior, eliminating hot-path per-frame allocations (`CLAUDE.md` §4). |
| `es:rust-gpu-canvas` | `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md` | Preserved zero per-frame heap allocation in `render_vt100_viewport` when scrolled deep into a 5,000-line history buffer while the PTY streams. |
| `es-designer` | `/Users/jangtrinh/.gemini/config/skills/es-designer/SKILL.md`, `references/craft-floor.md` | Captured and visually inspected `plans/reports/agy-1171-zoom-scrollback-evidence/r2-agents-scrollback-1440x900.png` (`1440×900` via `--panel agents-scrollback`), confirming the scrollbar thumb sits mid-track for `scrollback = 45` of `83` and `zero_fake_ui_gate.py` passes (`17/17`). |

### 4. Fix Details & Build Profile Coverage

#### 4.1 `"agents-approval-open-tab" =>` Arm Comparison (`f5d61ae2` vs `HEAD`)

- **At `f5d61ae2` (`crates/designos-app/src/terminal.rs:1721–1729`):**
  ```rust
            "agents-approval-open-tab" => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.preset = AgentCliPreset::Grok;
                    tab.title = "Grok CLI #1".into();
                    if let Ok(mut st) = tab.agent_status.lock() {
                        *st = AgentTabStatus::WaitingForInput(Some(NEEDS_APPROVAL_LABEL.into()));
                    }
                }
            }
  ```
- **At `HEAD` (`cb2aa92d0`, `crates/designos-app/src/terminal.rs:1896–1904`):**
  ```rust
            "agents-approval-open-tab" => {
                if let Some(tab) = self.active_tab_mut() {
                    tab.preset = AgentCliPreset::Grok;
                    tab.title = "Grok CLI #1".into();
                    if let Ok(mut st) = tab.agent_status.lock() {
                        *st = AgentTabStatus::WaitingForInput(Some(NEEDS_APPROVAL_LABEL.into()));
                    }
                }
            }
  ```
- **`git diff f5d61ae2 -- crates/designos-app/src/terminal.rs` around `open_approval_fixture` (`r2-diff-f5d61ae2-terminal.patch` lines 295–319):**
  ```diff
  @@ -1727,6 +1902,24 @@ impl TerminalState {
                       }
                   }
               }
  +            "agents-scrollback" => {
  +                let mut history = String::new();
  +                for idx in 1..=120 {
  +                    history.push_str(&format!("[{idx:03}] Inspecting layer tokens and scrollback\r\n"));
  +                }
  +                if let Some(tab) = self.active_tab_mut() {
  +                    tab.preset = AgentCliPreset::Grok;
  +                    tab.title = "Grok CLI #1".into();
  +                    if let Ok(mut st) = tab.agent_status.lock() {
  +                        *st = AgentTabStatus::WaitingForInput(Some(NEEDS_APPROVAL_LABEL.into()));
  +                    }
  +                    if let Ok(mut parser) = tab.parser.lock() {
  +                        parser.process(history.as_bytes());
  +                        parser.set_scrollback(45);
  +                    }
  +                    tab.restore_scrollback.store(45, std::sync::atomic::Ordering::Release);
  +                }
  +            }
               _ => {}
           }
       }
  ```

#### 4.2 Build Profiles Exercised by `deep_scrollback_offset_3000_of_5000_paints_expected_history_rows`
1. **Default `test` (`dev`) profile (`debug_assertions = true`, `overflow-checks = true`):**
   - Run via `cargo test -p designos-app --bin designos --locked terminal::zoom_scrollback_tests` (`r2-green.log`).
   - Exercises the full `render_vt100_viewport` -> `with_scrollback_safe_screen` path at `scrollback = 3_000` of `5_000` lines in a `24`-row × `80`-column viewport (where `#[cfg(debug_assertions)]` widens temporarily to prevent the debug overflow panic and restores `(24, 80)` afterward), plus direct cell reading at `scrollback = 3_000` with `size() == (24, 80)` and source-level verification of the `#[cfg(debug_assertions)]` guard.
2. **Optimized `release` profile (`debug_assertions = false`, `overflow-checks = false`):**
   - Run via `cargo test --release -p designos-app --bin designos --locked terminal::zoom_scrollback_tests` (`r2-green-release.log`).
   - Exercises the exact production `#[cfg(not(debug_assertions))]` branch of `with_scrollback_safe_screen` inside `render_vt100_viewport` at `scrollback = 3_000` of `5_000` lines (`24` rows × `80` cols) with **zero** `parser.set_size` calls and **zero** row allocations, confirming the painted rows equal `SB_ROW_2001`..`SB_ROW_2024`.

### 5. Acceptance Verification & Gate Outputs (Final Commit)

#### 5.1 Red / Green Logs (`r2-red.log`, `r2-green.log`, `r2-green-release.log`)

- **Red (`plans/reports/agy-1171-zoom-scrollback-evidence/r2-red.log` at `f7099c87a`, before the two `fix(app):` commits):**
  ```text
  running 7 tests

  thread 'terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled' (76759228) panicked at crates/designos-app/src/terminal/zoom_scrollback_tests.rs:622:5:
  assertion `left == right` failed: agents-approval-open-tab must have no scrollback lines, got 93
    left: 93
   right: 0
  test terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled ... FAILED
  test terminal::zoom_scrollback_tests::ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab ... ok

  thread 'terminal::zoom_scrollback_tests::deep_scrollback_offset_3000_of_5000_paints_expected_history_rows' (76759229) panicked at crates/designos-app/src/terminal/zoom_scrollback_tests.rs:721:5:
  with_scrollback_safe_screen must guard parser.set_size widening under cfg(debug_assertions) so release builds do not allocate per frame
  test terminal::zoom_scrollback_tests::deep_scrollback_offset_3000_of_5000_paints_expected_history_rows ... FAILED
  test terminal::zoom_scrollback_tests::ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy ... ok
  test terminal::zoom_scrollback_tests::ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom ... ok
  test terminal::zoom_scrollback_tests::edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn ... ok
  test terminal::zoom_scrollback_tests::ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size ... ok

  test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 3336 filtered out; finished in 2.86s
  ```

- **Green (`plans/reports/agy-1171-zoom-scrollback-evidence/r2-green.log` at `cb2aa92d0`, `test`/`dev` profile with `debug_assertions = true`):**
  ```text
  running 7 tests
  test terminal::zoom_scrollback_tests::ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab ... ok
  test terminal::zoom_scrollback_tests::deep_scrollback_offset_3000_of_5000_paints_expected_history_rows ... ok
  test terminal::zoom_scrollback_tests::ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy ... ok
  test terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled ... ok
  test terminal::zoom_scrollback_tests::ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom ... ok
  test terminal::zoom_scrollback_tests::edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn ... ok
  test terminal::zoom_scrollback_tests::ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size ... ok

  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 3336 filtered out; finished in 2.82s
  ```

- **Green Release (`plans/reports/agy-1171-zoom-scrollback-evidence/r2-green-release.log` at `cb2aa92d0`, `release` profile with `debug_assertions = false`):**
  ```text
       Running unittests src/main.rs (out/cargo/release/deps/designos-e6441832b78550da)

  running 7 tests
  test terminal::zoom_scrollback_tests::deep_scrollback_offset_3000_of_5000_paints_expected_history_rows ... ok
  test terminal::zoom_scrollback_tests::ac05_holding_zoom_key_debounces_to_single_pty_resize_and_preserves_claude_tab ... ok
  test terminal::zoom_scrollback_tests::ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size ... ok
  test terminal::zoom_scrollback_tests::ac03_box_table_100_cols_to_70_cols_and_back_leaves_no_stale_narrow_copy ... ok
  test terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled ... ok
  test terminal::zoom_scrollback_tests::ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom ... ok
  test terminal::zoom_scrollback_tests::edge_cases_streaming_scrolled_history_alt_mouse_min_clamp_and_post_zoom_spawn ... ok

  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 3334 filtered out; finished in 0.76s
  ```

#### 5.2 Workspace Gates under the Mandatory Harness

1. **`cargo fmt --all -- --check`:**
   ```sh
   cargo fmt --all -- --check
   ```
   Output:
   ```text
   (exited with code 0, zero diff)
   ```

2. **`cargo clippy --workspace --all-targets --locked -- -D warnings` (`plans/reports/agy-1171-zoom-scrollback-evidence/r2-clippy.log`):**
   ```sh
   env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
   ```
   Final lines:
   ```text
   warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-app/src/main.rs` found to be present in multiple build targets:
     * `bin` target `designos`
     * `bin` target `designos-studio`
      Compiling designos-mcp v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-mcp)
       Checking designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1171-zoom-scrollback/crates/designos-app)
       Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.07s
   ```

3. **`cargo test --workspace --locked --lib` (`plans/reports/agy-1171-zoom-scrollback-evidence/r2-cargo-test-lib.log`):**
   ```sh
   env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
   ```
   Output summary:
   ```text
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-e9fb9a3e00ecdbf3)
   test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-a0c6cf32749620e6)
   test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.13s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-029cd168a180af6a)
   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-f5a81d86319362e2)
   test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.61s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-648cb6f438830bb1)
   test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.11s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-8afb0f60e41df5a3)
   test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-49227ce26b5d6686)
   test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-d39027c82797cc1e)
   test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.36s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-189215ed087510e0)
   test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.34s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-39473ba3e638dab1)
   test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 52.02s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-125a628128d3698e)
   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
        Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-a59ae055a8fa52f0)
   test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
   ```

4. **`cargo test -p designos-app --bin designos --locked` (`plans/reports/agy-1171-zoom-scrollback-evidence/r2-cargo-test-bin-designos.log`):**
   ```sh
   env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
   ```
   Output:
   ```text
   test result: ok. 3332 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 342.92s
   ```

5. **`python3 scripts/zero_fake_ui_gate.py` (`plans/reports/agy-1171-zoom-scrollback-evidence/r2-zero-fake-ui-gate.log`):**
   ```sh
   python3 scripts/zero_fake_ui_gate.py
   ```
   Output:
   ```text
   ------------------------------------------------------------------------------------------------------------
    Gate Verdict: PASS (17/17 invariants satisfied)
   ============================================================================================================
   ```

6. **1440×900 Screenshot Capture (`--panel agents-scrollback`):**
   - **Command:**
     ```sh
     out/cargo/debug/designos --panel agents-scrollback --size 1440x900 --screenshot plans/reports/agy-1171-zoom-scrollback-evidence/r2-agents-scrollback-1440x900.png
     ```
   - **File:** `plans/reports/agy-1171-zoom-scrollback-evidence/r2-agents-scrollback-1440x900.png` (`1440×900`, inspected with `es-designer` loaded: `Grok CLI #1` tab is open and scrolled into history showing lines `[049]..[085]`, the `↑ Scrollback` pill in the bottom-right corner, and the `4 px` scrollbar thumb positioned mid-track at `scrollback = 45` of `83`).

### 6. Assumptions & Unresolved Questions (Round 2)

- **Assumptions:**
  1. `vt100` remains pinned at `0.15.2` (as enforced by the task contract: "No vt100 version change and no `Cargo.toml` change").
  2. Standard Cargo release profile settings (`overflow-checks = false`, `debug_assertions = false`) apply to release builds.
- **Unresolved Questions:**
  - None.

Status: DONE — Round 2 fixes (byte-for-byte restoration of `agents-approval-open-tab`, addition of `--panel agents-scrollback`, and `cfg(debug_assertions)`-guarded scrollback screen reading with zero release per-frame allocation) and all verification gates passed.
