# Report — #1003: Headless Input Checks from the QA Red Team (`US-10`)

## 1. What Could NOT Be Verified

- Nothing was skipped or left unverified.
  - `swiftc` is installed on this host (`/usr/bin/swiftc`, Apple Swift `6.2.3`), so `AC-01` (`RT-03`) was verified both via unit tests (`crates/designos-export/src/swiftui.rs`) and by piping `designos-studio --codegen swiftui` output through `swiftc -parse -` (`exit code: 0`).
  - All 5 acceptance criteria (`AC-01..AC-05`, covering `RT-03..RT-07`) and all required workspace gates were executed under the isolated test harness.

---

## 2. Drift Check ("Spec Says / Base HEAD Says")

Base commit checked in Step 0: `6f0c3bb21` (`origin/release/v0.6.4`).

| Criterion | Spec (`US-10` / `#1003` PM Ruling) | Base HEAD (`6f0c3bb21`) | Action Taken |
|---|---|---|---|
| **AC-01 (`RT-03`)** | SwiftUI codegen keeps `${y}` as `"${y}"` (never `"\${y}"`), escapes `\(` as `"\\("`, and parses under `swiftc -parse`. | **GREEN on base** (shipped in `#1090` / `45d919b35` via `swift_quote` in `crates/designos-export/src/codegen.rs:541`). Base `cargo test -p designos-export --locked --lib swiftui` passed (`8 passed; 0 failed`). | Added guard test `text_nodes_with_dollar_braces_and_swift_interpolation_parse_cleanly` in `crates/designos-export/src/swiftui.rs` + CLI `swiftc -parse -` verification in `cli_and_swiftc.log`. No production code changed in `designos-export`. |
| **AC-02 (`RT-04`)** | `designos-studio trunc.fig --screenshot s.png` (and 0-byte `empty.fig`) exits non-zero, names the broken file on stderr, and writes no `s.png`. | **GREEN on base** (shipped in `#1117` / `eac72061a` in `crates/designos-app/src/main.rs:265-272`). Base `cargo test -p designos-app --test cli_screenshot --locked` passed (`1 passed; 0 failed`). | Extended `crates/designos-app/tests/cli_screenshot.rs` guard test to include 0-byte `empty.fig` alongside `trunc.fig`, `corrupt.designos`, and `garbage.fig`. No production code changed in `designos-app`. |
| **AC-03 (`RT-05`)** | `designos_save_file` (and shared output-name helper) refuses names with control chars (`\n`, `\r`, `\x07`, `\x7f`), bidi controls (`U+202A..=U+202E`, `U+2066..=U+2069`, `U+200E`, `U+200F`, `U+061C`), or whitespace-only names (`""`, `"   "`, `"   .designos"`, `"dir/   .designos"`, `"   /file.designos"`) with typed `INVALID_NAME` (`ErrorCode::InvalidName`), creating no file; valid Vietnamese and CJK names save cleanly. | **RED on base**: `validate_output_name` (`crates/designos-mcp/src/engine.rs:1536`) returned `ErrorCode::InvalidArguments` instead of `ErrorCode::InvalidName`, did not reject whitespace-only names/stems/components, and `output_path_allow_overwrite` (`crates/designos-mcp/src/server.rs:655`) returned `PATH_OUTSIDE_WORKSPACE` on `""` or missing-dir `INVALID_ARGUMENTS` on `"   /file.designos"`. | Added `ErrorCode::InvalidName`, updated `validate_output_name` + `is_whitespace_only_output_name` in `engine.rs` and path resolution in `server.rs`, added fixture in `every_code_has_a_fixture`, and added red-first tests in `crates/designos-mcp/tests/mcp_paths.rs`. |
| **AC-04 (`RT-06`)** | `designos_save_file` (and shared output-name helper) with a name over 255 UTF-8 bytes returns `NAME_TOO_LONG` (`ErrorCode::NameTooLong`) with no overwrite hint (`corrected_example: None`); 255-byte names (including multi-byte UTF-8) succeed. | **RED on base**: `validate_output_name` (`crates/designos-mcp/src/engine.rs:1545-1552`) returned `ErrorCode::InvalidArguments` instead of `ErrorCode::NameTooLong`. | Added `ErrorCode::NameTooLong`, updated `validate_output_name` in `engine.rs`, added fixture in `every_code_has_a_fixture`, regenerated tool docs (`gen-tool-docs`), and added red-first tests (260/291/300-byte ASCII + 255/256-byte UTF-8 boundary) in `crates/designos-mcp/tests/mcp_paths.rs`. |
| **AC-05 (`RT-07`)** | `--apply-spec` / `designos_apply_spec` with `"children": [[{"kind": "rectangle"}]]` fails with plain-words `"children must be an array of NodeSpec"`, naming `children` and its JSON path (`children/0/children`) when nested. | **GREEN on base** (shipped in `#1117` / `eac72061a` in `crates/designos-mcp/src/tools/errors.rs:1338-1350`). Base `cargo test -p designos-mcp --test spec_errors --locked` (`8 passed`) and `cargo test -p designos-app --bin designos --locked cli_apply_spec` (`5 passed`) were green. | Added nested `children/0/children` array-of-arrays guard assertion in `crates/designos-mcp/tests/spec_errors.rs` (`round3_nested_spec_errors_carry_path_from_root`) and CLI verification in `cli_and_swiftc.log`. No production code changed in the `NodeSpec` error path. |

