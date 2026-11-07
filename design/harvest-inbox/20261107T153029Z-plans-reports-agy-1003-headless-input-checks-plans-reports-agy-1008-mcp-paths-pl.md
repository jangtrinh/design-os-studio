# harvest-extract-v1

You are extracting durable lessons from a finished piece of work. You did not do this
work and have no stake in it looking good — be a skeptical outside reader, not a
cheerleader.

You will be given one or more end-of-phase reports, each fenced between
`--- REPORT: <path> ---` and `--- END REPORT: <path> --- `.

## Output

Respond with **raw JSON only** — no prose before or after, no markdown fence required but
allowed. Shape:

```json
{"v": 1, "candidates": [
  {
    "kind": "insight",
    "text": "...",
    "evidence": "...",
    "source": "<path exactly as given in the REPORT header>",
    "durable": true,
    "actionable": true,
    "confidence": 0.8,
    "target": "taste-rubric.md#motion",
    "gapKind": "rubric-gap"
  }
]}
```

`target` and `gapKind` are required when `kind` is `"gap"`; omit them for `"insight"`.

## Fields

- **`evidence` must be copied VERBATIM from the report you read it in** — a candidate whose
  `evidence` is not a literal quote of the source text will be discarded by a deterministic
  gate before it ever reaches a human. This is not a soft preference; quote exactly.
- **`durable`**: would this help on a *different* project, not just this one? A one-off
  fact about this run ("took 2 hours", "ran npm test, all green") is not durable.
- **`actionable`**: does it change what someone would DO differently next time? A
  restated todo-list item is not actionable — it is a description, not a lesson.
- **`kind`**: `"insight"` is a durable lesson worth remembering. `"gap"` is knowledge
  missing from the studio's shared `knowledge/` — set `target` to `<file>.md[#section]`
  and `gapKind` to one of: `rubric-gap`, `persona-gap`, `recipe-gap`, `benchmark-stale`,
  `guardrail-lesson`. A taste observation is a `gap` with `gapKind: "rubric-gap"`.
- **`confidence`**: 0.0–1.0, your honest estimate that this candidate survives scrutiny.

## Discipline

- **Prefer none to noise.** An empty `candidates` array is a valid, respected answer — do
  not manufacture a lesson to have something to say.
- **Never** write files, never run commands, never emit prose outside the JSON object.

--- REPORT: plans/reports/agy-1003-headless-input-checks.md ---
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

--- END REPORT: plans/reports/agy-1003-headless-input-checks.md ---

--- REPORT: plans/reports/agy-1008-mcp-paths.md ---
# Report — `designos-mcp` File Paths: Export `.fig`, File Names, Echoed Paths (`#1008`, `#1003` RT-05/RT-06, `#1087`)

## 1. What could NOT be verified

- **Nothing in the assigned scope was left unverified.** Per the task constraints ("do not build `designos-app`'s test binaries; the controller runs those"), `designos-app` test binaries and live GUI terminal workspace binding (`#1074`, owned by another worker in `crates/designos-app/src/terminal.rs`) were not built or executed here.
- **Conclusion (`FACT`):**
  - `FACT`: `designos_export_fig_file` (`Session` in `crates/designos-mcp/src/server.rs` and `Engine::call` in `crates/designos-mcp/src/engine.rs`) now refuses symbolic links at the final path component (pointing inside the workspace, pointing outside the workspace, or dangling), refuses folders and non-plain files (FIFOs/sockets/hard-linked files), refuses existing files unless `overwrite: true` is passed (`ErrorCode::FileExists`), compares `.fig` extensions case-insensitively (`UPPER.FIG`, `Mixed.Fig`), and writes exclusively through `crate::engine::write_checked_output_file` (which delegates to `session_file_io::truncate(resolved, false)` + `session_file_io::append(resolved, bytes, true)` using `O_NOFOLLOW | O_NONBLOCK` + `fstat` without `O_TRUNC`).
  - `FACT`: Every file-creating MCP tool (`designos_save_file`, `designos_render_png`, `designos_export_fig_file`) refuses paths containing control characters (`\n`, `\r`, C0/DEL/C1 controls) or Unicode bidi override/isolate characters (`U+202A..=U+202E`, `U+2066..=U+2069`, `U+200E`, `U+200F`, `U+061C`) with `ErrorCode::InvalidArguments`.
  - `FACT`: Every file-creating MCP tool checks path component byte length (`> 255` bytes) before opening or creating the file and returns `ErrorCode::InvalidArguments` (`"<p> has a name longer than 255 bytes. Use a shorter name."`) rather than `ErrorCode::FileExists`.
  - `FACT`: `ErrorCode::PathOutsideWorkspace` never stores or serializes `received` in `ToolError` (`crates/designos-mcp/src/tools/errors.rs`), and `input_path`, `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` in `crates/designos-mcp/src/server.rs` check workspace containment before any error branch that echoes the caller's path (`designos_open_file`, `designos_save_file`, `designos_render_png`, `designos_export_fig_file`, `designos_import_web_capture`).
  - `FACT`: Outside-workspace canary files remain byte-for-byte identical (verified via FIPS 180-4 SHA-256 digests before and after every refused call) across all 12 tests in `crates/designos-mcp/tests/mcp_paths.rs`.
- **Weakest link:** Bare `Engine::call("designos_export_fig_file", ...)` has no workspace root (unlike `Session` in `server.rs`), so it enforces name, extension, target-type, `overwrite`, and `O_NOFOLLOW` write checks on the path directly while workspace boundary enforcement lives in `Session`.

---

## 2. Acceptance Table (1–6)

| # | Gate | Result | Evidence |
|---|---|---|---|
| 1 | Red first: tests for every case in objectives 1–4 fail on `dc23fdd4` for the stated reason and pass on head (`red.log` and `green.log` saved) | **PASS** | `plans/reports/agy-1008-mcp-paths-evidence/red.log` (`0 passed; 12 failed`) and `plans/reports/agy-1008-mcp-paths-evidence/green.log` (`12 passed; 0 failed`) |
| 2 | Canary file outside the workspace is byte-identical after every refused call, shown by a test that hashes it before and after | **PASS** | `export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched`, `outside_workspace_refusal_never_echoes_path_and_preserves_canary`, `save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary`, `bare_engine_export_fig_enforces_all_target_checks` in `crates/designos-mcp/tests/mcp_paths.rs` |
| 3 | `cargo fmt --all -- --check` exits 0 | **PASS** | Exited `0` |
| 4 | `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0 | **PASS** | Exited `0` |
| 5 | `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked` pass (mandatory harness) | **PASS** | `designos-mcp`: 377 passed, 0 failed; `designos-eval`: 25 passed, 0 failed |
| 6 | Delete `out/` at the end | **PASS** | `rm -rf out && test ! -e out` exited `0` |

### Acceptance 1 & 2 — Red (`dc23fdd4d`) and Green (`HEAD`) Commands & Output

#### Red Run on `dc23fdd4d` (`plans/reports/agy-1008-mcp-paths-evidence/red.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/red.log 2>&1
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/red.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.16s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 12 tests
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... FAILED
test export_fig_validates_extension_case_insensitively ... FAILED
test file_creation_tools_report_name_too_long_instead_of_file_exists ... FAILED
test file_creation_tools_refuse_bidi_override_characters_in_names ... FAILED
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... FAILED
test file_creation_tools_refuse_control_characters_in_names ... FAILED
test export_fig_refuses_folder ... FAILED
test export_fig_refuses_existing_file_unless_overwrite_is_true ... FAILED
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... FAILED
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... FAILED
test export_fig_refuses_symlink_inside_workspace ... FAILED
test bare_engine_export_fig_enforces_all_target_checks ... FAILED

failures:

---- save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary stdout ----

thread 'save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary' (45442105) panicked at crates/designos-mcp/tests/mcp_paths.rs:639:13:
assertion `left == right` failed: live.designos overwrite=false: {"code":"FILE_EXISTS","corrected_example":{"overwrite":true,"path":"live.designos"},"error":{"code":"FILE_EXISTS","corrected_example":{"overwrite":true,"path":"live.designos"},"message":"Cannot create file (choose an unused .designos or .designos filename, or set overwrite=true): File exists (os error 17)","next_step":"Pass overwrite=true or choose a new path.","path":"/path","received":"live.designos","safe_to_retry":true},"message":"Cannot create file (choose an unused .designos or .designos filename, or set overwrite=true): File exists (os error 17)","next_step":"Pass overwrite=true or choose a new path.","path":"/path","received":"live.designos","safe_to_retry":true}
  left: FileExists
 right: InvalidArguments
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- export_fig_validates_extension_case_insensitively stdout ----

thread 'export_fig_validates_extension_case_insensitively' (45442100) panicked at crates/designos-mcp/tests/mcp_paths.rs:268:33:
UPPER.FIG should be accepted: {"code":"INVALID_ARGUMENTS","error":{"code":"INVALID_ARGUMENTS","expected":"*.fig","message":"Export path must end in .fig","next_step":"Adjust the value at `path` using `valid_options` or `corrected_example` and retry.","path":"/output_path","received":"UPPER.FIG","safe_to_retry":true},"expected":"*.fig","message":"Export path must end in .fig","next_step":"Adjust the value at `path` using `valid_options` or `corrected_example` and retry.","path":"/output_path","received":"UPPER.FIG","safe_to_retry":true}

---- file_creation_tools_report_name_too_long_instead_of_file_exists stdout ----

thread 'file_creation_tools_report_name_too_long_instead_of_file_exists' (45442103) panicked at crates/designos-mcp/tests/mcp_paths.rs:496:5:
assertion `left != right` failed: must not report FILE_EXISTS
  left: FileExists
 right: FileExists

---- file_creation_tools_refuse_bidi_override_characters_in_names stdout ----

thread 'file_creation_tools_refuse_bidi_override_characters_in_names' (45442101) panicked at crates/designos-mcp/tests/mcp_paths.rs:439:14:
save_file must refuse bidi char: Object {"path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-rt05-bidi-19153-1792896390673243000-7/rlo\u{202e}evil.designos"), "saved": Bool(true)}

---- outside_workspace_refusal_never_echoes_path_and_preserves_canary stdout ----

thread 'outside_workspace_refusal_never_echoes_path_and_preserves_canary' (45442104) panicked at crates/designos-mcp/tests/mcp_paths.rs:587:9:
designos_open_file {"path":"/etc/passwd"} must not set received on PATH_OUTSIDE_WORKSPACE: {"code":"PATH_OUTSIDE_WORKSPACE","error":{"code":"PATH_OUTSIDE_WORKSPACE","message":"Path must stay inside the configured workspace","next_step":"Pass a relative path inside the active workspace.","path":"/path","received":"/etc/passwd","safe_to_retry":true},"message":"Path must stay inside the configured workspace","next_step":"Pass a relative path inside the active workspace.","path":"/path","received":"/etc/passwd","safe_to_retry":true}

---- file_creation_tools_refuse_control_characters_in_names stdout ----

thread 'file_creation_tools_refuse_control_characters_in_names' (45442102) panicked at crates/designos-mcp/tests/mcp_paths.rs:374:14:
save_file must refuse control char: Object {"path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-rt05-ctrl-19153-1792896390673932000-10/line\nbreak.designos"), "saved": Bool(true)}

---- export_fig_refuses_folder stdout ----

thread 'export_fig_refuses_folder' (45442097) panicked at crates/designos-mcp/tests/mcp_paths.rs:245:9:
assertion `left == right` failed: {"output_path":"dir.fig"}: {"code":"INTERNAL_ERROR","error":{"code":"INTERNAL_ERROR","message":"Cannot write .fig file: Is a directory (os error 21)","next_step":"Call designos_get_document to inspect current state before retrying.","path":"/output_path","safe_to_retry":true},"message":"Cannot write .fig file: Is a directory (os error 21)","next_step":"Call designos_get_document to inspect current state before retrying.","path":"/output_path","safe_to_retry":true}
  left: InternalError
 right: InvalidArguments

---- export_fig_refuses_existing_file_unless_overwrite_is_true stdout ----

thread 'export_fig_refuses_existing_file_unless_overwrite_is_true' (45442096) panicked at crates/designos-mcp/tests/mcp_paths.rs:209:10:
existing .fig without overwrite must fail: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-overwrite-19153-1792896390673102000-3/existing.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-overwrite-19153-1792896390673102000-3/existing.fig"), "saved": Bool(true)}

---- export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched stdout ----

thread 'export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched' (45442099) panicked at crates/designos-mcp/tests/mcp_paths.rs:130:14:
symlink at last path component must be refused: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-out-19153-1792896390673211000-5/evil.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-out-19153-1792896390673211000-5/evil.fig"), "saved": Bool(true)}

---- export_fig_refuses_dangling_symlink_inside_and_outside_workspace stdout ----

thread 'export_fig_refuses_dangling_symlink_inside_and_outside_workspace' (45442095) panicked at crates/designos-mcp/tests/mcp_paths.rs:186:18:
dangling symlink must be refused: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-dangling-19153-1792896390673008000-1/dangling_out.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-dangling-19153-1792896390673008000-1/dangling_out.fig"), "saved": Bool(true)}

---- export_fig_refuses_symlink_inside_workspace stdout ----

thread 'export_fig_refuses_symlink_inside_workspace' (45442098) panicked at crates/designos-mcp/tests/mcp_paths.rs:158:14:
in-workspace symlink must be refused: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-in-19153-1792896390673134000-4/link_in.fig"), "path": String("/private/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-ws-sym-in-19153-1792896390673134000-4/link_in.fig"), "saved": Bool(true)}

---- bare_engine_export_fig_enforces_all_target_checks stdout ----

thread 'bare_engine_export_fig_enforces_all_target_checks' (45442094) panicked at crates/designos-mcp/tests/mcp_paths.rs:297:10:
bare Engine must refuse symlink: Object {"bytes": Number(22649), "omitted_comment_threads": Number(0), "omitted_extensions": Array [], "output_path": String("/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-engine-direct-19153-1792896390672957000-0/sym.fig"), "path": String("/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IODkB1/designos-mcp-paths-engine-direct-19153-1792896390672957000-0/sym.fig"), "saved": Bool(true)}


failures:
    bare_engine_export_fig_enforces_all_target_checks
    export_fig_refuses_dangling_symlink_inside_and_outside_workspace
    export_fig_refuses_existing_file_unless_overwrite_is_true
    export_fig_refuses_folder
    export_fig_refuses_symlink_inside_workspace
    export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched
    export_fig_validates_extension_case_insensitively
    file_creation_tools_refuse_bidi_override_characters_in_names
    file_creation_tools_refuse_control_characters_in_names
    file_creation_tools_report_name_too_long_instead_of_file_exists
    outside_workspace_refusal_never_echoes_path_and_preserves_canary
    save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary

test result: FAILED. 0 passed; 12 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

error: test failed, to rerun pass `-p designos-mcp --test mcp_paths`
```

#### Green Run on `HEAD` (`plans/reports/agy-1008-mcp-paths-evidence/green.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/green.log 2>&1
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/green.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.64s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 12 tests
test export_fig_refuses_symlink_inside_workspace ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_folder ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### Acceptance 3 — `cargo fmt --all -- --check`

Command:
```sh
cargo fmt --all -- --check && echo "FMT_CHECK=0"
```

Output:
```text
FMT_CHECK=0
```

### Acceptance 4 — `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cargo clippy --workspace --all-targets --locked -- -D warnings
echo "CLIPPY_EXIT=$?"
```

Output:
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-doc)
    Checking designos-fonts v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-fonts)
    Checking designos-render v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-render)
    Checking designos-diff v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-diff)
    Checking designos-svg v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-svg)
    Checking designos-export v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-export)
    Checking designos-webcapture v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-webcapture)
    Checking designos-fig v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-fig)
    Checking designos-library v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-library)
    Checking designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-eval)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-gpu)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 42.38s
CLIPPY_EXIT=0
```

### Acceptance 5 — `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked`

Command (`designos-mcp`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked
```

Output (`designos-mcp`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.44s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-616b53869353a607)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-f413a393b4d55d8f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-1aa6ab2233591601)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-01318f1c86eb2572)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-f067c8c79555baaa)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-378a191ea16cbc7f)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.54s
     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-23ae83d21f949530)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-ab113f1c25ffacf0)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/session.rs (out/cargo/debug/deps/session-70f6c9de7d7f9973)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
     Running tests/session_file_sites/main.rs (out/cargo/debug/deps/session_file_sites-1a885e87e98ff7af)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/session_files/main.rs (out/cargo/debug/deps/session_files-657090e65be613f1)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-75b28c66cba35b01)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
   Doc-tests designos_mcp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Command (`designos-eval`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked
```

Output (`designos-eval`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)

running 1 test
test hash::tests::fips_180_4_vectors ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (out/cargo/debug/deps/designos_eval-30aca93b7d8d8c4a)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)

running 16 tests
test bootstrap_known_case ... ok
test split_fraction_within_bounds ... ok
test missing_register_refuses_the_run ... ok
test held_out_refused_in_tuning ... ok
test split_lock_mismatch_refused ... ok
test unapproved_source_refused ... ok
test bootstrap_is_reproducible ... ok
test chatty_agent_is_not_recorded_as_a_timeout ... ok
test unsaved_session_is_rebuilt_from_the_transcript ... ok
test noop_agent_is_not_a_render_success ... ok
test unimplemented_subcommands_fail_instead_of_pretending ... ok
test replay_cli_uses_the_current_engine_for_every_tool ... ok
test live_runs_wire_the_agent_to_designos_mcp_not_to_the_eval_binary ... ok
test stub_run_writes_valid_jsonl ... ok
test timed_out_agent_gets_sigterm_before_sigkill ... ok
test jobs_run_tasks_side_by_side_and_keep_record_order ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.65s

     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)

running 8 tests
test started_timestamp_is_rfc3339_utc ... ok
test only_a_growing_undo_stack_counts_as_an_edit ... ok
test mcp_client_reports_a_failing_tool_as_a_result_not_a_transport_error ... ok
test transcript_logging_failure_fails_the_call ... ok
test replay_stops_with_replay_diverged_when_a_recorded_success_now_fails ... ok
test replay_runs_in_a_sandbox_prepared_like_the_run ... ok
test replay_follows_ids_and_creates_after_apply_spec_do_not_collide ... ok
test replay_reproduces_metrics_of_the_original_run ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.83s

   Doc-tests designos_eval

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Acceptance 6 — Delete `out/` at the end

Command:
```sh
rm -rf out && test ! -e out && echo "OUT_REMOVED=0"
```

Output:
```text
OUT_REMOVED=0
```

---

## 3. Changes per File

- `crates/designos-mcp/src/tools/errors.rs`:
  - Updated `ToolError::with_received` and `ToolError::to_json_value` so `ErrorCode::PathOutsideWorkspace` never records or emits `received`.
- `crates/designos-mcp/src/engine.rs`:
  - Added `#[serde(default)] pub overwrite: bool` to `ExportFigArgs` (made `pub(crate)`) and added `"overwrite": {"type": "boolean", "default": false}` to `designos_export_fig_file`'s input schema in `engine::tools()`.
  - Added `has_control_or_bidi_char`, `validate_output_name`, `validate_existing_output_target`, and `write_checked_output_file` (which delegates to `crate::session_file_io::truncate(resolved, false)` and `crate::session_file_io::append(resolved, bytes, true)` and maps `session_file_io::refusal_of`).
  - Updated `"designos_export_fig_file" | "export_fig_file"` in `Engine::call` to validate output names, symlinks/folders/non-plain files, case-insensitive `.fig` extension, `overwrite`, and write via `write_checked_output_file`.
