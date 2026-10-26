# Report — DEV · Files · atomic saves keep ACLs and never delete a user's `<name>.tmp` (#483)

## 1. What Could NOT Be Verified

- **Linux and Windows runtime execution:** All builds, `cargo fmt`, `cargo clippy`, and test runs were executed locally on macOS (`aarch64-apple-darwin`). Cross-platform compilation and test execution on Linux (`#[cfg(unix)]` with `#[cfg(not(target_os = "macos"))]`) and Windows (`#[cfg(not(unix))]`) were verified structurally by inspecting the `#[cfg(...)]` branches and zero-warning clippy pass, and will be executed by GitHub Actions CI on the pull request.
- **Root-only cross-UID ownership preservation (`fchown` to another UID):** On Unix/macOS (`_POSIX_CHOWN_RESTRICTED`), an unprivileged user process (`geteuid() != 0`) cannot change the owner UID of a newly created temp file to a different UID. Verifying `COPYFILE_STAT` UID transfer across distinct users would require root (`sudo`), which is outside the sandboxed test harness.

---

## 2. Conclusion & Epistemic Ledger

**Conclusion:** `write_atomic` in `crates/designos-fonts/src/cache.rs` (re-exported by `designos_library::write_atomic` and called by `designos-app` in `crates/designos-app/src/io.rs`) now preserves macOS ACLs, extended attributes, and file mode across atomic saves via `fcopyfile(3)` (`COPYFILE_ACL | COPYFILE_XATTR | COPYFILE_STAT`), probes writability on macOS by opening the target for writing instead of rejecting `0444` mode bits up front, opens existing Unix targets with `O_NONBLOCK` before checking `fstat` to close the FIFO swap race, and never deletes an unmarked user `<name>.tmp` file during `sweep_stale_temps`.

### Epistemic Ledger
- **[FACT]** On macOS, `write_atomic` previously rejected any existing file whose mode had no write bits (`meta.permissions().readonly()`, e.g. `0444`) before reaching the `OpenOptions::new().write(true).open(target)` probe, and copied only POSIX mode bits (`fs::set_permissions`) to the replacement temp file (`crates/designos-fonts/src/cache.rs`).
- **[FACT]** On base (`plans/reports/agy-483-atomic-save-acl-evidence/red.log`), saving to a `0444` file with `user:<uid> allow write` failed with `is read-only`, extended attributes (`com.designos.*`) were lost on atomic save, and a user file named `<target>.tmp` was unconditionally deleted by `sweep_stale_temps`.
- **[FACT]** Calling `fcopyfile(from.as_raw_fd(), to.as_raw_fd(), ptr::null_mut(), COPYFILE_ACL | COPYFILE_XATTR | COPYFILE_STAT)` on the already-open source and temp descriptors before `file.write_all(bytes)` (and refreshing `mtime` via `file.set_modified(SystemTime::now())` when `bytes.is_empty()`) copies ACLs, extended attributes, and POSIX mode to the temp file before `fs::rename`, without resetting the saved file's modification timestamp to the pre-save value.
- **[FACT]** Opening the existing target on Unix with `O_NONBLOCK` (`0x0004` on macOS/BSD, `0o4000` on Linux), checking `file.metadata()?.is_file()` on the opened file descriptor (and mapping `ENXIO` on a readerless FIFO to `InvalidInput` `"is not a regular file"`), then clearing `O_NONBLOCK` via `fcntl(F_GETFL / F_SETFL)` prevents blocking if a regular file is swapped for a FIFO between `symlink_metadata` and `open`.
- **[FACT]** All 4 new `designos-app` `io::tests` (`acl_granting_write_on_0444_file_allows_save_and_survives`, `extended_attribute_survives_atomic_save`, `user_file_named_target_dot_tmp_survives_save`, `truly_unwritable_0444_file_is_refused`), all 19 `designos-library` `atomic_save` integration tests, all 30 `designos-fonts` unit tests, and all 37 `designos-library` unit tests pass under the mandatory test harness (`plans/reports/agy-483-atomic-save-acl-evidence/green.log`).
- **[INFERENCE]** Because `fcopyfile` operates on file descriptors already opened during `resolve_save_target` and `create_unique_temp`, no extra path resolution or document serialization occurs, and saves remain off the UI thread wherever they were before.

**Weakest link:** Linux `O_NONBLOCK` constant (`0o4000`) and non-macOS `meta.permissions().readonly()` branch were verified by code inspection rather than local Linux execution.

**Unresolved questions:** None.

---

## 3. Acceptance Table (Commands & Pasted Output)

