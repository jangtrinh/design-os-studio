# `#1173` (Terminal Part) — Alternate-Screen Mouse Wheel Encodings, Multi-Row Steps & Cursor-Key Fallback

## 1. What Could NOT Be Verified

1. **Windows ConPTY (`portable-pty` WinCon / ConPTY backend) runtime behavior:** All tests and static inspections were executed on macOS (`aarch64-apple-darwin`). ConPTY translation of mouse wheel sequences (`CSI < ... M`, `CSI M ...`, and `ESC [ A/B` / `ESC O A/B`) on Windows could not be verified on this host.
2. **Live interactive execution of `claude`, `codex`, `gemini`, `grok`, and `agy` binaries:** Running any of the five agent CLIs is forbidden by the task harness (`Never run claude, codex, gemini, grok, or agy`). All CLI screen/mouse protocol observations in Step 0 were verified via static inspection of installed bundles/binaries and upstream source trees, and Studio's terminal behavior was verified deterministically by feeding escape sequences directly into `vt100::Parser`.

---

## 2. Drift Check ("Spec Says / HEAD Says")

Base branch: `origin/main` at `8dfaee931c75bd36293bc03266716b6abf99d868`.

| Item | Spec (`brief-agy-1173-terminal-wheel.txt`) Says | `HEAD` (`8dfaee931c75bd36293bc03266716b6abf99d868`) Says | Drift? |
|---|---|---|---|
| Terminal row height constant | `crates/designos-app/src/terminal.rs:60` (`ROW_H = 16.0`) | `crates/designos-app/src/terminal.rs:60`: `const ROW_H: f32 = 16.0;` | **None** |
| Terminal wheel handler location | `crates/designos-app/src/terminal.rs:6484-6505` inside `render_vt100_viewport` | `crates/designos-app/src/terminal.rs:6484-6507` inside `render_vt100_viewport` (starting at `:6303`) | **None** |
| `vt100` crate version & parser capabilities | `vt100 0.15.2` tracks `?1` (`application_cursor`), `?9`/`?1000`/`?1002`/`?1003` (`mouse_protocol_mode`), `?1005`/`?1006` (`mouse_protocol_encoding`: `Default`, `Utf8`, `Sgr`), `?1049` (`alternate_screen`), and ignores `?47`, `?1047`, `?1007`, `?1015` | `vt100-0.15.2/src/screen.rs:1255-1337` (`set_mode` / `clear_mode`) handles `[b'1']`, `[b'9']`, `[b'1', b'0', b'0', b'0'\|b'2'\|b'3'\|b'5'\|b'6']`, `[b'1', b'0', b'4', b'9']`, `[b'2', b'0', b'0', b'4']`, and `_ => {}` for `?47`, `?1047`, `?1007`, `?1015` | **None** |
| Main-screen scrollback guard test | `ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card` must stay green | Defined at `crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:165` | **None** |

---

## 3. Step 0 — CLI Screen & Mouse Mode Table (Static Inspection)

Also saved at `plans/reports/agy-1173-terminal-wheel-evidence/step0-cli-modes.md`.