---

## 3. Root Cause (`file:line` Evidence)

1. **Missing typed error codes (`crates/designos-mcp/src/tools/errors.rs:12-103`, `145-217`, `219-293`, `324-399`):**
   - `ErrorCode` lacked `InvalidName` (`"INVALID_NAME"`) and `NameTooLong` (`"NAME_TOO_LONG"`).
2. **Untyped error codes & missing whitespace-only filename check in shared output helper (`crates/designos-mcp/src/engine.rs:1536-1554`):**
   - `validate_output_name(display_path, ptr)` returned `ErrorCode::InvalidArguments` on `has_control_or_bidi_char(display_path)` and on `Component::Normal(part)` with `part.len() > 255`, instead of `ErrorCode::InvalidName` and `ErrorCode::NameTooLong`.
   - `validate_output_name` did not check whether `display_path` was empty/whitespace-only (`""`, `"   "`), had a whitespace-only path component (`"   /file.designos"`), or had a whitespace-only file stem (`"   .designos"`, `" \u{00A0} .designos"`, `"dir/   .designos"`).
3. **Early parent-directory canonicalization before name validation on blank/non-existent parent paths (`crates/designos-mcp/src/server.rs:655-773`):**
   - In `Session::output_path_allow_overwrite`, `Session::png_output_path`, and `Session::fig_output_path`, `self.root.join("")` resolved to `self.root`, whose `.parent()` was outside `self.root` (falsely returning `PATH_OUTSIDE_WORKSPACE`), and a relative path with a non-existent invalid parent component (e.g. `"   /file.designos"` or `"a\nb/file.designos"`) failed in `parent.canonicalize()` with missing-directory `INVALID_ARGUMENTS` before reaching `validate_output_name`.
   - Checking `if path.trim().is_empty() { crate::engine::validate_output_name(path, ptr)?; }` at entry and checking `crate::engine::validate_output_name(path, ptr)` in the relative non-`..` `canonicalize` error branch ensures `INVALID_NAME` / `NAME_TOO_LONG` is returned while preserving `PATH_OUTSIDE_WORKSPACE` priority on any `..`, absolute, or symlink-escape path.

---

## 4. Skills Loaded & What Changed Because of Them