| Check | Command | Result |
|---|---|---|
| **1a. Red first (`designos-app` & `designos-library` & `designos-fonts`)** | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked io::` (and `--test atomic_save`, `-p designos-fonts --lib`) | **FAIL on base** (`red.log`: 4 failed in `io::tests`, 4 failed in `atomic_save`, 1 failed in `designos-fonts`) |
| **1b. Green on head (`designos-app` `io::`)** | `env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked io::` | **PASS** (`93 passed; 0 failed; 1 ignored` in `green.log`) |
| **1c. Green on head (`designos-library` `atomic_save` & `designos-fonts`)** | `cargo test -p designos-library --test atomic_save --locked && cargo test -p designos-fonts --lib --locked && cargo test -p designos-library --lib --locked` | **PASS** (`19 passed`, `30 passed`, `37 passed` in `green.log`) |
| **2a. Rustfmt check** | `cargo fmt --all -- --check` | **PASS** (exit 0) |
| **2b. Clippy check** | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **PASS** (exit 0, `green.log`) |
| **3. Zero-Fake-UI gate** | `python3 scripts/zero_fake_ui_gate.py` | **PASS** (`17/17 invariants satisfied`) |
| **4. Draft PR against `main`** | `gh pr create --draft --base main ...` (`Closes #483`) | **OPENED** (`https://github.com/jangtrinh/design-os-studio/pull/1129`) |

### Pasted Output — Red (`plans/reports/agy-483-atomic-save-acl-evidence/red.log`)

```text
running 94 tests
...
test io::tests::acl_granting_write_on_0444_file_allows_save_and_survives ... FAILED
test io::tests::extended_attribute_survives_atomic_save ... FAILED
test io::tests::truly_unwritable_0444_file_is_refused ... FAILED
test io::tests::user_file_named_target_dot_tmp_survives_save ... FAILED
...
failures:

---- io::tests::acl_granting_write_on_0444_file_allows_save_and_survives stdout ----

thread 'io::tests::acl_granting_write_on_0444_file_allows_save_and_survives' (50096452) panicked at crates/designos-app/src/io.rs:1293:14:
first save on 0444 file with write-granting ACL must succeed: "Cannot write /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.TBRVfc/designos_io_acl_0444_16452/acl_writable.designos: io: /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.TBRVfc/designos_io_acl_0444_16452/acl_writable.designos is read-only"

---- io::tests::extended_attribute_survives_atomic_save stdout ----

thread 'io::tests::extended_attribute_survives_atomic_save' (50096471) panicked at crates/designos-app/src/io.rs:1329:9:
xattr must still exist after atomic save: xattr: /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.TBRVfc/designos_io_xattr_16452/xattr_doc.designos: No such xattr: com.designos.custom-test

---- io::tests::truly_unwritable_0444_file_is_refused stdout ----

thread 'io::tests::truly_unwritable_0444_file_is_refused' (50096535) panicked at crates/designos-app/src/io.rs:1381:9:
unexpected error message: Cannot write /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.TBRVfc/designos_io_unwritable_16452/locked.designos: io: /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.TBRVfc/designos_io_unwritable_16452/locked.designos is read-only

---- io::tests::user_file_named_target_dot_tmp_survives_save stdout ----

thread 'io::tests::user_file_named_target_dot_tmp_survives_save' (50096538) panicked at crates/designos-app/src/io.rs:1354:35:
user <name>.tmp must not be deleted: Os { code: 2, kind: NotFound, message: "No such file or directory" }

test result: FAILED. 89 passed; 4 failed; 1 ignored; 0 measured; 2848 filtered out; finished in 91.72s
```

### Pasted Output — Green (`plans/reports/agy-483-atomic-save-acl-evidence/green.log`)