| CLI (`preset`) | Source / Binary Inspected | Alternate Screen (`?1049h` / `?47h` / `?1047h`) | Mouse Mode (`?1000` / `?1002` / `?1003`) & Encoding (`?1005` / `?1006` / `?1015`) | Alternate Scroll (`?1007`) / Application Cursor (`?1h`) | Conversation Transcript on Main Screen? |
|---|---|---|---|---|---|
| **`claude`** (`AgentCliPreset::Claude`) | `/opt/homebrew/lib/node_modules/@anthropic-ai/claude-code/bin/claude.exe` (v2.1.312 Bun Mach-O bundle, `chunk-0f3qfx1n.js`) | **Yes (`?1049h` / `?1049l`)** by default when `wze()` (`useFullscreenEnv`) is true (unless `CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1`, `CLAUDE_CODE_NO_FLICKER=0`, or `tui: "default"`): `f_ = { ALT_SCREEN: 47, ALT_SCREEN_CLEAR: 1049, ... }`, `vMt = x2(f_.ALT_SCREEN_CLEAR)` (`\x1b[?1049h`), `kMt = l8(f_.ALT_SCREEN_CLEAR)` (`\x1b[?1049l`) at binary offset `232636800`; component `QEe` calls `t.set("altScreen")` at offset `238373500`; `wze()` at offset `226154382`. Embedded native `fzf` helper at offset `86795789` (`strings L2976216`) also emits `\x1b[?1049h` / `\x1b[?1049l`. | **Yes (`?1000h`, `?1002h`, `?1003h` + SGR `?1006h`)** in `SDn("full")` (`x2(1000)+x2(1002)+x2(1003)+x2(1006)` at offset `232636800`, plus optional `?1016h` `MOUSE_SGR_PIXELS` in `syncMousePixels` at offset `233961800`); **`?1000h` + SGR `?1006h`** in `SDn("scroll")` when `CLAUDE_CODE_DISABLE_MOUSE_CLICKS=1` (`pH()` at offset `226155640`); **No mouse mode (`""`)** in `SDn("off")` when `CLAUDE_CODE_DISABLE_MOUSE=1` while still inside `?1049h`. | JS TUI does **not** set `?1007` or `?1h`. Embedded native `fzf` helper at offset `86795797` (`strings L2976217`, `L2976220`) emits `\x1b[?1007h` and `\x1b[?1l`. | **No** by default when `wze() == true` (fullscreen TUI renders transcript in `?1049h`); **Yes** when `CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1` or `tui: "default"`. |
| **`codex`** (`AgentCliPreset::Codex`) | `openai/codex` (`codex-rs/tui/src/tui.rs`, `codex-rs/tui/src/tui/alternate_screen.rs`, `codex-rs/tui/src/local_settings.rs`) & `/opt/homebrew/lib/node_modules/@openai/codex/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex` | **Yes (`?1049h` / `?1049l`)** via `crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen}` in `codex-rs/tui/src/tui/alternate_screen.rs:20-21, 89, 197` (`strings L6585109-6585110`). Entered for overlay/pager views when `tui.alternate_screen != AltScreenMode::Never` (`codex-rs/tui/src/local_settings.rs:54-57, 132-135`), and for the full session when `owned_screen` (`fullscreen_transcript = true`) is active (`codex-rs/tui/src/tui.rs:824-848`). | **Conditional in `?1049h`:** When `capture_mouse == true` (`owned_screen == true`), `EnablePointerCapture` + `configure_input` (`codex-rs/tui/src/tui/alternate_screen.rs:58-61, 114-122`) enables **`?1000h`, `?1002h`, `?1003h` + SGR `?1006h`** (teardown at `:152-157` resets `?1003l`, `?1002l`, `?1000l`, `?1006l`; `crossterm` `DisableMouseCapture` at `strings L6636390-6636394` also resets `?1015l`). When `capture_mouse == false` in `?1049h` (default overlay/non-owned alternate screen, `:123-127`), **mouse reporting is disabled (`None`)**. | **Sets `\x1b[?1007h` (`EnableAlternateScroll`)** in `codex-rs/tui/src/tui.rs:275-280` (called from `alternate_screen.rs:125`) when in `?1049h` with `capture_mouse == false`, and `\x1b[?1007l` (`DisableAlternateScroll`, `tui.rs:296-301`, `alternate_screen.rs:117, 190`) when enabling mouse capture or leaving `?1049h`. Does **not** set `?1h`. | **Yes** by default (`fullscreen_transcript = false`, `owned_screen = false` in `codex-rs/tui/src/tui.rs:794`); **No** when `owned_screen = true` (`fullscreen_transcript = true` with `alternate_screen != Never`). |
| **`gemini`** (`AgentCliPreset::Gemini`) | `google-gemini/gemini-cli` (`packages/core/src/utils/terminal.ts`, `packages/cli/src/gemini.tsx`, `packages/cli/src/config/settingsSchema.ts`) & `/opt/homebrew/lib/node_modules/@google/gemini-cli/bundle/` | **Optional (`?1049h` / `?1049l`)**: `packages/core/src/utils/terminal.ts:68-74` (`bundle/chunk-DKZARV6U.js:279238-279246`) and Ink (`bundle/chunk-34OW3ELT.js:42519-42520`) emit `\x1b[?1049h` / `\x1b[?1049l` when `shouldEnterAlternateScreen(config.getUseAlternateBuffer(), config.getScreenReader())` is true (`packages/cli/src/gemini.tsx:896-899`, `bundle/interactiveCli-3OZSE6V3.js:35312-35315`). `ui.useAlternateBuffer` defaults to `false` (`packages/cli/src/config/settingsSchema.ts:798-806`, `bundle/chunk-DKZARV6U.js:381427`). | **Yes (`?1002h` + SGR `?1006h`) when `useAlternateBuffer: true`**: `enableMouseEvents()` (`packages/core/src/utils/terminal.ts:24-29`, `bundle/chunk-DKZARV6U.js:279208-279210`, called from `bundle/interactiveCli-3OZSE6V3.js:35316-35322`) writes `\x1b[?1002h\x1b[?1006h`; teardown (`packages/core/src/utils/terminal.ts:13-22`, `packages/cli/src/ui/utils/terminalCapabilityManager.ts:31`) resets `?1000l`, `?1002l`, `?1003l`, `?1015l`, `?1006l`. **None** when `useAlternateBuffer: false` (default). | **Neither** `?1007` nor `?1h` is present in source or bundle. | **Yes** by default (`ui.useAlternateBuffer: false`); **No** when `ui.useAlternateBuffer: true` (`?1049h`). |
| **`grok`** (`AgentCliPreset::Grok`) | `/Users/jangtrinh/.grok/downloads/grok-1.0.70-macos-aarch64` (`xai-grok-pager` Rust/crossterm binary) | **Yes (`?1049h` / `?1049l`)**: `\x1b[?1049h` / `\x1b[?1049l` at `strings L4144343-4144344` (`crossterm::terminal::LeaveAlternateScreen` at `strings L4143759`; teardown strings also include `[?47l` at `strings L4154072` and `[?1047l` at `strings L4154177`). | **Yes (`?1000h`, `?1002h`, `?1003h` + `?1015h` + SGR `?1006h`)**: `strings L4144534-4144543` (`[?1000h`, `[?1002h`, `[?1003h`, `[?1015h`, `[?1006h`; mouse handler in `crates/codegen/xai-grok-pager/src/app/mouse.rs:925` at `strings L4144661` and `reassert_mouse_capture_on_focus` at `strings L4977870`; teardown at `strings L4154170-4154176` also resets `[?1005l` and `[?1016l`). | **UNKNOWN / None found** in binary strings (neither `?1007` nor `?1h` appears in `strings`). | **No** (`xai-grok-pager` renders its full conversation TUI inside `?1049h`). |
| **`agy`** (`AgentCliPreset::Antigravity`) | `/opt/homebrew/bin/agy` (Go Mach-O binary) | **Contains `?1049h` / `?1049l`** (`\x1b[?1049h` / `\x1b[?1049l` at `strings L2011534-2011535`, `L2012328`, binary offset `54828465`). | **Contains `?1002h` + `?1003h` + SGR `?1006h`** (`\x1b[?1002h\x1b[?1006h\x1b[?1003h\x1b[?1006h` at `strings L2012493-2012496`, binary offset `55618212`; teardown `\x1b[?1002l\x1b[?1003l\x1b[?1006l` at `strings L2012844-2012846`, offset `56210061`). | **UNKNOWN / None found** in binary strings (neither `?1007` nor `?1h` appears in `strings`). | **Yes** in standard main-screen transcript mode (emits `ESC[2J ESC[3J ESC[H` on resize on main screen, cf. `crates/designos-app/src/terminal/zoom_scrollback_tests.rs:11`); **No** when TUI mode enters `?1049h`. |

---

## 4. Root Cause (`file:line` Evidence)

In `crates/designos-app/src/terminal.rs:6484-6507` (on `origin/main` at `8dfaee931c75bd36293bc03266716b6abf99d868`), the terminal viewport wheel handler had four defects when a CLI or TUI owned the alternate screen (`?1049h`):

1. **Single-report throttling regardless of `smooth_scroll_delta.y` (`crates/designos-app/src/terminal.rs:6494-6498`):**
   When `alt_screen && mouse_active && !shift`, the handler wrote `format!("\x1b[<{button};{};{}M", c0 + 1, r0 + 1)` **once per UI frame**, ignoring the row magnitude `ceil(|delta_y| / ROW_H)` (whereas the main-screen scrollback branch at `:6501` scaled by `((delta_y.abs() / ROW_H).ceil() as usize).max(1)`). Multi-row wheel/trackpad gestures therefore scrolled only 1 line per frame in `?1049h` TUIs (`claude`, `codex` fullscreen, `gemini` alternate buffer, `grok`).
