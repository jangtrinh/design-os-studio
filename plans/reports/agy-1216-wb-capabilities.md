# Report: DEV · #1216 WB-1 — Replace `is_board()` Gates with Named Capability Checks

## 1. What Could NOT Be Verified

- **Dedicated `NodeKind::Table` on the AC-13 board document (`#1231 WB-16`):** `designos_doc::NodeKind` (`crates/designos-doc/src/lib.rs`) has no `Table` variant yet (tracked separately in `#1231 WB-16`). The AC-13 board fixture (`build_wb1_board_document` in `crates/designos-app/src/shell/tests/board.rs`) represents a table with a 2×2 `NodeKind::Frame` grid alongside `NodeKind::Section`, 3 `NodeKind::Sticky` notes, `NodeKind::ShapeWithText`, 2 `NodeKind::Connector` edges attached to stickies, and `NodeKind::Stamp`.
- **Serialized timer state in `.designos` (`App::board.timer`):** `FacilitatorTimer` (`crates/designos-app/src/board/mod.rs:342`) is runtime UI state on `App::board.timer` and is not serialized in `designos_doc::Document`. `ac13_wb1_open_render_save` starts and ticks `app.board.timer` during the `Sim` render pass before saving to `.designos`.
- **`agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` under macOS `$TMPDIR` sandbox:** Under the mandatory test harness (`T0="${TMPDIR:-/tmp}"`, which expands on macOS to `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T`), `$S/home` is 62 bytes and the derived Unix domain socket path `.../home/Library/Application Support/designos/ap/<16-hex>/codex.sock` is 130 bytes, exceeding macOS `SUN_LEN` (103 bytes). This single test fails identically on both unmodified `origin/main` (`0c8941235018ad09f6e6d1135400a9bcdf9041df`) and the head commit (`3964 passed; 1 failed; 13 ignored` before vs `3966 passed; 1 failed; 13 ignored` after).

---

## 2. Conclusion & Epistemic Ledger

### Conclusion
All 16 production `App::is_board()` call sites across `crates/designos-app/src/` have been replaced with 10 named capability methods in `crates/designos-app/src/board/caps.rs` (`wb_tools_enabled`, `wb_contextual_bar_enabled`, `wb_connectors_enabled`, `wb_section_defaults`, `wb_text_defaults`, `wb_dotted_background`, `dev_mode_available`, `frame_stepping_available`, `rulers_available`, `inspector_panel_visible`). `App::is_board()` remains the single file-kind query (`self.doc.kind == DocumentKind::Board`) and is called in production only from `crates/designos-app/src/board/caps.rs`. No behaviour changed: AC-13 saved files are byte-for-byte identical before and after, and 1440×900 captures of both `board.designos` (with `"Sticky 1"` selected) and the APFS-cloned Untitled UI kit (`kit.fig`) have `0` differing pixels (`SSIM = 1.0`).

### Epistemic Ledger
- **FACT:** `git grep -n 'is_board()' -- crates/designos-app/src | grep -v '/tests/' | grep -v 'board/caps.rs' | grep -v 'fn is_board'` returns empty output (exit code 1, 0 matches).
- **FACT:** `board::caps::tests::wb_capabilities_table_for_board_design_and_deck` passes for `DocumentKind::Board`, `DocumentKind::Design`, and `DocumentKind::Deck`, and fails immediately (`assertion left == right failed: Board: wb_tools_enabled`) when `wb_tools_enabled()` is mutated to `!self.is_board()`.
- **FACT:** AC-13 sha256 digests before (`0c894123`) and after are identical across all four documents:
  - `board.designos`: `3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf`
  - `designos-studio-landing.designos`: `3afd94482f64975ec4c18d668236a3e4f7be14a54a5cd7d088154083aa3a9f3b`
  - `frame-labels-1197.designos`: `3d5beaae9469bf61b6f6841a4ca7e8a7705a21aeac544a62dabbe1432ed2b8ae`
  - `kit.designos` (saved from APFS clone of Untitled UI PRO VARIABLES v8.0 `.fig`): `f6150b2dfde90c60d1a5f5519c9580af5446c5512fb3336031b5a5b8d55bdb43`