- `crates/designos-mcp/src/server.rs`:
  - Updated `input_path`, `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` to check workspace containment first (before any error branch that echoes the caller's path), validate output names (`validate_output_name`), inspect `symlink_metadata` on both the uncanonicalized requested path and resolved target (`validate_existing_output_target`), and compare `.fig` extensions case-insensitively.
  - Updated `"designos_save_file"`, `"designos_render_png"`, and `"designos_export_fig_file" | "export_fig_file"` in `Session::call_inner` to write exclusively through `crate::engine::write_checked_output_file`.
- `crates/designos-mcp/src/rules.rs`:
  - Updated `tool_error_codes("designos_export_fig_file")` to `&["\`INVALID_ARGUMENTS\`", "\`PATH_OUTSIDE_WORKSPACE\`", "\`FILE_EXISTS\`", "\`TOOL_DISABLED\`"]` (matching `designos_save_file` and staying within the 24,000-char rules pack budget).
- `crates/designos-mcp/tests/snapshots/tools.json`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`:
  - Updated via `cargo run -p designos-mcp --bin gen-tool-docs --locked` solely for `designos_export_fig_file`'s new `overwrite` property and `FILE_EXISTS` error code (required by `tests/tools_snapshot.rs::docs_blocks_are_current` and `rules::tests::prompt_resource_tool_identical`).
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Added 12 red-first integration tests covering all cases in Objectives 1–4 and Acceptance #2 (outside-workspace canary SHA-256 verification).
- `plans/reports/agy-1008-mcp-paths-evidence/red.log` & `plans/reports/agy-1008-mcp-paths-evidence/green.log`:
  - Saved verbatim red (`dc23fdd4d`) and green (`HEAD`) test outputs.

---

## 4. Every Error String Added, Verbatim

1. `"{display_path} has a name with a line break or a direction-control character. Rename it, then try again."` (`ErrorCode::InvalidArguments`)
2. `"{display_path} has a name longer than 255 bytes. Use a shorter name."` (`ErrorCode::InvalidArguments`)
3. `"{display_path} is a link, not a file. Delete the link, then try again."` (`ErrorCode::InvalidArguments`)
4. `"{display_path} is a folder, not a file. Choose a file path, then try again."` (`ErrorCode::InvalidArguments`)
5. `"{display_path} is not a plain file. Delete it, then try again."` (`ErrorCode::InvalidArguments`)
6. `"{display_path} already exists. Pass overwrite: true to replace it."` (`ErrorCode::FileExists`)
7. `"{display_path} already exists. Choose a new path, then try again."` (`ErrorCode::FileExists`, for tools without an `overwrite` parameter such as `designos_render_png`)
8. `"{display_path} is read-only. Make it writable first, or choose a new path."` (`ErrorCode::InvalidArguments`)
9. `"{display_path} has a second name (a hard link). Delete it, then try again."` (`ErrorCode::InvalidArguments`)
10. `"{display_path} can't be written: {e}. Check its permissions and free space, then try again."` (`ErrorCode::InternalError`)
11. `"{path} does not end with .fig. Choose a path ending with .fig, then try again."` / `"{} does not end with .fig. Choose a path ending with .fig, then try again."` (`ErrorCode::InvalidArguments`)

---

## 5. Findings Outside Scope

- **#1074 (`crates/designos-app/src/terminal.rs`):** When a document has not been saved to disk yet, Studio launches the agent terminal with `/` as the workspace root. Per task instructions, `#1074` is context-only and owned by another worker in `crates/designos-app/`.
- **`designos_save_file` and `designos_render_png` extension case-sensitivity:** Objective 1 specifically requested case-insensitive `.fig` extension comparison on `designos_export_fig_file`. `output_path_allow_overwrite` (`.designos`) and `png_output_path` (`.png`) still use exact lowercase extension checks, left unchanged to avoid unrequested behavior changes outside scope.

---

## 6. Assumptions

- **`ASSUMPTION` (verified against `rules.rs` and `tools_snapshot.rs`):** In `crates/designos-mcp/src/rules.rs`, `tool_error_codes("designos_export_fig_file")` replaced `` "`INTERNAL_ERROR`" `` with `` "`FILE_EXISTS`" `` (matching `tool_error_codes("designos_save_file")`, which lists `INVALID_ARGUMENTS`, `PATH_OUTSIDE_WORKSPACE`, `FILE_EXISTS`, `TOOL_DISABLED`) because adding both `(optional: \`overwrite\`)` (+23 chars) and `, \`FILE_EXISTS\`` (+15 chars) while retaining `\`INTERNAL_ERROR\`` would push `generated/rules.md` to `24,010` chars (`> MAX_RULES_CHARS = 24,000`). With this alignment, `generated/rules.md` is `23,992` chars (`<= 24,000`).
- **`ASSUMPTION` (verified against `tools_snapshot.rs`):** Running `cargo run -p designos-mcp --bin gen-tool-docs` updates `crates/designos-mcp/tests/snapshots/tools.json` together with `crates/designos-mcp/generated/rules.md` and `skills/design-os-studio/rules.md`, which `tests/tools_snapshot.rs::docs_blocks_are_current` and `src/rules.rs::tests::prompt_resource_tool_identical` assert are byte-for-byte identical to `generate_default_rules_pack()`.

---

## Round 2

### 1. What Could NOT Be Verified

- **Windows cross-target check (`x86_64-pc-windows-gnu`):** Running `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked` on this macOS worker stops at `cfg-if`/`core` with `error[E0463]: can't find crate for core` (`the x86_64-pc-windows-gnu target may not be installed`). Adding `#![cfg(unix)]` at line 1 of `crates/designos-mcp/tests/mcp_paths.rs` gates the entire unix-specific integration test crate off on Windows so Windows CI does not compile `std::os::unix` imports or `libc::mkfifo`/`RLIMIT_FSIZE`.

---

### 2. Acceptance Table & Command Outputs

| # | Gate | Command | Status |
|---|---|---|---|
| 1 | Red / Green (`r2-red.log` -> `r2-green.log`) | `cargo test -p designos-mcp --test mcp_paths --locked` on `f2b533c1` (8 Round 2 tests fail) and on `HEAD` (20/20 pass) | PASS |
| 2 | Rustfmt | `cargo fmt --all -- --check` | PASS (`exit 0`) |
| 3 | Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS (`exit 0`) |
| 4 | Test suites (`designos-mcp` & `designos-eval`) | `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked` under short-`$TMPDIR` harness | PASS (`exit 0`) |
| 5 | Windows target check (`x86_64-pc-windows-gnu`) | `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked` | Target stdlib not installed locally (`#![cfg(unix)]` added at top of `tests/mcp_paths.rs`) |
| 6 | Clean workspace (`out/` removed) | `rm -rf out && test ! -e out` | PASS (`exit 0`) |

#### Acceptance 1 — Red / Green (`r2-red.log` and `r2-green.log`)

- Full logs:
  - `plans/reports/agy-1008-mcp-paths-evidence/r2-red.log`
  - `plans/reports/agy-1008-mcp-paths-evidence/r2-green.log`

Command (`r2-red.log` against `f2b533c1` `src/`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r2-red.log 2>&1 || true
rm -rf "$S"
```

Output (`r2-red.log` summary):
```text
thread 'export_fig_rules_list_both_file_exists_and_internal_error' (47039331) panicked at crates/designos-mcp/tests/mcp_paths.rs:677:5:
designos_export_fig_file rules must list INTERNAL_ERROR: ["`INVALID_ARGUMENTS`", "`PATH_OUTSIDE_WORKSPACE`", "`FILE_EXISTS`", "`TOOL_DISABLED`"]

thread 'argument_errors_never_echo_outside_paths_in_corrected_example_or_received' (47039323) panicked at crates/designos-mcp/tests/mcp_paths.rs:695:9:
designos_render_png {"bogus":1,"node_id":1,"path":"/etc/SECRETNAME.png"} leaked outside path in argument error reply: ...

thread 'in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted' (47039337) panicked at crates/designos-mcp/tests/mcp_paths.rs:662:33:
save_file via alias (/var/folders/.../alias/alias.designos) must succeed: {"code":"PATH_OUTSIDE_WORKSPACE",...}

thread 'new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes' (47039338) panicked at crates/designos-mcp/tests/mcp_paths.rs:608:9:
assertion `left == right` failed: a.designos mode was 600, expected 644 (not 0600)
  left: 384
 right: 420

thread 'concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json' (47039325) panicked at crates/designos-mcp/tests/mcp_paths.rs:497:5:
assertion `left == right` failed: two concurrent no-overwrite saves must never both succeed (both_ok=298/300)
  left: 298
 right: 0

thread 'file_planted_during_no_overwrite_export_is_never_overwritten' (47039336) panicked at crates/designos-mcp/tests/mcp_paths.rs:535:5:
assertion `left == right` failed: a file planted between check and write must never be overwritten without overwrite: true (lost=276/300)
  left: 276
 right: 0

thread 'partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry' (47039351) panicked at crates/designos-mcp/tests/mcp_paths.rs:577:5:
failed write must not leave a partial new.designos on disk

thread 'trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first' (47039354) panicked at crates/designos-mcp/tests/mcp_paths.rs:716:41:
trailing slash must be refused as folder: Object {"bytes": Number(22756), ..., "saved": Bool(true)}

failures:
    argument_errors_never_echo_outside_paths_in_corrected_example_or_received
    concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json
    export_fig_rules_list_both_file_exists_and_internal_error
    file_planted_during_no_overwrite_export_is_never_overwritten
    in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted
    new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes
    partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry
    trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first

test result: FAILED. 12 passed; 8 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.65s
```

Command (`r2-green.log` on `HEAD`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r2-green.log 2>&1
rm -rf "$S"
```

Output (`r2-green.log`):
```text
running 20 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok
test partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.96s
```

#### Acceptance 2 — `cargo fmt --all -- --check`

Command:
```sh
cargo fmt --all -- --check && echo "FMT_CHECK=0"
```

Output:
```text
FMT_CHECK=0
```

#### Acceptance 3 — `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings && echo "CLIPPY_EXIT=0"
rm -rf "$S"
```

Output:
```text
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.90s
CLIPPY_EXIT=0
```

#### Acceptance 4 — `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked
rm -rf "$S"
```

Output (`designos-mcp` & `designos-eval`):
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.67s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.71s
     Running tests/session_file_safety.rs (out/cargo/debug/deps/session_file_safety-...)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/session_file_sites.rs (out/cargo/debug/deps/session_file_sites-...)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

#### Acceptance 5 — `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu`

Command:
```sh
cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked
```

Output:
```text
error[E0463]: can't find crate for `core`
  |
  = note: the `x86_64-pc-windows-gnu` target may not be installed
  = help: consider downloading the target with `rustup target add x86_64-pc-windows-gnu`
```

---

### 3. Changes per File (Round 2)

- `crates/designos-mcp/src/engine.rs`:
  - Consolidated output target validation into `validate_existing_output_target(target_meta, display_path, ptr, ext, overwrite, supports_overwrite)` called by both `Engine::call` and `Session` (`output_path_allow_overwrite`, `png_output_path`, `fig_output_path`). It validates `validate_output_name`, refuses symlinks, refuses folders and paths ending in `/` (or `\.` / `.`), checks the required extension (`.designos`, `.png`, or case-insensitive `.fig`) with cause-first messages, and refuses existing files (when `!overwrite`), hard links (`nlink > 1`), non-plain files (FIFOs/sockets/devices), and read-only files.
  - Updated `write_checked_output_file(resolved, display_path, ptr, bytes, overwrite)`:
    - When `overwrite == false`: opens `resolved` with `OpenOptions::new().write(true).create_new(true)` (`O_CREAT | O_EXCL`) and `O_NOFOLLOW` on unix (using the process's normal umask `0644`, not `0600`), maps `ErrorKind::AlreadyExists` to `ErrorCode::FileExists`, and if `write_all`/`sync_all` fails, drops the handle, removes the newly created file, and sets `safe_to_retry` to whether the partial file was removed.
    - When `overwrite == true`: checks `symlink_metadata(resolved)` and refuses symlinks, folders, non-plain files, hard links (`nlink > 1`), and read-only files; opens a short-named sibling temp file `.designos-out-{pid}-{seq}.tmp` in the same directory with `create_new(true)` + `O_NOFOLLOW` (so 255-byte target filenames never exceed `NAME_MAX`), copies `old.permissions()` when replacing an existing file (or respects default umask `0644` when creating a new file), calls `write_all` + `sync_all`, and atomically renames `&tmp` over `resolved`, unlinking `&tmp` on any failure so the original file is never destroyed and no partial file remains.
- `crates/designos-mcp/src/server.rs`:
  - Deleted the three pre-check blocks in `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` that called `Path::new(path).canonicalize()` on a not-yet-existing file (which rejected valid in-workspace absolute paths through symlink aliases or macOS `/var` vs `/private/var`).
  - Delegated all output target checks in `output_path_allow_overwrite`, `png_output_path`, and `fig_output_path` to `crate::engine::validate_existing_output_target`.
  - Updated `call_with_safety` so `safe = err.safe_to_retry && (after == before) && (files_created == 0)`.
  - Passed `a.overwrite`, `false`, and `a.overwrite` into `write_checked_output_file` for `designos_save_file`, `designos_render_png`, and `designos_export_fig_file`.
- `crates/designos-mcp/src/tools/errors.rs`:
  - Updated `build_unknown_field_corrected_example` and `contextualize_corrected_example` to strip `"path"` and `"output_path"` from `corrected_example` and `received` on argument errors (`err.code != ErrorCode::FileExists && err.code != ErrorCode::UnsavedChanges`), and prevented `contextualize_corrected_example` from re-inserting unknown fields or path arguments on `ErrorCode::UnknownField`.
- `crates/designos-mcp/src/rules.rs`, `crates/designos-mcp/generated/rules.md`, `skills/design-os-studio/rules.md`:
  - Restored `` "`INTERNAL_ERROR`" `` alongside `` "`FILE_EXISTS`" `` in `tool_error_codes("designos_export_fig_file")`, adjusted `MAX_PURPOSE_CHARS` from `106` to `104` to keep `generated/rules.md` within the 24,000-character budget (`23,972` chars), and regenerated the rules files via `gen-tool-docs`.
- `crates/designos-mcp/tests/session_file_sites/allowlist.rs`:
  - Added the single allowlist entry `Allowed { file: "src/engine.rs", func: Some("write_checked_output_file"), why: WORKSPACE_PATH },`.
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Added `#![cfg(unix)]` at the top of the test file and added 8 Round 2 tests (`20` tests total).

---

### 4. User-Facing Strings Added or Changed in Round 2 (Verbatim) & Full Refused Control / Bidi Set

#### User-Facing Strings Added or Changed
1. `"{display_path} does not end with .designos. Choose a path ending with .designos, then try again."` (`ErrorCode::InvalidArguments`, replaces `"Save path must end in .designos"`)
2. `"{display_path} does not end with .png. Choose a path ending with .png, then try again."` (`ErrorCode::InvalidArguments`, replaces `"Render path must end in .png"`)
3. `"{display_path} does not end with .fig. Choose a path ending with .fig, then try again."` (`ErrorCode::InvalidArguments`, unified across `Engine` and `Session` via `"{display_path} does not end with .{ext}. Choose a path ending with .{ext}, then try again."`)
4. `"{display_path} is a folder, not a file. Choose a file path, then try again."` (`ErrorCode::InvalidArguments`, now also returned for paths ending with `/`, `/.`, or `.`)

#### Full Refused Control & Bidirectional Character Set (`has_control_or_bidi_char`)
- **Unicode Cc control characters (`char::is_control()`):**
  - `U+0000..=U+001F` (C0 controls, including `\0`, `\t`, `\n`, `\r`, `\x1B`)
  - `U+007F` (`DELETE`)
  - `U+0080..=U+009F` (C1 controls)
- **Bidirectional embedding & override controls:**
  - `U+202A` (`LEFT-TO-RIGHT EMBEDDING`, `LRE`)
  - `U+202B` (`RIGHT-TO-LEFT EMBEDDING`, `RLE`)
  - `U+202C` (`POP DIRECTIONAL FORMATTING`, `PDF`)
  - `U+202D` (`LEFT-TO-RIGHT OVERRIDE`, `LRO`)
  - `U+202E` (`RIGHT-TO-LEFT OVERRIDE`, `RLO`)
- **Bidirectional isolate controls:**
  - `U+2066` (`LEFT-TO-RIGHT ISOLATE`, `LRI`)
  - `U+2067` (`RIGHT-TO-LEFT ISOLATE`, `RLI`)
  - `U+2068` (`FIRST STRONG ISOLATE`, `FSI`)
  - `U+2069` (`POP DIRECTIONAL ISOLATE`, `PDI`)
- **Implicit directional marks (retained per Ruling 8):**
  - `U+200E` (`LEFT-TO-RIGHT MARK`, `LRM`)
  - `U+200F` (`RIGHT-TO-LEFT MARK`, `RLM`)
  - `U+061C` (`ARABIC LETTER MARK`, `ALM`)

---

## Round 3

### 1. What could NOT be verified

- **Nothing in the assigned Round 3 scope was left unverified locally.** Cross-OS runtime execution on Windows and Ubuntu is handled by GitHub Actions CI (`#![cfg(unix)]` is at line 1 of both `crates/designos-mcp/tests/mcp_paths.rs` and `crates/designos-mcp/tests/mcp_paths_full_disk.rs`).
- **Conclusion (`FACT`):**
  - `FACT`: Reproduced finding **N-10** on `d044dca4` before editing: building `mcp_paths` (`out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c`) and running it at the default thread count hung on run 2 (`rc=142`, `SIGALRM`) because six tests in `mcp_paths.rs` called `TempDir::new` twice in one function while `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` queued `RLIMIT_LOCK.write()` in between.
  - `FACT`: Moved `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` into its own `#![cfg(unix)]` integration test binary `crates/designos-mcp/tests/mcp_paths_full_disk.rs` (running in its own process) and deleted `RLIMIT_LOCK` and its guard from `TempDir` in `crates/designos-mcp/tests/mcp_paths.rs`.
  - `FACT`: In `crates/designos-mcp/src/engine.rs`, updated `write_checked_output_file` and `validate_existing_output_target` for **N-1** so that when `designos_render_png` hits an existing `.png` file or loses a race at `create_new`, it returns the base (`dc23fdd4`) message `format!("Cannot create PNG file (choose an unused .png filename): {e}")` without mentioning `overwrite` or setting `corrected_example`, while tools that support `overwrite` (`designos_save_file`, `designos_export_fig_file`) keep `format!("{display_path} already exists. Pass overwrite: true to replace it.")` with `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`.
  - `FACT`: Added `render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint` in `crates/designos-mcp/tests/mcp_paths.rs`, verified it fails on `d044dca4` (`plans/reports/agy-1008-mcp-paths-evidence/r3-red.log`) and passes on `HEAD` (`plans/reports/agy-1008-mcp-paths-evidence/r3-green.log`), and ran 40 back-to-back runs of `mcp_paths` (`ok=40 hang=0 fail=0`) plus 20 back-to-back runs of `mcp_paths_full_disk` (`ok=20 hang=0 fail=0`) at the default thread count under a 60 s timeout (`plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`).
- **Weakest link:** `write_checked_output_file` infers `supports_overwrite` from whether `display_path`'s extension is `.png` because `server.rs` is outside the Round 3 allowed-files list and every caller validates the required extension (`.designos`, `.png`, `.fig`) before calling `write_checked_output_file`.
- **Assumptions (`ASSUMPTION`):**
  - `ASSUMPTION` (verified against `server.rs` and `engine.rs`): All three callers of `write_checked_output_file` (`designos_save_file`, `designos_render_png`, `designos_export_fig_file`) run `validate_existing_output_target` first, which guarantees `display_path` has extension `.designos`, `.png`, or `.fig` before `write_checked_output_file` executes.
- **Unresolved questions:** None.

---

### 2. Acceptance Table & Command Outputs

| # | Gate | Command | Result |
|---|---|---|---|
| 1 | 40x `mcp_paths` loop + 20x `mcp_paths_full_disk` loop at default thread count under 60 s timeout (`r3-loop.log`) | `cargo test -p designos-mcp --test mcp_paths --no-run --locked` + 40 runs; `cargo test -p designos-mcp --test mcp_paths_full_disk --no-run --locked` + 20 runs | **PASS** (`mcp_paths`: `runs=40 ok=40 hang=0 fail=0`; `mcp_paths_full_disk`: `runs=20 ok=20 hang=0 fail=0`) |
| 2 | N-1 test fails on `d044dca4` (`r3-red.log`) and passes on head (`r3-green.log`) | `cargo test -p designos-mcp --test mcp_paths --locked` | **PASS** (`r3-red.log`: `19 passed; 1 failed`; `r3-green.log`: `20 passed; 0 failed`) |
| 3 | `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --locked -- -D warnings` | `cargo fmt --all -- --check` && `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (`FMT_EXIT=0`, `CLIPPY_EXIT=0`) |
| 4 | `cargo test -p designos-mcp --locked` and `cargo test -p designos-eval --locked` | Both run inside mandatory short-`$TMPDIR` harness | **PASS** (`MCP_TEST_EXIT=0`, `EVAL_TEST_EXIT=0`) |
| 5 | Commit, push, update PR #1103 body with Round 3 line, keep draft, delete `out/` | `rm -rf out && test ! -e out` | **PASS** (`OUT_REMOVED=0`) |

#### N-10 Reproduction on `d044dca4` (Before Fix)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
BIN=$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --no-run --locked 2>&1 | sed -n 's/.*Executable tests\/mcp_paths.rs (\(.*\))/\1/p')
echo "BIN=$BIN"
for i in $(seq 1 20); do
  if perl -e 'alarm 15; exec @ARGV' env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$BIN" >/dev/null 2>&1; then
    echo "run $i: ok"
  else
    rc=$?
    echo "run $i: HUNG/FAILED rc=$rc"
    break
  fi
done
rm -rf "$S"
```

Output:
```text
BIN=out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c
run 1: ok
run 2: HUNG/FAILED rc=142
```

#### Acceptance 1 — 40x `mcp_paths` & 20x `mcp_paths_full_disk` Loops (`plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
LOG="plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log"
: > "$LOG"
BUILD_PATHS=$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --no-run --locked 2>&1)
echo "$BUILD_PATHS" >> "$LOG"
BIN_PATHS=$(printf '%s\n' "$BUILD_PATHS" | sed -n 's/.*Executable tests\/mcp_paths.rs (\(.*\))/\1/p')
echo "BIN_PATHS=$BIN_PATHS" >> "$LOG"

ok=0; hang=0; fail=0
for i in $(seq 1 40); do
  OUT=$(perl -e 'alarm 60; exec @ARGV' env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$BIN_PATHS" 2>&1)
  rc=$?
  RES=$(printf '%s\n' "$OUT" | grep "test result:" || true)
  if [ "$rc" -eq 0 ]; then
    ok=$((ok + 1))
    echo "mcp_paths run $i/40: rc=0 $RES" >> "$LOG"
  elif [ "$rc" -eq 142 ]; then
    hang=$((hang + 1))
    echo "mcp_paths run $i/40: HUNG rc=$rc" >> "$LOG"
  else
    fail=$((fail + 1))
    echo "mcp_paths run $i/40: FAIL rc=$rc $RES" >> "$LOG"
  fi
done
echo "mcp_paths summary: runs=40 ok=$ok hang=$hang fail=$fail" >> "$LOG"

BUILD_FD=$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths_full_disk --no-run --locked 2>&1)
echo "$BUILD_FD" >> "$LOG"
BIN_FD=$(printf '%s\n' "$BUILD_FD" | sed -n 's/.*Executable tests\/mcp_paths_full_disk.rs (\(.*\))/\1/p')
echo "BIN_FD=$BIN_FD" >> "$LOG"

fd_ok=0; fd_hang=0; fd_fail=0
for i in $(seq 1 20); do
  OUT=$(perl -e 'alarm 60; exec @ARGV' env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" "$BIN_FD" 2>&1)
  rc=$?
  RES=$(printf '%s\n' "$OUT" | grep "test result:" || true)
  if [ "$rc" -eq 0 ]; then
    fd_ok=$((fd_ok + 1))
    echo "mcp_paths_full_disk run $i/20: rc=0 $RES" >> "$LOG"
  elif [ "$rc" -eq 142 ]; then
    fd_hang=$((fd_hang + 1))
    echo "mcp_paths_full_disk run $i/20: HUNG rc=$rc" >> "$LOG"
  else
    fd_fail=$((fd_fail + 1))
    echo "mcp_paths_full_disk run $i/20: FAIL rc=$rc $RES" >> "$LOG"
  fi
done
echo "mcp_paths_full_disk summary: runs=20 ok=$fd_ok hang=$fd_hang fail=$fd_fail" >> "$LOG"
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.22s
  Executable tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
BIN_PATHS=out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c
mcp_paths run 1/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 2/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 3/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.03s
mcp_paths run 4/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
mcp_paths run 5/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 6/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.95s
mcp_paths run 7/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.74s
mcp_paths run 8/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.00s
mcp_paths run 9/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
mcp_paths run 10/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.02s
mcp_paths run 11/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.04s
mcp_paths run 12/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s
mcp_paths run 13/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.01s
mcp_paths run 14/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s
mcp_paths run 15/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.98s
mcp_paths run 16/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.74s
mcp_paths run 17/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.02s
mcp_paths run 18/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.80s
mcp_paths run 19/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
mcp_paths run 20/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.70s
mcp_paths run 21/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.85s
mcp_paths run 22/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.63s
mcp_paths run 23/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.75s
mcp_paths run 24/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
mcp_paths run 25/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.93s
mcp_paths run 26/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.97s
mcp_paths run 27/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.77s
mcp_paths run 28/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
mcp_paths run 29/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.22s
mcp_paths run 30/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.02s
mcp_paths run 31/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s
mcp_paths run 32/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.97s
mcp_paths run 33/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.90s
mcp_paths run 34/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.00s
mcp_paths run 35/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.12s
mcp_paths run 36/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.82s
mcp_paths run 37/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
mcp_paths run 38/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s
mcp_paths run 39/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.94s
mcp_paths run 40/40: rc=0 test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.00s
mcp_paths summary: runs=40 ok=40 hang=0 fail=0
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.53s
  Executable tests/mcp_paths_full_disk.rs (out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b)
BIN_FD=out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b
mcp_paths_full_disk run 1/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
mcp_paths_full_disk run 2/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 3/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 4/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 5/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 6/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 7/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 8/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 9/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 10/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 11/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 12/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 13/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 14/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 15/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 16/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 17/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 18/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 19/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk run 20/20: rc=0 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
mcp_paths_full_disk summary: runs=20 ok=20 hang=0 fail=0
```

#### Acceptance 2 — N-1 Red (`d044dca4`) and Green (`HEAD`) (`r3-red.log` and `r3-green.log`)

Command (`r3-red.log` against `d044dca4` `src/`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r3-red.log 2>&1 || true
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r3-red.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.04s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 20 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... FAILED
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

failures:

---- render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint stdout ----

thread 'render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint' (48115139) panicked at crates/designos-mcp/tests/mcp_paths.rs:571:13:
render_png has no overwrite option and must never tell the caller to pass overwrite: png-race-0.png already exists. Pass overwrite: true to replace it.
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint

test result: FAILED. 19 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.91s

error: test failed, to rerun pass `-p designos-mcp --test mcp_paths`
```

Command (`r3-green.log` on `HEAD`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r3-green.log 2>&1
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r3-green.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.72s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 20 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.75s
```

#### Acceptance 3 — `cargo fmt --all -- --check` & `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cargo fmt --all -- --check && echo "FMT_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings && echo "CLIPPY_EXIT=$?"
rm -rf "$S"
```

Output:
```text
FMT_EXIT=0
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-eval)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-gpu)
    Checking egui-wgpu v0.35.0
    Checking eframe v0.35.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.88s
CLIPPY_EXIT=0
```

#### Acceptance 4 — `cargo test -p designos-mcp --locked` & `cargo test -p designos-eval --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked && echo "MCP_TEST_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked && echo "EVAL_TEST_EXIT=$?"
rm -rf "$S"
```

Output:
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.63s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-616b53869353a607)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-f413a393b4d55d8f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-1aa6ab2233591601)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-01318f1c86eb2572)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-f067c8c79555baaa)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-378a191ea16cbc7f)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.18s
     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-23ae83d21f949530)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-ab113f1c25ffacf0)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.90s
     Running tests/mcp_paths_full_disk.rs (out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/session.rs (out/cargo/debug/deps/session-70f6c9de7d7f9973)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/session_file_sites/main.rs (out/cargo/debug/deps/session_file_sites-1a885e87e98ff7af)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/session_files/main.rs (out/cargo/debug/deps/session_files-657090e65be613f1)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-75b28c66cba35b01)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
   Doc-tests designos_mcp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MCP_TEST_EXIT=0
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_eval-30aca93b7d8d8c4a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.11s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
   Doc-tests designos_eval
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EVAL_TEST_EXIT=0
```

#### Acceptance 5 — Delete `out/`

Command:
```sh
rm -rf out && test ! -e out && echo "OUT_REMOVED=0"
```

Output:
```text
OUT_REMOVED=0
```

---

### 3. Changes per File (Round 3)

- `crates/designos-mcp/tests/mcp_paths_full_disk.rs`:
  - Created a dedicated `#![cfg(unix)]` integration test file containing `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` so `RLIMIT_FSIZE` manipulation runs in its own isolated test process.
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Removed `static RLIMIT_LOCK: RwLock<()>` and the `RwLockReadGuard` field from `TempDir`, eliminating the reader/writer lock acquisition deadlock across multi-`TempDir` tests.
  - Moved `partial_write_failure_preserves_original_leaves_no_partial_file_and_reports_honest_retry` out to `mcp_paths_full_disk.rs`.
  - Added `render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint` testing 100 concurrent `designos_render_png` races and verifying that `render_png` `FILE_EXISTS` errors use the base wording (`"Cannot create PNG file (choose an unused .png filename): ..."`) without mentioning `overwrite` or setting `corrected_example`, while `designos_save_file` and `designos_export_fig_file` retain `"Pass overwrite: true to replace it."` and `corrected_example`.
- `crates/designos-mcp/src/engine.rs`:
  - Updated `write_checked_output_file` and `validate_existing_output_target` so `AlreadyExists` on `.png` outputs (`!supports_overwrite`) returns `ToolError::new(ErrorCode::FileExists, ptr, format!("Cannot create PNG file (choose an unused .png filename): {e}")).with_received(json!(display_path))`, and on tools supporting `overwrite` returns `format!("{display_path} already exists. Pass overwrite: true to replace it.")` with `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`.
- `plans/reports/agy-1008-mcp-paths-evidence/r3-red.log`, `plans/reports/agy-1008-mcp-paths-evidence/r3-green.log`, `plans/reports/agy-1008-mcp-paths-evidence/r3-loop.log`:
  - Saved verbatim Red, Green, and 40x/20x loop outputs.

---

## Round 4

### 1. What Could NOT Be Verified

- **Windows cross-target check (`x86_64-pc-windows-gnu`):** Running `cargo check -p designos-mcp --tests --target x86_64-pc-windows-gnu --locked` on this macOS worker cannot compile `core` because the `x86_64-pc-windows-gnu` target standard library is not installed locally (`#![cfg(unix)]` remains at line 1 of `crates/designos-mcp/tests/mcp_paths.rs` and `crates/designos-mcp/tests/mcp_paths_full_disk.rs`).

---

### 2. Acceptance Table & Command Outputs (Round 4)

- **Epistemic Ledger:**
  - **FACT:** On `20913aef`, `validate_existing_output_target` (`crates/designos-mcp/src/engine.rs:1605`) returned `Ok(())` when `target_meta` existed and `!overwrite && !supports_overwrite`, causing `designos_render_png` to render the full PNG before failing at `write_checked_output_file`, and `write_checked_output_file` (`crates/designos-mcp/src/engine.rs:1657-1666`) sniffed the `.png` extension and returned `Cannot create PNG file (choose an unused .png filename): {e}` on `AlreadyExists`.
  - **FACT:** Restoring the early refusal in `validate_existing_output_target` for `!supports_overwrite` and passing `supports_overwrite: bool` explicitly to `write_checked_output_file` from its four call sites (`engine.rs:911`, `server.rs:1037`, `server.rs:1069`, `server.rs:1079`) makes both early validation and race-loser `AlreadyExists` return the exact cause-first wording `"{display_path} already exists. Choose a new path, then try again."` with `received: Some(json!(display_path))` and `corrected_example: None`, while tools with `supports_overwrite = true` keep `"{display_path} already exists. Pass overwrite: true to replace it."` and `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`.
  - **FACT:** Both the updated race test (`render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint`) and the new non-race existing-PNG test (`render_png_refuses_existing_file_early_and_preserves_bytes`) fail on `20913aef` (`r4-red.log`) and pass on `HEAD` (`r4-green.log`), and 20 back-to-back runs of the `mcp_paths` test binary at default thread count pass with 0 hangs (`r4-loop.log`).
  - **Weakest link:** Windows target stdlib is not installed on this macOS worker, so Windows compilation of `designos-mcp` relies on CI (`#![cfg(unix)]` gates unix-only integration tests).

| # | Gate | Command | Status |
|---|---|---|---|
| 1 | Red / Green (`r4-red.log` -> `r4-green.log`) | `cargo test -p designos-mcp --test mcp_paths --locked` against `20913aef` `src/` (`19 passed; 2 failed`) and on `HEAD` (`21 passed; 0 failed`) | PASS |
| 2 | Rustfmt & Clippy | `cargo fmt --all -- --check` (`exit 0`) & `cargo clippy --workspace --all-targets --locked -- -D warnings` (`exit 0`) | PASS |
| 3 | Test suites (`designos-mcp` & `designos-eval`) | `cargo test -p designos-mcp --locked` & `cargo test -p designos-eval --locked` under short-`$TMPDIR` harness | PASS |
| 4 | 20x back-to-back `mcp_paths` binary loop (`r4-loop.log`) | 20 runs of built `mcp_paths` binary at default thread count, 60 s timeout each (`runs=20 ok=20 hang=0 fail=0`) | PASS |
| 5 | Clean workspace (`out/` removed) | `rm -rf out && test ! -e out` | PASS |

#### Acceptance 1 — Red (`20913aef`) and Green (`HEAD`) (`r4-red.log` and `r4-green.log`)

Command (`r4-red.log` against `20913aef` `src/`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r4-red.log 2>&1 || true
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r4-red.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.28s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 21 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test render_png_refuses_existing_file_early_and_preserves_bytes ... FAILED
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... FAILED
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test export_fig_validates_extension_case_insensitively ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

failures:

---- render_png_refuses_existing_file_early_and_preserves_bytes stdout ----

thread 'render_png_refuses_existing_file_early_and_preserves_bytes' (48471518) panicked at crates/designos-mcp/tests/mcp_paths.rs:621:5:
assertion `left == right` failed
  left: "Cannot create PNG file (choose an unused .png filename): File exists (os error 17)"
 right: "existing.png already exists. Choose a new path, then try again."
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint stdout ----

thread 'render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint' (48471517) panicked at crates/designos-mcp/tests/mcp_paths.rs:571:13:
assertion `left == right` failed: render_png FILE_EXISTS message must use ruling-1 wording, got: Cannot create PNG file (choose an unused .png filename): File exists (os error 17)
  left: "Cannot create PNG file (choose an unused .png filename): File exists (os error 17)"
 right: "png-race-0.png already exists. Choose a new path, then try again."


failures:
    render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint
    render_png_refuses_existing_file_early_and_preserves_bytes

test result: FAILED. 19 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.93s

error: test failed, to rerun pass `-p designos-mcp --test mcp_paths`
```

Command (`r4-green.log` on `HEAD`):
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --locked > plans/reports/agy-1008-mcp-paths-evidence/r4-green.log 2>&1
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r4-green.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.43s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)

running 21 tests
test export_fig_rules_list_both_file_exists_and_internal_error ... ok
test export_fig_refuses_folder ... ok
test export_fig_refuses_dangling_symlink_inside_and_outside_workspace ... ok
test export_fig_refuses_symlink_inside_workspace ... ok
test export_fig_refuses_symlink_to_outside_canary_and_leaves_canary_untouched ... ok
test argument_errors_never_echo_outside_paths_in_corrected_example_or_received ... ok
test file_creation_tools_report_name_too_long_instead_of_file_exists ... ok
test trailing_slash_is_refused_as_folder_and_extension_messages_put_cause_first ... ok
test file_creation_tools_refuse_control_characters_in_names ... ok
test render_png_refuses_existing_file_early_and_preserves_bytes ... ok
test file_creation_tools_refuse_bidi_override_characters_in_names ... ok
test save_file_and_render_png_refuse_dangling_and_live_symlinks_and_preserve_canary ... ok
test outside_workspace_refusal_never_echoes_path_and_preserves_canary ... ok
test export_fig_refuses_existing_file_unless_overwrite_is_true ... ok
test export_fig_validates_extension_case_insensitively ... ok
test bare_engine_export_fig_enforces_all_target_checks ... ok
test in_workspace_absolute_paths_via_symlink_and_var_alias_are_accepted ... ok
test new_files_respect_umask_0644_and_255_byte_names_work_in_both_modes ... ok
test render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint ... ok
test concurrent_no_overwrite_saves_never_both_succeed_or_corrupt_json ... ok
test file_planted_during_no_overwrite_export_is_never_overwritten ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
```

#### Acceptance 2 — `cargo fmt --all -- --check` & `cargo clippy --workspace --all-targets --locked -- -D warnings`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
cargo fmt --all -- --check && echo "FMT_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings && echo "CLIPPY_EXIT=$?"
rm -rf "$S"
```

Output:
```text
FMT_EXIT=0
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-eval)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-gpu)
    Checking egui-wgpu v0.35.0
    Checking eframe v0.35.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.95s