2. **Hard-coded SGR encoding (`crates/designos-app/src/terminal.rs:6497`):**
   The handler unconditionally formatted SGR `\x1b[<{button};{};{}M` without consulting `pg.screen().mouse_protocol_encoding()` (`vt100-0.15.2/src/screen.rs:344-346`), breaking programs in `?1049h` that enable mouse reporting (`?9h`/`?1000h`/`?1002h`/`?1003h`) with `Default` (X10 `CSI M Cb Cx Cy`) or `Utf8` (`?1005h`) encoding instead of `?1006h`.
3. **Dead wheel in `?1049h` when mouse reporting is off (`crates/designos-app/src/terminal.rs:6494, 6499-6505`):**
   When `alt_screen && !mouse_active` (for example `codex` in non-owned alternate-screen overlays which sets `\x1b[?1049h\x1b[?1007h` without mouse capture in `codex-rs/tui/src/tui/alternate_screen.rs:123-127`, `claude` with `CLAUDE_CODE_DISABLE_MOUSE=1`, `less`, or `vim`), execution fell into the `else if let Ok(mut pg) = tab.parser.lock()` branch and called `pg.set_scrollback(next_sb)`. Because `vt100`'s alternate grid has a 0-line scrollback buffer (`vt100-0.15.2/src/grid.rs`), `set_scrollback` clamped to `0` and nothing was written to the PTY instead of translating wheel steps into Up/Down cursor keys (`\x1b[A`/`\x1b[B` or `\x1bOA`/`\x1bOB` when `application_cursor()` is active).
4. **Shift+wheel in `?1049h` with mouse reporting fell into `set_scrollback` (`crates/designos-app/src/terminal.rs:6494, 6499-6505`):**
   Holding Shift while scrolling in `?1049h` with mouse reporting active skipped the PTY write as intended, but fell into the `else` branch and reset `tab.restore_scrollback` to `0` / attempted to mutate scrollback instead of being a pure no-op (`WheelAction::Noop`).

---

## 5. Acceptance Table & Verification Evidence

### 5.1 Red-Then-Green TDD (`wheel_modes_tests`) & `ac01` Guard Test

| Check | Command (Isolated Harness) | Result |
|---|---|---|
| **Red run (`test(terminal)` commit `d5caccde`)** | `cargo test -p designos-app --bin designos --locked wheel_modes_tests` | **1 passed (main-screen guard); 7 failed** (`plans/reports/agy-1173-terminal-wheel-evidence/red.log`) |
| **Green run (`fix(terminal)`)** | `cargo test -p designos-app --bin designos --locked wheel_modes_tests` | **8 passed; 0 failed** (`plans/reports/agy-1173-terminal-wheel-evidence/green.log`) |
| **`ac01` main-screen scrollback guard** | `cargo test -p designos-app --bin designos --locked ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card` | **1 passed; 0 failed** (`plans/reports/agy-1173-terminal-wheel-evidence/ac01.log`) |

#### `red.log` Output
```text
running 8 tests
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_default_mouse_encoding_sends_x10_reports ... FAILED
test terminal::wheel_modes_tests::wheel_on_main_screen_scrolls_studio_scrollback_even_with_mouse_reporting ... ok
test terminal::wheel_modes_tests::wheel_in_alt_screen_without_mouse_sends_cursor_keys ... FAILED
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_sgr_mouse_sends_one_sgr_report_per_row ... FAILED
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_application_cursor_sends_ss3_cursor_keys ... FAILED
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_utf8_mouse_encoding_sends_utf8_coordinates ... FAILED
test terminal::wheel_modes_tests::fast_wheel_steps_are_capped_per_frame ... FAILED
test terminal::wheel_modes_tests::shift_wheel_in_alt_screen_with_mouse_writes_nothing ... FAILED

test result: FAILED. 1 passed; 7 failed; 0 ignored; 0 measured; 3506 filtered out; finished in 0.01s
```

#### `green.log` Output
```text
running 8 tests
test terminal::wheel_modes_tests::fast_wheel_steps_are_capped_per_frame ... ok
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_application_cursor_sends_ss3_cursor_keys ... ok
test terminal::wheel_modes_tests::shift_wheel_in_alt_screen_with_mouse_writes_nothing ... ok
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_utf8_mouse_encoding_sends_utf8_coordinates ... ok
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_default_mouse_encoding_sends_x10_reports ... ok
test terminal::wheel_modes_tests::wheel_on_main_screen_scrolls_studio_scrollback_even_with_mouse_reporting ... ok
test terminal::wheel_modes_tests::wheel_in_alt_screen_with_sgr_mouse_sends_one_sgr_report_per_row ... ok
test terminal::wheel_modes_tests::wheel_in_alt_screen_without_mouse_sends_cursor_keys ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 3506 filtered out; finished in 0.00s
```

#### `ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card` Output
```text
running 1 test
test shell::tests::agent_turn_card_scroll::ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3513 filtered out; finished in 0.70s
```

### 5.2 Hex Dumps per Encoding (`plans/reports/agy-1173-terminal-wheel-evidence/hex-dumps.txt`)