- `es:dev` & `es:feature-dev`: Enforced Step 0 base verification before touching code, scoped changes strictly to allowed files, and committed failing tests (`test(mcp,app,export): ...`) before implementation (`fix(mcp): ...`).
- `es:rust`: Used zero-allocation iterator checks (`Path::new(display_path).components()`, `file_stem()`) in `is_whitespace_only_output_name` and preserved `ErrorCode` `Copy` enum invariants.
- `es:rust-gpu-canvas`: Confirmed no GPU or canvas rendering paths were touched.
- `es:fable-thinking`: Used falsification-first (`red.log`) which caught `path = ""` hitting `self.root.join("").parent()` -> `PATH_OUTSIDE_WORKSPACE` in `server.rs:676` and caught `gen-tool-docs` hitting the 24,000-char rules budget (`MAX_PURPOSE_CHARS` adjusted from `104` to `100`, matching the pattern in commit `f56440467`).
- `es:astra-work`: Verified every AC with concrete test and CLI artifacts (`red.log`, `green.log`, `cli_and_swiftc.log`) under the isolated harness and cleaned up `out/cargo`.

---

## 5. Acceptance Table & Gate Outputs (Run on Final Commit)

| # | Check / Gate | Command | Result |
|---|---|---|---|
| **Step 0** | Base check on `6f0c3bb21` | `cargo test -p designos-export --locked --lib swiftui`, `cargo test -p designos-app --test cli_screenshot --locked`, `cargo test -p designos-mcp --test spec_errors --test mcp_paths --locked` | `AC-01`, `AC-02`, `AC-05` green on base; `AC-03`, `AC-04` red on base |
| **AC-01 (`RT-03`)** | SwiftUI codegen escapes `\(` and keeps `${y}` as `"${y}"`; passes `swiftc -parse` | `cargo test -p designos-export --locked --lib swiftui` + `designos-studio ac01.designos --codegen swiftui --select PriceBanner \| ... \| swiftc -parse -` | **PASS** (`9 passed; 0 failed`, `swiftc -parse exit code: 0`) |
| **AC-02 (`RT-04`)** | `--screenshot` on truncated & 0-byte `.fig` exits non-zero, names file on stderr, writes no PNG | `cargo test -p designos-app --test cli_screenshot --locked` | **PASS** (`1 passed; 0 failed`, `exit=1`, `s.png_exists=no`) |
| **AC-03 (`RT-05`)** | `designos_save_file` returns `INVALID_NAME` on control, bidi, and whitespace-only names; saves Vietnamese & CJK names | `cargo test -p designos-mcp --test mcp_paths --locked` | **PASS** (`22 passed; 0 failed`) |
| **AC-04 (`RT-06`)** | `designos_save_file` returns `NAME_TOO_LONG` on >255 UTF-8 bytes with no overwrite hint; accepts 255 UTF-8 bytes | `cargo test -p designos-mcp --test mcp_paths --locked` | **PASS** (`22 passed; 0 failed`) |
| **AC-05 (`RT-07`)** | `--apply-spec` / `designos_apply_spec` refuses `children: [[...]]` with plain-words message & JSON path | `cargo test -p designos-mcp --test spec_errors --locked` | **PASS** (`8 passed; 0 failed`) |
| **Fixtures & Docs** | `every_code_has_a_fixture` + `gen-tool-docs` | `cargo test -p designos-mcp --locked --lib every_code_has_a_fixture` & `cargo test -p designos-mcp --locked --lib rules` | **PASS** (`71/71` error codes covered; `25/25` rules tests pass) |
| **Gate 1** | Rustfmt check | `cargo fmt --all -- --check` | **PASS** (`exit 0`) |
| **Gate 2** | Clippy (`-D warnings`) | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (`exit 0`, 0 warnings) |
| **Gate 3** | Workspace lib tests | `cargo test --workspace --locked --lib` | **PASS** (`1,282 passed; 0 failed`) |
| **Gate 4** | App bin tests (`designos`) | `cargo test -p designos-app --bin designos --locked` | **PASS** (`3,335 passed; 0 failed; 11 ignored`, 0 reruns needed) |
| **Gate 5** | Python UI & spacing gates | `python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py` | **PASS** (`Z01..Z21 PASS`, `check_spacing_relationships PASS`) |