CLIPPY_EXIT=0
```

#### Acceptance 3 — `cargo test -p designos-mcp --locked` & `cargo test -p designos-eval --locked`

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --locked && echo "MCP_TEST_EXIT=$?"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-eval --locked && echo "EVAL_TEST_EXIT=$?"
rm -rf "$S"
```

Output:
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-96fd55246d4a3a66)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.16s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-616b53869353a607)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-f413a393b4d55d8f)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/colour_detach.rs (out/cargo/debug/deps/colour_detach-1aa6ab2233591601)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/colour_var_library.rs (out/cargo/debug/deps/colour_var_library-01318f1c86eb2572)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/colour_var_refs.rs (out/cargo/debug/deps/colour_var_refs-f067c8c79555baaa)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/list_lint_rules.rs (out/cargo/debug/deps/list_lint_rules-378a191ea16cbc7f)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s
     Running tests/live_doc_epoch.rs (out/cargo/debug/deps/live_doc_epoch-23ae83d21f949530)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/live_session_guards.rs (out/cargo/debug/deps/live_session_guards-ab113f1c25ffacf0)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.14s
     Running tests/mcp_paths_full_disk.rs (out/cargo/debug/deps/mcp_paths_full_disk-a818e059bf9cad4b)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/session.rs (out/cargo/debug/deps/session-70f6c9de7d7f9973)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/session_file_sites/main.rs (out/cargo/debug/deps/session_file_sites-1a885e87e98ff7af)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running tests/session_files/main.rs (out/cargo/debug/deps/session_files-657090e65be613f1)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/spec_text_colour.rs (out/cargo/debug/deps/spec_text_colour-75b28c66cba35b01)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/stdio.rs (out/cargo/debug/deps/stdio-9963b9496c82d7e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/text_fill.rs (out/cargo/debug/deps/text_fill-b3d88a68a195ca3e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/tools_snapshot.rs (out/cargo/debug/deps/tools_snapshot-e6488781337fbb09)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/version.rs (out/cargo/debug/deps/version-7878cb959d478560)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
   Doc-tests designos_mcp
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
MCP_TEST_EXIT=0
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-9a9f1f166d66684f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_eval-30aca93b7d8d8c4a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/harness.rs (out/cargo/debug/deps/harness-28beba16668beb59)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s
     Running tests/real_engine.rs (out/cargo/debug/deps/real_engine-4a6cab08ce582fe8)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
   Doc-tests designos_eval
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EVAL_TEST_EXIT=0
```

#### Acceptance 4 — 20 back-to-back runs of `mcp_paths` test binary (`r4-loop.log`)

Command:
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  BIN="$(env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-mcp --test mcp_paths --no-run --locked 2>&1 | tee /dev/stderr | sed -n 's/.*Executable tests\/mcp_paths\.rs (\(.*\))/\1/p' | tail -n 1)"
  echo "BIN=$BIN"
  ok=0; hang=0; fail=0
  for i in $(seq 1 20); do
    out="$(python3 -c '
import os, subprocess, sys
bin_path, idx = sys.argv[1], sys.argv[2]
try:
    p = subprocess.run([bin_path], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=60)
    last = [l for l in p.stdout.splitlines() if "test result:" in l]
    summary = last[-1] if last else p.stdout[-200:]
    print(f"mcp_paths run {idx}/20: rc={p.returncode} {summary}")
    sys.exit(p.returncode)
except subprocess.TimeoutExpired:
    print(f"mcp_paths run {idx}/20: TIMEOUT (>60s)")
    sys.exit(124)
' "$BIN" "$i")"
    rc=$?
    echo "$out"
    if [ "$rc" -eq 0 ]; then ok=$((ok+1)); elif [ "$rc" -eq 124 ]; then hang=$((hang+1)); else fail=$((fail+1)); fi
  done
  echo "mcp_paths summary: runs=20 ok=$ok hang=$hang fail=$fail"
  test "$ok" -eq 20 -a "$hang" -eq 0 -a "$fail" -eq 0
} > plans/reports/agy-1008-mcp-paths-evidence/r4-loop.log 2>&1
rm -rf "$S"
```

Output (`plans/reports/agy-1008-mcp-paths-evidence/r4-loop.log`):
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1008-mcp-paths/crates/designos-mcp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.55s
  Executable tests/mcp_paths.rs (out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c)
BIN=out/cargo/debug/deps/mcp_paths-b3ed97f18484d57c
mcp_paths run 1/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.10s
mcp_paths run 2/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.19s
mcp_paths run 3/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 4/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.98s
mcp_paths run 5/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s
mcp_paths run 6/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s
mcp_paths run 7/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.94s
mcp_paths run 8/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
mcp_paths run 9/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.97s
mcp_paths run 10/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 11/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.05s
mcp_paths run 12/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
mcp_paths run 13/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s
mcp_paths run 14/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 15/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 16/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.12s
mcp_paths run 17/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.10s
mcp_paths run 18/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
mcp_paths run 19/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.08s
mcp_paths run 20/20: rc=0 test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s
mcp_paths summary: runs=20 ok=20 hang=0 fail=0
```

#### Acceptance 5 — Delete `out/`

Command:
```sh
rm -rf out && test ! -e out && echo "OUT_REMOVED=0"
```

Output:
```text
OUT_REMOVED=0
```

---

### 3. Changes per File (Round 4)

- `crates/designos-mcp/src/engine.rs`:
  - Restored the early refusal in `validate_existing_output_target` when `!overwrite && !supports_overwrite`, returning `ToolError::new(ErrorCode::FileExists, ptr, format!("{display_path} already exists. Choose a new path, then try again.")).with_received(json!(display_path))` (with no `corrected_example`).
  - Added `supports_overwrite: bool` parameter to `write_checked_output_file`, removed `.png` extension sniffing, and updated the `AlreadyExists` branch for `!supports_overwrite` to return `ToolError::new(ErrorCode::FileExists, ptr, format!("{display_path} already exists. Choose a new path, then try again.")).with_received(json!(display_path))` (no OS error text, no `corrected_example`), while `supports_overwrite = true` keeps `format!("{display_path} already exists. Pass overwrite: true to replace it.")` and `with_corrected_example(json!({ field_name: display_path, "overwrite": true }))`.
  - Updated the `write_checked_output_file` call site in `Engine::call` (`"designos_export_fig_file" | "export_fig_file"`) to pass `supports_overwrite = true`.
- `crates/designos-mcp/src/server.rs`:
  - Updated the three `write_checked_output_file` call sites in `Session::call_inner`:
    - `"designos_save_file"` (`:1037`): `supports_overwrite = true`
    - `"designos_render_png"` (`:1069`): `supports_overwrite = false`
    - `"designos_export_fig_file" | "export_fig_file"` (`:1079`): `supports_overwrite = true`
- `crates/designos-mcp/tests/mcp_paths.rs`:
  - Updated `render_png_race_loser_asks_for_unused_png_filename_without_overwrite_hint` to assert the exact ruling-1 message `format!("{name} already exists. Choose a new path, then try again.")`, `err.received == Some(json!(name))`, and `err.corrected_example.is_none()`, while keeping the `designos_save_file` and `designos_export_fig_file` assertions.
  - Added `render_png_refuses_existing_file_early_and_preserves_bytes` testing `designos_render_png` on an already-existing PNG outside any race (both with a valid `node_id` and with a nonexistent `node_id` to verify early refusal in `validate_existing_output_target` before node lookup/rendering) and verifying `FILE_EXISTS`, exact ruling-1 message, `received`, `corrected_example == None`, and unchanged file bytes/SHA-256.
- `plans/reports/agy-1008-mcp-paths-evidence/r4-red.log`, `plans/reports/agy-1008-mcp-paths-evidence/r4-green.log`, `plans/reports/agy-1008-mcp-paths-evidence/r4-loop.log`:
  - Saved verbatim Red (`20913aef`), Green (`HEAD`), and 20x loop outputs.

---

### 4. Every User-Facing String, Verbatim (Round 4)

1. `"{display_path} already exists. Choose a new path, then try again."` (`ErrorCode::FileExists`, `received: Some(json!(display_path))`, `corrected_example: None`, returned by both `validate_existing_output_target` and `write_checked_output_file` when `!overwrite && !supports_overwrite`)
2. `"{display_path} already exists. Pass overwrite: true to replace it."` (`ErrorCode::FileExists`, `received: Some(json!(display_path))`, `corrected_example: Some(json!({ field_name: display_path, "overwrite": true }))`, returned by both `validate_existing_output_target` and `write_checked_output_file` when `!overwrite && supports_overwrite`)

Status: DONE

--- END REPORT: plans/reports/agy-1008-mcp-paths.md ---

--- REPORT: plans/reports/agy-1026-typography-detach.md ---
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
| **Contributor Credit Preserved** | `git log -2` | `0` | **PASS (`198a4689` authored by `NamAIEngineer`; `1090fa72` has `Co-authored-by: NamAIEngineer <NamAIEngineer@users.noreply.github.com>`, followed by Round 2 commits `712a4cf3`, `0f176b20`, `9269c066`, `0035df42`, `94b4b75e`, `20c41790`)** |
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

---

## Round 2

### Root cause

1. **`canvas/textsession.rs::text_command` bypassed style detach (`MAJOR-1`)**:
   - Keyboard shortcuts (`Cmd+B`, `Cmd+I`, `Cmd+U` in `crates/designos-app/src/canvas/textsession.rs:802-804` (`key_event`) and `crates/designos-app/src/shell.rs:937-941` (`Shell::key`), plus `Cmd+Shift+.` / `Cmd+Shift+,` in `crates/designos-app/src/canvas/textsession.rs:805-806` (`key_event`) and `crates/designos-app/src/shell.rs:838-844` (`Shell::try_typography_shortcut`)) and the top-bar **Text** menu (`Bold`, `Italic`, `Underline`, `Increase font size`, `Decrease font size` in `crates/designos-app/src/panels/menus.rs:216-220` (`menus::run`)) dispatch through `canvas::textsession::text_command`.
   - In an active text session, `text_command` called `style_selection(app, f)` (`detach_style: false` / `LinkEdit::Keep`) instead of `style_selection_detaching_as(app, f, EditKind::Other)`.
   - Outside a text session, `text_command` called `app.edit_nodes(&[id], |n| { richtext::apply_all(t, &f); ... })` directly without removing `n.style_refs[text_ref()]`.
2. **Paragraph fields in `panels/typography.rs` bypassed style detach (`MINOR-2`)**:
   - In the Typography **Type settings -> Details** popover (`crates/designos-app/src/panels/typography.rs:806-838`), `Paragraph spacing` (`:813`) goes through `set_base`, and `Paragraph indent` (`:808`), `List spacing` (`:818`), and `Vertical trim` (`:836`) go through `set_text_props`.
   - Both `set_base` and `set_text_props` mutated `NodeKind::Text(t)` (outside a session) or `ts::base_style` / `ts::edit_text_props` (inside a session) without detaching `n.style_refs[text_ref()]`, even though `designos_doc::lint::token_match::same_text_style` and `styles/text_style_ops.rs` treat `paragraph_spacing`, `paragraph.indent`, `paragraph.list_spacing`, and `paragraph.leading_trim` as shared-text-style-owned fields (while `align_h`, `fills`, `hanging_punctuation` (`:824`), `hanging_list` (`:829`), `direction` (`:848`), and `text_wrap` (`:860`) are not style-owned).
3. **Unified shared helper**:
   - Extracted `style_owned_changed` / `style_owned_props_changed` and `pub fn edit_node_text_detaching(n: &mut Node, f: impl FnOnce(&mut TextProps))` in `crates/designos-app/src/canvas/textsession.rs:333-362`, comparing style-owned fields before and after via `designos_doc::lint::token_match::same_text_style` plus `(paragraph.indent, paragraph.list_spacing, paragraph.leading_trim)` and per-run styles, then removing `n.style_refs[text_ref()]` when any style-owned field changed.
   - Routed `panels/typography.rs::set_style`, `panels/typography.rs::set_base`, `panels/typography.rs::set_text_props`, and `canvas/textsession.rs::text_command` (outside a session) through `ts::edit_node_text_detaching`, and routed `canvas/textsession.rs::text_command` (in a session) through `style_selection_detaching_as(app, f, EditKind::Other)` and `base_style` / `edit_text_props` through `LinkEdit::DetachIfStyleOwnedChanged`. Because `edit_node_text_detaching` uses `n.kind.any_text_mut()`, `set_style`, `set_base`, and `set_text_props` also edit `Sticky`, `ShapeWithText`, and `Connector`-label text in a mixed selection outside a session (matching `text_command`).

### Files changed

| File | Line Ranges | Commits | Summary of Changes |
|---|---|---|---|
| `crates/designos-app/src/canvas/textsession.rs` | `L16`, `L331-L363`, `L369`, `L408-L411`, `L570-L571`, `L607`, `L619`, `L644`, `L669` | `9269c066`, `94b4b75e`, `20c41790` | Added `style_owned_changed`, `style_owned_props_changed`, and `pub fn edit_node_text_detaching`; added `LinkEdit::DetachIfStyleOwnedChanged` for `base_style` and `edit_text_props`; routed `text_command` through `style_selection_detaching_as` (in session) and `edit_node_text_detaching` (outside session); removed unused `style_selection` wrapper. |
| `crates/designos-app/src/panels/typography.rs` | `L78`, `L114-L120`, `L568-L574`, `L943` | `712a4cf3`, `9269c066`, `94b4b75e`, `20c41790` | Routed `set_style`, `set_base` (temporarily `pub(crate)` in Round 2; restored to private `fn set_base` in Round 3 `03017f0b`), and `set_text_props` through `ts::edit_node_text_detaching` (which also widens mixed-selection edits to `Sticky`, `ShapeWithText`, and `Connector` labels via `any_text_mut`); removed duplicate `#[path = "../shell/tests/typography_style_detach.rs"]` module mount. |
| `crates/designos-app/src/shell/tests.rs` | `L4-L5`, `L1235` | `712a4cf3`, `20c41790` | Registered `mod typography_style_detach;` next to `mod typography;` so it shares `shell::tests::Sim` (temporary `#[allow(unused_imports)]` re-export at `L4-L5` removed in Round 3 `03017f0b`). |
| `crates/designos-app/src/shell/tests/typography_style_detach.rs` | `L1-L611` | `712a4cf3`, `0f176b20`, `0035df42`, `20c41790` | Removed the copied `Sim` struct to use shared `super::Sim`; added 8 `Sim` tests for `Cmd+B`, `Cmd+I`, `Cmd+U`, and menu `Increase font size` (outside and inside a session); added 4 `Sim` tests for `paragraph_spacing`, `paragraph.indent`, `paragraph.list_spacing`, and `paragraph.leading_trim` (`Vertical trim`); added 2 `Sim` tests for no-op same-family re-pick (`pick_family` / `"Inter"`) outside and inside a session. |
| `plans/reports/agy-1026-typography-detach-evidence/r2-*` | `r2-red.log`, `r2-green.log`, `r2-probes.log`, `r2-mutations.log` | — | Round 2 red/green logs, reviewer probe output, and 5-mutation (`M1`, `M2`, `M2b`, `M4a`, `M4b`) verification log. |

**Branch commit log (`git log --oneline 198a4689~1..HEAD`)**:

- `198a4689` — `fix(app): detach text styles on typography edits` (`NamAIEngineer`)
- `1090fa72` — `test(app): add sim tests for typography style detach and avoid unlinked clone`
- `712a4cf3` — `test(app): mount typography_style_detach in shell::tests with shared Sim`
- `0f176b20` — `test(app): add sim tests for text_command style detach` (red before `9269c066`)
- `9269c066` — `fix(app): detach linked text style in text_command via shared helper`
- `0035df42` — `test(app): add sim tests for paragraph field style detach` (red before `94b4b75e`)
- `94b4b75e` — `fix(app): detach linked text style on style-owned paragraph edits`
- `20c41790` — `test(app): add no-op family repick tests that keep text style link`

### Red -> Green

- **`plans/reports/agy-1026-typography-detach-evidence/r2-red.log`** (running the full 34-test `typography` suite against pre-Round-2 `textsession.rs` + `typography.rs`, `exit=101`):

```text
failures:
    shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_b_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_i_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_u_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::list_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::menu_increase_font_size_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_indent_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::vertical_trim_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo

test result: FAILED. 22 passed; 12 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 5.94s
```

- **`plans/reports/agy-1026-typography-detach-evidence/r2-green.log`** (running on Round 2 `HEAD` `20c41790`, `exit=0`):

```text
running 34 tests
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step ... ok
test check_designs::tests::rules::a_typography_number_match_or_binding_does_not_hide_the_text_style_rule ... ok
test panels::inspect::tests::typography_and_assets ... ok
test panels::inspect::tests::inspect_code_connect_redlines_mcp_and_typography_style_mutate_state ... ok
test panels::typography::tests::alignment_and_fill_edits_keep_a_linked_text_style ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test shell::tests::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link ... ok
test shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches ... ok
test shell::tests::typography_style_detach::cmd_b_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_u_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::list_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::paragraph_indent_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::paragraph_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_in_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_outside_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::vertical_trim_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 6.53s
```

### Verification

#### 1. Six Mandatory Acceptance Commands

```text
=== 1. cargo fmt --all -- --check ===
fmt exit=0

=== 2. cargo clippy --workspace --all-targets --locked -- -D warnings ===
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
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 174.12s
clippy exit=0

=== 3. cargo test -p designos-app --bin designos --locked typography ===
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 6.62s

=== 4. cargo test -p designos-app --bin designos --locked textsession ===
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 3794 filtered out; finished in 0.84s

=== 5. cargo test -p designos-app --bin designos --locked text_style ===
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 3759 filtered out; finished in 13.41s

=== 6. cargo test --workspace --locked --lib ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-84b9f79dcf53b1e4)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-815df7cac2f24cbf)
test result: ok. 672 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.87s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-875c5e9aa7515b1d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-ab132beff113d839)
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.35s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-b26ed997a061f36b)
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.16s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-45d3e160fa7875ce)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-071f7214381262c2)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-7d0983952c42b7d7)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.27s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-440ebce98f1a928a)
test result: ok. 262 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.74s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-25f4198b0cded696)
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 50.65s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-82f009e569d124ef)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-a99dd1a1e2689cdb)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

#### 2. Reviewer Probes (`plans/reports/agy-1026-typography-detach-evidence/r2-probes.log`)

All 5 `text_command` probes (`cmdB-out`, `cmdB-in`, `cmdI-out`, `cmdU-out`, `menuSizeUp-out`) plus the paragraph probes (`pspacing`, `indent`) now report `link=None`, `repick-same-family` reports `link=Some("Body/Regular")`, and `cmdShiftDot` (`cmdShiftDot-in` `size=18`, `cmdShiftDot-out` `size=17`) is unchanged:

```text
PROBE cmdB-in before: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=true
PROBE cmdB-in after: content="Continue" size=16 weight=700 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=1 session=true
PROBE cmdB-out before: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=false
PROBE cmdB-out after: content="Continue" size=16 weight=700 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
PROBE cmdI-out after: content="Continue" size=16 weight=400 italic=true deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
PROBE cmdU-out after: content="Continue" size=16 weight=400 italic=true deco=Underline pspacing=0 indent=0 runs=[] link=None depth=3 session=false
PROBE menuSizeUp-out after: content="Continue" size=17 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
PROBE pspacing after: content="Continue" size=16 weight=400 italic=false deco=None pspacing=12 indent=0 runs=[] link=None depth=2 session=false
PROBE indent after: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=8 runs=[] link=None depth=2 session=false
PROBE repick-same-family after: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=false
PROBE cmdShiftDot-in before: content="Continue" size=16 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=Some("Body/Regular") depth=1 session=true
PROBE cmdShiftDot-in after: content="Continue" size=18 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=1 session=true
PROBE cmdShiftDot-out after: content="Continue" size=17 weight=400 italic=false deco=None pspacing=0 indent=0 runs=[] link=None depth=2 session=false
```

#### 3. Mutation Testing (`plans/reports/agy-1026-typography-detach-evidence/r2-mutations.log`)

All 5 reviewer mutations (`M1`, `M2`, `M2b`, `M4a`, `M4b`) fail at least one test in `cargo test -p designos-app --bin designos --locked typography`:

| Mutation | Description | Result | Failing Tests |
|---|---|---|---|
| **`M1`** | Remove `n.style_refs.remove(text_ref())` in `edit_node_text_detaching` | **KILLED (`18 passed; 16 failed`)** | 16 out-of-session style/command/paragraph detach tests fail |
| **`M2`** | Call `style_selection_as` instead of `style_selection_detaching_as` in `set_style` | **KILLED (`32 passed; 2 failed`)** | `active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link`, `canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session` |
| **`M2b`** | Make `LinkEdit::DetachIfChanged` arm a no-op in `edit_props_with_link` (Round-1 `M2b`) / flip `set_style_preserving_link` (`r2-mutations.log`) | **KILLED (`28 passed; 6 failed` on Round-1 `M2b`; `33 passed; 1 failed` in `r2-mutations.log`)** | Round-1 `M2b` fails all 6 in-session style/command detach tests (`typography_edit_detaches_a_linked_text_style_in_an_active_text_session`, `active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link`, `cmd_b_in_session_*`, `cmd_i_in_session_*`, `cmd_u_in_session_*`, `menu_increase_font_size_in_session_*`) |
| **`M4a`** | Remove `style_owned_changed` guard in `edit_node_text_detaching` (unconditional detach) | **KILLED (`31 passed; 3 failed`)** | `panels::typography::tests::alignment_and_fill_edits_keep_a_linked_text_style`, `alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches`, `repicking_same_font_family_outside_session_keeps_linked_text_style` |
| **`M4b`** | Remove `if props_changed` guard on `LinkEdit::DetachIfChanged` in `edit_props_with_link` | **KILLED (`33 passed; 1 failed`)** | `repicking_same_font_family_in_session_keeps_linked_text_style` |

```text
=== M1 ===
test result: FAILED. 18 passed; 16 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 5.10s
=== M2 ===
test result: FAILED. 32 passed; 2 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 53.14s
=== M2b ===
test result: FAILED. 33 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.04s
=== M4a ===
test result: FAILED. 31 passed; 3 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 6.62s
=== M4b ===
test result: FAILED. 33 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.55s
```

### Residual risks

The following three behaviors noted in the Round 1 review remain intentionally out of scope for `#1026`:

1. **Collapsed-caret pending typing style + subsequent typing (`MINOR-1`)**: When a text session has a collapsed caret on non-empty text (`range.is_empty() && !empty_text`), `style_selection_impl` (`crates/designos-app/src/canvas/textsession.rs:581-590`) only updates the transient `sess.typing: Option<TextStyle>` without mutating `TextProps` or recording an undo entry; subsequent character insertion (`insert_text`, `crates/designos-app/src/canvas/textsession.rs:451-468`) splices `sess.typing` into `t.runs` via `edit_props` (`LinkEdit::Keep`). Changing `insert_text` to detach on typing-style divergence is a separate typing-pipeline policy change outside `#1026`'s Typography/shortcut style-edit scope.
2. **Double font-size step on `Cmd+Shift+.` / `Cmd+Shift+,` inside an active text session**: Both `crates/designos-app/src/canvas/textsession.rs:805-806` (`key_event`, called from `text_keys` at `canvas/textsession.rs:774`) and `crates/designos-app/src/shell.rs:838-844` (`Shell::try_typography_shortcut`, called from `Shell::handle_keys` at `shell.rs:776`) match `Cmd+Shift+.` / `Cmd+Shift+,` in the same frame when a text session is open, stepping font size by `+2.0` (`16 -> 18`) in-session vs `+1.0` (`16 -> 17`) outside a session. Both paths now detach the linked text style; deduplicating the key routing across `textsession::key_event` and `Shell::try_typography_shortcut` is orthogonal to `#1026`.
3. **In-session undo coalescing of consecutive `EditKind::Style` edits (`fill` + `size`)**: `UndoLog::record` (`crates/designos-app/src/textedit.rs:129-139`) coalesces consecutive `EditKind::Style` steps within `BURST_SECONDS: f64 = 1.0` (`crates/designos-app/src/textedit.rs:124`) by retaining the earlier `Snap` (`props` + `style_ref`), so an in-session fill edit followed within `1.0s` by an in-session size edit shares one session undo step. Adjusting `EditKind` coalescing boundaries in `textedit.rs` is outside `#1026`'s scope and file boundary.

---

## Round 3

### What could NOT be verified

- Live interactive desktop window manipulation (`designos-studio` GUI manual clicking) was not run in a human window session; all UI, popover, shortcut, and text-session behaviors were verified headlessly through `shell::tests::Sim` in `cargo test -p designos-app --bin designos`.

### Acceptance Table

| Check | Status | Evidence |
|---|---|---|
| **MINOR-A (2 in-session paragraph tests + 2 run tests; kill mutants `N3`, `N4`, `N6`, `N8`, `N9`)** | **PASS** | Added `paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`, `paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`, `run_merge_into_base_on_cmd_i_detaches_linked_text_style`, and `run_only_style_change_on_cmd_i_detaches_linked_text_style` in [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L613-L735). All 5 mutants (`N3`, `N4`, `N6`, `N8`, `N9`) fail with `exit=101` in `plans/reports/agy-1026-typography-detach-evidence/r3-mutants.log`. |
| **MINOR-B (`[Text, Sticky]` mixed-selection test + wider-reach disclosure)** | **PASS** | Added `mixed_text_and_sticky_selection_shortcuts_edit_both_and_detach_linked_text_style` in [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L737-L773) verifying `Cmd+Shift+.` (`Text` `16 -> 17`, `Sticky` `32 -> 33`, `Text` detached) and `Alt+ArrowRight` (`Text` and `Sticky` `letter_spacing` `0.0 -> 0.5`). Disclosed wider reach in Round 2 and Round 3 report sections. |
| **MINOR-C (remove unused `shell/tests.rs` re-export; restore `fn set_base` visibility)** | **PASS** | Removed `#[allow(unused_imports)] use crate::panels::typography::{pick_family, set_base, set_style, set_text_props};` from [`crates/designos-app/src/shell/tests.rs`](../../crates/designos-app/src/shell/tests.rs#L1-L6) and restored private `fn set_base` at [`crates/designos-app/src/panels/typography.rs`](../../crates/designos-app/src/panels/typography.rs#L114). |
| **MINOR-D (fix Round 2 report citations)** | **PASS** | Corrected Round 2 citations in [`plans/reports/agy-1026-typography-detach.md`](../../plans/reports/agy-1026-typography-detach.md#L393-L601) to reference `canvas/textsession.rs:802-806`, `shell.rs:838-844` / `:937-941`, `panels/menus.rs:216-220`, `panels/typography.rs:806-862` (`hanging_list`, `text_wrap`), `UndoLog::record` (`textedit.rs:129-139`), `BURST_SECONDS = 1.0` (`textedit.rs:124`), and Round-1 `M2b`. |
| **MINOR-E (one rule `LinkEdit::DetachIfStyleOwnedChanged` + in/out-of-session fill tests)** | **PASS** | Removed `LinkEdit::DetachIfChanged` from [`crates/designos-app/src/canvas/textsession.rs`](../../crates/designos-app/src/canvas/textsession.rs#L365-L408) and routed `style_selection_impl` (`:591-602`) through `LinkEdit::DetachIfStyleOwnedChanged`. Added `set_style_fill_change_outside_session_keeps_linked_text_style` and `set_style_fill_change_in_session_keeps_linked_text_style` in [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L775-L803) (`r3-red.log` -> `r3-green.log`). |
| **All 6 mandatory gates green on final commit** | **PASS** | `fmt` (`exit=0`), `clippy` (`exit=0`), `typography` (`41 passed; 0 failed`), `textsession` (`42 passed; 0 failed`), `text_style` (`84 passed; 0 failed`), `lib` (`1,380 passed; 0 failed`) in `plans/reports/agy-1026-typography-detach-evidence/r3-gates.log`. |

### Files Changed in Round 3

| File | Line Ranges | Commits | Summary of Changes |
|---|---|---|---|
| [`crates/designos-app/src/canvas/textsession.rs`](../../crates/designos-app/src/canvas/textsession.rs#L365-L604) | `L365-L369`, `L404-L408`, `L591-L602` | `78e81b72` | Removed `LinkEdit::DetachIfChanged` and used `LinkEdit::DetachIfStyleOwnedChanged` in `style_selection_impl` so in-session and out-of-session edits share the exact same style-owned detach rule. |
| [`crates/designos-app/src/panels/typography.rs`](../../crates/designos-app/src/panels/typography.rs#L114-L120) | `L114` | `03017f0b` | Restored `fn set_base` from `pub(crate)` to private module visibility. |
| [`crates/designos-app/src/shell/tests.rs`](../../crates/designos-app/src/shell/tests.rs#L1-L6) | `L4-L5` | `03017f0b` | Removed unused `#[allow(unused_imports)] use crate::panels::typography::{pick_family, set_base, set_style, set_text_props};`. |
| [`crates/designos-app/src/shell/tests/typography_style_detach.rs`](../../crates/designos-app/src/shell/tests/typography_style_detach.rs#L613-L803) | `L613-L803` | `efc9eb76`, `3a86cbf4`, `7aecd293` | Added 7 new tests: 2 in-session paragraph tests (`paragraph_spacing`, `paragraph.indent` via Type Details popover), 2 run tests (`N3` run merge into base on `Cmd+I`, `N4` single-run style change on `Cmd+I`), 1 `[Text, Sticky]` mixed-selection test (`Cmd+Shift+.` and `Alt+ArrowRight`), and 2 `set_style` fill tests (outside and inside a session). |
| [`plans/reports/agy-1026-typography-detach.md`](../../plans/reports/agy-1026-typography-detach.md) | `L393-L880` | — | Fixed Round 2 citations (`MINOR-D`), disclosed mixed-selection wider reach (`MINOR-B`), and added Round 3 report + evidence (`r3-red.log`, `r3-green.log`, `r3-probes.log`, `r3-mutants.log`, `r3-gates.log`). |

### Verbatim Verification Output

#### 1. Reviewer `probe-r2.rs` Results (`plans/reports/agy-1026-typography-detach-evidence/r3-probes.log`)

Not recorded: how `probe-r2.rs` was compiled once `set_base` went back to private (MINOR-C); this log cannot be reproduced from this report alone. All 17 `rv1424_*` probes from `probe-r2.rs` pass (`exit=0`) and match `probes-head.log` except `PROBE setstyle-fill-in after`, which now keeps `link=Some("Body/Regular")`:

```text
running 17 tests
test shell::tests::typography_style_detach::rv1424_in_session_list_spacing ... ok
test shell::tests::typography_style_detach::rv1424_in_session_paragraph_indent ... ok
test shell::tests::typography_style_detach::rv1424_in_session_paragraph_spacing ... ok
test shell::tests::typography_style_detach::rv1424_in_session_paragraph_spacing_through_the_popover ... ok
test shell::tests::typography_style_detach::rv1424_in_session_vertical_trim ... ok
test shell::tests::typography_style_detach::rv1424_mixed_text_and_sticky_selection ... ok
test shell::tests::typography_style_detach::rv1424_non_owned_fields_keep_link ... ok
test shell::tests::typography_style_detach::rv1424_noop_on_redundant_run ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_b_i_u_in_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_b_outside_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_i_u_outside_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_cmd_shift_period_in_and_out_of_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_menu_text_commands_in_and_out_of_session ... ok
test shell::tests::typography_style_detach::rv1424_r1_same_family_repick_keeps_link ... ok
test shell::tests::typography_style_detach::rv1424_run_merge_change_detaches ... ok
test shell::tests::typography_style_detach::rv1424_run_only_change_detaches ... ok
test shell::tests::typography_style_detach::rv1424_set_style_fill_only_in_vs_out_of_session ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 3843 filtered out; finished in 7.53s

PROBE lspacing-in after-edit: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=6 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE indent-in after-edit: size=16 weight=400 italic=false deco=None pspacing=0 indent=8 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE pspacing-in after-edit: size=16 weight=400 italic=false deco=None pspacing=12 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE pspacing-ui-in after-edit: size=16 weight=400 italic=false deco=None pspacing=12 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE trim-in after-edit: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=CapHeight hangp=false hangl=false align=Left runs=[] link=None depth=1 session=true log(can_undo)=Some(true)
PROBE mixed before: sticky(size,lh,ls)=(32.0, Auto, 0.0) text_size=16 depth=1
PROBE mixed after Cmd+Shift+.: sticky(size,lh,ls)=(33.0, Auto, 0.0) text_size=17 depth=2
PROBE mixed after Alt+Right: sticky(size,lh,ls)=(33.0, Auto, 0.5) depth=3
PROBE mixed after set_style(30): sticky(size,lh,ls)=(30.0, Auto, 0.5) text_size=30
PROBE nonowned-out after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=true hangl=true align=Center runs=[] link=Some("Body/Regular") depth=6 session=false log(can_undo)=None
PROBE nonowned-in after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=true hangl=true align=Center runs=[] link=Some("Body/Regular") depth=1 session=true log(can_undo)=Some(true)
PROBE runmerge before: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[(0, 3, 16.0, 400, false)] link=Some("Body/Regular") depth=2 session=false log(can_undo)=None
PROBE runmerge after Cmd+I: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=None depth=3 session=false log(can_undo)=None
PROBE runonly before: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[(0, 3, 16.0, 700, false)] link=Some("Body/Regular") depth=2 session=false log(can_undo)=None
PROBE runonly after Cmd+I: size=16 weight=400 italic=true deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[(0, 3, 16.0, 700, true)] link=None depth=3 session=false log(can_undo)=None
PROBE setstyle-fill-out after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=Some("Body/Regular") depth=2 session=false log(can_undo)=None
PROBE setstyle-fill-in after: size=16 weight=400 italic=false deco=None pspacing=0 indent=0 lspacing=0 trim=None hangp=false hangl=false align=Left runs=[] link=Some("Body/Regular") depth=1 session=true log(can_undo)=Some(true)
```

#### 2. Red -> Green (`plans/reports/agy-1026-typography-detach-evidence/r3-red.log` and `r3-green.log`)

- **`r3-red.log`** (on `7aecd293` before `78e81b72`, `exit=101`):

```text
---- shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style stdout ----
designos: --select "Continue" matched 2 layers; selected id 10 (first in page order; use id:<n> for another)

thread 'shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style' (40070581) panicked at crates/designos-app/src/shell/tests/typography_style_detach.rs:801:5:
assertion `left == right` failed: fill change via set_style in a session must keep the text style link
  left: None
 right: Some("Body/Regular")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style

test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 8.13s
```

- **`r3-green.log`** (on `78e81b72`, `exit=0`):

```text
running 41 tests
test panels::inspect::tests::typography_and_assets ... ok
test check_designs::tests::rules::a_typography_number_match_or_binding_does_not_hide_the_text_style_rule ... ok
test panels::inspect::tests::inspect_code_connect_redlines_mcp_and_typography_style_mutate_state ... ok
test panels::typography::tests::alignment_and_fill_edits_keep_a_linked_text_style ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session ... ok
test canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_the_same_undo_step ... ok
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link ... ok
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_i_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_b_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::cmd_u_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
test shell::tests::typography_style_detach::alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_in_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::list_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::menu_increase_font_size_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::set_style_fill_change_in_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::repicking_same_font_family_outside_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::mixed_text_and_sticky_selection_shortcuts_edit_both_and_detach_linked_text_style ... ok
test shell::tests::typography_style_detach::paragraph_indent_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::set_style_fill_change_outside_session_keeps_linked_text_style ... ok
test shell::tests::typography_style_detach::paragraph_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok
test shell::tests::typography_style_detach::run_only_style_change_on_cmd_i_detaches_linked_text_style ... ok
test shell::tests::typography_style_detach::run_merge_into_base_on_cmd_i_detaches_linked_text_style ... ok
test shell::tests::typography_style_detach::vertical_trim_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 8.70s
```

#### 3. Mutants `N3`, `N4`, `N6`, `N8`, `N9` (`plans/reports/agy-1026-typography-detach-evidence/r3-mutants.log`)

All 5 mutants from `mutate-r2.py` are killed (`exit=101`):

| Mutant | Mutation in `crates/designos-app/src/canvas/textsession.rs` | Result | Failing Test(s) |
|---|---|---|---|
| **`N3_drop_run_count_compare`** | Drop `\|\| runs.len() != after.runs.len()` in `style_owned_changed` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::run_merge_into_base_on_cmd_i_detaches_linked_text_style` |
| **`N4_drop_per_run_compare`** | Drop `\|\| runs.iter().zip(&after.runs).any(...)` in `style_owned_changed` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::run_only_style_change_on_cmd_i_detaches_linked_text_style` |
| **`N6_style_owned_arm_noop`** | Make `LinkEdit::DetachIfStyleOwnedChanged` arm a no-op in `edit_props_with_link` | **KILLED (`33 passed; 8 failed`, `exit=101`)** | `canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session`, `active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link`, `cmd_b_in_session_*`, `cmd_i_in_session_*`, `cmd_u_in_session_*`, `menu_increase_font_size_in_session_*`, `paragraph_indent_in_session_*`, `paragraph_spacing_in_session_*` |
| **`N8_base_style_keeps`** | Replace `LinkEdit::DetachIfStyleOwnedChanged` with `LinkEdit::Keep` in `base_style` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo` |
| **`N9_edit_text_props_keeps`** | Replace `LinkEdit::DetachIfStyleOwnedChanged` with `LinkEdit::Keep` in `edit_text_props` | **KILLED (`40 passed; 1 failed`, `exit=101`)** | `shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo` |

```text
=== N3_drop_run_count_compare (exit=101) ===
failures:
    shell::tests::typography_style_detach::run_merge_into_base_on_cmd_i_detaches_linked_text_style
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.44s

=== N4_drop_per_run_compare (exit=101) ===
failures:
    shell::tests::typography_style_detach::run_only_style_change_on_cmd_i_detaches_linked_text_style
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.29s

=== N6_style_owned_arm_noop (exit=101) ===
failures:
    canvas::textsession::tests::typography_edit_detaches_a_linked_text_style_in_an_active_text_session
    shell::tests::typography_style_detach::active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link
    shell::tests::typography_style_detach::cmd_b_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_i_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::cmd_u_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
    shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
test result: FAILED. 33 passed; 8 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.18s

=== N8_base_style_keeps (exit=101) ===
failures:
    shell::tests::typography_style_detach::paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 7.26s

=== N9_edit_text_props_keeps (exit=101) ===
failures:
    shell::tests::typography_style_detach::paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo
test result: FAILED. 40 passed; 1 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 9.97s
```

#### 4. Six Mandatory Acceptance Gates (`plans/reports/agy-1026-typography-detach-evidence/r3-gates.log`)

```text
=== 1. cargo fmt --all -- --check ===
fmt exit=0
=== 2. cargo clippy --workspace --all-targets --locked -- -D warnings ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.85s
clippy exit=0
=== 3. cargo test -p designos-app --bin designos --locked typography ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 10.35s
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 3802 filtered out; finished in 8.70s
typography exit=0
=== 4. cargo test -p designos-app --bin designos --locked textsession ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 7.45s
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 3801 filtered out; finished in 0.46s
textsession exit=0
=== 5. cargo test -p designos-app --bin designos --locked text_style ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 8.12s
test result: ok. 84 passed; 0 failed; 0 ignored; 0 measured; 3759 filtered out; finished in 12.73s
text_style exit=0
=== 6. cargo test --workspace --locked --lib ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.72s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 672 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.38s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 153 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.18s
test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.59s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.22s
test result: ok. 262 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.10s
test result: ok. 56 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 34.88s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
lib exit=0
```

### Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **`FACT`**: Removing `LinkEdit::DetachIfChanged` and using `LinkEdit::DetachIfStyleOwnedChanged` in `style_selection_impl` (`crates/designos-app/src/canvas/textsession.rs:591-602`) makes `set_style` fill-only edits keep `style_refs[text_ref()]` both outside and inside an active text session (`set_style_fill_change_outside_session_keeps_linked_text_style` and `set_style_fill_change_in_session_keeps_linked_text_style`, `r3-red.log` -> `r3-green.log`), while `set_style_preserving_link` (`style_selection_as` -> `LinkEdit::Keep`) keeps the link for dedicated colour-picker edits. Correction: it does not avoid cloning; `edit_props_with_link` clones the props into `snap` on every edit (`crates/designos-app/src/canvas/textsession.rs:394`).
- **`FACT`**: Opening the Type Details popover inside an active text session and editing `"det-pspacing"` (`set_base` -> `ts::base_style`) or `"det-indent"` (`set_text_props` -> `ts::edit_text_props`) detaches `style_refs[text_ref()]` in one session undo step and one committed document undo step (`paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`, `paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo`), killing mutants `N6`, `N8`, and `N9` without exposing `set_base` as `pub(crate)`.
- **`FACT`**: In `style_owned_changed` (`crates/designos-app/src/canvas/textsession.rs:333-339`), both `runs.len() != after.runs.len()` (`N3`) and `runs.iter().zip(&after.runs).any(...)` (`N4`) are necessary and exercised by `run_merge_into_base_on_cmd_i_detaches_linked_text_style` and `run_only_style_change_on_cmd_i_detaches_linked_text_style`.
- **`FACT` (MINOR-B wider-reach disclosure)**: Routing `panels/typography.rs::{set_style, set_base, set_text_props}` through `ts::edit_node_text_detaching` (`n.kind.any_text_mut()`) widens their reach in a mixed selection (`[Text, Sticky]`, `ShapeWithText`, or `Connector` label) so that `Alt+Arrow` kerning/line-height shortcuts (`Shell::try_typography_shortcut`, `shell.rs:830` -> `set_style`) and Typography inspector controls edit non-`Text` text-bearing nodes alongside `Text` nodes, matching `text_command` (`Cmd+B/I/U`, `Cmd+Shift+./,`). Locked in by `mixed_text_and_sticky_selection_shortcuts_edit_both_and_detach_linked_text_style`.
- **`INFERENCE`**: Because `style_owned_changed` compares the exact same tuple (`same_text_style` on base + `(paragraph.indent, paragraph.list_spacing, paragraph.leading_trim)` + run count and per-run `(start, end)` + `same_text_style`) in both `edit_node_text_detaching` and `LinkEdit::DetachIfStyleOwnedChanged`, no style-owned or non-style-owned field can diverge in detach behavior between in-session and out-of-session edits.
- **`ASSUMPTION`**: None.

- **Weakest link**: Collapsed-caret pending typing style (`sess.typing`) in an active text session remains a transient cursor state until characters are typed via `insert_text`, which is outside `#1026`'s Typography/shortcut style-edit scope.

### Skills Loaded & What Changed Because of Them

1. **`es-astra-work`**: Enforced the strict red-before-green verification loop (`r3-red.log` on `7aecd293` before `78e81b72`) and post-mutation verification across all 17 reviewer probes, 5 mutants, and 6 cargo gates.
2. **`es-fable-thinking`**: Guided the falsification test design for `N3` (run merge into base where base style is unchanged) vs `N4` (single-run style change where run count and base style are unchanged) and verified every file/line citation in the report against the actual source files.
3. **`es-rust`**: Restored minimal module visibility (`fn set_base` private in `panels/typography.rs`, no unused re-exports in `shell/tests.rs`) while testing `base_style` and `edit_text_props` through the Type Details popover UI in `Sim`.
4. **`es-rust-gpu-canvas`** & **`es-dev` / `es-feature-dev`**: Preserved unlinked fast-path invariants (`before` snapshot only when `n.style_refs.contains_key(text_ref())`) and atomic commit hygiene.

### Unresolved Questions

None.

Status: DONE

--- END REPORT: plans/reports/agy-1026-typography-detach.md ---

--- REPORT: plans/reports/agy-1028-text-style-ops-to-doc.md ---
# Report: Move `create_text_style_from_nodes` and `detach_text_style` into `designos-doc` (`crates/designos-doc/src/text_style_ops.rs:1-249`)

## 0. What Could NOT Be Verified First

1. **Full-suite `cargo test -p designos-app --bin designos --locked` exit code `0` in a single un-filtered invocation under macOS `/var/folders/...` `TMPDIR` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4312-4315`):**
   - In the full parallel binary run (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4312`), `3` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4312`) pre-existing tests outside `text_style_ops` (`crates/designos-doc/src/text_style_ops.rs:1-249`) failed:
     1. `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4290-4294`) panicked at `crates/designos-app/src/agent_approval_channel.rs:286:9` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4292`) because macOS default `TMPDIR` (`/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IKWOmO/...`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4293`) produces a `130`-byte (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4293`) Unix domain socket path exceeding the `103`-byte `SUN_PATH_MAX` limit (`crates/designos-app/src/agent_approval_channel.rs:286`). When solo-rerun with default `TMPDIR` it failed identically (`130` bytes, `rc=101`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:13-30`), and when solo-rerun with `TMPDIR=/tmp` it passed (`1 passed; 0 failed`, `rc=0`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:71-75`).
     2. `shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4296-4300`) timed out waiting for the background editor notification under full CPU parallelism (`EDITOR_NOT_NOTIFIED`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4299`), and passed on solo rerun (`1 passed; 0 failed`, `rc=0`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:41-45`).
     3. `vecops::tests::two_deferred_text_commands_both_convert_in_order` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4301-4305`) hit its font-load deadline under full CPU parallelism (`both texts still waiting for slow fonts`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4304`), and passed on solo rerun (`1 passed; 0 failed`, `rc=0`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:56-60`).
2. **Non-macOS CI targets (`ubuntu` / `windows`):** Not executed locally; verified on macOS `aarch64` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:1-3`) and delegated to GitHub Actions on Draft PR `#1580` (`https://github.com/jangtrinh/design-os-studio/pull/1580`).

---

## 1. Conclusion

`create_text_style_from_nodes` (`crates/designos-doc/src/text_style_ops.rs:60`), `detach_text_style` (`crates/designos-doc/src/text_style_ops.rs:92`), their companion functions `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`), `text_layers` (`crates/designos-doc/src/text_style_ops.rs:21`), `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`), `next_free_name` (`crates/designos-doc/src/text_style_ops.rs:49`), `TEXT_REF` (`crates/designos-doc/src/text_style_ops.rs:11`), and all `6` (`crates/designos-doc/src/text_style_ops.rs:136-215`) existing unit tests have been moved without behavioral or body changes (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:1-20`) from `crates/designos-app/src/styles/text_style_ops.rs` (`crates/designos-app/src/styles/text_style_ops.rs:1-3`) into `crates/designos-doc/src/text_style_ops.rs` (`crates/designos-doc/src/text_style_ops.rs:1-249`) and re-exported at the root of `designos-doc` (`crates/designos-doc/src/lib.rs:76,114`) as well as in `crates/designos-app/src/styles/text_style_ops.rs` (`crates/designos-app/src/styles/text_style_ops.rs:3`). One new document-only unit test, `create_and_detach_round_trip_on_plain_document` (`crates/designos-doc/src/text_style_ops.rs:218-248`), verifies creating and detaching a text style and undoing both steps on a plain `Document` (`crates/designos-doc/src/lib.rs:183`) without `designos-app`. All `7` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670`) `text_style_ops::tests::*` tests run and pass in `designos-doc` (`752 passed; 0 failed` unit tests, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:764`), `0` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:1-4315`) `text_style_ops` tests remain duplicated in `designos-app`, and all formatting (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:3`), clippy (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/02-clippy.log:49`), `designos-mcp` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/05-test-mcp.log:728`), workspace library (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/06-test-workspace-lib.log:1620`), and zero-fake-UI (`17/17`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/07-zero-fake-ui-gate.log:24-26`) gates pass.

---

## 2. Step 1 — Facts First (Recorded Before Any Code Edit)

### 2.1 Every Item `create_text_style_from_nodes` and `detach_text_style` Depend On

Source file before move on `origin/main` (`1e46e34f2bf93eea3b2374a672a71c7f8097e267`): `crates/designos-app/src/styles/text_style_ops.rs:1-215` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:23-239`), with `create_text_style_from_nodes` at `crates/designos-doc/src/text_style_ops.rs:60` and `detach_text_style` at `crates/designos-doc/src/text_style_ops.rs:92`.

| Item | Kind | Location (`file:line`) | Lives in `designos-doc`? |
|---|---|---|---|
| `TEXT_REF` (`crates/designos-doc/src/text_style_ops.rs:11`) | `const &str` (`"text"`) | `crates/designos-doc/src/text_style_ops.rs:11` (was `crates/designos-app/src/styles/text_style_ops.rs:11`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:35`) | Yes — moved with module (`FACT`) |
| `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`) | `pub fn(&TextProps) -> bool` | `crates/designos-doc/src/text_style_ops.rs:16` (was `crates/designos-app/src/styles/text_style_ops.rs:16`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:40`) | Yes — moved with module (`FACT`) |
| `text_layers` (`crates/designos-doc/src/text_style_ops.rs:21`) | `fn(&Document, &[NodeId]) -> Result<Vec<(&Node, &TextProps)>, StyleOpError>` | `crates/designos-doc/src/text_style_ops.rs:21` (was `crates/designos-app/src/styles/text_style_ops.rs:21`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:45`) | Yes — moved with module (`FACT`) |
| `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`) | `pub fn(&Document, &[NodeId]) -> Option<TextStyle>` | `crates/designos-doc/src/text_style_ops.rs:35` (was `crates/designos-app/src/styles/text_style_ops.rs:35`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:59`) | Yes — moved with module (`FACT`) |
| `next_free_name` (`crates/designos-doc/src/text_style_ops.rs:49`) | `fn(&Document) -> String` | `crates/designos-doc/src/text_style_ops.rs:49` (was `crates/designos-app/src/styles/text_style_ops.rs:49`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:73`) | Yes — moved with module (`FACT`) |
| `same_text_style` (`crates/designos-doc/src/lint/token_match.rs:628`) | `pub fn(&TextStyle, &TextStyle) -> bool` | `crates/designos-doc/src/lint/token_match.rs:628` | Yes (`FACT`) |
| `same_text_style_props` (`crates/designos-doc/src/lint/token_match.rs:648`) | `pub fn(&TextProps, &TextStyle) -> bool` | `crates/designos-doc/src/lint/token_match.rs:648` | Yes (`FACT`) |
| `Document` (`crates/designos-doc/src/lib.rs:183`) | Struct (`styles` at `crates/designos-doc/src/lib.rs:191`, `style_meta` at `crates/designos-doc/src/lib.rs:194`, `style_name` at `crates/designos-doc/src/lib.rs:750`, `get` at `crates/designos-doc/src/lib.rs:1048`) | `crates/designos-doc/src/lib.rs:183` | Yes (`FACT`) |
| `NodeId` (`crates/designos-doc/src/lib.rs:133`) | Struct | `crates/designos-doc/src/lib.rs:133` | Yes (`FACT`) |
| `DocError` / `DocError::NotFound` (`crates/designos-doc/src/lib.rs:136-138`) | Error enum & variant | `crates/designos-doc/src/lib.rs:136` (`NotFound` at `crates/designos-doc/src/lib.rs:138`) | Yes (`FACT`) |
| `StyleOpError` (`crates/designos-doc/src/styles_ops.rs:17`) | Error enum (`Doc` at `crates/designos-doc/src/styles_ops.rs:19`, `WrongNodeKind` at `crates/designos-doc/src/styles_ops.rs:25`, `NothingToStyle` at `crates/designos-doc/src/styles_ops.rs:27`) | `crates/designos-doc/src/styles_ops.rs:17` | Yes (`FACT`) |
| `Node` (`crates/designos-doc/src/node.rs:65`) | Struct (`id` at `crates/designos-doc/src/node.rs:66`, `kind` at `crates/designos-doc/src/node.rs:71`, `style_refs` at `crates/designos-doc/src/node.rs:113`) | `crates/designos-doc/src/node.rs:65` | Yes (`FACT`) |
| `NodeKind` / `NodeKind::Text` (`crates/designos-doc/src/node.rs:488,516`) | Enum & variant | `crates/designos-doc/src/node.rs:488` (`Text` at `crates/designos-doc/src/node.rs:516`) | Yes (`FACT`) |
| `TextProps` (`crates/designos-doc/src/node.rs:679`) | Struct (`style` at `crates/designos-doc/src/node.rs:681`, `runs` at `crates/designos-doc/src/node.rs:690`, `paragraph` at `crates/designos-doc/src/node.rs:692`) | `crates/designos-doc/src/node.rs:679` | Yes (`FACT`) |
| `ParagraphProps` (`crates/designos-doc/src/textmodel.rs:130`) | Struct (`indent` at `crates/designos-doc/src/textmodel.rs:132`, `list_spacing` at `crates/designos-doc/src/textmodel.rs:134`, `leading_trim` at `crates/designos-doc/src/textmodel.rs:136`) | `crates/designos-doc/src/textmodel.rs:130` | Yes (`FACT`) |
| `TextStyle` (`crates/designos-doc/src/paint.rs:462`) | Struct | `crates/designos-doc/src/paint.rs:462` | Yes (`FACT`) |
| `TextAlignH` / `TextAlignV` (`crates/designos-doc/src/paint.rs:409,418`) | Enums (`TextAlignH::Left`, `TextAlignV::Top`) | `crates/designos-doc/src/paint.rs:409`, `crates/designos-doc/src/paint.rs:418` | Yes (`FACT`) |
| `Style` / `Style::Text` (`crates/designos-doc/src/lib.rs:1589,1591`) | Enum & variant | `crates/designos-doc/src/lib.rs:1589` (`Text` at `crates/designos-doc/src/lib.rs:1591`) | Yes (`FACT`) |
| `StyleMeta` (`crates/designos-doc/src/lib.rs:1571`) | Struct | `crates/designos-doc/src/lib.rs:1571` | Yes (`FACT`) |
| `StyleProperty` / `StyleProperty::Text` (`crates/designos-doc/src/lib.rs:1561,1564`) | Enum & variant | `crates/designos-doc/src/lib.rs:1561` (`Text` at `crates/designos-doc/src/lib.rs:1564`) | Yes (`FACT`) |
| `Command` (`crates/designos-doc/src/history.rs:8`) | Enum (`Update` at `crates/designos-doc/src/history.rs:24`, `SetStyles` at `crates/designos-doc/src/history.rs:40`, `Batch` at `crates/designos-doc/src/history.rs:76`) | `crates/designos-doc/src/history.rs:8` | Yes (`FACT`) |
| `std::collections::BTreeSet` (`crates/designos-doc/src/text_style_ops.rs:94`) | Standard library collection | `crates/designos-doc/src/text_style_ops.rs:94` | `std` (`FACT`) |
| Test items (`Document::new` at `crates/designos-doc/src/lib.rs:610`, `alloc_id` at `crates/designos-doc/src/lib.rs:706`, `add_page` at `crates/designos-doc/src/lib.rs:1022`, `insert` at `crates/designos-doc/src/lib.rs:1033`, `get_mut` at `crates/designos-doc/src/lib.rs:1052`, `Node::rectangle` at `crates/designos-doc/src/node.rs:426`, `Node::text` at `crates/designos-doc/src/node.rs:436`, `Rect::new` at `crates/designos-doc/src/geom.rs:26`, `History::exec` at `crates/designos-doc/src/history.rs:361`, `TextRun` at `crates/designos-doc/src/paint.rs:570`, `Paint::solid` at `crates/designos-doc/src/paint.rs:231`, `Color::rgb` at `crates/designos-doc/src/paint.rs:19`) | Document & paint test APIs | `crates/designos-doc/src/lib.rs:610-1052`, `crates/designos-doc/src/node.rs:426-436`, `crates/designos-doc/src/geom.rs:26`, `crates/designos-doc/src/history.rs:361`, `crates/designos-doc/src/paint.rs:19-570` | Yes (`FACT`) |
| App-only state / types | None (`0` items) | None (`crates/designos-doc/src/text_style_ops.rs:1-249`) | Zero app-only dependencies (`FACT`) |

### 2.2 Every Caller and Test of the Helpers Across the Workspace

1. **Module declaration and re-export in `designos-app`:**
   - `crates/designos-app/src/styles.rs:501` — `mod text_style_ops;`
   - `crates/designos-app/src/styles.rs:503` — `pub use text_style_ops::{create_text_style_from_nodes, detach_text_style, shared_text_style};`
   - `crates/designos-app/src/styles/text_style_ops.rs:3` — `pub use designos_doc::text_style_ops::{create_text_style_from_nodes, detach_text_style, shared_text_style};`
2. **Production callers of `create_text_style_from_nodes` (`crates/designos-doc/src/text_style_ops.rs:60`):**
   - `crates/designos-app/src/styles.rs:247` — inside `App::create_text_style_from` (`crates/designos-app/src/styles.rs:241`), called by `App::create_style_from` (`crates/designos-app/src/styles.rs:230`) and `crates/designos-app/src/panels/style_picker.rs:256`.
3. **Production callers of `detach_text_style` (`crates/designos-doc/src/text_style_ops.rs:92`):**
   - `crates/designos-app/src/styles.rs:272` — inside `App::detach_style_of` (`crates/designos-app/src/styles.rs:266`), called by `crates/designos-app/src/panels/style_picker.rs:276` and `crates/designos-app/src/panels/style_picker.rs:350`.
   - `crates/designos-app/src/styles/new_text_box.rs:80` — inside `App::detach_text_style_in_new_box` (`crates/designos-app/src/styles/new_text_box.rs:74`), called by `crates/designos-app/src/styles.rs:269`.
4. **Callers of `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`) and `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`):**
   - `crates/designos-doc/src/text_style_ops.rs:44` — `shared_text_style` calls `has_single_text_style`.
   - `crates/designos-doc/src/text_style_ops.rs:65` — `create_text_style_from_nodes` calls `shared_text_style`.
   - `crates/designos-app/src/panels/style_picker.rs:10` and `crates/designos-app/src/panels/style_picker.rs:108` — imports and calls `shared_text_style(&app.doc, sel)`.
5. **Unit tests in `crates/designos-doc/src/text_style_ops.rs:105-249` (`6` moved + `1` new = `7` total, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670`):**
   - `creating_copies_the_layer_style_and_links_the_layer` (`crates/designos-doc/src/text_style_ops.rs:136`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:665`)
   - `names_skip_keys_and_display_names_in_use` (`crates/designos-doc/src/text_style_ops.rs:148`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:667`)
   - `only_a_text_layer_with_one_style_can_be_turned_into_a_style` (`crates/designos-doc/src/text_style_ops.rs:160`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:668`)
   - `several_layers_with_the_same_type_get_one_style_and_all_link_to_it` (`crates/designos-doc/src/text_style_ops.rs:178`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:670`)
   - `several_layers_with_different_type_have_no_single_style` (`crates/designos-doc/src/text_style_ops.rs:192`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:669`)
   - `detaching_drops_the_link_and_keeps_the_style_and_the_values` (`crates/designos-doc/src/text_style_ops.rs:204`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:666`)
   - `create_and_detach_round_trip_on_plain_document` (new no-app test at `crates/designos-doc/src/text_style_ops.rs:218`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664`)
6. **App-side tests exercising `App::create_text_style_from`, `App::create_style_from`, and `App::detach_style_of` via the re-exports (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3408-3416,3678`):**
   - `styles::tests::text_style_apply_keeps_alignment_and_carries_paragraph_settings` (`crates/designos-app/src/styles.rs:624`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3678`)
   - `shell::tests::text_styles::*` (`crates/designos-app/src/shell/tests/text_styles.rs:64-619`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3408-3416`)
7. **How `designos-mcp` reaches doc-side style ops:**
   - `crates/designos-doc/src/lib.rs:73,111` exports `styles_ops` via `pub use styles_ops::*;`, and `crates/designos-mcp/src/engine.rs:410` calls `designos_doc::style_property(...)` directly. Adding `pub mod text_style_ops;` (`crates/designos-doc/src/lib.rs:76`) and `pub use text_style_ops::*;` (`crates/designos-doc/src/lib.rs:114`) makes `designos_doc::create_text_style_from_nodes`, `designos_doc::detach_text_style`, `designos_doc::shared_text_style`, and `designos_doc::has_single_text_style` directly callable from `designos-mcp` (`crates/designos-mcp/src/engine.rs:410`).

---

## 3. Skills Loaded and Rulings Made

| Skill | Path (`file:line`) | Ruling Applied to This Task |
|---|---|---|
| `es:astra-work` | `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md:1-130` | Locked the 4-file scope (`crates/designos-doc/src/text_style_ops.rs:1`, `crates/designos-doc/src/lib.rs:76,114`, `crates/designos-app/src/styles/text_style_ops.rs:1-3`, `plans/reports/agy-1028-text-style-ops-to-doc.md:1`), re-ran all verification gates after the final edit (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:3` through `plans/reports/agy-1028-text-style-ops-to-doc-evidence/09-ls-out-cargo.log:2`), and placed unverified items at the top of the report. |
| `es:fable-thinking` | `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1-140` | Recorded Step 1 facts and the `FACT` / `INFERENCE` / `ASSUMPTION` ledger before editing code; falsified the assumption that `has_single_text_style` was re-exported by `crates/designos-app/src/styles.rs:503` when `cargo clippy` flagged an unused re-export in `crates/designos-app/src/styles/text_style_ops.rs:3`, narrowing the app re-export to `{create_text_style_from_nodes, detach_text_style, shared_text_style}` (`crates/designos-app/src/styles/text_style_ops.rs:3`). |
| `es:rust` | `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1-120` | Kept function signatures, error types (`StyleOpError` at `crates/designos-doc/src/styles_ops.rs:17`), borrowing lifetimes (`text_layers<'a>` at `crates/designos-doc/src/text_style_ops.rs:21`), and `Command` batching (`crates/designos-doc/src/text_style_ops.rs:60-103`) byte-for-byte identical (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:1-20`). |
| `es:rust-gpu-canvas` | `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1-120` | Confirmed that moving pure document `Command` constructors (`crates/designos-doc/src/text_style_ops.rs:60,92`) touches no GPU/render pipeline or `SceneIndex` paths (`crates/designos-doc/src/lib.rs:76,114`). |
| `es:dev` | `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md:1-40` | Delegated to `es:feature-dev` workflow for scoped refactor execution and verification. |
| `es:feature-dev` | `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md:1-120` | Read `.brv/context-tree/session-handoffs/latest.md:1-50` and `.brv/context-tree/architecture/crate-map.md:1-80` directly when `brv` CLI prompted for an interactive version upgrade, keeping the crate boundary clean (`designos-doc` owns pure `Document` style commands). |

---

## 4. Acceptance Criteria & Verification Gates Table

| Gate / Requirement | Command | Exit Code / Verdict | Citation (`log-file:line` / `file:line`) |
|---|---|---|---|
| `create_text_style_from_nodes` & `detach_text_style` in `designos-doc` | `git diff --no-index -U3 <(git show origin/main:crates/designos-app/src/styles/text_style_ops.rs) crates/designos-doc/src/text_style_ops.rs` | Byte-for-byte identical bodies; only `use designos_doc::` -> `use crate::` (`lines 7-8`) + `1` new test (`lines 217-248`) | `crates/designos-doc/src/text_style_ops.rs:60,92`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:9-54` |
| Exported from `designos-doc` crate root | Inspect `crates/designos-doc/src/lib.rs` | `pub mod text_style_ops;` and `pub use text_style_ops::*;` | `crates/designos-doc/src/lib.rs:76,114`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:506,514` |
| Re-exported in `designos-app` without duplicate tests | Inspect `crates/designos-app/src/styles/text_style_ops.rs` | `pub use designos_doc::text_style_ops::{create_text_style_from_nodes, detach_text_style, shared_text_style};` (`3` lines total, `0` tests in `designos-app`) | `crates/designos-app/src/styles/text_style_ops.rs:1-3`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:23-240` |
| `6` moved unit tests + `1` new no-app test in `designos-doc` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-doc --locked` | `7` `text_style_ops::tests::*` passed; `752 passed; 0 failed` unit tests + `3 + 5 + 2` integration tests (`test_doc_rc=0`) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670,764,773,784,792,800` |
| `cargo fmt --all -- --check` | `cargo fmt --all -- --check` | `fmt_rc=0` | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:3` |
| `cargo clippy --workspace --all-targets --locked` | `CARGO_TARGET_DIR=out/cargo cargo clippy --workspace --all-targets --locked` | `clippy_rc=0` (`0` warnings in `designos-doc` or `text_style_ops`; only `main`'s `3` known warnings in `agent_permissions.rs:1176,1194` and `inspector_var_binds.rs:1182`) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/02-clippy.log:8-49` |
| `cargo test -p designos-app --bin designos --locked` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked` + solo reruns | `4240 passed; 3 failed; 13 ignored` (`04-test-app-bin.log:4312`); all `3` failures are known pre-existing flakes (`04-test-app-bin.log:4308-4310`) that pass on solo/short-`/tmp` rerun (`04b-test-app-bin-reruns.log:43,58,73`) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3408-3416,3678,4290-4315`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:13-75` |
| `cargo test -p designos-mcp --locked` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-mcp --locked` | `test_mcp_rc=0` (`327 passed; 0 failed` unit tests + `19` integration test suites passed) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/05-test-mcp.log:339,353-728` |
| `cargo test --workspace --locked --lib` | `CARGO_TARGET_DIR=out/cargo cargo test --workspace --locked --lib` | `test_workspace_lib_rc=0` (`13 + 752 + 1 + 164 + 143 + 30 + 5 + 37 + 327 + 56 + 1 + 8` library unit tests passed, `0` failed) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/06-test-workspace-lib.log:25,783,790,961,1110,1146,1157,1200,1533,1597,1604,1618,1620` |
| `python3 -B scripts/zero_fake_ui_gate.py` | `python3 -B scripts/zero_fake_ui_gate.py` | `Gate Verdict: PASS (17/17 invariants satisfied)`, `zero_fake_rc=0` | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/07-zero-fake-ui-gate.log:24-26` |
| Cleanup `out/cargo` | `rm -rf out/cargo && ls out/cargo` | `ls: out/cargo: No such file or directory`, `ls_out_cargo_rc=1` | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/09-ls-out-cargo.log:1-2` |

---

## 5. Pasted Command Outputs

### 5.1 `cargo fmt --all -- --check` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:1-3`)

```text
Filesystem        Size    Used   Avail Capacity iused ifree %iused  Mounted on
/dev/disk3s1s1   926Gi    12Gi    96Gi    11%    459k  1.0G    0%   /
fmt_rc=0
```

### 5.2 `CARGO_TARGET_DIR=out/cargo cargo clippy --workspace --all-targets --locked` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/02-clippy.log:1-49`)

```text
Filesystem        Size    Used   Avail Capacity iused ifree %iused  Mounted on
/dev/disk3s1s1   926Gi    12Gi    96Gi    11%    459k  1.0G    0%   /
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.7.7 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-mcp)
    Checking designos-app v0.7.7 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-app)
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
warning: very complex type used. Consider factoring parts into `type` definitions
    --> crates/designos-app/src/shell/tests/inspector_var_binds.rs:1182:21
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.07s
clippy_rc=0
```

### 5.3 `CARGO_TARGET_DIR=out/cargo cargo test -p designos-doc --locked` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670,764-800`)

```text
test text_style_ops::tests::create_and_detach_round_trip_on_plain_document ... ok
test text_style_ops::tests::creating_copies_the_layer_style_and_links_the_layer ... ok
test text_style_ops::tests::detaching_drops_the_link_and_keeps_the_style_and_the_values ... ok
test text_style_ops::tests::names_skip_keys_and_display_names_in_use ... ok
test text_style_ops::tests::only_a_text_layer_with_one_style_can_be_turned_into_a_style ... ok
test text_style_ops::tests::several_layers_with_different_type_have_no_single_style ... ok
test text_style_ops::tests::several_layers_with_the_same_type_get_one_style_and_all_link_to_it ... ok
...
test result: ok. 752 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.69s

     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-c3f526bb6ce5e706)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-65955caffc7324cb)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/shadow_blend.rs (out/cargo/debug/deps/shadow_blend-081ec078cae3c8a2)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests designos_doc
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

test_doc_rc=0
```

### 5.4 `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked` & Solo Reruns (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4288-4315`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:12-75`)

```text
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root
    shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 4240 passed; 3 failed; 13 ignored; 0 measured; 0 filtered out; finished in 391.58s
test_app_bin_rc=101

=== RERUN: shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required ===
test shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4255 filtered out; finished in 2.50s
rc=0

=== RERUN: vecops::tests::two_deferred_text_commands_both_convert_in_order ===
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4255 filtered out; finished in 1.64s
rc=0

=== RERUN (short /tmp root): agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ===
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4255 filtered out; finished in 0.00s
rc=0
```

### 5.5 `CARGO_TARGET_DIR=out/cargo cargo test -p designos-mcp --locked` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/05-test-mcp.log:8,339-728`)

```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-b500445f18a41338)
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.80s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-d0dc86933af4af20)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-4a5f14536e654b8a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/button_label_centre.rs -> ok. 4 passed; 0 failed
     Running tests/close_document.rs -> ok. 28 passed; 0 failed
     Running tests/colour_detach.rs -> ok. 11 passed; 0 failed
     Running tests/colour_var_library.rs -> ok. 1 passed; 0 failed
     Running tests/colour_var_refs.rs -> ok. 16 passed; 0 failed
     Running tests/hug_text_relayout.rs -> ok. 11 passed; 0 failed
     Running tests/list_lint_rules.rs -> ok. 4 passed; 0 failed
     Running tests/live_doc_epoch.rs -> ok. 3 passed; 0 failed
     Running tests/live_session_guards.rs -> ok. 3 passed; 0 failed
     Running tests/mcp_paths.rs -> ok. 23 passed; 0 failed
     Running tests/mcp_paths_full_disk.rs -> ok. 1 passed; 0 failed
     Running tests/session.rs -> ok. 24 passed; 0 failed
     Running tests/session_file_sites/main.rs -> ok. 9 passed; 0 failed
     Running tests/session_files/main.rs -> ok. 42 passed; 0 failed
     Running tests/set_slot_batch.rs -> ok. 14 passed; 0 failed
     Running tests/spec_errors.rs -> ok. 8 passed; 0 failed
     Running tests/spec_text_colour.rs -> ok. 5 passed; 0 failed
     Running tests/stdio.rs -> ok. 1 passed; 0 failed
     Running tests/text_fill.rs -> ok. 7 passed; 0 failed
     Running tests/text_truncation_roundtrip.rs -> ok. 2 passed; 0 failed
     Running tests/tools_snapshot.rs -> ok. 8 passed; 0 failed
     Running tests/version.rs -> ok. 4 passed; 0 failed
     Running tests/wrapped_fill_text_height.rs -> ok. 2 passed; 0 failed
test_mcp_rc=0
```

### 5.6 `CARGO_TARGET_DIR=out/cargo cargo test --workspace --locked --lib` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/06-test-workspace-lib.log:8-1620`)

```text
     Running unittests src/lib.rs (designos_diff) -> ok. 13 passed; 0 failed
     Running unittests src/lib.rs (designos_doc) -> ok. 752 passed; 0 failed
     Running unittests src/lib.rs (designos_eval) -> ok. 1 passed; 0 failed
     Running unittests src/lib.rs (designos_export) -> ok. 164 passed; 0 failed; 1 ignored
     Running unittests src/lib.rs (designos_fig) -> ok. 143 passed; 0 failed
     Running unittests src/lib.rs (designos_fonts) -> ok. 30 passed; 0 failed
     Running unittests src/lib.rs (designos_gpu) -> ok. 5 passed; 0 failed
     Running unittests src/lib.rs (designos_library) -> ok. 37 passed; 0 failed
     Running unittests src/lib.rs (designos_mcp) -> ok. 327 passed; 0 failed
     Running unittests src/lib.rs (designos_render) -> ok. 56 passed; 0 failed; 2 ignored
     Running unittests src/lib.rs (designos_svg) -> ok. 1 passed; 0 failed
     Running unittests src/lib.rs (designos_webcapture) -> ok. 8 passed; 0 failed
test_workspace_lib_rc=0
```

### 5.7 `python3 -B scripts/zero_fake_ui_gate.py` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/07-zero-fake-ui-gate.log:1-26`)

```text
============================================================================================================
 DESIGN:OS Studio — Zero-Fake-UI Architectural Gate
============================================================================================================
Gate  | Status | File                             | Invariant
------------------------------------------------------------------------------------------------------------
Z01   | PASS   | terminal.rs                      | Native Embedded PTY Terminal (portable_pty + alacritty_terminal)
Z02   | PASS   | terminal.rs                      | Spawned Agent CLI gets live DESIGNOS_LIVE_DOC / DESIGNOS_MCP_BIN env vars
Z03   | PASS   | terminal.rs                      | dispatch_figma_ai_prompt sends executable command without # comment prefix
Z04   | PASS   | terminal.rs + agent_header.rs + agent_composer_toolbar.rs | Agents header and composer buttons have Sense::click() handlers; fake mic removed
Z05   | PASS   | specs.rs                         | specs-ask-agent-bar hosts editable TextEdit::singleline(ask_input)
Z06   | PASS   | specs.rs                         | Right Inspector dynamic active_section_toc & interactive SpecComment
Z07   | PASS   | specs.rs                         | Unwrapped draw_wrapped_prose & render_tab_blueprints renders pkg.ui_contract
Z08   | PASS   | panels/toolbar.rs + cmdk.rs      | Contextual sub-toolbar buttons mutate app.doc / app.tool; ctx-crop / ctx-remove-bg / cmdk Remove background stay removed
Z09   | PASS   | home/mod.rs                      | Act::ExportFig writes .fig to disk & Nav::AllFolders/Templates/Libraries filter distinctly
Z10   | PASS   | panels/right.rs                  | Share copies to clipboard & Dev Mode 2nd tab renders Dev Plugins inspector
Z11   | PASS   | panels/inspect.rs + properties.rs | Dev Mode Code Connect/MCP/Redlines/Keyframes & Copy to Slides mutate state/clipboard
Z12   | PASS   | widgets/picker.rs + panels/mod.rs + variables/view.rs | Mode::Image creates Paint::Image/doc.images & scrollbars support Sense::click_and_drag()
Z13   | PASS   | panels/menus.rs                  | Zoom menu rows are the allow-listed set on real Actions; no hard-coded checked state
Z14   | PASS   | updater.rs + tabs.rs             | Sample-manifest updater UI stays hidden (guard is code in tabs, sidebar card, modal) until a real manifest fetch exists
Z15   | PASS   | terminal.rs + cmdk.rs            | No fake template synthesis or canned Executed via transcript in Agents panel; synthesize_spec_from_prompt stays private to cmdk.rs
Z16   | PASS   | registered label helpers in designos-app/src | UX writing: action labels at registered button/menu helpers and label fns <=3 words, no count in them (ratchet vs ux_writing_baseline.json)
Z17   | PASS   | terminal.rs + mcp/server.rs      | No global /tmp/designos-live-* or designos-live-selection.spec.json fallback in shipped code
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
zero_fake_rc=0
```

### 5.8 `git diff --stat origin/main...HEAD` and Move Diff (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:1-30,240-256,497-517`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:1-54`)

```text
 crates/designos-app/src/styles/text_style_ops.rs   |  216 +-
 crates/designos-doc/src/lib.rs                     |    2 +
 crates/designos-doc/src/text_style_ops.rs          |  249 ++
```

```diff
diff --git a/dev/fd/12 b/crates/designos-doc/src/text_style_ops.rs
index 00000000..0f49557f 100644
--- a/dev/fd/12
+++ b/crates/designos-doc/src/text_style_ops.rs
@@ -4,8 +4,8 @@
 //! returns a [`Command`] and leaves the document alone, so the caller decides how the change joins
 //! the undo stack.
 
-use designos_doc::lint::token_match::{same_text_style, same_text_style_props};
-use designos_doc::*;
+use crate::lint::token_match::{same_text_style, same_text_style_props};
+use crate::*;
 
 /// Key of the `style_refs` entry that links a text layer to its text style.
 const TEXT_REF: &str = "text";
@@ -212,4 +212,38 @@ mod tests {
         assert!(d.styles.contains_key(&key));
         assert_eq!(d.get(t).unwrap().kind, before);
     }
+
+    #[test]
+    fn create_and_detach_round_trip_on_plain_document() {
+        let mut doc = Document::new("plain");
+        let page = doc.add_page("Page 1");
+        let tid = doc.alloc_id();
+        let mut text = Node::text(
+            tid,
+            Rect::new(0.0, 0.0, 120.0, 24.0),
+            "Heading",
+            TextStyle { font_size: 28.0, font_weight: 600, align_h: TextAlignH::Center, ..Default::default() },
+        );
+        if let NodeKind::Text(tp) = &mut text.kind {
+            tp.paragraph.indent = 8.0;
+        }
+        let text_id = doc.insert(page, text).unwrap();
+
+        let mut hist = History::default();
+        let (key, create_cmd) = crate::create_text_style_from_nodes(&doc, &[text_id]).unwrap();
+        hist.exec(&mut doc, create_cmd).unwrap();
+        assert_eq!(doc.get(text_id).unwrap().style_refs.get("text"), Some(&key));
+        assert!(matches!(&doc.styles[&key], Style::Text(ts) if ts.font_size == 28.0 && ts.font_weight == 600 && ts.paragraph_indent == 8.0));
+
+        let detach_cmd = crate::detach_text_style(&doc, &[text_id]).unwrap();
+        hist.exec(&mut doc, detach_cmd).unwrap();
+        assert!(!doc.get(text_id).unwrap().style_refs.contains_key("text"));
+        assert!(doc.styles.contains_key(&key));
+
+        assert!(hist.undo(&mut doc).unwrap());
+        assert_eq!(doc.get(text_id).unwrap().style_refs.get("text"), Some(&key));
+        assert!(hist.undo(&mut doc).unwrap());
+        assert!(doc.styles.is_empty());
+        assert!(!doc.get(text_id).unwrap().style_refs.contains_key("text"));
+    }
 }
```

### 5.9 `ls out/cargo` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/09-ls-out-cargo.log:1-2`)

```text
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

## 6. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), Assumptions & Weakest Link

### 6.1 Epistemic Ledger
- **`FACT`**: Every symbol referenced by `create_text_style_from_nodes` (`crates/designos-doc/src/text_style_ops.rs:60`), `detach_text_style` (`crates/designos-doc/src/text_style_ops.rs:92`), `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`), `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`), `text_layers` (`crates/designos-doc/src/text_style_ops.rs:21`), and `next_free_name` (`crates/designos-doc/src/text_style_ops.rs:49`) already resided in `designos-doc` (`crates/designos-doc/src/lib.rs:133-1591`, `crates/designos-doc/src/styles_ops.rs:17`, `crates/designos-doc/src/lint/token_match.rs:628,648`).
- **`FACT`**: The moved implementations and `6` unit tests in `crates/designos-doc/src/text_style_ops.rs:1-215` are byte-for-byte identical to `origin/main:crates/designos-app/src/styles/text_style_ops.rs:1-215` except for lines `7-8` (`use designos_doc::` -> `use crate::`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:9-12`).
- **`FACT`**: `crates/designos-app/src/styles.rs:503` re-exports `{create_text_style_from_nodes, detach_text_style, shared_text_style}` from `crates/designos-app/src/styles/text_style_ops.rs:3`; re-exporting `has_single_text_style` in `crates/designos-app/src/styles/text_style_ops.rs` triggers `unused_imports` because no caller in `designos-app` imports `has_single_text_style` through `styles::text_style_ops`.
- **`INFERENCE`**: Exporting both `pub mod text_style_ops;` (`crates/designos-doc/src/lib.rs:76`) and `pub use text_style_ops::*;` (`crates/designos-doc/src/lib.rs:114`) satisfies both module-qualified callers (`designos_doc::text_style_ops::*`, `crates/designos-app/src/styles/text_style_ops.rs:3`) and crate-root callers (`designos_doc::create_text_style_from_nodes`, `crates/designos-doc/src/text_style_ops.rs:233`), matching `styles_ops` (`crates/designos-doc/src/lib.rs:73,111`).
- **`ASSUMPTION`**: None remaining (`0` unverified assumptions).

### 6.2 Weakest Link
- `crates/designos-app/src/styles/text_style_ops.rs:3` re-exports `{create_text_style_from_nodes, detach_text_style, shared_text_style}` rather than `pub use designos_doc::text_style_ops::*;` so that `rustc`'s `unused_imports` lint stays clean while `crates/designos-app/src/styles.rs:501-503` keeps `mod text_style_ops;` private and re-exports those `3` symbols. If a future caller in `crates/designos-app/src/styles.rs` needs `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`), it can import it either from `designos_doc::has_single_text_style` (`crates/designos-doc/src/lib.rs:114`) or add it to the list in `crates/designos-app/src/styles/text_style_ops.rs:3`.

---

## 7. Unresolved Questions

None.

Status: DONE

--- END REPORT: plans/reports/agy-1028-text-style-ops-to-doc.md ---

--- REPORT: plans/reports/agy-1039-1027.md ---
# Properties Panel — Library Styles Section, Page Export `+`, and Typography Pill (`#1039`, `#1027`)

## 1. What could NOT be verified

- **FACT:** Interactive GUI window launch was not executed because the task instructions explicitly forbid launching the GUI app (`"Do not launch the GUI app"`). All UI layout, hit-testing, style application, and page export behaviors were verified through the headless `Sim` test harness (`egui::Context::run_ui`) and `designos_render` pixel/file assertions.
- **Conclusion:**
  - **FACT (`#1039`):** Selection `Styles` section (`crates/designos-app/src/panels/properties.rs:1032`) now shows whenever any `StyleKind` accepted by the selection has local styles (`app.doc.styles`) **or** enabled-library styles (`app.library_styles()`), filtering both `styles_list` and `library_styles_list` to the `StyleKind`s applicable to `target` and importing+applying library styles in a single undo step via `app.apply_library_style`.
  - **FACT (`#1027`, part 1):** Page-level `Export` section in `styles_section` (`crates/designos-app/src/panels/properties.rs:466-467`) is wired to `super::export::section(app, ui, pane.x0, pane.oy + pane.y, &[page])`, using Studio's existing `NodeKind::Page` export pipeline (`designos_export::{bounds,raster,svg}` and `panels/export.rs`).
  - **FACT (`#1027`, part 2):** `typo-var-pill` (`crates/designos-app/src/panels/typography.rs:195-199`) is positioned in the Typography header row to the left of `text-style-button` (`x = 127..151`, `24x24` = `theme::PANEL_ACTION_BUTTON` >= `theme::TARGET_MIN`), uncovering it from `typo-more-btn` (`x = 209..233`) so clicking `typo-var-pill` opens the Variables modal (`app.vars.open = true`).
- **Weakest link:** Under full-suite parallel PTY load (`3278` concurrent tests), PTY reader timing tests `shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one` (known flake `#996`) and `terminal::gemini_narrow_tests::output_the_cli_prints_through_its_own_terminal_is_read` can time out on reader thread scheduling; both pass `3/3` when run alone.

---

## 2. Acceptance table (1–6), with exact commands and pasted output

| # | Gate | Verdict | Evidence |
|---|---|---|---|
| 1 | Red first (`dc23fdd4`) -> Green (`HEAD`), saved to `red.log` and `green.log` | **PASS** | `plans/reports/agy-1039-1027-evidence/red.log` (all 3 fail on `dc23fdd4` for the stated reasons) and `plans/reports/agy-1039-1027-evidence/green.log` (all 3 pass on `HEAD`) |
| 2 | `cargo fmt --all -- --check` exits 0 | **PASS** | Exit code `0` |
| 3 | `cargo clippy --workspace --all-targets --locked -- -D warnings` exits 0 | **PASS** | Exit code `0` |
| 4 | `cargo test -p designos-app --bin designos --locked` and `--bin designos-studio` | **PASS** | `3278` tests run per binary; all touched/new tests pass; known flake `#996` and `gemini_narrow_tests::output_the_cli_prints_through_its_own_terminal_is_read` pass `3/3` solo reruns |
| 5 | `python3 scripts/zero_fake_ui_gate.py` passes | **PASS** | `Gate Verdict: PASS (17/17 invariants satisfied)` (plus `check_spacing_relationships.py` PASS) |
| 6 | Delete `out/` at the end | **PASS** | `rm -rf out && test ! -e out` exits `0` |

### Acceptance 1 — Red first (`plans/reports/agy-1039-1027-evidence/red.log`) and Green (`plans/reports/agy-1039-1027-evidence/green.log`)

#### Red command (on `dc23fdd4` production code)
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked selection_styles_section_shows_library_styles_when_file_has_no_local_styles -- --nocapture || true
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked page_panel_export_plus_adds_a_page_export_row_and_exports_the_page -- --nocapture || true
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked clicking_typo_var_pill_opens_variables_without_toggling_more -- --nocapture || true
} 2>&1 | tee plans/reports/agy-1039-1027-evidence/red.log
rm -rf "$S"
```

#### Pasted output (`plans/reports/agy-1039-1027-evidence/red.log`)
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 14.41s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test

thread 'shell::tests::libraries::selection_styles_section_shows_library_styles_when_file_has_no_local_styles' (44688277) panicked at crates/designos-app/src/shell/tests.rs:194:113:
nothing recorded for libstyle:Brand/Red
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test shell::tests::libraries::selection_styles_section_shows_library_styles_when_file_has_no_local_styles ... FAILED

failures:

failures:
    shell::tests::libraries::selection_styles_section_shows_library_styles_when_file_has_no_local_styles

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.45s

error: test failed, to rerun pass `-p designos-app --bin designos`
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.70s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test

thread 'shell::tests::text_styles::page_panel_export_plus_adds_a_page_export_row_and_exports_the_page' (44690531) panicked at crates/designos-app/src/shell/tests/text_styles.rs:822:5:
assertion `left == right` failed: clicking page Export + adds a page export preset
  left: 0
 right: 1
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test shell::tests::text_styles::page_panel_export_plus_adds_a_page_export_row_and_exports_the_page ... FAILED

failures:

failures:
    shell::tests::text_styles::page_panel_export_plus_adds_a_page_export_row_and_exports_the_page

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.30s

error: test failed, to rerun pass `-p designos-app --bin designos`
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.75s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test

thread 'shell::tests::typography::clicking_typo_var_pill_opens_variables_without_toggling_more' (44692420) panicked at crates/designos-app/src/shell/tests/typography.rs:149:5:
clicking typo-var-pill opens the Variables modal instead of being swallowed by typo-more-btn
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
test shell::tests::typography::clicking_typo_var_pill_opens_variables_without_toggling_more ... FAILED

failures:

failures:
    shell::tests::typography::clicking_typo_var_pill_opens_variables_without_toggling_more

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.26s

error: test failed, to rerun pass `-p designos-app --bin designos`
```

#### Green command (on `HEAD`)
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked selection_styles_section_shows_library_styles_when_file_has_no_local_styles -- --nocapture
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked page_panel_export_plus_adds_a_page_export_row_and_exports_the_page -- --nocapture
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked clicking_typo_var_pill_opens_variables_without_toggling_more -- --nocapture
} 2>&1 | tee plans/reports/agy-1039-1027-evidence/green.log
rm -rf "$S"
```

#### Pasted output (`plans/reports/agy-1039-1027-evidence/green.log`)
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 20.00s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test shell::tests::libraries::selection_styles_section_shows_library_styles_when_file_has_no_local_styles ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 2.38s

warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 20.60s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test shell::tests::text_styles::page_panel_export_plus_adds_a_page_export_row_and_exports_the_page ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 1.52s

warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 18.11s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test shell::tests::typography::clicking_typo_var_pill_opens_variables_without_toggling_more ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.81s
```

---

### Acceptance 2 — `cargo fmt --all -- --check`

#### Command
```sh
cargo fmt --all -- --check
echo "fmt_exit=$?"
```

#### Pasted output
```text
fmt_exit=0
```

---

### Acceptance 3 — `cargo clippy --workspace --all-targets --locked -- -D warnings`

#### Command
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
echo "clippy_exit=$?"
rm -rf "$S"
```

#### Pasted output
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-diff v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-diff)
    Checking designos-doc v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-doc)
    Checking designos-fonts v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-fonts)
    Checking designos-render v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-render)
    Checking designos-svg v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-svg)
    Checking designos-export v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-export)
    Checking designos-gpu v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-gpu)
    Checking designos-webcapture v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-webcapture)
    Checking designos-fig v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-fig)
    Checking designos-library v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-library)
    Checking designos-eval v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-eval)
    Checking designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 57s
clippy_exit=0
```

---

### Acceptance 4 — `cargo test -p designos-app --bin designos --locked` and `--bin designos-studio`

#### Full-suite command
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos-studio --locked
rm -rf "$S"
```

#### Full-suite summary output
```text
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)
running 3278 tests
...
test shell::tests::libraries::selection_styles_section_shows_library_styles_when_file_has_no_local_styles ... ok
test shell::tests::text_styles::page_panel_export_plus_adds_a_page_export_row_and_exports_the_page ... ok
test shell::tests::typography::clicking_typo_var_pill_opens_variables_without_toggling_more ... ok
...
failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one
    terminal::gemini_narrow_tests::output_the_cli_prints_through_its_own_terminal_is_read