```text
=== 1. SGR mouse encoding (?1049h + ?1000h + ?1006h), delta_rows=3 (up), cell=(r0=4, c0=9) -> col=10, row=5 ===
00000000: 1b5b 3c36 343b 3130 3b35 4d1b 5b3c 3634  .[<64;10;5M.[<64
00000010: 3b31 303b 354d 1b5b 3c36 343b 3130 3b35  ;10;5M.[<64;10;5
00000020: 4d                                       M

=== 2. SGR mouse encoding (?1049h + ?1000h + ?1006h), delta_rows=-2 (down), cell=(r0=0, c0=0) -> col=1, row=1 ===
00000000: 1b5b 3c36 353b 313b 314d 1b5b 3c36 353b  .[<65;1;1M.[<65;
00000010: 313b 314d                                1;1M

=== 3. Default X10 mouse encoding (?1049h + ?1000h), delta_rows=2 (up), cell=(r0=4, c0=9) -> col=10, row=5 ===
00000000: 1b5b 4d60 2a25 1b5b 4d60 2a25            .[M`*%.[M`*%

=== 4. Default X10 mouse encoding (?1049h + ?1000h), clamped past 223: delta_rows=1 (up), cell=(r0=249, c0=299) -> col=223, row=223 ===
00000000: 1b5b 4d60 ffff                           .[M`..

=== 5. UTF-8 mouse encoding (?1049h + ?1000h + ?1005h), delta_rows=2 (up), cell=(r0=4, c0=99) -> col=100 (U+0084 = c2 84), row=5 (0x25) ===
00000000: 1b5b 4d60 c284 251b 5b4d 60c2 8425       .[M`..%.[M`..%

=== 6. UTF-8 mouse encoding (?1049h + ?1000h + ?1005h), delta_rows=-1 (down), cell=(r0=199, c0=249) -> col=250 (U+011A = c4 9a), row=200 (U+00E8 = c3 a8) ===
00000000: 1b5b 4d61 c49a c3a8                      .[Ma....

=== 7. Alternate screen without mouse (?1049h, normal cursor ?1l), delta_rows=3 (up) and delta_rows=-2 (down) ===
00000000: 1b5b 411b 5b41 1b5b 41                   .[A.[A.[A
00000000: 1b5b 421b 5b42                           .[B.[B

=== 8. Alternate screen without mouse + application cursor (?1049h + ?1h), delta_rows=3 (up) and delta_rows=-2 (down) ===
00000000: 1b4f 411b 4f41 1b4f 41                   .OA.OA.OA
00000000: 1b4f 421b 4f42                           .OB.OB
```

### 5.3 Mandatory Workspace & Binary Gates

| Gate | Command | Result |
|---|---|---|
| **Rustfmt** | `cargo fmt --all -- --check` | **PASS (exit 0)** |
| **Clippy** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS (exit 0, 0 warnings)** |
| **Workspace lib tests** | `cargo test --workspace --locked --lib` | **PASS (exit 0: `13 + 671 + 1 + 153 + 143 + 30 + 4 + 37 + 255 + 56 + 1 + 8 = 1,372 passed; 0 failed`)** |
| **Zero Fake UI Gate** | `python3 scripts/zero_fake_ui_gate.py` | **PASS (`Gate Verdict: PASS (17/17 invariants satisfied)`)** |
| **Spacing Relationships Gate** | `python3 scripts/check_spacing_relationships.py` | **PASS (exit 0)** |
| **App binary test suite** | `cargo test -p designos-app --bin designos --locked` | **PASS (`test result: ok. 3502 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 418.92s`)** |

### 5.4 Investigation of Unrelated Parallel PTY Prompt Race in `agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled`

During the full `cargo test -p designos-app --bin designos --locked` run under default high thread concurrency with default `$SHELL` (`/bin/zsh`), `terminal::zoom_scrollback_tests::agents_approval_open_tab_has_no_scrollback_and_agents_scrollback_preset_is_scrolled` (`crates/designos-app/src/terminal/zoom_scrollback_tests.rs:612-650`, which does not invoke mouse wheel events) failed with `assertion left == right failed: agents-scrollback must contain all 120 history lines, got 119 (left: 119, right: 120)`, and passed `3/3` times when run in isolation (`0.35s`, `0.38s`, `0.39s`).

- **Root cause (`crates/designos-app/src/terminal.rs:1951, 2020-2036` & `crates/designos-app/src/terminal/zoom_scrollback_tests.rs:641-647`):**
  `open_approval_fixture("agents-scrollback", None)` calls `self.ensure_default_tab(file_path)` at `:1951` (spawning a live shell in a PTY) and then feeds `history` (`[001] Inspecting layer tokens and scrollback\r\n` .. `[120] ...\r\n`, with no leading `\r\n` before `[001]`) into `tab.parser` at `:2032`. Under heavy 3,500-test CPU contention, the background PTY reader thread can process the shell's initial prompt (which ends without `\n`) before `open_approval_fixture` locks `tab.parser`, concatenating `[001] Inspecting layer tokens and scrollback` onto the shell prompt line so `l.trim() == expected` at `zoom_scrollback_tests.rs:644` matches `119` (`[002]`..`[120]`) instead of `120`.
- Per the brief's strict scope boundaries (`terminal.rs:6484-6505`, pure helper, and `mod wheel_modes_tests;` only), `open_approval_fixture` and `zoom_scrollback_tests.rs` were left untouched and this finding is reported here for a separate fixture cleanup if desired.

---

## 6. Skills Loaded & Applied

- **`es:dev` & `es:feature-dev`:** Applied the Classify -> Plan -> Red Test (`test(terminal)`) -> Implement (`fix(terminal)`) -> Verify -> Report workflow with strict file-scope boundaries.
- **`es:rust`:** Extracted a zero-side-effect pure helper `wheel_bytes(screen: &vt100::Screen, delta_rows: isize, cell: (u16, u16), shift: bool) -> WheelAction` and released the `tab.parser` mutex guard before calling `tab.write_bytes(&bytes)` to preserve lock-order safety.
- **`es:rust-gpu-canvas`:** Kept viewport interaction handling in `render_vt100_viewport` allocation-minimal and bounded per-frame PTY writes with `MAX_WHEEL_STEPS_PER_FRAME = 10` so trackpad flicks cannot stall the UI frame or flood the child PTY buffer.
- **`es:astra-work`:** Recorded pre-fix failure evidence (`red.log`), post-fix verification (`green.log`, `ac01.log`, `hex-dumps.txt`), and verified all workspace gates under the isolated temporary-home harness.
- **`es:fable-thinking`:** Maintained an explicit epistemic ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), stated predictions before test runs, and re-modeled when two surprises occurred (`?1000h` mapping to `MouseProtocolMode::PressRelease` vs `?9h` `Press` in `vt100 0.15.2`, and the `119` vs `120` shell-prompt race in `open_approval_fixture("agents-scrollback")`).

---

## 7. Epistemic Ledger, Assumptions, Weakest Link & Unresolved Questions

### Epistemic Ledger & Assumptions
- **`FACT`:** `vt100 0.15.2` (`vt100-0.15.2/src/screen.rs:1255-1337`) tracks `?1` (`application_cursor`), `?9`/`?1000`/`?1002`/`?1003` (`mouse_protocol_mode`), `?1005`/`?1006` (`mouse_protocol_encoding`), and `?1049` (`alternate_screen`), while ignoring `?47`, `?1047`, `?1007`, and `?1015`.
- **`FACT`:** In `codex-rs/tui/src/tui/alternate_screen.rs:114-127`, Codex enables `?1000h`/`?1002h`/`?1003h`/`?1006h` in `?1049h` only when `capture_mouse` (`owned_screen`) is true, and otherwise enables `?1007h` (`EnableAlternateScroll`) with mouse reporting off (`mouse_protocol_mode() == None`).
- **`FACT`:** On the main screen (`!screen.alternate_screen()`), `wheel_bytes` always returns `WheelAction::Scrollback(delta_rows)`, preserving Studio's 5,000-line scrollback behavior even if a CLI leaves mouse reporting enabled.
- **`ASSUMPTION`:** None required beyond the controller rulings in `brief-agy-1173-terminal-wheel.txt` (all five design choices—main screen scrollback precedence, `MAX_WHEEL_STEPS_PER_FRAME = 10`, cursor-key fallback in `?1049h` without mouse, `Default`/`Utf8`/`Sgr` encodings, and Shift+wheel behavior—were explicitly ruled by the controller).

### Weakest Link
- **`vt100 0.15.2` ignores `?47h` / `?1047h` and `?1015h` (urxvt mouse encoding):** Any legacy third-party TUI that enters the alternate screen via `?47h`/`?1047h` instead of `?1049h` will report `screen.alternate_screen() == false` until `vt100` is updated or extended (none of the 5 preset agent CLIs use `?47h`/`?1047h` to enter the alternate screen).

### Unresolved Questions
- None.

Status: DONE

---

## r2 — Review Follow-Up, Contributor PR #1186 Point-by-Point Audit & Full #1173 Verification

### r2.1 What Could Not Be Verified

1. **Pre-existing Clippy `-D warnings` failures in two unrelated files on `origin/main` (`f566f16de`):**
   - Running `cargo clippy --workspace --all-targets --locked -- -D warnings` exits with code `101` (`plans/reports/agy-1173-terminal-wheel-evidence/r2-clippy.log:151`) due to `3` pre-existing warnings introduced on `origin/main` in files outside this PR's scope:
     - `clippy::needless_return` at `crates/designos-app/src/agent_permissions.rs:1176:9` (`r2-clippy.log:121-133`)
     - `clippy::needless_return` at `crates/designos-app/src/agent_permissions.rs:1194:9` (`r2-clippy.log:135-145`)
     - `clippy::type_complexity` at `crates/designos-app/src/shell/tests/inspector_var_binds.rs:1182:21` (`r2-clippy.log:149-158`)
   - Neither `crates/designos-app/src/terminal.rs` nor `crates/designos-app/src/terminal/wheel_modes_tests.rs` emits any clippy warning. Running `cargo clippy --workspace --all-targets --locked -- -D warnings -A clippy::needless-return -A clippy::type-complexity` (and the CI command `cargo clippy --workspace --all-targets --locked` from `.github/workflows/ci.yml:179` / `.github/workflows/pr-gate.yml:50`) passes with exit code `0` (`r2-clippy.log:152-171`, `CLIPPY_SCOPED_EXIT=0`).
2. **Interactive live-network agent CLI turn (`claude` / `codex` / `grok` / `gemini` / `agy`):**
   - Per the brief's hard constraint (`Never run brv, claude, codex, gemini, grok, or agy`), live remote-API agent CLI binaries were not invoked during testing. Instead, `render_vt100_viewport` was exercised against a live `portable-pty` tab (`crates/designos-app/src/terminal/wheel_modes_tests.rs:68-115`), `Sim` GUI integration tests (`crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:159-462`), and 1440 headless `--screenshot` captures on an APFS clone of the Untitled UI PRO VARIABLES v8.0 kit (`plans/reports/agy-1173-terminal-wheel-evidence/agents-panel-1440-expanded.png`, `plans/reports/agy-1173-terminal-wheel-evidence/agents-panel-1440-collapsed.png`).

---

### r2.2 Issue #1173 Point-by-Point Verification Table

| #1173 Requirement | Implementation Location | Verification Test / Visual Capture | Status |
|---|---|---|---|
| **1. Terminal mouse wheel in full-screen agent CLIs (`?1049h` alternate screen)** forwards wheel reports in active mouse encoding (`Sgr` `?1006h`, `Default`/X10, `Utf8` `?1005h` with `2015` coordinate clamp) or cursor keys (`\x1b[A`/`\x1b[B` normal, `\x1bOA`/`\x1bOB` `?1h` application cursor) when mouse reporting is off, capped at `MAX_WHEEL_STEPS_PER_FRAME = 10` | `crates/designos-app/src/terminal.rs:6500-6524`, `crates/designos-app/src/terminal.rs:6968-7044` | `terminal::wheel_modes_tests` (`9/9` pass in `plans/reports/agy-1173-terminal-wheel-evidence/r2-green.log:9-20`): `wheel_in_alt_screen_with_sgr_mouse_sends_one_sgr_report_per_row` (`r2-green.log:14`), `wheel_in_alt_screen_with_default_mouse_encoding_sends_x10_reports` (`r2-green.log:13`), `wheel_in_alt_screen_with_utf8_mouse_encoding_sends_utf8_coordinates` (`r2-green.log:15`), `wheel_in_alt_screen_with_utf8_mouse_encoding_clamps_coordinates_at_2015` (`r2-green.log:16`), `wheel_in_alt_screen_without_mouse_sends_cursor_keys` (`r2-green.log:17`), `wheel_in_alt_screen_with_application_cursor_sends_ss3_cursor_keys` (`r2-green.log:12`), `fast_wheel_steps_are_capped_per_frame` (`r2-green.log:11`), `shift_wheel_in_alt_screen_with_mouse_writes_nothing` (`r2-green.log:10`) | **PASS** |
| **2. Main-screen terminal mouse wheel** always scrolls Studio's 5,000-line scrollback buffer through `render_vt100_viewport` (even when a CLI leaves mouse reporting enabled) and never leaks mouse reports to the shell PTY | `crates/designos-app/src/terminal.rs:6502-6524`, `crates/designos-app/src/terminal.rs:6995-7001` | `terminal::wheel_modes_tests::wheel_on_main_screen_scrolls_studio_scrollback_even_with_mouse_reporting` (`crates/designos-app/src/terminal/wheel_modes_tests.rs:68-115`, `r2-green.log:18`) and `shell::tests::agent_turn_card_scroll::ac01_wheel_over_terminal_scrolls_scrollback_with_82_row_expanded_card` (`crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:159-169`, `r2-agent-turn-card-scroll.log:11`) | **PASS** |
| **3. Expanded "changed layers" card scrolls independently** when an agent turn modifies many layers (e.g., 82 layers) and the user clicks `Show all`, with clipped row list (`panels::clipped`), smooth wheel scrolling (`panels::wheel_scroll`), scrollbar thumb (`panels::scrollbar`), row selection on click after scroll, and AccessKit row-count label (`"82 changed layers"`) | `crates/designos-app/src/agent_turn_card.rs:367-453` | `shell::tests::agent_turn_card_scroll::ac02_wheel_over_expanded_card_scrolls_to_row_82_with_header_and_actions_visible` (`r2-agent-turn-card-scroll.log:12`), `edge_clicking_row_after_scrolling_selects_correct_layer` (`r2-agent-turn-card-scroll.log:15`), `a11y_list_region_announces_row_count_when_rows_off_screen` (`r2-agent-turn-card-scroll.log:10`), and 1440 capture `plans/reports/agy-1173-terminal-wheel-evidence/agents-panel-1440-expanded.png` | **PASS** |
| **4. Review card footer (`Show all` / `Show less`, `Revert`, `Keep`) and terminal viewport remain reachable** after a large agent edit: card reserve is capped at 50% of panel height while preserving at least 8 terminal rows (`min_term_h = (panel_h * 0.5).max(theme::LINE_BODY * 8.0)`), and footer controls stay pinned inside the bottom of the card | `crates/designos-app/src/panels/left.rs:15-16, 52-59, 69-81`, `crates/designos-app/src/agent_turn_card.rs:205-215, 373-388, 454-521` | `shell::tests::agent_turn_card_scroll::ac03_terminal_keeps_min_height_across_panel_heights_and_row_counts` (`panel_h ∈ {400, 900, 2000}` × `rows ∈ {5, 82, 300}`, `r2-agent-turn-card-scroll.log:18`), `ac04_show_less_and_keep_restore_terminal_height_next_frame_without_gap` (`r2-agent-turn-card-scroll.log:13`), `ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout` (`r2-agent-turn-card-scroll.log:14`), `turn_card_revert_and_keep_are_control_xs_secondary_buttons` (`r2-agent-turn-card-scroll.log:19`), and 1440 captures `agents-panel-1440-collapsed.png` & `agents-panel-1440-expanded.png` | **PASS** |

---

### r2.3 Part 1 — Review Findings Addressed & Red/Green Evidence

Both minor test findings from `plans/reports/review-1173-terminal-wheel-grok.md` were addressed in `crates/designos-app/src/terminal/wheel_modes_tests.rs` (and issue numbers were removed from comments in `crates/designos-app/src/terminal.rs:6501, 6969, 6972` and `crates/designos-app/src/terminal/wheel_modes_tests.rs:1-2`):

1. **UTF-8 (`?1005h`) coordinate coverage above column 250 and at/above the `2015` clamp (`crates/designos-app/src/terminal/wheel_modes_tests.rs:163-180`):**
   - Added `wheel_in_alt_screen_with_utf8_mouse_encoding_clamps_coordinates_at_2015` testing:
     - 0-based `(r0=4, c0=299)` -> 1-based `col=300` (`32 + 300 = 332 = U+014C` -> `[0xc5, 0x8c]`), `row=5` (`37 = 0x25`).
     - 0-based `(r0=2014, c0=2014)` -> 1-based `col=2015, row=2015` (`32 + 2015 = 2047 = U+07FF` -> `[0xdf, 0xbf]`).
     - 0-based `(r0=2400, c0=3000)` -> 1-based `col=3001, row=2401`, clamped by `col.min(2015)` and `row.min(2015)` at `crates/designos-app/src/terminal.rs:7023-7026` to `[0xdf, 0xbf, 0xdf, 0xbf]` for both wheel up (`button=64` -> `96`) and wheel down (`button=65` -> `97`).
2. **Main-screen scrollback driven through the real `render_vt100_viewport` wheel path (`crates/designos-app/src/terminal/wheel_modes_tests.rs:10-115`):**
   - Updated `wheel_on_main_screen_scrolls_studio_scrollback_even_with_mouse_reporting` to spawn a quiet stub tab in `TerminalState` (`spawn_quiet_tab_state`, lines `10-22`), warm up the viewport widget rect in `egui::Context` (`warmup_viewport`, lines `33-43`), populate 60 history lines with SGR mouse reporting active on the main screen (`\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h`), and dispatch `Event::MouseWheel` through `render_vt100_viewport` (`send_viewport_wheel`, lines `45-66`).
   - Verified that wheel up advances `parser.screen().scrollback()` to `up_rows` (`>= 3`), clears `tab.restore_scrollback` from `7` to `0`, and writes `0` bytes to the PTY; and wheel down reduces `parser.screen().scrollback()` to `(up_rows + down_rows)` while still writing `0` bytes to the PTY.
3. **Red -> Green Verification (`plans/reports/agy-1173-terminal-wheel-evidence/r2-red.log` & `r2-green.log`):**
   - **RED (`r2-red.log:17-44`):** With `pg.set_scrollback(next_sb)` at `crates/designos-app/src/terminal.rs:6514` and `.min(2015)` at `crates/designos-app/src/terminal.rs:7023,7026` temporarily faulted out:
     - `wheel_in_alt_screen_with_utf8_mouse_encoding_clamps_coordinates_at_2015` failed (`r2-red.log:17, 23-29`): `left: WritePty([27, 91, 77, 96, 224, 175, 153, 224, 166, 129])` vs `right: WritePty([27, 91, 77, 96, 223, 191, 223, 191])`.
     - `wheel_on_main_screen_scrolls_studio_scrollback_even_with_mouse_reporting` failed (`r2-red.log:18, 31-36`): `left: 0` vs `right: 3`.
   - **GREEN (`r2-green.log:9-20`):** With production code restored, all `9/9` tests in `terminal::wheel_modes_tests` pass (`test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 4551 filtered out; finished in 0.08s`, `r2-green.log:20`).

---

### r2.4 Part 2 — Contributor PR #1186 Point-by-Point Audit (`SKIP` with Evidence)

Contributor PR `#1186` by `@NamAIEngineer` (`fix(agents): keep changed-layer review footer reachable`, head `5c865a542`, 9 commits `1119c3c21..5c865a542` against `release/v0.7.0`) was fetched (`refs/remotes/contrib/pr-1186`) and audited point-by-point against `origin/main` (`f566f16de`), which already merged `#1276` (`ca1ef6f16`, `fix(agents): scroll expanded changed-layers card in Agents panel (#1173)`).

| PR #1186 Point (`1119c3c21..5c865a542`) | What PR #1186 Did | What `origin/main` (`#1276`, `ca1ef6f16`) Already Has | Verdict & Evidence |
|---|---|---|---|
| **1. Cap expanded card height & preserve terminal height** | Added `MAX_VISIBLE_ROWS: usize = 8` in `agent_turn_card.rs` (`compute_turn_card_height` capped at 8 rows) and changed `panels/left.rs` from `.min(inner.height() * 0.5)` to `.min(inner.height())` (which would allow the card to consume >50% of a 400 px panel and violate `AC-03`). | `crates/designos-app/src/panels/left.rs:15-16, 52-59` (`agent_card_reserve`) and `crates/designos-app/src/agent_turn_card.rs:205-215` cap the card reserve at `50%` of panel height (`PANEL_CARD_CAP_RATIO = 0.5`) while guaranteeing at least `8` terminal rows (`min_term_h = (panel_h * 0.5).max(theme::LINE_BODY * 8.0)`). | **SKIP (already covered & superseded on `main`)** — Verified across `panel_h ∈ {400, 900, 2000}` × `rows ∈ {5, 82, 300}` by `shell::tests::agent_turn_card_scroll::ac03_terminal_keeps_min_height_across_panel_heights_and_row_counts` (`crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:202-238`, `r2-agent-turn-card-scroll.log:18`). |
| **2. Scrollable changed-layer list inside expanded card** | Stored an integer row-slice index `start: usize` stepped by `±1` row per wheel frame via `expanded_layer_window`, without a scrollbar thumb or `Show less` action. | `crates/designos-app/src/agent_turn_card.rs:389-453` uses smooth pixel scrolling via `panels::wheel_scroll`, `panels::clipped`, and `panels::scrollbar`, plus adds `Show less` (`TurnCardAction::Collapse`, lines `374-388, 454-476, 551-555`) and AccessKit list region announcement (`"{row_count} changed layers"`, lines `449-452`). | **SKIP (already covered & superseded on `main`)** — Verified by `ac02_wheel_over_expanded_card_scrolls_to_row_82_with_header_and_actions_visible` (`r2-agent-turn-card-scroll.log:12`), `ac04_show_less_and_keep_restore_terminal_height_next_frame_without_gap` (`r2-agent-turn-card-scroll.log:13`), `ac05_three_row_card_has_no_scrollbar_and_preserves_base_layout` (`r2-agent-turn-card-scroll.log:14`), `edge_clicking_row_after_scrolling_selects_correct_layer` (`r2-agent-turn-card-scroll.log:15`), and `a11y_list_region_announces_row_count_when_rows_off_screen` (`r2-agent-turn-card-scroll.log:10`). |
| **3. Keep footer (`Show all`/`Show less`, `Revert`, `Keep`) reachable at bottom of card** | Anchored `buttons_top` and `show_all` at the bottom of `inner` below the 8-row slice. | `crates/designos-app/src/agent_turn_card.rs:373-388, 454-521` anchors `buttons_top = inner.bottom() - ACTION_H` and `toggle_top = buttons_top - ACTION_GAP - ROW_H` from the bottom of the clamped `card_rect` upward, keeping header and footer pinned while `list_rect` scrolls between them. | **SKIP (already covered on `main`)** — Verified by `ac02` (`r2-agent-turn-card-scroll.log:12`), `ac03` (`r2-agent-turn-card-scroll.log:18`), and 1440 captures `agents-panel-1440-collapsed.png` & `agents-panel-1440-expanded.png`. |
| **4. Per-tab and per-turn scroll state isolation (`5c865a542`)** | Scoped scroll state by `(tab_id, turn_id)` so two tabs with matching `turn.id` do not share scroll offset. | `#1276` already carried and credited `@NamAIEngineer`'s per-tab scroll scoping in `crates/designos-app/src/agent_turn_card.rs:20-86` (`TabScrollScope { instance_id, tab_idx, tab_id }` and `TurnScrollEntry { turn_id, created_at, scroll, pin_bottom }`). | **SKIP (already carried in `#1276` on `main`)** — Verified by `edge_scroll_state_isolated_per_tab_and_turn` (`crates/designos-app/src/shell/tests/agent_turn_card_scroll.rs:299-355`, `r2-agent-turn-card-scroll.log:20`) and `edge_new_turn_while_scrolled_resets_to_top_without_panic` (`r2-agent-turn-card-scroll.log:16`). |

---

### r2.5 1440 Visual Captures on APFS Clone of Untitled UI Kit

- **Idle check:** `HIDIdleTime = 15079s >= 300s`.
- **Kit clone:** APFS-cloned `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` to `out/evidence/untitled-clone.fig`, captured 1440x900 screenshots with `--no-toast --panel agents-turn-card` and `--no-toast --panel agents-turn-card-collapsed`, cropped strictly to the left rail + Agents panel (`448 x 900`, containing zero canvas or ruler pixels), inspected both images with `read_file`, and deleted `out/evidence/`:
  - `plans/reports/agy-1173-terminal-wheel-evidence/agents-panel-1440-collapsed.png` (`448 x 900`): Shows 27 terminal rows (`[54]..[79]` + prompt), composer, and collapsed `Claude Code changed layers 82` card (`Row 01`..`Row 05`, `Show all`, `Revert`, `Keep`).
  - `plans/reports/agy-1173-terminal-wheel-evidence/agents-panel-1440-expanded.png` (`448 x 900`): Shows 18 terminal rows (`[54]..[70]` + prompt), composer, and expanded `Claude Code changed layers 82` card capped at <= 50% panel height, scrolled to the bottom (`Row 73`..`Row 82`) with scrollbar thumb, pinned `Show less`, and pinned `Revert` + `Keep` buttons.

---

### r2.6 Mandatory Gates & Exit Codes

| Gate | Command | Exit Code & Evidence Citation |
|---|---|---|
| **Rustfmt** | `cargo fmt --all -- --check` | **Exit `0`** (`plans/reports/agy-1173-terminal-wheel-evidence/r2-gates.log:1`, `FMT_EXIT=0`) |
| **Clippy (strict `-D warnings`)** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **Exit `101` on `3` pre-existing `origin/main` warnings** in `agent_permissions.rs:1176,1194` and `inspector_var_binds.rs:1182` (`r2-clippy.log:121-151`, `CLIPPY_EXIT=101`); **Exit `0`** with `-A clippy::needless-return -A clippy::type-complexity` and on CI's `cargo clippy --workspace --all-targets --locked` (`r2-clippy.log:152-171`, `CLIPPY_SCOPED_EXIT=0`; `0` warnings in `terminal.rs` / `wheel_modes_tests.rs`) |
| **App binary tests (`designos`)** | `cargo test -p designos-app --bin designos --locked` | **Full parallel run:** `4521 passed; 23 failed; 16 ignored` (`r2-bin-designos-tests.log:4729`, `BIN_DESIGNOS_EXIT=101`; the `23` failures occurred only when `TEST_HOME` was placed under `/tmp`, which `agent_permissions::path_is_agent_temp` at `crates/designos-app/src/agent_permissions.rs:1217-1226` rejects as `"no private auto-allow log"`). **Rerun with `TEST_HOME` in repo-sibling directory (`$(dirname "$PWD")/dos-h.XXXXXX`) and short `TMPDIR` (`/tmp/dos-t.XXXXXX`):** `wheel_modes_tests` `9/9` pass (`r2-bin-designos-rerun.log:20`), `agent_turn_card_scroll` `11/11` pass (`r2-bin-designos-rerun.log:43`), `agent_permissions::tests` `47/47` pass (`r2-bin-designos-rerun.log:103`), `agent_turn_ledger::tests` `28/28` pass (`r2-bin-designos-rerun.log:143`), `terminal::ask_mode_tests` `25/25` pass (`r2-bin-designos-rerun.log:180`), **Exit `0`** (`r2-bin-designos-rerun.log:182`, `RERUN_EXIT=0`; `4,544 / 4,544` runnable tests pass) |
| **Workspace library tests** | `cargo test --workspace --locked --lib` | **Exit `0`** (`plans/reports/agy-1173-terminal-wheel-evidence/r2-workspace-lib-tests.log:1885`, `WORKSPACE_LIB_EXIT=0`: `13 + 816 + 7 + 1 + 178 + 159 + 30 + 5 + 38 + 377 + 82 + 2 + 8 = 1,716 passed; 0 failed; 3 ignored`) |
| **Zero Fake UI Gate** | `python3 scripts/zero_fake_ui_gate.py` | **Exit `0`** (`r2-gates.log:23-25`, `Gate Verdict: PASS (17/17 invariants satisfied)`, `ZERO_FAKE_UI_EXIT=0`) |
| **Spacing Relationships Gate** | `python3 -B scripts/check_spacing_relationships.py` | **Exit `0`** (`r2-gates.log:26-29`, `PASS: no count rose above the baseline`, `SPACING_EXIT=0`) |
| **DS Ontology Parity Gate** | `python3 -B scripts/check_ds_ontology_parity.py` | **Exit `0`** (`r2-gates.log:30-42`, `RESULT: PASS (P1, P2, P3, P4, P5, P6, P7, P9)`, `ONTOLOGY_EXIT=0`) |

---

### r2.7 Skills Loaded & Changes Made

- **`es:dev` / `es:feature-dev`:** Synced branch with `origin/main` (`4c509ffa9`), audited contributor PR `#1186` point-by-point against `origin/main` (`#1276`), wrote red-then-green tests for both Grok review findings, and verified all gates.
- **`es:rust` & `es:rust-gpu-canvas`:** Spawned `/bin/sleep 5` directly via `portable_pty::CommandBuilder` in `spawn_quiet_tab_state` (`crates/designos-app/src/terminal/wheel_modes_tests.rs:12-13`) so login-shell prompt bytes cannot race `tab.restore_scrollback` on the background PTY reader thread during `render_vt100_viewport` wheel tests; added `wheel_in_alt_screen_with_utf8_mouse_encoding_clamps_coordinates_at_2015` (`wheel_modes_tests.rs:163-180`).
- **`es:designer`:** Ran `find-design-context.sh . --target crates/designos-app/src/agent_turn_card.rs`, verified `check_spacing_relationships.py` and `check_ds_ontology_parity.py` pass without regressions, and inspected 1440 cropped Agents panel captures (`agents-panel-1440-collapsed.png`, `agents-panel-1440-expanded.png`).
- **`es:astra-work` & `es:fable-thinking`:** Diagnosed two test surprises via falsification:
  1. `egui 0.35.0` smooths `Event::MouseWheel` deltas across frames (`smooth_scroll_delta.y ≈ dy / 3` on a single 60 Hz frame) and requires a warmup frame after `theme::install_fonts(&ctx)` before hit-testing `vp_resp.hovered()`, solved by `warmup_viewport` and capturing `up_rows` / `down_rows` from `ui.input(|i| i.smooth_scroll_delta.y)` (`wheel_modes_tests.rs:33-66`).
  2. `agent_permissions::path_is_agent_temp` (`crates/designos-app/src/agent_permissions.rs:1217-1226`) rejects `HOME` paths inside `/tmp`, `/var/tmp`, or `$TMPDIR`; placing `TEST_HOME` in the repo sibling directory (`$(dirname "$PWD")/dos-h.XXXXXX`) resolved all 23 `no private auto-allow log` failures (`r2-bin-designos-rerun.log:53-182`).

---

### r2.8 Epistemic Ledger, Weakest Link & Closure

- **`FACT`:** `wheel_in_alt_screen_with_utf8_mouse_encoding_clamps_coordinates_at_2015` exercises 1-based column `300` (`> 250`), boundary `(2015, 2015)`, and clamped `(3001, 2401)` -> `[0xdf, 0xbf]` (`crates/designos-app/src/terminal/wheel_modes_tests.rs:163-180`, `r2-red.log:23-29`, `r2-green.log:16`).
- **`FACT`:** `wheel_on_main_screen_scrolls_studio_scrollback_even_with_mouse_reporting` drives scrolling through `render_vt100_viewport` with `Event::MouseWheel` (`crates/designos-app/src/terminal/wheel_modes_tests.rs:68-115`, `r2-red.log:31-36`, `r2-green.log:18`).
- **`FACT`:** Every point in contributor PR `#1186` (`1119c3c21..5c865a542`) is already covered and superseded on `origin/main` by `#1276` (`ca1ef6f16`), verified by `11/11` passing tests in `shell::tests::agent_turn_card_scroll` (`r2-agent-turn-card-scroll.log:10-22`).
- **`INFERENCE`:** Because both `#1276` (merged on `main`) and `#1337` (merged with `origin/main` at `4c509ffa9`) satisfy all four points of issue `#1173`, PR `#1337` can close `#1173`.
- **`ASSUMPTION`:** None.
- **Weakest Link:** The `3` pre-existing clippy warnings on `origin/main` (`crates/designos-app/src/agent_permissions.rs:1176,1194` and `crates/designos-app/src/shell/tests/inspector_var_binds.rs:1182`) cause `cargo clippy --workspace --all-targets --locked -- -D warnings` to exit `101` until cleaned up on `main` (CI's `.github/workflows/ci.yml:179` and `.github/workflows/pr-gate.yml:50` run without `-D warnings` and pass).

- **Status:** DONE
- **Summary:** Addressed both review findings in `crates/designos-app/src/terminal/wheel_modes_tests.rs` with red-then-green evidence (`r2-red.log`, `r2-green.log`), verified point-by-point that contributor PR `#1186` by `@NamAIEngineer` is already covered on `origin/main` by `#1276` (`SKIP` with evidence), captured and cropped 1440 Agents panel screenshots on an APFS clone of the Untitled UI kit, and verified all test and design gates.
- **Concerns:** `origin/main` (`f566f16de`) has `3` pre-existing clippy warnings in `crates/designos-app/src/agent_permissions.rs:1176,1194` (`needless_return`) and `crates/designos-app/src/shell/tests/inspector_var_binds.rs:1182` (`type_complexity`) outside this PR's file scope.

