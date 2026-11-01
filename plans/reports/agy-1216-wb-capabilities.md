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
- **Weakest link:** `wb_section_defaults()` and `wb_text_defaults()` are separate capabilities rather than a single shape/text creation defaults capability because section styling in `canvas/create.rs:117-129` (`Color::WHITE` fill with `1.0px` `#CFCFCF` inside stroke and `Vec2::ZERO` initial size vs `#F5F5F5` fill with no stroke) and default text size in `canvas/create.rs:198` (`24.0` vs `theme::DEFAULT_TEXT_SIZE` `16.0`) are independent creation defaults.

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

All remaining `is_board()` occurrences across `crates/designos-app/src` (Round 1 committed state `176c0cb5`):
```text
crates/designos-app/src/board/caps.rs:11:        self.is_board()
crates/designos-app/src/board/caps.rs:16:        self.is_board()
crates/designos-app/src/board/caps.rs:21:        self.is_board()
crates/designos-app/src/board/caps.rs:26:        self.is_board()
crates/designos-app/src/board/caps.rs:31:        self.is_board()
crates/designos-app/src/board/caps.rs:36:        self.is_board()
crates/designos-app/src/board/caps.rs:41:        !self.is_board()
crates/designos-app/src/board/caps.rs:46:        !self.is_board()
crates/designos-app/src/board/caps.rs:51:        !self.is_board()
crates/designos-app/src/board/caps.rs:56:        !self.is_board()
crates/designos-app/src/board/mod.rs:183:    pub fn is_board(&self) -> bool {
crates/designos-app/src/shell/tests/board.rs:62:    assert!(s.app().is_board());
crates/designos-app/src/shell/tests/board.rs:72:    assert!(!d.app().is_board());
crates/designos-app/src/shell/tests/board.rs:1092:            if s.app().is_board() {
crates/designos-app/src/shell/tests/board_home.rs:36:    assert!(s.app().is_board());
crates/designos-app/src/shell/tests/board_home.rs:58:    assert!(!s.app().is_board());
crates/designos-app/src/shell/tests/board_home.rs:83:    assert!(s.app().is_board(), "File > New board opens an empty board");
crates/designos-app/src/shell/tests/dev_mode_shortcut.rs:130:    assert!(s.app().is_board(), "precondition: a board has no </> button");
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

---

## Round 2

### 1. What Could NOT Be Verified
- **Connector following at `crates/designos-app/src/board/connect.rs:158` (`reroute_touched`):** `if doc.kind != DocumentKind::Board { return; }` takes `&mut Document` rather than `&App` and remains gated on `DocumentKind::Board`. Per Controller Ruling 7, the controller files this under WB-7 (not in this PR).
- **Board `Tab` non-sticky sibling fallthrough at `crates/designos-app/src/shell.rs:1119`:** `crate::board::create::tab_next(app)` returns a `bool` that is currently ignored on boards, so pressing `Tab` on a non-sticky layer in a board does not fall through to `app.select_sibling(true)`. Per Controller Ruling 7, this is deferred to a later PR.
- **`agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` under the macOS `$TMPDIR` sibling harness:** Fails identically on `origin/main` (`64f2dd07`) and on `HEAD` because `$S/home` under `/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.XXXXXX/home` produces a 130-byte Unix domain socket path (`> SUN_LEN = 103`).

### 2. Conclusion & Epistemic Ledger
- **Conclusion:** Applied all Round 2 controller rulings (M1, M2, M3, L1, L2, L3, PR body update) after merging `origin/main` (`64f2dd07` -> merge commit `08f494a8`). `crates/designos-app/src/board/caps.rs` now defines 11 single-question capabilities (`wb_tools_enabled`, `board_key_map`, `wb_contextual_bar_enabled`, `wb_connectors_enabled`, `wb_section_defaults`, `wb_text_defaults`, `wb_dotted_background`, `board_chrome`, `dev_mode_available`, `frame_stepping_available`, `ruler_strip_skips_canvas_menu`). The phantom `ac13_wb1_open_render_save` test and `build_wb1_board_document` helper were deleted and replaced with `section_and_text_creation_defaults_across_document_kinds` plus three additional behaviour tests (`dotted_background_paints_on_boards_and_solid_page_fill_on_design_canvas`, `frame_stepping_shortcut_steps_top_level_frames_only_when_available`, `right_click_on_ruler_strip_skips_canvas_menu_only_when_enabled`). Every single-capability flip is killed by both `wb_capabilities_table_for_board_design_and_deck` and at least one behaviour test.
- **Epistemic Ledger:**
  - **FACT:** `git grep -n 'is_board()' -- crates/designos-app/src | grep -v '/tests/' | grep -v 'board/caps.rs' | grep -v 'fn is_board'` prints nothing (exit code 1).
  - **FACT:** Flipping each of the 11 capabilities alone in `crates/designos-app/src/board/caps.rs` causes `board::caps::tests::wb_capabilities_table_for_board_design_and_deck` to fail (`11/11` `KILLED`) AND causes that capability's behaviour test(s) to fail (`1` to `20` failing behaviour tests per capability; `plans/reports/agy-1216-wb-capabilities-evidence/r2-mutants.log`).
  - **FACT:** `cargo fmt --all -- --check` exits `0`; `cargo clippy --workspace --all-targets --locked -- -D warnings` reports only `origin/main`'s 2 remaining errors in `crates/designos-app/src/agent_permissions.rs:1163,1181` (after `#1417` on `origin/main` fixed the 3 in `panels/components.rs`); `cargo test -p designos-app --bin designos --locked` runs `3973 passed; 1 failed; 13 ignored` (`origin/main` `3968` + `5` new tests on this branch); `cargo test --workspace --locked --lib` passes all 12 workspace lib suites (`1474 passed; 0 failed`).
  - **INFERENCE:** None required for acceptance; all Round 2 acceptance checks were executed and observed directly.
  - **ASSUMPTION:** Naming the `shell.rs:1204` capability `ruler_strip_skips_canvas_menu()` directly states what the call site decides ("whether a right-click on the ruler strip skips the canvas menu", Ruling 4 / L1).
  - **Weakest link:** `board_chrome()` at `shell.rs:1791` still gates five editor chrome surfaces together (right panel, toolbar choice, contextual bar, zoom pill, Specs reader) until WB-3 / WB-5 split their draw order.

