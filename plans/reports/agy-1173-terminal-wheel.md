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