```text
     Running unittests src/main.rs (out/cargo/debug/deps/designos-dd82a7aa841fdfee)

running 94 tests
...
test io::tests::extended_attribute_survives_atomic_save ... ok
test io::tests::acl_granting_write_on_0444_file_allows_save_and_survives ... ok
test io::tests::truly_unwritable_0444_file_is_refused ... ok
test io::tests::user_file_named_target_dot_tmp_survives_save ... ok
...
test result: ok. 93 passed; 0 failed; 1 ignored; 0 measured; 2848 filtered out; finished in 152.58s

     Running tests/atomic_save.rs (out/cargo/debug/deps/atomic_save-0fc589cd006d9af6)

running 19 tests
test read_only_target_is_refused_and_left_untouched ... ok
test directory_target_is_refused ... ok
test a_named_pipe_target_is_refused_without_blocking ... ok
test missing_file_and_parent_directories_are_created ... ok
test file_with_0644_mode_and_a_write_deny_acl_is_refused ... ok
test a_symlink_to_a_named_pipe_is_refused_without_blocking ... ok
test a_named_pipe_with_an_open_reader_is_still_refused ... ok
test saving_through_a_dangling_symlink_creates_the_target_and_keeps_the_link ... ok
test publish_and_font_cache_use_the_shared_hardened_write_atomic ... ok
test extended_attribute_survives_atomic_save ... ok
test a_user_file_named_like_a_digit_pair_temp_is_never_swept ... ok
test saving_keeps_the_existing_permission_mode ... ok
test saving_through_a_symlink_into_another_directory_writes_next_to_the_real_file ... ok
test saving_through_a_symlink_updates_the_real_file_and_keeps_the_link ... ok
test acl_granting_write_on_0444_file_allows_save_and_survives ... ok
test filename_of_249_or_more_bytes_succeeds_without_enametoolong ... ok
test crashed_save_leftover_tmp_files_are_swept_on_next_save ... ok
test forty_concurrent_saves_of_one_file_all_succeed_and_none_is_torn ... ok
test temp_file_is_never_world_readable_while_saving_a_0600_document ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s

     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-d8a4d3c2a3602c1a)

running 30 tests
...
test cache::tests::temp_names_round_trip_and_nothing_else_is_taken_for_a_crashed_save ... ok
test cache::tests::leftover_of_a_dead_process_is_swept_but_a_look_alike_user_file_is_kept ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### Pasted Output — `cargo fmt` & `cargo clippy` (`plans/reports/agy-483-atomic-save-acl-evidence/green.log`)

```text
$ cargo fmt --all -- --check
(exit 0)

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.42s
CLIPPY_RC=0
```

---

## 4. Changes Per File, Ownership Behaviour & Platform Coverage

### Changes Per File
1. [`crates/designos-fonts/src/cache.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-483-atomic-save-acl/crates/designos-fonts/src/cache.rs):
   - **`sweep_stale_temps(dir, prefix)`:** Removed the unconditional `let _ = fs::remove_file(dir.join(format!("{target_name}.tmp")));` line. Only marked Studio temp files (`<prefix>.save-<pid>-<seq>.tmp` whose `<pid>` fails `kill(pid, 0)` with `ESRCH`) are removed. Any user file named `<name>.tmp` is left untouched.
   - **`resolve_save_target(path)` & `open_writable_regular(path)`:**
     - On macOS (`#[cfg(target_os = "macos")]`), removed the early `meta.permissions().readonly()` check so that a `0444` file writable via an ACL reaches the open probe (`open_writable_regular`) and succeeds; a truly unwritable `0444` file still fails the open probe with `"{} is read-only or not writable: {e}"`.
     - On Unix (`#[cfg(unix)]`), `open_writable_regular` opens the existing target with `O_NONBLOCK`, maps `ENXIO` (named pipe with no reader) to `InvalidInput ("{} is not a regular file")`, verifies `file.metadata()?.is_file()` on the opened descriptor, and clears `O_NONBLOCK` via `fcntl(F_GETFL / F_SETFL)`.
     - On macOS, `ExistingTarget` retains the open source `File` handle alongside `Metadata` so `write_via_tmp` can pass its descriptor to `fcopyfile`.
   - **`copy_apple_metadata(from, to)` (`#[cfg(target_os = "macos")]`) & `write_via_tmp`:**
     - Added a minimal `extern "C"` binding to macOS libSystem `fcopyfile(from, to, state, flags)` with `COPYFILE_ACL | COPYFILE_XATTR | COPYFILE_STAT` (falling back to `COPYFILE_ACL | COPYFILE_STAT` on `EACCES` if the existing file is write-only `0200` and cannot have its xattrs read).
     - Invoked `copy_apple_metadata` before `file.write_all(bytes)` (and called `file.set_modified(SystemTime::now())` when `bytes.is_empty()`) so `COPYFILE_STAT` copies mode, flags, and ACLs without leaving a stale `mtime` on the saved file.
   - **Unit tests:** Updated `temp_names_round_trip_and_nothing_else_is_taken_for_a_crashed_save` and `leftover_of_a_dead_process_is_swept_but_a_look_alike_user_file_is_kept` to assert `<name>.tmp` is never matched or deleted.