### Pasted Command Outputs

#### 5.1 Red-First & Green Test Runs (`red.log` & `green.log`)
- Full logs saved at:
  - `plans/reports/agy-1003-headless-input-checks-evidence/red.log`
  - `plans/reports/agy-1003-headless-input-checks-evidence/green.log`
  - `plans/reports/agy-1003-headless-input-checks-evidence/cli_and_swiftc.log`

Summary of failing tests in `red.log` (commit `b3901223a`, before `fix(mcp)`):
```text
failures:
    file_creation_tools_refuse_bidi_override_characters_in_names
    file_creation_tools_refuse_control_characters_in_names
    file_creation_tools_refuse_whitespace_only_names_and_accept_vietnamese_and_cjk
    file_creation_tools_report_name_too_long_instead_of_file_exists
    trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first

test result: FAILED. 17 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.19s
```

Summary of passing tests in `green.log` (after `fix(mcp)`):
```text
=== [1] cargo test -p designos-export --locked --lib swiftui ===
running 9 tests
test swiftui::tests::text_nodes_with_dollar_braces_and_swift_interpolation_parse_cleanly ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 129 filtered out; finished in 0.88s

=== [2] cargo test -p designos-app --test cli_screenshot --locked ===
running 1 test
test cli_screenshot_refuses_broken_files_and_captures_good_file ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.55s

=== [3] cargo test -p designos-mcp --test spec_errors --locked ===
running 8 tests
test round3_nested_spec_errors_carry_path_from_root ... ok
test apply_spec_reports_field_and_expected_type_in_plain_words ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

=== [4] cargo test -p designos-mcp --test mcp_paths --locked ===
running 22 tests
test file_creation_tools_refuse_control_characters_in_names ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test file_creation_tools_refuse_whitespace_only_names_and_accept_vietnamese_and_cjk ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.25s

=== [5] cargo test -p designos-mcp --locked --lib every_code_has_a_fixture ===
running 1 test
test server::tests::every_code_has_a_fixture ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 244 filtered out; finished in 0.16s

=== [6] cargo test -p designos-mcp --locked --lib rules ===
running 25 tests
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 220 filtered out; finished in 0.15s
```

#### 5.2 CLI & `swiftc -parse` Output (`cli_and_swiftc.log`)
```text
=== [AC-01 / RT-03] swiftc version & CLI --codegen swiftui piped to swiftc -parse ===
/usr/bin/swiftc
swift-driver version: 1.127.14.1 Apple Swift version 6.2.3 (swiftlang-6.2.3.3.21 clang-1700.6.3.2)
Target: arm64-apple-macosx26.0
--- Generated SwiftUI from designos-studio ac01.designos --codegen swiftui --select PriceBanner ---
VStack(alignment: .leading, spacing: 8) {
    Text("${y}")
        .font(.custom("Inter", size: 14))
        .foregroundColor(Color.black)
    Text("\\(total)")
        .font(.custom("Inter", size: 14))
        .foregroundColor(Color.black)
}
    .frame(width: 240, height: 80, alignment: .topLeading)
    .background(Color.white)
--- Piping wrapped SwiftUI output to swiftc -parse - ---
swiftc -parse exit code: 0

=== [AC-02 / RT-04] designos-studio trunc.fig & empty.fig --screenshot ===
trunc.fig exit=1 s.png_exists=no stderr=trunc.fig could not be opened: Cannot import .fig: not a fig file: zip: invalid Zip archive: Could not find EOCD. Check the file, then try again.
empty.fig exit=1 s_empty.png_exists=no stderr=empty.fig could not be opened: Cannot import .fig: not a fig file: missing fig-kiwi magic or zip header. Check the file, then try again.

=== [AC-05 / RT-07] designos-studio --apply-spec with children: [[...]] ===
bad_children.json exit=2 stderr=designos: Invalid NodeSpec JSON in bad_children.json: children must be an array of NodeSpec objects. Pass each child as a JSON object.
bad_nested_children.json exit=2 stderr=designos: Invalid NodeSpec JSON in bad_nested_children.json: children/0/children must be an array of NodeSpec objects. Pass each child as a JSON object.
```