### 3. Skills Loaded & Impact (Round 2)
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`) & `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`): Executed the Round 2 rulings directly as the approved DEV spec without pausing for brainstorm or spawning a sub-verifier.
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`): Kept all 11 capability methods on `App` zero-allocation (`&self -> bool`) and wrote deterministic unit/UI behaviour tests without plan/issue IDs in symbol names or comments.
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Verified `wb_dotted_background()` behaviour via the emitted `egui::Shape::Mesh` (`theme::BOARD_DOT` vertices) in `Sim` without perturbing tile/GPU rendering paths.
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`) & `es:astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`): Falsified all 11 capabilities individually with single-flip mutation runs (`r2-mutants.log`) and corrected the two L2 report discrepancies from the code.

### 4. L2 Corrections from Code
1. **`crates/designos-app/src/board/caps.rs` grep line numbers:** In Round 1 (`176c0cb5`), `rustfmt` collapsed single-line `{ self.is_board() }` bodies to 3-line methods at lines `11, 16, 21, 26, 31, 36, 41, 46, 51, 56` (updated in §5.1 above). In Round 2 (`f76a626d`), with 11 capabilities in `crates/designos-app/src/board/caps.rs`, the `self.is_board()` lines are `11, 16, 21, 26, 31, 36, 41, 46, 51, 56, 61` (see §6.1 below and `plans/reports/agy-1216-wb-capabilities-evidence/r2-grep-is-board.txt`).
2. **`wb_section_defaults()` values in `crates/designos-app/src/canvas/create.rs:117-129`:** `create.rs` does not set a `280×180` size or `#DCDCDC` stroke. Every shape returned by `canvas::create::new_shape` has `n.size = Vec2::ZERO` (`create.rs:170`). For `Tool::Section`, non-board documents set `s.fills = vec![Paint::solid(Color::rgb(0xF5, 0xF5, 0xF5))]` with an empty stroke, and when `app.wb_section_defaults()` is `true`, `create.rs:120-126` sets `s.fills = vec![Paint::solid(Color::WHITE)]`, `s.stroke.paints = vec![Paint::solid(Color::rgb(0xCF, 0xCF, 0xCF))]`, `s.stroke.weight = 1.0`, and `s.stroke.align = StrokeAlign::Inside`.

