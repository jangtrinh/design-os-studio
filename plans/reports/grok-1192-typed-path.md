# #1192 — a typed import path opens the file, or says why

Branch `jangtrinh/fix-1192-typed-path` (from `origin/release/v0.6.3` at `f5d61ae2`). Not committed.

## Root cause

`home/dialog.rs` `finish` (`Intent::ImportPath`, pre-fix around line 453) trimmed the prompt, stripped one pair of quotes, and called `app.open_path(PathBuf::from(unquoted))`. `PathBuf::from` keeps `~` and `file://` literal. An absolute path already opened (the red run's `typed_import_path_keeps_absolute` passed). A typed `~/…`, a relative path, or a pasted `file://` URI did not. `open_path` drops the error after `try_open_path` shows a 4 s toast (`app.rs` `show_toast`, `until = now + TOAST_SECONDS` at show time). On a slow first paint that toast was already expired, so QA saw the prompt close and then nothing. The "No file dialog" toast (10 s, also timed from `show_toast`) had the same clock bug (`shell.rs` `fn toast` expired on `now > until` before the first paint).

## What changed

- `crates/designos-app/src/home/dialog.rs` — `typed_import_path` (line 644). Trim, strip one matching quote pair, expand `~` / `~/` / `~\` via `home_dir` (test override, else `crate::io::user_home()`), decode `file://` with `%XX`, refuse a non-absolute path with `Type the full path to the file, starting with / or ~/.`. `finish` (line 454) calls it. On `Err`, `warn_import_path` plus an error toast. On `Ok`, `try_open_path`; its `Err` is the same warn line (the toast is already shown inside `try_open_path`). Stderr: `designos: warn: import path <path>: <reason>`. Tests inject home with `set_import_home_for_test` (thread-local, dropped by `ImportHomeGuard`), not `std::env::set_var`.
- `crates/designos-app/src/io.rs` — `pub(crate) use fallback_save::user_home` (line 11). Existing helper: `HOME`, then `USERPROFILE`, skipping empty.
- `crates/designos-app/src/app.rs` — `Toast { duration, armed }` (lines 149–152) and `stay_for` (line 157). `show_toast` sets `duration: TOAST_SECONDS, armed: false`. `show_toast_with_reveal` uses `stay_for`.
- `crates/designos-app/src/shell.rs` — `fn toast` (line 1014) arms `until = now + duration` on the first paint, and holds the toast while a screenshot is not yet requested.
- `crates/designos-app/src/io/file_dialog.rs` — `show_no_file_dialog_toast` (line 138) uses `stay_for` for the 10 s Enter-a-path toast. No `ScriptedDialogs` or `--panel export-toast` changes.
- `crates/designos-app/src/shell/tests.rs` — `mod toast_clock;` (line 1164).
- `crates/designos-app/src/shell/tests/toast_clock.rs` — new. The two clock tests and their helper.
- `crates/designos-app/src/shell/tests/home.rs` — `typed_import_path_from_home_opens_the_fig` and `typed_import_path_relative_shows_the_message`.

## Commands and output

Harness: `bash /private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-eMjL4g/4a19d7c5-37f7-497b-bf78-6cbf99e83d54/scratchpad/harness-cargo.sh`.

Red, before the resolver and the clock (stub returned the literal path; toast still timed at `show_toast`):

```
test -p designos-app --bin designos --locked typed_import_path
test result: FAILED. 1 passed; 7 failed; 0 ignored; 0 measured; 3338 filtered out; finished in 0.19s
```

The one pass was `typed_import_path_keeps_absolute`. Failures: `~/x.fig` stayed `~/x.fig`; `~` stayed `~`; `'~/a b.fig'` stayed `~/a b.fig`; `file:///tmp/a%20b.fig` stayed the URI; `docs/x.fig` was `Ok` instead of the refuse string; the GUI `~/` open did not enter the editor; the relative GUI toast was `Cannot read docs/x.fig: …` instead of `Type the full path to the file, starting with / or ~/.`.

```
test -p designos-app --bin designos --locked toast_clock
test result: FAILED. 0 passed; 2 failed; finished in 0.29s
```

Both panicked: `the frame after a 30 s block paints the toast, got None`.

Green, after the fix (same binary `out/cargo/debug/deps/designos-f103f8deb2404a79`):

```
test -p designos-app --bin designos --locked typed_import_path
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 3338 filtered out; finished in 0.30s

test -p designos-app --bin designos --locked toast_clock
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 3344 filtered out; finished in 0.28s
```

Required filters, after rustfmt hand-edits (behavior unchanged):

```
fmt --all -- --check
exit 0

clippy -p designos-app --all-targets --locked -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 50.51s
exit 0

test … --locked dialog
test result: FAILED. 74 passed; 13 failed; 0 ignored; 0 measured; 3259 filtered out; finished in 1.84s

test … --locked home
test result: FAILED. 98 passed; 1 failed; 0 ignored; 0 measured; 3247 filtered out; finished in 10.05s

test … --locked toast
test result: ok. 35 passed; 0 failed; 1 ignored; 0 measured; 3310 filtered out; finished in 109.87s

test … --locked import
test result: ok. 60 passed; 0 failed; 0 ignored; 0 measured; 3286 filtered out; finished in 7.21s

python3 scripts/zero_fake_ui_gate.py
Gate Verdict: PASS (17/17 invariants satisfied)

python3 scripts/check_spacing_relationships.py
PASS: no count rose above the baseline
```

Every cargo invocation also prints the existing warning that `src/main.rs` is in both the `designos` and `designos-studio` bins.

The 13 `dialog` failures and the 1 `home` failure are this runner's sandbox, not the import path. Each Gemini failure panics at `terminal/gemini_test_support.rs:126` with `openpty failed: … Operation not permitted`. `shell::tests::agent_panel_keyboard::…esc_for_a_dialog_…` panics the same way (`stub claude starts: openpty failed`). `shell::tests::home::userflows_uf01_to_uf08_end_to_end_figma_parity` asserts `tabs.len() == 3` and got `0` after `use_stub_agent_clis()` (the stub PTYs never started). The new tests inside those filters passed: all six `typed_import_path_*` unit tests, `typed_import_path_from_home_opens_the_fig`, `typed_import_path_relative_shows_the_message`, and both `toast_clock_*` tests.

`git diff --stat` was refused by this runner's command policy, so the file list above is from the edits, not from git.

## Not verified

No live Linux GUI here, and no portal-less / zenity-less desktop session. The stderr line is `eprintln!` in `warn_import_path`; a passing `cargo test` hides it, and `--nocapture` was not in the allowed command list, so that line was not seen in the green output. The relative-path GUI test reaches that function (prompt closes, the exact refuse toast is painted, Home stays open).