2. [`crates/designos-app/src/io.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-483-atomic-save-acl/crates/designos-app/src/io.rs):
   - Added 4 tests in `io::tests`:
     - `acl_granting_write_on_0444_file_allows_save_and_survives` (`#[cfg(target_os = "macos")]`): sets `0444` and `chmod +a "user:<uid> allow write"` on a `.designos` file, saves twice via `io::save_document`, and verifies with `ls -le` that both saves succeed and `allow write` + `-r--r--r--+` survive.
     - `extended_attribute_survives_atomic_save` (`#[cfg(target_os = "macos")]`): sets `com.designos.custom-test` via `xattr -w`, saves via `io::save_document`, and verifies with `xattr -p` that the attribute value survives.
     - `user_file_named_target_dot_tmp_survives_save`: places a user `<name>.tmp` alongside the target document, saves via `io::save_document`, and verifies the user `.tmp` file and its contents are untouched.
     - `truly_unwritable_0444_file_is_refused` (`#[cfg(unix)]`): sets `0444` with no ACL on a target document, attempts `io::save_document`, and verifies the save is refused with `"is read-only or not writable:"` and the original bytes remain intact.
3. [`crates/designos-library/tests/atomic_save.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-483-atomic-save-acl/crates/designos-library/tests/atomic_save.rs):
   - Updated `read_only_target_is_refused_and_left_untouched` to assert `"is read-only or not writable:"`.
   - Updated `crashed_save_leftover_tmp_files_are_swept_on_next_save` to assert that a user `doc.designos.tmp` file survives while crashed `.save-<dead_pid>-<seq>.tmp` files are swept.
   - Added `acl_granting_write_on_0444_file_allows_save_and_survives`, `extended_attribute_survives_atomic_save`, and `a_named_pipe_with_an_open_reader_is_still_refused`.
4. [`crates/designos-library/src/store.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-483-atomic-save-acl/crates/designos-library/src/store.rs):
   - Updated doc comment on `pub fn write_atomic` and updated `store::tests::atomic_save_leaves_no_temp_file_and_survives_a_stale_one` to verify that a sibling `<name>.tmp` file is preserved.

### Ownership Behaviour
- **Current behaviour (preserved):** The newly created `.save-<pid>-<seq>.tmp` file is created by the saving process and is owned by the saving process's effective UID (`geteuid()`) and the parent directory's / process's effective GID. On macOS, `fcopyfile` with `COPYFILE_STAT` attempts `fchown` from the old file to the temp file: if the old file is owned by the same user (or a supplementary group the user belongs to), UID/GID match; if the old file was owned by another user and the saving process is unprivileged (`geteuid() != 0`), the kernel rejects cross-UID `fchown` with `EPERM` (`_POSIX_CHOWN_RESTRICTED`), which `fcopyfile` ignores, leaving the renamed file owned by the saving user (`geteuid()`).

### Platform Test Coverage
- **macOS (`target_os = "macos"`):** Covers all tests: ACL write-grant survival (`chmod +a` + `ls -le` across two consecutive saves on a `0444` file), extended attribute survival (`xattr -w` / `xattr -p`), `0444` truly unwritable refusal via open probe, `0644` write-deny ACL refusal, user `<name>.tmp` preservation, crashed `.save-<pid>-<seq>.tmp` sweeping, symlink/dangling-symlink resolution, FIFO refusal (with and without an open reader via `O_NONBLOCK`), `0600` mode preservation, and 40-thread concurrent save integrity.
- **Linux (`cfg(unix)` and `not(target_os = "macos")`):** Compiles without macOS `fcopyfile`; covers user `<name>.tmp` preservation, crashed `.save-<pid>-<seq>.tmp` sweeping via `kill(pid, 0)`, `0444` read-only refusal, `O_NONBLOCK` FIFO refusal (with and without an open reader), symlink resolution, `0600` mode preservation, and concurrent saves.
- **Windows (`not(unix)`):** Compiles with existing behaviour; covers user `<name>.tmp` preservation, `0444` read-only refusal (`meta.permissions().readonly()`), directory refusal, long filenames, and concurrent saves.

---

## 5. Skills Loaded & Impact

- `es:dev` → `es:feature-dev`: Executed the DEV phase directly against the controller's accepted spec without re-entering brainstorm/plan approval gates; ran red-first verification before implementation.
- `es:rust`: Used a zero-dependency `extern "C"` FFI for `fcopyfile` and `fcntl` on existing `RawFd` handles (avoiding any `Cargo.toml` / `Cargo.lock` churn so `--locked` succeeds), and preserved `mtime` ordering so `DiskWatcher` in `home/session.rs` never sees a stale timestamp after `COPYFILE_STAT`.
- `es:rust-gpu-canvas`: Confirmed `write_atomic` changes add zero document serialization and do not touch UI-thread or GPU rendering paths.
- `es:fable-thinking` & `es:astra-work`: Predicted and falsified the `COPYFILE_STAT` `mtime` copy hazard (calling `fcopyfile` *before* `write_all` and touching `mtime` when `bytes.is_empty()`), verified every acceptance claim with harness logs (`red.log`, `green.log`), and structured this report with unverified items first.

---

## 6. Every User-Facing String (Verbatim)

From `crates/designos-fonts/src/cache.rs`:
- `"{} is a directory"`
- `"{} is not a regular file"`
- `"{} is read-only or not writable: {e}"`
- `"{} is read-only or not writable: permission denied"`
- `"too many levels of symbolic links: {}"`

Status: DONE