### 5. Updated Site → Capability Table (16 Production Sites, 11 Capabilities)

| # | `file:line` | Old Expression | Round 2 Capability Call | Value (Board / Design / Deck) |
|---|---|---|---|---|
| 1 | `crates/designos-app/src/board/bar.rs:203` | `!app.is_board()` | `!app.wb_contextual_bar_enabled()` | `false` / `true` / `true` |
| 2 | `crates/designos-app/src/board/canvas.rs:55` | `!app.is_board()` | `!app.wb_connectors_enabled()` | `false` / `true` / `true` |
| 3 | `crates/designos-app/src/board/connect.rs:45` | `!app.is_board()` | `!app.wb_connectors_enabled()` | `false` / `true` / `true` |
| 4 | `crates/designos-app/src/canvas/create.rs:120` | `app.is_board()` | `app.wb_section_defaults()` | `true` / `false` / `false` |
| 5 | `crates/designos-app/src/canvas/create.rs:198` | `app.is_board()` | `app.wb_text_defaults()` | `true` / `false` / `false` |
| 6 | `crates/designos-app/src/canvas/interact.rs:712` | `app.is_board()` | `app.wb_connectors_enabled()` | `true` / `false` / `false` |
| 7 | `crates/designos-app/src/canvas/interact.rs:2378` | `app.is_board()` | `app.wb_connectors_enabled()` | `true` / `false` / `false` |
| 8 | `crates/designos-app/src/canvas/interact.rs:2415` | `app.is_board()` | `app.wb_connectors_enabled()` | `true` / `false` / `false` |
| 9 | `crates/designos-app/src/canvas/mod.rs:393` | `app.is_board()` | `app.wb_dotted_background()` | `true` / `false` / `false` |
| 10 | `crates/designos-app/src/canvas/tiles.rs:123` | `!app.is_board()` | `!app.wb_dotted_background()` | `false` / `true` / `true` |
| 11 | `crates/designos-app/src/shell.rs:1119` | `app.is_board()` | `app.wb_tools_enabled()` | `true` / `false` / `false` |
| 12 | `crates/designos-app/src/shell.rs:1148` | `!app.is_board()` | `app.dev_mode_available()` | `false` / `true` / `true` |
| 13 | `crates/designos-app/src/shell.rs:1156` | `!app.is_board()` | `app.frame_stepping_available()` | `false` / `true` / `true` |
| 14 | `crates/designos-app/src/shell.rs:1189` | `app.is_board()` | `app.board_key_map()` | `true` / `false` / `false` |
| 15 | `crates/designos-app/src/shell.rs:1204` | `!self.app.is_board()` | `self.app.ruler_strip_skips_canvas_menu()` | `false` / `true` / `true` |
| 16 | `crates/designos-app/src/shell.rs:1791` | `self.app.is_board()` | `self.app.board_chrome()` | `true` / `false` / `false` |

### 6. Round 2 Acceptance Table & Verbatim Outputs

#### 6.1 Acceptance #1 — `git grep` for `is_board()`
Command:
```sh
git grep -n 'is_board()' -- crates/designos-app/src | grep -v '/tests/' | grep -v 'board/caps.rs' | grep -v 'fn is_board'
```
Output:
```text
(empty output; exit code 1)
```