#### 5.3 Gates Under the Harness
```text
$ cargo fmt --all -- --check
fmt_exit=0

$ cargo clippy --workspace --all-targets --locked -- -D warnings
    Checking designos-export v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1003-headless-input-checks/crates/designos-export)
    Checking designos-gpu v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1003-headless-input-checks/crates/designos-gpu)
    Checking eframe v0.35.0
    Checking designos-webcapture v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1003-headless-input-checks/crates/designos-webcapture)
    Checking designos-fig v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1003-headless-input-checks/crates/designos-fig)
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1003-headless-input-checks/crates/designos-library)
    Checking designos-eval v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1003-headless-input-checks/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 12s
clippy_exit=0

$ cargo test --workspace --locked --lib
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-e9fb9a3e00ecdbf3)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-a0c6cf32749620e6)
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.35s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-029cd168a180af6a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-f5a81d86319362e2)
test result: ok. 137 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.63s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-648cb6f438830bb1)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 63.70s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-8afb0f60e41df5a3)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-49227ce26b5d6686)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-d39027c82797cc1e)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.43s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-189215ed087510e0)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.28s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-39473ba3e638dab1)
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 82.15s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-125a628128d3698e)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-a59ae055a8fa52f0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

$ cargo test -p designos-app --bin designos --locked
     Running unittests src/main.rs (out/cargo/debug/deps/designos-f103f8deb2404a79)
test result: ok. 3335 passed; 0 failed; 11 ignored; 0 measured; 0 filtered out; finished in 396.25s

$ python3 scripts/zero_fake_ui_gate.py && python3 scripts/check_spacing_relationships.py
RESULT: 21/21 Zero-Fake-UI invariants PASSED. Gate status: GREEN.
PASS: spacing relationships hold across theme.rs and design.tokens.json
```

---

## 6. Assumptions & Scope Check

- **Files modified (all within allowed scope):**
  - `crates/designos-export/src/swiftui.rs` (guard test only)
  - `crates/designos-app/tests/cli_screenshot.rs` (guard test only)
  - `crates/designos-mcp/tests/spec_errors.rs` (guard test only)
  - `crates/designos-mcp/tests/mcp_paths.rs` (red-first tests for `INVALID_NAME` / `NAME_TOO_LONG` / whitespace-only / Vietnamese & CJK / 255-byte UTF-8 boundary)
  - `crates/designos-mcp/src/tools/errors.rs` (`ErrorCode::InvalidName`, `ErrorCode::NameTooLong`)
  - `crates/designos-mcp/src/engine.rs` (`is_whitespace_only_output_name`, `validate_output_name`)
  - `crates/designos-mcp/src/server.rs` (`output_path_allow_overwrite`, `png_output_path`, `fig_output_path`, `every_code_has_a_fixture`)
  - `crates/designos-mcp/src/rules.rs` (`tool_error_codes`, `MAX_PURPOSE_CHARS`)
  - `crates/designos-mcp/generated/rules.md` & `skills/design-os-studio/rules.md` (regenerated by `gen-tool-docs`)
  - `plans/reports/agy-1003-headless-input-checks.md` & `plans/reports/agy-1003-headless-input-checks-evidence/`
- **Files outside scope:** None.
- **Assumptions:** None (`validate_output_name` in `crates/designos-mcp/src/engine.rs` is shared by `designos_save_file`, `designos_render_png`, and `designos_export_fig_file`; neither the CLI nor the desktop app calls this helper).

Status: DONE