test result: FAILED. 3265 passed; 2 failed; 11 ignored; 0 measured; 0 filtered out; finished in 435.87s

     Running unittests src/main.rs (out/cargo/debug/deps/designos_studio-a3645dd294d29853)
running 3278 tests
...
test shell::tests::libraries::selection_styles_section_shows_library_styles_when_file_has_no_local_styles ... ok
test shell::tests::text_styles::page_panel_export_plus_adds_a_page_export_row_and_exports_the_page ... ok
test shell::tests::typography::clicking_typo_var_pill_opens_variables_without_toggling_more ... ok
...
failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one

test result: FAILED. 3266 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 360.67s
```

#### 3x solo reruns of the two untouched PTY timing tests (per Acceptance 4 & flake rules)
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
for i in 1 2 3; do
  echo "=== designos rerun $i ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked output_the_cli_prints_through_its_own_terminal_is_read
  echo "=== designos-studio rerun $i ==="
  env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos-studio --locked an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one
done
rm -rf "$S"
```

#### Pasted output of 3x solo reruns
```text
=== designos rerun 1 ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.65s

     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test terminal::gemini_narrow_tests::output_the_cli_prints_through_its_own_terminal_is_read ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 1.00s

=== designos-studio rerun 1 ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos_studio-a3645dd294d29853)

running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.56s

=== designos rerun 2 ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.55s

     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test terminal::gemini_narrow_tests::output_the_cli_prints_through_its_own_terminal_is_read ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 1.10s

=== designos-studio rerun 2 ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos_studio-a3645dd294d29853)

running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.56s

=== designos rerun 3 ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.55s

     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 1 test
test terminal::gemini_narrow_tests::output_the_cli_prints_through_its_own_terminal_is_read ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 1.12s

=== designos-studio rerun 3 ===
     Running unittests src/main.rs (out/cargo/debug/deps/designos_studio-a3645dd294d29853)

running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3277 filtered out; finished in 0.54s
```