All `is_board()` occurrences across `crates/designos-app/src` (`plans/reports/agy-1216-wb-capabilities-evidence/r2-grep-is-board.txt`):
```text
crates/designos-app/src/board/caps.rs:11:        self.is_board()
crates/designos-app/src/board/caps.rs:16:        self.is_board()
crates/designos-app/src/board/caps.rs:21:        self.is_board()
crates/designos-app/src/board/caps.rs:26:        self.is_board()
crates/designos-app/src/board/caps.rs:31:        self.is_board()
crates/designos-app/src/board/caps.rs:36:        self.is_board()
crates/designos-app/src/board/caps.rs:41:        self.is_board()
crates/designos-app/src/board/caps.rs:46:        self.is_board()
crates/designos-app/src/board/caps.rs:51:        !self.is_board()
crates/designos-app/src/board/caps.rs:56:        !self.is_board()
crates/designos-app/src/board/caps.rs:61:        !self.is_board()
crates/designos-app/src/shell/tests/board.rs:62:    assert!(s.app().is_board());
crates/designos-app/src/shell/tests/board.rs:72:    assert!(!d.app().is_board());
crates/designos-app/src/shell/tests/board_home.rs:36:    assert!(s.app().is_board());
crates/designos-app/src/shell/tests/board_home.rs:58:    assert!(!s.app().is_board());
crates/designos-app/src/shell/tests/board_home.rs:83:    assert!(s.app().is_board(), "File > New board opens an empty board");
crates/designos-app/src/shell/tests/dev_mode_shortcut.rs:130:    assert!(s.app().is_board(), "precondition: a board has no </> button");
```

#### 6.2 Acceptance #2 — Single-Capability Mutant Flip Table (All 11 Capabilities)

| Capability Flipped Alone | Table Test (`wb_capabilities_table_for_board_design_and_deck`) | Behaviour Tests Failing (in `shell::tests::{board,dev_mode_shortcut}`) | Failing Behaviour Test Names |
|---|---|---|---|
| `wb_tools_enabled` | **KILLED** | **1** | `shell::tests::board::tab_works_on_a_selected_note_too_and_undoes` |
| `board_key_map` | **KILLED** | **10** | `shell::tests::board::toolbar_buttons_pick_tools_and_shortcuts_follow_the_board_keys`, `tab_makes_the_next_sticky_to_the_right`, `sticky_tool_places_a_note_and_typing_goes_into_it`, `section_tool_makes_a_section_that_adopts_notes_dropped_into_it`, `marker_draws_thick_round_strokes_and_stays_active`, `highlighter_is_wide_and_translucent`, `the_connector_tool_connects_two_notes_and_the_connector_survives_a_move_of_either_end`, +3 more |
| `wb_contextual_bar_enabled` | **KILLED** | **7** | `shell::tests::board::author_line_toggle_on_the_bar`, `contextual_bar_for_connectors_has_line_style_and_caps`, `contextual_bar_for_shapes_swaps_the_shape`, `contextual_bar_recolors_notes_and_undoes`, `contextual_bar_text_size_and_alignment`, `facilitator_timer_hud_stays_usable_over_a_selection_bar_pinned_to_the_bottom`, `undo_and_redo_cover_every_board_edit_in_one_history` |
| `wb_connectors_enabled` | **KILLED** | **5** | `shell::tests::board::hovering_a_shape_shows_four_magnet_dots`, `dragging_from_a_dot_to_another_shape_attaches_a_connector_and_moving_reroutes_it`, `dropping_a_connector_on_empty_canvas_leaves_a_free_end`, `moving_a_free_connector_carries_its_loose_ends`, `undo_and_redo_cover_every_board_edit_in_one_history` |
| `wb_section_defaults` | **KILLED** | **1** | `shell::tests::board::section_and_text_creation_defaults_across_document_kinds` |
| `wb_text_defaults` | **KILLED** | **2** | `shell::tests::board::section_and_text_creation_defaults_across_document_kinds`, `shell::tests::board::text_tool_in_a_board_makes_larger_text` |
| `wb_dotted_background` | **KILLED** | **1** | `shell::tests::board::dotted_background_paints_on_boards_and_solid_page_fill_on_design_canvas` |
| `board_chrome` | **KILLED** | **20** | `shell::tests::board::board_files_get_the_board_ui`, `contextual_bar_*`, `facilitator_timer_*`, `toolbar_buttons_pick_tools_and_shortcuts_follow_the_board_keys`, `dev_mode_shortcut::shift_d_enters_dev_mode_like_the_toolbar_button`, `dev_mode_shortcut::shift_d_leaves_dev_mode_like_the_exit_button`, +9 more |
| `dev_mode_available` | **KILLED** | **3** | `shell::tests::dev_mode_shortcut::shift_d_does_nothing_on_a_board`, `shift_d_enters_dev_mode_like_the_toolbar_button`, `shift_d_leaves_dev_mode_like_the_exit_button` |
| `frame_stepping_available` | **KILLED** | **1** | `shell::tests::board::frame_stepping_shortcut_steps_top_level_frames_only_when_available` |
| `ruler_strip_skips_canvas_menu` | **KILLED** | **1** | `shell::tests::board::right_click_on_ruler_strip_skips_canvas_menu_only_when_enabled` |

