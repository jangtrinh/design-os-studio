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

Status: DONE