---

### Acceptance 5 — `python3 scripts/zero_fake_ui_gate.py`

#### Command
```sh
python3 scripts/zero_fake_ui_gate.py
```

#### Pasted output
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

### Acceptance 6 — Delete `out/`

#### Command
```sh
rm -rf out
test ! -e out && echo "out_deleted=0"
```

#### Pasted output
```text
out_deleted=0
```

---

## 3. Changes per file

- `crates/designos-app/src/panels/properties.rs`:
  - Wired the page-level `Export` section in `styles_section` (`super::export::section(app, ui, pane.x0, pane.oy + pane.y, &[page])`) instead of calling `title(ui, pane, "Export", Some(Icon::Plus), true)` and dropping the return value (`#1027` part 1).
  - Added `STYLE_GROUP_KINDS`, `target_accepts_style_kind`, `style_kind_of`, and `has_styles_for` so the selection's `Styles` section (`selection_ui`) renders whenever any `StyleKind` accepted by the selection has local styles or enabled-library styles (`#1039`).
  - Updated `styles_list` and `library_styles_list` to filter style groups by `target_accepts_style_kind`, support all 4 `StyleKind`s (`Text`, `Color`, `Effect`, `Grid`) for library styles, use `theme::STYLES_LABEL_GAP` when the first visible group comes from a library (`shown_groups == 0`), and apply library styles in a single undo step via `app.apply_library_style` (`#1039`).
  - Removed the covered `var_pill` (`typo-var-pill`) from `selection_ui` (`#1027` part 2, moved to `panels/typography.rs`).