Verbatim `plans/reports/agy-1216-wb-capabilities-evidence/r2-mutants.log`:
```text
capabilities (11): ['wb_tools_enabled', 'board_key_map', 'wb_contextual_bar_enabled', 'wb_connectors_enabled', 'wb_section_defaults', 'wb_text_defaults', 'wb_dotted_background', 'board_chrome', 'dev_mode_available', 'frame_stepping_available', 'ruler_strip_skips_canvas_menu']

== flip wb_tools_enabled: table=KILLED | behaviour_failures=1 | test result: FAILED. 49 passed; 2 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 9.33s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::tab_works_on_a_selected_note_too_and_undoes

== flip board_key_map: table=KILLED | behaviour_failures=10 | test result: FAILED. 40 passed; 11 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 8.74s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::a_typed_sticky_text_shrinks_instead_of_overflowing_the_note
   FAILED: shell::tests::board::facilitator_timer_runs_while_canvas_is_in_use
   FAILED: shell::tests::board::highlighter_is_wide_and_translucent
   FAILED: shell::tests::board::marker_draws_thick_round_strokes_and_stays_active
   FAILED: shell::tests::board::section_tool_makes_a_section_that_adopts_notes_dropped_into_it
   FAILED: shell::tests::board::sticky_creation_and_text_undo
   FAILED: shell::tests::board::sticky_tool_places_a_note_and_typing_goes_into_it
   FAILED: shell::tests::board::tab_makes_the_next_sticky_to_the_right
   FAILED: shell::tests::board::the_connector_tool_connects_two_notes_and_the_connector_survives_a_move_of_either_end
   FAILED: shell::tests::board::toolbar_buttons_pick_tools_and_shortcuts_follow_the_board_keys

== flip wb_contextual_bar_enabled: table=KILLED | behaviour_failures=7 | test result: FAILED. 43 passed; 8 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 8.80s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::author_line_toggle_on_the_bar
   FAILED: shell::tests::board::contextual_bar_for_connectors_has_line_style_and_caps
   FAILED: shell::tests::board::contextual_bar_for_shapes_swaps_the_shape
   FAILED: shell::tests::board::contextual_bar_recolors_notes_and_undoes
   FAILED: shell::tests::board::contextual_bar_text_size_and_alignment
   FAILED: shell::tests::board::facilitator_timer_hud_stays_usable_over_a_selection_bar_pinned_to_the_bottom
   FAILED: shell::tests::board::undo_and_redo_cover_every_board_edit_in_one_history

== flip wb_connectors_enabled: table=KILLED | behaviour_failures=5 | test result: FAILED. 45 passed; 6 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 8.51s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::dragging_from_a_dot_to_another_shape_attaches_a_connector_and_moving_reroutes_it
   FAILED: shell::tests::board::dropping_a_connector_on_empty_canvas_leaves_a_free_end
   FAILED: shell::tests::board::hovering_a_shape_shows_four_magnet_dots
   FAILED: shell::tests::board::moving_a_free_connector_carries_its_loose_ends
   FAILED: shell::tests::board::undo_and_redo_cover_every_board_edit_in_one_history

== flip wb_section_defaults: table=KILLED | behaviour_failures=1 | test result: FAILED. 49 passed; 2 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 10.37s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::section_and_text_creation_defaults_across_document_kinds

== flip wb_text_defaults: table=KILLED | behaviour_failures=2 | test result: FAILED. 48 passed; 3 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 9.91s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::section_and_text_creation_defaults_across_document_kinds
   FAILED: shell::tests::board::text_tool_in_a_board_makes_larger_text

== flip wb_dotted_background: table=KILLED | behaviour_failures=1 | test result: FAILED. 49 passed; 2 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 9.72s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::dotted_background_paints_on_boards_and_solid_page_fill_on_design_canvas

== flip board_chrome: table=KILLED | behaviour_failures=20 | test result: FAILED. 30 passed; 21 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 7.64s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::author_line_toggle_on_the_bar
   FAILED: shell::tests::board::board_files_get_the_board_ui
   FAILED: shell::tests::board::contextual_bar_for_connectors_has_line_style_and_caps
   FAILED: shell::tests::board::contextual_bar_for_shapes_swaps_the_shape
   FAILED: shell::tests::board::contextual_bar_recolors_notes_and_undoes
   FAILED: shell::tests::board::contextual_bar_text_size_and_alignment
   FAILED: shell::tests::board::facilitator_timer_chrome_sits_on_the_four_pixel_grid
   FAILED: shell::tests::board::facilitator_timer_hud_stays_usable_over_a_selection_bar_pinned_to_the_bottom
   FAILED: shell::tests::board::facilitator_timer_keeps_requesting_frames_after_start_and_stops_when_paused
   FAILED: shell::tests::board::facilitator_timer_label_uses_tabular_digits
   FAILED: shell::tests::board::facilitator_timer_runs_while_canvas_is_in_use
   FAILED: shell::tests::board::facilitator_timer_stops_asking_for_frames_at_zero
   FAILED: shell::tests::board::plus_one_minute_extends_the_countdown_but_reset_returns_to_the_preset
   FAILED: shell::tests::board::shapes_are_created_with_text_and_the_shape_picker_changes_what_is_placed
   FAILED: shell::tests::board::stamp_tool_places_stamps
   FAILED: shell::tests::board::tool_popovers_stay_above_the_timer_hud
   FAILED: shell::tests::board::toolbar_buttons_pick_tools_and_shortcuts_follow_the_board_keys
   FAILED: shell::tests::board::undo_and_redo_cover_every_board_edit_in_one_history
   FAILED: shell::tests::dev_mode_shortcut::shift_d_enters_dev_mode_like_the_toolbar_button
   FAILED: shell::tests::dev_mode_shortcut::shift_d_leaves_dev_mode_like_the_exit_button

== flip dev_mode_available: table=KILLED | behaviour_failures=3 | test result: FAILED. 47 passed; 4 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 9.69s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::dev_mode_shortcut::shift_d_does_nothing_on_a_board
   FAILED: shell::tests::dev_mode_shortcut::shift_d_enters_dev_mode_like_the_toolbar_button
   FAILED: shell::tests::dev_mode_shortcut::shift_d_leaves_dev_mode_like_the_exit_button

== flip frame_stepping_available: table=KILLED | behaviour_failures=1 | test result: FAILED. 49 passed; 2 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 9.98s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::frame_stepping_shortcut_steps_top_level_frames_only_when_available

== flip ruler_strip_skips_canvas_menu: table=KILLED | behaviour_failures=1 | test result: FAILED. 49 passed; 2 failed; 0 ignored; 0 measured; 3936 filtered out; finished in 9.83s
   FAILED: board::caps::tests::wb_capabilities_table_for_board_design_and_deck
   FAILED: shell::tests::board::right_click_on_ruler_strip_skips_canvas_menu_only_when_enabled

restored caps.rs
```