- **FACT:** `designos-diff compare --no-aa --json` on `out/evidence/board-{before,after}.png` (`88bffecc53c2f42874975850773d3708eaac74c22b0feac285055b026310fbbf`) and `out/evidence/kit-{before,after}.png` (`b2eb8173307579d9e01f08290c5979102ea8e36de49367ccafca554671acfff6`) reports `mismatched: 0`, `ratio: 0.0`, `ssim: 1.0`, `pass: true`.
- **FACT:** Full `cargo test -p designos-app --bin designos --locked` went from `3964 passed; 1 failed; 13 ignored` on base to `3966 passed; 1 failed; 13 ignored` on head (+2 new tests: `board::caps::tests::wb_capabilities_table_for_board_design_and_deck` and `shell::tests::board::ac13_wb1_open_render_save`).
- **INFERENCE:** None required for acceptance; all 7 acceptance criteria were directly observed on this machine.
- **ASSUMPTION:** Grouping the 16 call sites into 10 distinct semantic questions (`wb_tools_enabled`, `wb_contextual_bar_enabled`, `wb_connectors_enabled`, `wb_section_defaults`, `wb_text_defaults`, `wb_dotted_background`, `dev_mode_available`, `frame_stepping_available`, `rulers_available`, `inspector_panel_visible`) matches the granularity intended by Controller Ruling 1 ("One capability per question the code asks... Group sites that ask the same question under one method; do not make 16 one-use methods if fewer questions exist").
- **Weakest link:** `wb_section_defaults()` and `wb_text_defaults()` are separate capabilities rather than a single shape/text creation defaults capability because section styling (`280×180` white panel with `1px` `#DCDCDC` stroke) and default text size (`24.0` vs `DEFAULT_TEXT_SIZE`) are independent creation defaults in `canvas/create.rs`.

---

## 3. Skills Loaded & Impact

- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`) & `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Treated the brief and controller rulings as the approved plan; executed the DEV phase directly without pausing for brainstorm or spawning a verifier.
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Kept capability methods zero-allocation (`&self -> bool`) and inlineable on `App`, avoiding any borrowing or state overhead in hot canvas/input paths.
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Preserved exact background/tile paint ordering (`wb_dotted_background()` in `canvas/mod.rs` and `canvas/tiles.rs`) and connector overlay/hover checks (`wb_connectors_enabled()`), ensuring zero GPU/CPU render divergence.
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`) & `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Recorded baseline (`before`) test counts, AC-13 sha256 checksums, and idle-gated screenshots on unmodified `0c8941235018ad09f6e6d1135400a9bcdf9041df` prior to making any edits, and falsified the capability table test with a deliberate `!self.is_board()` mutation.

---

## 4. Site → Capability Table (16 Production Sites)