- `crates/designos-app/src/panels/typography.rs`:
  - Placed `typo-var-pill` (`24x24` = `theme::PANEL_ACTION_BUTTON` >= `theme::TARGET_MIN`) in the Typography header row immediately left of `style_btn` (`text-style-button`) with `theme::SPACE_FACT` spacing so `typo-more-btn` no longer overlaps or intercepts clicks on `typo-var-pill` (`#1027` part 2).
- `crates/designos-app/src/panels/export.rs`:
  - Recorded `"title-action:Export"` and `"export-add"` on `plus` and `"export-btn"` on the export button for deterministic `Sim` hit-testing (`#1027` part 1).
  - Extended `plan_export` so a `NodeKind::Page` with a PDF export preset expands its visible `NodeKind::Frame` children into the PDF job (`#1027` part 1).
- `crates/designos-app/src/shell/tests/libraries.rs`:
  - Added `selection_styles_section_shows_library_styles_when_file_has_no_local_styles` (`#1039`).
- `crates/designos-app/src/shell/tests/text_styles.rs`:
  - Added `page_panel_export_plus_adds_a_page_export_row_and_exports_the_page` (`#1027` part 1).
- `crates/designos-app/src/shell/tests/typography.rs`:
  - Added `clicking_typo_var_pill_opens_variables_without_toggling_more` (`#1027` part 2).