#### 6.3 Acceptance #3 — Gates Under the Harness

##### `cargo fmt --all -- --check`
Command:
```sh
cargo fmt --all -- --check
```
Output:
```text
(empty output; exit code 0)
```

##### `cargo clippy --workspace --all-targets --locked -- -D warnings`
Command:
```sh
env HOME="$S/home" TMPDIR="$S/tmp/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings > plans/reports/agy-1216-wb-capabilities-evidence/r2-clippy.log 2>&1
grep -E '^(error|warning)' plans/reports/agy-1216-wb-capabilities-evidence/r2-clippy.log
```
Output (`plans/reports/agy-1216-wb-capabilities-evidence/r2-clippy.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1216-wb-capabilities/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1216-wb-capabilities/crates/designos-app/src/main.rs` found to be present in multiple build targets:
error: unneeded `return` statement
error: unneeded `return` statement
error: could not compile `designos-app` (bin "designos-studio") due to 2 previous errors
warning: build failed, waiting for other jobs to finish...
error: could not compile `designos-app` (bin "designos") due to 2 previous errors
error: could not compile `designos-app` (bin "designos-studio" test) due to 2 previous errors
error: could not compile `designos-app` (bin "designos" test) due to 2 previous errors
```
(Only `origin/main`'s 2 remaining errors in `agent_permissions.rs:1163,1181` appear; the other 3 from `panels/components.rs` were fixed on `origin/main` in `#1417` `64f2dd07`.)

##### `cargo test -p designos-app --bin designos --locked` (Counts vs `origin/main`)
- `origin/main` (`64f2dd07`): `3968 passed; 1 failed (SUN_LEN); 13 ignored`
- `HEAD` (`f76a626d`): `3973 passed; 1 failed (SUN_LEN); 13 ignored` (`+5` new tests on this branch: `board::caps::tests::wb_capabilities_table_for_board_design_and_deck`, `shell::tests::board::section_and_text_creation_defaults_across_document_kinds`, `shell::tests::board::dotted_background_paints_on_boards_and_solid_page_fill_on_design_canvas`, `shell::tests::board::frame_stepping_shortcut_steps_top_level_frames_only_when_available`, `shell::tests::board::right_click_on_ruler_strip_skips_canvas_menu_only_when_enabled`).

Command & Output (`plans/reports/agy-1216-wb-capabilities-evidence/r2-test-bin.log`):
```text
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root

test result: FAILED. 3973 passed; 1 failed; 13 ignored; 0 measured; 0 filtered out; finished in 380.43s
```

##### `cargo test --workspace --locked --lib`
Command & Summary Output (`plans/reports/agy-1216-wb-capabilities-evidence/r2-test-workspace-lib.log`):
```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 706 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.78s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.42s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.14s
test result: ok. 322 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.69s
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 33.77s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

#### 6.4 Acceptance #4 — Pushed Branch Log & Cleanup
Command:
```sh
git log --oneline origin/main..HEAD
```
Output:
```text
f76a626d refactor(app): split board_chrome and board_key_map capabilities and add behaviour pins (#1216)
08f494a8 Merge remote-tracking branch 'origin/main' into jangtrinh/agy-1216-wb-capabilities
176c0cb5 refactor(app): gate whiteboard behaviour on named capabilities, not the file kind (#1216)
```
(Plus the report update commit at the tip of `jangtrinh/agy-1216-wb-capabilities`; `out/cargo` removed and verified absent via `rm -rf out/cargo && test ! -e out/cargo`.)

### 7. Round 2 Assumptions & Unresolved Questions
- **Assumptions:**
  1. `ruler_strip_skips_canvas_menu()` is the clearest name for the capability at `shell.rs:1204` because that site decides whether a right-click inside the top/left ruler strip returns early from `Shell::context_menu` instead of opening the canvas context menu.
  2. `section_and_text_creation_defaults_across_document_kinds` both replaces `ac13_wb1_open_render_save` (Ruling 3 / M3) and serves as the behaviour test that fails when `wb_section_defaults()` (or `wb_text_defaults()`) is flipped alone (Ruling 5 / L3).
- **Unresolved questions:** None.

Status: DONE — Round 2 complete: 11 single-question capabilities in board/caps.rs, phantom test replaced with creation default and capability behaviour tests, all 11 single-flip mutants killed by both table and behaviour tests, PR #1460 body updated, out/cargo deleted, and pushed.