| # | `file:line` (base → head) | Old Expression | New Call | Why Identical |
|---|---|---|---|---|
| 1 | `crates/designos-app/src/board/bar.rs:203` | `!app.is_board()` | `!app.wb_contextual_bar_enabled()` | `wb_contextual_bar_enabled()` returns `self.is_board()`, so `!app.wb_contextual_bar_enabled()` is `!app.is_board()`. |
| 2 | `crates/designos-app/src/board/canvas.rs:55` | `!app.is_board()` | `!app.wb_connectors_enabled()` | `wb_connectors_enabled()` returns `self.is_board()`, so `!app.wb_connectors_enabled()` is `!app.is_board()`. |
| 3 | `crates/designos-app/src/board/connect.rs:45` | `!app.is_board()` | `!app.wb_connectors_enabled()` | `wb_connectors_enabled()` returns `self.is_board()`, so `!app.wb_connectors_enabled()` is `!app.is_board()`. |
| 4 | `crates/designos-app/src/canvas/create.rs:120` | `app.is_board()` | `app.wb_section_defaults()` | `wb_section_defaults()` returns `self.is_board()`. |
| 5 | `crates/designos-app/src/canvas/create.rs:198` | `app.is_board()` | `app.wb_text_defaults()` | `wb_text_defaults()` returns `self.is_board()`. |
| 6 | `crates/designos-app/src/canvas/interact.rs:712` | `app.is_board()` | `app.wb_connectors_enabled()` | `wb_connectors_enabled()` returns `self.is_board()`. |
| 7 | `crates/designos-app/src/canvas/interact.rs:2378` | `app.is_board()` | `app.wb_connectors_enabled()` | `wb_connectors_enabled()` returns `self.is_board()`. |
| 8 | `crates/designos-app/src/canvas/interact.rs:2415` | `app.is_board()` | `app.wb_connectors_enabled()` | `wb_connectors_enabled()` returns `self.is_board()`. |
| 9 | `crates/designos-app/src/canvas/mod.rs:393` | `app.is_board()` | `app.wb_dotted_background()` | `wb_dotted_background()` returns `self.is_board()`. |
| 10 | `crates/designos-app/src/canvas/tiles.rs:123` | `!app.is_board()` | `!app.wb_dotted_background()` | `wb_dotted_background()` returns `self.is_board()`, so `!app.wb_dotted_background()` is `!app.is_board()`. |
| 11 | `crates/designos-app/src/shell.rs:1119` | `app.is_board()` | `app.wb_tools_enabled()` | `wb_tools_enabled()` returns `self.is_board()`. |
| 12 | `crates/designos-app/src/shell.rs:1148` | `!app.is_board()` | `app.dev_mode_available()` | `dev_mode_available()` returns `!self.is_board()`. |
| 13 | `crates/designos-app/src/shell.rs:1156` | `!app.is_board()` | `app.frame_stepping_available()` | `frame_stepping_available()` returns `!self.is_board()`. |
| 14 | `crates/designos-app/src/shell.rs:1189` | `app.is_board()` | `app.wb_tools_enabled()` | `wb_tools_enabled()` returns `self.is_board()`. |
| 15 | `crates/designos-app/src/shell.rs:1203` (`1203–1204` after `rustfmt`) | `!self.app.is_board()` | `self.app.rulers_available()` | `rulers_available()` returns `!self.is_board()`. |
| 16 | `crates/designos-app/src/shell.rs:1788` (`1791` after `rustfmt`) | `let board = self.app.is_board();` | `let board = !self.app.inspector_panel_visible();` | `inspector_panel_visible()` returns `!self.is_board()`, so `!self.app.inspector_panel_visible()` is `!!self.app.is_board() == self.app.is_board()`. |

---

## 5. Acceptance Table & Verbatim Outputs

| # | Acceptance Check | Result | Evidence |
|---|---|---|---|
| 1 | `git grep -n 'is_board()' -- crates/designos-app/src \| grep -v '/tests/' \| grep -v 'board/caps.rs' \| grep -v 'fn is_board'` prints nothing | **PASS** | Empty output (see §5.1) |
| 2 | Capability table test passes for Board, Design, Deck; fails with one capability flipped (`!self.is_board()`) | **PASS** | `cap-table-pass.log` & `mutant-run.log` (see §5.2) |
| 3 | AC-13 byte-stability: sha256 before and after equal for `board.designos`, `designos-studio-landing.designos`, `frame-labels-1197.designos`, `kit.designos` | **PASS** | `ac13-before.txt` & `ac13-after.txt` (see §5.3) |
| 4a | `cargo fmt --all -- --check` exits 0 | **PASS** | Exit code 0 (see §5.4) |
| 4b | `cargo clippy --workspace --all-targets --locked -- -D warnings` shows only `main`'s known errors | **PASS** | `clippy.log` (see §5.4) |
| 4c | `cargo test -p designos-app --bin designos --locked` before/after counts match apart from new tests | **PASS** | Before: `3964 passed; 1 failed; 13 ignored`; After: `3966 passed; 1 failed; 13 ignored` (see §5.4) |
| 4d | `cargo test --workspace --locked --lib` passes | **PASS** | All 12 workspace lib test suites `ok` (see §5.4) |
| 5 | Captures (`board-{before,after}.png` with `--select "Sticky 1"` and `kit-{before,after}.png`) pixel-equal | **PASS** | `0` mismatched pixels (`SSIM = 1.0`) on both 1440×900 pairs (see §5.5) |
| 6 | PR opened against `main` | **PASS** | https://github.com/jangtrinh/design-os-studio/pull/1460 (see §5.6) |
| 7 | `out/cargo` deleted at the end | **PASS** | Verified removed (see §5.7) |