---

## 4. Decisions: Export `+` wired or removed, and why

- **Decision:** **Wired** the page-level `Export` `+` button by delegating the page panel's Export section in `styles_section` (`crates/designos-app/src/panels/properties.rs:466-467`) to `super::export::section(app, ui, pane.x0, pane.oy + pane.y, &[page])`.
- **Why (code citations):**
  1. **FACT:** Every page is a real node (`NodeKind::Page { background }`) in `app.doc.nodes` (`app.page: NodeId`) carrying `export_settings: Vec<ExportPreset>`.
  2. **FACT:** `crates/designos-export/src/bounds.rs:84-86` (`bounds_of`) explicitly handles `NodeKind::Page { .. }` by unioning visible child bounds.
  3. **FACT:** `crates/designos-export/src/raster.rs:85,94-98` (`render_export`) explicitly handles `NodeKind::Page { .. }` and renders the page via `designos_render::render_page`.
  4. **FACT:** `crates/designos-export/src/svg.rs:53-55` (`export_svg_plain`) explicitly handles `NodeKind::Page { .. }` and emits `<svg>` over the page's visible children.
  5. **FACT:** `crates/designos-app/src/panels/export.rs:188` (`pub fn section(app: &mut App, ui: &mut Ui, x0: f32, top: f32, sel: &[NodeId]) -> f32`) already manages `export_settings`, preview thumbnails (`thumbnail` calls `render_export`), and `"Export <page name>"` for any `&[NodeId]` slice including `&[app.page]`.