### 5.1 Acceptance #1 — `git grep` for `is_board()`
Command:
```sh
git grep -n 'is_board()' -- crates/designos-app/src | grep -v '/tests/' | grep -v 'board/caps.rs' | grep -v 'fn is_board'
```
Output:
```text
(empty output; exit code 1)
```

All remaining `is_board()` occurrences across `crates/designos-app/src`:
```text
crates/designos-app/src/board/caps.rs:11:        self.is_board()
crates/designos-app/src/board/caps.rs:17:        self.is_board()
crates/designos-app/src/board/caps.rs:23:        self.is_board()
crates/designos-app/src/board/caps.rs:29:        self.is_board()
crates/designos-app/src/board/caps.rs:35:        self.is_board()
crates/designos-app/src/board/caps.rs:41:        self.is_board()
crates/designos-app/src/board/caps.rs:47:        !self.is_board()
crates/designos-app/src/board/caps.rs:53:        !self.is_board()
crates/designos-app/src/board/caps.rs:59:        !self.is_board()
crates/designos-app/src/board/caps.rs:65:        !self.is_board()
crates/designos-app/src/board/mod.rs:183:    pub fn is_board(&self) -> bool {
crates/designos-app/src/shell/tests/board.rs:56:    assert!(s.app.is_board());
crates/designos-app/src/shell/tests/board.rs:274:    assert!(s.app.is_board());
crates/designos-app/src/shell/tests/board.rs:292:    assert!(!s.app.is_board(), "standard design file should not be a board");
crates/designos-app/src/shell/tests/board_home.rs:45:    assert!(shell.app.is_board());
crates/designos-app/src/shell/tests/dev_mode_shortcut.rs:46:    assert!(!s.app.is_board());
crates/designos-app/src/shell/tests/dev_mode_shortcut.rs:84:    assert!(s.app.is_board());
```

### 5.2 Acceptance #2 — Capability Table Test & Mutant Run
Command (Pass):
```sh
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked wb_capabilities_table_for_board_design_and_deck
```
Output (`plans/reports/agy-1216-wb-capabilities-evidence/cap-table-pass.log`):
```text
     Running unittests src/main.rs (out/cargo/debug/deps/designos-2d905839eaba8cb2)

running 1 test
test board::caps::tests::wb_capabilities_table_for_board_design_and_deck ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3979 filtered out; finished in 0.02s
```

Command (Mutant run with `wb_tools_enabled` flipped to `!self.is_board()`):
```sh
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked wb_capabilities_table_for_board_design_and_deck
```
Output (`plans/reports/agy-1216-wb-capabilities-evidence/mutant-run.log`):
```text
     Running unittests src/main.rs (out/cargo/debug/deps/designos-2d905839eaba8cb2)

running 1 test
test board::caps::tests::wb_capabilities_table_for_board_design_and_deck ... FAILED

failures:

---- board::caps::tests::wb_capabilities_table_for_board_design_and_deck stdout ----

thread 'board::caps::tests::wb_capabilities_table_for_board_design_and_deck' (48475876) panicked at crates/designos-app/src/board/caps.rs:82:13:
assertion `left == right` failed: Board: wb_tools_enabled
  left: false
 right: true
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    board::caps::tests::wb_capabilities_table_for_board_design_and_deck

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3979 filtered out; finished in 0.02s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

### 5.3 Acceptance #3 — AC-13 Byte-Stability (Before vs After sha256)

| File | Before (`0c8941235018`) sha256 | After (head) sha256 | Equal? |
|---|---|---|---|
| `board.designos` (source fixture) | `3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf` | `3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf` | **YES** |
| `board.designos` (opened, rendered with timer ticking, saved) | `3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf` | `3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf` | **YES** |
| `designos-studio-landing.designos` | `3afd94482f64975ec4c18d668236a3e4f7be14a54a5cd7d088154083aa3a9f3b` | `3afd94482f64975ec4c18d668236a3e4f7be14a54a5cd7d088154083aa3a9f3b` | **YES** |
| `frame-labels-1197.designos` | `3d5beaae9469bf61b6f6841a4ca7e8a7705a21aeac544a62dabbe1432ed2b8ae` | `3d5beaae9469bf61b6f6841a4ca7e8a7705a21aeac544a62dabbe1432ed2b8ae` | **YES** |
| `kit.designos` (opened from APFS clone `$S/kit.fig`, rendered, saved) | `f6150b2dfde90c60d1a5f5519c9580af5446c5512fb3336031b5a5b8d55bdb43` | `f6150b2dfde90c60d1a5f5519c9580af5446c5512fb3336031b5a5b8d55bdb43` | **YES** |

Verbatim `plans/reports/agy-1216-wb-capabilities-evidence/ac13-before.txt`:
```text
3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/board.designos
3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/before/board.designos
3afd94482f64975ec4c18d668236a3e4f7be14a54a5cd7d088154083aa3a9f3b  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/before/designos-studio-landing.designos
3d5beaae9469bf61b6f6841a4ca7e8a7705a21aeac544a62dabbe1432ed2b8ae  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/before/frame-labels-1197.designos
f6150b2dfde90c60d1a5f5519c9580af5446c5512fb3336031b5a5b8d55bdb43  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/before/kit.designos
```

Verbatim `plans/reports/agy-1216-wb-capabilities-evidence/ac13-after.txt`:
```text
3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/board.designos
3dda5f6f3902340d5c4e7bd76b1921d3f22716f6f67feb86e256bfea11bb9bcf  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/after/board.designos
3afd94482f64975ec4c18d668236a3e4f7be14a54a5cd7d088154083aa3a9f3b  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/after/designos-studio-landing.designos
3d5beaae9469bf61b6f6841a4ca7e8a7705a21aeac544a62dabbe1432ed2b8ae  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/after/frame-labels-1197.designos
f6150b2dfde90c60d1a5f5519c9580af5446c5512fb3336031b5a5b8d55bdb43  /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.OTRaov/after/kit.designos
```

### 5.4 Acceptance #4 — Gates Under the Harness

#### `cargo fmt --all -- --check`
Command:
```sh
cargo fmt --all -- --check
```
Output:
```text
(empty output; exit code 0)
```

#### `cargo clippy --workspace --all-targets --locked -- -D warnings`
Command:
```sh
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings > plans/reports/agy-1216-wb-capabilities-evidence/clippy.log 2>&1
grep -E '^(error|warning)' plans/reports/agy-1216-wb-capabilities-evidence/clippy.log
```
Output:
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1216-wb-capabilities/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1216-wb-capabilities/crates/designos-app/src/main.rs` found to be present in multiple build targets:
error: unneeded `return` statement
error: unneeded `return` statement
error: writing `&mut Vec` instead of `&mut [_]` involves a new object where a slice will do
error: could not compile `designos-app` (bin "designos") due to 3 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `designos-app` (bin "designos-studio") due to 3 previous errors
error: items after a test module
error: unused return value of `egui::Context::run_ui` that must be used
error: could not compile `designos-app` (bin "designos" test) due to 5 previous errors
error: could not compile `designos-app` (bin "designos-studio" test) due to 5 previous errors
```
(Only `main`'s known errors in `agent_permissions.rs:1163,1181` and `panels/components.rs` appear.)

#### `cargo test -p designos-app --bin designos --locked` (Before vs After)
Before (`plans/reports/agy-1216-wb-capabilities-evidence/test-before.log`):
```text
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root

test result: FAILED. 3964 passed; 1 failed; 13 ignored; 0 measured; 0 filtered out; finished in 570.99s
```
After (`plans/reports/agy-1216-wb-capabilities-evidence/test-after.log`):
```text
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root