---

## 5. Findings outside scope

- **FACT (`#1027#issuecomment-6365418258`, `crates/designos-app/src/panels/style_picker.rs:89-127`):** In `style_picker::menu_model`, `MenuItem::disabled` rows (`Create style` when the selection is already linked to a style, and `Rename style` / `Edit style` / `Detach style` when no style is linked) only set `.why(...)` on `Create style` when `multiple_text_styles` is true (`"Selected text has mixed styles"`). The other disabled states have no hover reason (`why` is empty). `crates/designos-app/src/panels/style_picker.rs` is outside the allowed modification list for this task (`properties.rs`, `typography.rs`, `export.rs`, `shell/tests/`), so it was left untouched per task rules.

---

## 6. Assumptions

- **ASSUMPTION (verified against `styles.rs:84-128` and `properties.rs:891-971`):** For a non-empty selection `sel`, the style kinds applicable to `sel` are `StyleKind::Color` and `StyleKind::Effect` (accepted by any layer), `StyleKind::Text` (accepted when `sel` contains a `NodeKind::Text` node), and `StyleKind::Grid` (accepted when `sel` contains a frame node with `frame_props().is_some()`).
- **ASSUMPTION (verified against `typography.rs:167-198`):** Placing `typo-var-pill` (`24x24`) immediately to the left of `style_btn` (`text-style-button`) with `theme::SPACE_FACT` (`4px`) spacing keeps all four Typography title-row action buttons (`typo-var-pill` at `x = 127..151`, `text-style-button` at `x = 155..179`, `type-details` at `x = 183..207`, `typo-more-btn` at `x = 209..233`) non-overlapping, >= `24x24`, and to the right of the `"Typography"` title label (`x = 17..82`).
- **Unresolved questions:** None.

Status: DONE

---

## Round 2

### 1. What could NOT be verified

- **FACT:** Interactive GUI window launch and full `designos-app` test suite were not executed because the Round 2 instructions explicitly state `"Do not launch the GUI app"` and `"Do not run the full app suite; the controller runs it"`.
- **Conclusion:**
  - **FACT (`#1027`):** Removed the unpainted `typo-var-pill` hit area (`var_pill`), its `widgets::record(ui, "typo-var-pill", var_pill)`, and its `ui.interact(...).on_hover_text("Apply typography variable or style")` handler from `crates/designos-app/src/panels/typography.rs` without adding a replacement button.
  - **FACT (`#1027`):** Replaced `clicking_typo_var_pill_opens_variables_without_toggling_more` in `crates/designos-app/src/shell/tests/typography.rs` with `typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing`, which selects `Continue` and verifies that no `typo-var-pill` rect is recorded (`has_no_rect(&s, "typo-var-pill")`) and clicking the `24x24` area left of `text-style-button` leaves `vars.open` (`false`), the undo stack (`undo_depth() == 0`, `!can_undo()`), and the More toggle (`more_open(&s) == false`, `text-sw` top unchanged) untouched.
- **Weakest link:** None in the touched scope; full-suite regression verification is owned by the controller per the Round 2 task specification.

### 2. Acceptance table (1–7), with exact commands and pasted output

| # | Gate | Verdict | Evidence |
|---|---|---|---|
| 1 | The new test passes on head (`r2-green.log`) and the old test is gone | **PASS** | `plans/reports/agy-1039-1027-evidence/r2-green.log` (`6 passed; 0 failed` in `shell::tests::typography`, including `typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing`; `clicking_typo_var_pill_opens_variables_without_toggling_more` removed) |
| 2 | `cargo fmt --all -- --check` exits 0 | **PASS** | `plans/reports/agy-1039-1027-evidence/r2-fmt.log` (`fmt_exit=0`) |
| 3 | `cargo clippy -p designos-app --all-targets --locked -- -D warnings` exits 0 | **PASS** | `plans/reports/agy-1039-1027-evidence/r2-clippy.log` (`clippy_exit=0`) |
| 4 | `cargo test -p designos-app --bin designos --locked shell::tests::typography` passes under the mandatory harness | **PASS** | `plans/reports/agy-1039-1027-evidence/r2-green.log` (`6 passed; 0 failed`) |
| 5 | `python3 scripts/zero_fake_ui_gate.py` passes | **PASS** | `plans/reports/agy-1039-1027-evidence/r2-gate.log` (`Gate Verdict: PASS (17/17 invariants satisfied)`) |
| 6 | Commit as `fix(properties): remove the hidden typography variable button`, push, update PR #1092 body, keep draft | **PASS** | `plans/reports/agy-1039-1027-evidence/r2-pr.log` (`isDraft: true`, Round 2 line added to PR #1092 body, commit pushed to `origin/jangtrinh/agy-1039-1027-properties`) |
| 7 | Delete `out/` at the end | **PASS** | `plans/reports/agy-1039-1027-evidence/r2-out.log` (`out_deleted=0`) |

#### Acceptance 1 & 4 — `cargo test -p designos-app --bin designos --locked shell::tests::typography` (`plans/reports/agy-1039-1027-evidence/r2-green.log`)

##### Command
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked shell::tests::typography 2>&1 | tee plans/reports/agy-1039-1027-evidence/r2-green.log
rm -rf "$S"
```

##### Pasted output (`plans/reports/agy-1039-1027-evidence/r2-green.log`)
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
   Compiling designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 32.18s
     Running unittests src/main.rs (out/cargo/debug/deps/designos-d02548a0edc4e955)

running 6 tests
test shell::tests::typography::popover_lists_only_the_stylistic_sets_the_font_has ... ok
test shell::tests::typography::features_follow_the_selected_range_while_editing ... ok
test shell::tests::typography::typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing ... ok
test shell::tests::typography::tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus ... ok
test shell::tests::typography::paragraph_settings_apply_to_the_layer ... ok
test shell::tests::typography::feature_checks_toggle_on_the_layer ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 3272 filtered out; finished in 4.67s
```

#### Acceptance 2 — `cargo fmt --all -- --check` (`plans/reports/agy-1039-1027-evidence/r2-fmt.log`)

##### Command
```sh
{ cargo fmt --all -- --check; echo "fmt_exit=$?"; } 2>&1 | tee plans/reports/agy-1039-1027-evidence/r2-fmt.log
```

##### Pasted output (`plans/reports/agy-1039-1027-evidence/r2-fmt.log`)
```text
fmt_exit=0
```

#### Acceptance 3 — `cargo clippy -p designos-app --all-targets --locked -- -D warnings` (`plans/reports/agy-1039-1027-evidence/r2-clippy.log`)

##### Command
```sh
export PATH="$HOME/.cargo/bin:$PATH"; REAL_HOME="$HOME"; T0="${TMPDIR:-/tmp}"; T0="${T0%/}"; S="$(mktemp -d "$T0/x.XXXXXX")"; mkdir -p "$S/home"
export CARGO_TARGET_DIR="$PWD/out/cargo"
{ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy -p designos-app --all-targets --locked -- -D warnings; echo "clippy_exit=$?"; } 2>&1 | tee plans/reports/agy-1039-1027-evidence/r2-clippy.log
rm -rf "$S"
```

##### Pasted output (`plans/reports/agy-1039-1027-evidence/r2-clippy.log`)
```text
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-mcp)
    Checking designos-app v0.6.1 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1039-1027-properties/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.61s
clippy_exit=0
```

#### Acceptance 5 — `python3 scripts/zero_fake_ui_gate.py` (`plans/reports/agy-1039-1027-evidence/r2-gate.log`)

##### Command
```sh
python3 scripts/zero_fake_ui_gate.py 2>&1 | tee plans/reports/agy-1039-1027-evidence/r2-gate.log
```

##### Pasted output (`plans/reports/agy-1039-1027-evidence/r2-gate.log`)
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

#### Acceptance 6 — Commit, push, and PR #1092 update (`plans/reports/agy-1039-1027-evidence/r2-pr.log`)

##### Command
```sh
git add crates/designos-app/src/panels/typography.rs crates/designos-app/src/shell/tests/typography.rs plans/reports/agy-1039-1027.md plans/reports/agy-1039-1027-evidence/r2-*.log
git commit -m "fix(properties): remove the hidden typography variable button"
git push
gh pr view 1092 --json number,isDraft,body
```

##### Pasted output (`plans/reports/agy-1039-1027-evidence/r2-pr.log`)
```text
https://github.com/jangtrinh/design-os-studio/pull/1092
{"body":"## Summary\n- Show the selection `Styles` section when the selection's accepted style kinds have local styles or enabled-library styles, filter groups by applicable kinds, and apply picked library styles in a single undo step (`#1039`).\n- Wire the page-level `Export` section in `styles_section` to `panels::export::section` so clicking `+` on the page Export header adds a page export preset and exports the page (`#1027`).\n- Position `typo-var-pill` (`24x24`) to the left of `text-style-button` in the Typography header row so `typo-more-btn` no longer covers it (`#1027`).\n- Round 2: the typography variable button was never drawn and applied nothing, so it is removed (Zero Fake UI); the Variables manager opens from the left rail and the menus.\n\nCloses #1039\nCloses #1027\n","isDraft":true,"number":1092}
```

#### Acceptance 7 — Delete `out/` (`plans/reports/agy-1039-1027-evidence/r2-out.log`)

##### Command
```sh
rm -rf out && test ! -e out && echo "out_deleted=0" | tee plans/reports/agy-1039-1027-evidence/r2-out.log
```

##### Pasted output (`plans/reports/agy-1039-1027-evidence/r2-out.log`)
```text
out_deleted=0
```

### 3. Changes per file

- `crates/designos-app/src/panels/typography.rs`:
  - Deleted `var_pill`, `widgets::record(ui, "typo-var-pill", var_pill)`, and `ui.interact(var_pill, ui.id().with("typo-var-pill"), Sense::click()).on_hover_text("Apply typography variable or style")` from `section`.
- `crates/designos-app/src/shell/tests/typography.rs`:
  - Added `has_no_rect` and `more_open` helpers.
  - Replaced `clicking_typo_var_pill_opens_variables_without_toggling_more` with `typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing`, asserting that no `typo-var-pill` rect is recorded and that clicking in the `24x24` area left of `text-style-button` leaves `vars.open` (`false`), the undo stack (`undo_depth()`, `can_undo()`), and the More toggle (`more_open`, `text-sw` top) unchanged.
- `plans/reports/agy-1039-1027.md` and `plans/reports/agy-1039-1027-evidence/r2-*.log`:
  - Appended `## Round 2` report section and saved Round 2 evidence logs.

### 4. Assumptions

- **ASSUMPTION (verified against `typography.rs:193-194` and `style_picker.rs:150-156`):** The 24 x 24 area left of the Style button is `Rect::from_min_size(Pos2::new(style_btn.left() - 24.0, style_btn.top()), Vec2::splat(24.0))` where `style_btn = s.rect_of("text-style-button")`, whose center `(style_btn.left() - 12.0, style_btn.center().y)` also lies inside the former `var_pill` rect (`[style_btn.left() - 28.0 .. style_btn.left() - 4.0]`).
- **Unresolved questions:** None.

Status: DONE

--- END REPORT: plans/reports/agy-1039-1027.md ---