test result: FAILED. 3966 passed; 1 failed; 13 ignored; 0 measured; 0 filtered out; finished in 392.08s
```

#### `cargo test --workspace --locked --lib`
Command:
```sh
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
```
Summary output (`plans/reports/agy-1216-wb-capabilities-evidence/test-workspace-lib.log`):
```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 674 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.74s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.18s
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.52s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.48s
test result: ok. 296 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.64s
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.10s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
```

### 5.5 Acceptance #5 — Visual Captures & Pixel Diff
- `HIDIdleTime` before captures: `405 s` (`board-before.png`), `348 s` (`kit-before.png`), `1324 s` (`board-after.png`), `1327 s` (`kit-after.png`) — all `>= 300 s`.
- Commands:
  ```sh
  out/cargo/debug/designos "$S/board.designos" --size 1440x900 --no-toast --select "Sticky 1" --screenshot out/evidence/board-before.png
  out/cargo/debug/designos "$S/kit.fig" --size 1440x900 --no-toast --screenshot out/evidence/kit-before.png
  out/cargo/debug/designos "$S/board.designos" --size 1440x900 --no-toast --select "Sticky 1" --screenshot out/evidence/board-after.png
  out/cargo/debug/designos "$S/kit.fig" --size 1440x900 --no-toast --screenshot out/evidence/kit-after.png
  out/cargo/debug/designos-diff compare --no-aa --json --out out/evidence/board-diff.png out/evidence/board-before.png out/evidence/board-after.png
  out/cargo/debug/designos-diff compare --no-aa --json --out out/evidence/kit-diff.png out/evidence/kit-before.png out/evidence/kit-after.png
  ```
- Verbatim `plans/reports/agy-1216-wb-capabilities-evidence/pixel-diff.log`:
  ```text
  board: size=1440x900 total_px=1296000 diff_px=0 (0.0000%) sha256_before=88bffecc53c2f42874975850773d3708eaac74c22b0feac285055b026310fbbf sha256_after=88bffecc53c2f42874975850773d3708eaac74c22b0feac285055b026310fbbf
  kit: size=1440x900 total_px=1296000 diff_px=0 (0.0000%) sha256_before=b2eb8173307579d9e01f08290c5979102ea8e36de49367ccafca554671acfff6 sha256_after=b2eb8173307579d9e01f08290c5979102ea8e36de49367ccafca554671acfff6
  === designos-diff compare board-before.png board-after.png ===
  {"aa_pixels":0,"height":900,"ignored":0,"mismatched":0,"pass":true,"ratio":0.0,"size_a":[1440,900],"size_b":[1440,900],"size_mismatch":false,"ssim":1.0,"threshold":0.0,"width":1440}
  === designos-diff compare kit-before.png kit-after.png ===
  {"aa_pixels":0,"height":900,"ignored":0,"mismatched":0,"pass":true,"ratio":0.0,"size_a":[1440,900],"size_b":[1440,900],"size_mismatch":false,"ssim":1.0,"threshold":0.0,"width":1440}
  ```

### 5.6 Acceptance #6 — Pull Request URL
- PR URL: https://github.com/jangtrinh/design-os-studio/pull/1460

### 5.7 Acceptance #7 — Delete `out/cargo`
- `rm -rf out/cargo` executed after all builds/tests/diffs completed; verified `test ! -e out/cargo`.

---

## 6. Assumptions & Unresolved Questions

- **Assumptions:**
  1. The two canvas background call sites (`crates/designos-app/src/canvas/mod.rs:393` and `crates/designos-app/src/canvas/tiles.rs:123`) ask the same question ("does the canvas paint the whiteboard dotted background under transparent page tiles instead of the solid canvas background fill?") and are paired under `wb_dotted_background()`.
  2. The four connector interaction/overlay sites (`board/canvas.rs:55`, `board/connect.rs:45`, `canvas/interact.rs:712`, `:2378`, `:2415`) ask the same question ("are whiteboard connector magnet dots, hover tracking, and drag interactions active?") and are grouped under `wb_connectors_enabled()`.
  3. The two keyboard tool/Tab-sticky sites (`shell.rs:1119` and `shell.rs:1189`) are grouped under the issue-mandated `wb_tools_enabled()`.
- **Unresolved questions:** None.

Status: DONE — all 16 production is_board() sites replaced with named capabilities in board/caps.rs; AC-13 byte-stable, captures pixel-identical, gates green.
