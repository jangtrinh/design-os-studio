# Gated Tests on Windows (`#1158`)

Total tests gated with `#[cfg_attr(windows, ignore = "<reason>")]`: **35** across 6 files.

## 1. `crates/designos-app/src/agent_stop.rs` (23 tests)
All 23 tests spawn POSIX `/bin/sh` (`sh -c ...`) and/or a Python `tty` / `termios` PTY harness (`python3 -u -c 'import os, sys, tty, termios, time ...'`) via `portable_pty`. On Windows, Python's `tty` and `termios` standard-library modules do not exist (`ModuleNotFoundError: No module named 'tty'`) and `sh` is not the native ConPTY shell.

| # | Test (`agent_stop::tests::*`) | File : Line | Reason |
|---|---|---|---|
| 1 | `esc_in_the_chrome_stops` | `crates/designos-app/src/agent_stop.rs:647` | `needs POSIX sh and python tty/termios PTY stub` |
| 2 | `esc_in_the_terminal_reaches_the_cli_unchanged` | `crates/designos-app/src/agent_stop.rs:664` | `needs POSIX sh and python tty/termios PTY stub` |
| 3 | `stop_all_stops_every_running_tab` | `crates/designos-app/src/agent_stop.rs:692` | `needs POSIX sh and python tty/termios PTY stub` |
| 4 | `stop_ends_a_looping_stub_within_three_seconds` | `crates/designos-app/src/agent_stop.rs:762` | `needs POSIX sh and python tty/termios PTY stub` |
| 5 | `second_key_is_sent_only_if_still_running` | `crates/designos-app/src/agent_stop.rs:802` | `needs POSIX sh and python tty/termios PTY stub` |
| 6 | `no_key_is_sent_after_the_turn_ended` | `crates/designos-app/src/agent_stop.rs:823` | `needs POSIX sh and python tty/termios PTY stub` |
| 7 | `in_progress_flags_clear_on_stop` | `crates/designos-app/src/agent_stop.rs:845` | `needs POSIX sh and python tty/termios PTY stub` |
| 8 | `receipt_says_stopped_by_you` | `crates/designos-app/src/agent_stop.rs:869` | `needs POSIX sh and python tty/termios PTY stub` |
| 9 | `stopped_turn_closes_its_ledger_turn_and_revert_after_a_user_undo_still_covers_every_step` | `crates/designos-app/src/agent_stop.rs:942` | `needs POSIX sh and python tty/termios PTY stub` |
| 10 | `stop_after_user_shapes_and_earlier_turn_only_folds_active_turn_and_resets_receipt_clock` | `crates/designos-app/src/agent_stop.rs:977` | `needs POSIX sh and python tty/termios PTY stub` |
| 11 | `folding_a_stopped_turn_remaps_the_ledger_of_every_tab` | `crates/designos-app/src/agent_stop.rs:1029` | `needs POSIX sh and python tty/termios PTY stub` |
| 12 | `stopping_one_tab_keeps_the_in_progress_flags_of_a_parallel_tab_of_the_same_agent` | `crates/designos-app/src/agent_stop.rs:1084` | `needs POSIX sh and python tty/termios PTY stub` |
| 13 | `live_spec_flag_without_run_id_clears_on_stop_of_the_writing_tab` | `crates/designos-app/src/agent_stop.rs:1166` | `needs POSIX sh and python tty/termios PTY stub` |
| 14 | `nested_spec_flags_without_a_run_id_get_the_agent_tab_run_id_and_explicit_ones_keep_theirs` | `crates/designos-app/src/agent_stop.rs:1203` | `needs POSIX sh and python tty/termios PTY stub` |
| 15 | `applying_a_spec_does_not_give_the_tab_run_id_to_another_writers_flag_so_stop_leaves_it` | `crates/designos-app/src/agent_stop.rs:1264` | `needs POSIX sh and python tty/termios PTY stub` |
| 16 | `applying_a_spec_does_not_give_the_tab_run_id_to_another_writers_flag_so_turn_end_leaves_it` | `crates/designos-app/src/agent_stop.rs:1302` | `needs POSIX sh and python tty/termios PTY stub` |
| 17 | `stopped_turn_reads_as_stopped_in_the_receipt_row_and_header_status` | `crates/designos-app/src/agent_stop.rs:1340` | `needs POSIX sh and python tty/termios PTY stub` |
| 18 | `osc_status_in_pty_does_not_erase_stopped_until_new_working_turn` | `crates/designos-app/src/agent_stop.rs:1395` | `needs POSIX sh PTY stub` |
| 19 | `codex_heuristic_stop_folds_all_patches_and_preserves_elapsed_receipt` | `crates/designos-app/src/agent_stop.rs:1430` | `needs POSIX sh and python tty/termios PTY stub` |
| 20 | `stop_closes_when_the_cli_fires_no_interrupt_hook_and_goes_quiet` | `crates/designos-app/src/agent_stop.rs:1464` | `needs POSIX sh and python tty/termios PTY stub` |
| 21 | `a_chatty_cli_stays_pending_across_the_real_quiet_window` | `crates/designos-app/src/agent_stop.rs:1487` | `needs POSIX sh and python tty/termios PTY stub` |
| 22 | `a_new_prompt_closes_the_pending_stop_so_the_next_turn_never_folds_with_it` | `crates/designos-app/src/agent_stop.rs:1508` | `needs POSIX sh and python tty/termios PTY stub` |
| 23 | `a_cli_that_prints_nothing_after_the_key_is_not_closed_by_quiet_and_ends_in_the_still_running_notice` | `crates/designos-app/src/agent_stop.rs:1537` | `needs POSIX sh and python tty/termios PTY stub` |

## 2. `crates/designos-app/src/agent_usage.rs` (2 tests)
Both tests invoke `run_claude_hook_with_payload`, which executes a POSIX `sh -c` + `sed` hook command and formats `transcript.display()` directly into a raw JSON string literal (`format!(r#"{{"session_id":"{session_id}","transcript_path":"{}"}}"#, transcript.display())`), producing invalid JSON escape sequences on Windows backslash paths.

| # | Test (`agent_usage::tests::*`) | File : Line | Reason |
|---|---|---|---|
| 24 | `two_claude_sessions_in_one_session_dir_keep_distinct_transcript_pointers_and_readings` | `crates/designos-app/src/agent_usage.rs:1319` | `runs POSIX sh/sed transcript capture hook with unescaped Unix path in JSON` |
| 25 | `claude_clear_rotates_session_id_and_restarts_tab_usage_reading` | `crates/designos-app/src/agent_usage.rs:1388` | `runs POSIX sh/sed transcript capture hook with unescaped Unix path in JSON` |

## 3. `crates/designos-app/src/io/dialog_probe.rs` (1 test)
Tests Linux freedesktop portal probing (`probe_env`), which splits `PATH` and `XDG_DATA_DIRS` on `':'` (`split(':')`) and checks against `/usr/local/share:/usr/share`. On Windows, `PathBuf::from("/usr/bin")` and `std::env::temp_dir()` (`D:\a\_temp\...`) contain drive-letter colons (`D:`), breaking `split(':')`.

| # | Test (`io::dialog_probe::tests::*`) | File : Line | Reason |
|---|---|---|---|
| 26 | `the_environment_falls_back_to_the_standard_data_dirs` | `crates/designos-app/src/io/dialog_probe.rs:285` | `splits colon-separated Unix PATH and XDG_DATA_DIRS` |

## 4. `crates/designos-app/src/agent_composer.rs` (1 test)
Writes a POSIX `#!/bin/sh` script (`my-agent`) and `chmod 0o755`s it without a Windows executable extension, then spawns it through `portable_pty`.

| # | Test (`agent_composer::tests::*`) | File : Line | Reason |
|---|---|---|---|
| 27 | `a_custom_cli_tab_gets_the_skills_pasted_and_is_not_recorded_as_claude` | `crates/designos-app/src/agent_composer.rs:2061` | `spawns #!/bin/sh custom CLI stub via unquoted POSIX path` |

## 5. `crates/designos-app/src/terminal.rs` (7 tests)
Sorting alphabetically after `shell::tests::leave_prompt` (so previously unreachable on Windows before the stack overflow was fixed), 6 tests call `exited_stub_state()`, which spawns `sh -c 'echo STUB-AGENT-EXIT; exit 1'` and spins on `reader_eof` (POSIX PTY master EOF when the child closes the slave fd; Windows ConPTY `conhost.exe` keeps the output pipe open while `MasterPty` lives), and 1 test writes a POSIX `.sh` script reading `/dev/zero` (`head -c 100000 /dev/zero | tr '\000' p`) and invokes `sh '<path>'` with single quotes.

| # | Test (`terminal::tests::*`) | File : Line | Reason |
|---|---|---|---|
| 28 | `agent_that_exits_within_the_window_reports_a_start_failure_with_its_last_output` | `crates/designos-app/src/terminal.rs:7805` | `uses exited_stub_state which spawns POSIX sh and waits on POSIX PTY master reader_eof` |
| 29 | `agent_that_exits_after_the_window_is_not_a_start_failure` | `crates/designos-app/src/terminal.rs:7827` | `uses exited_stub_state which spawns POSIX sh and waits on POSIX PTY master reader_eof` |
| 30 | `clicking_the_fix_on_the_failed_tab_panel_opens_a_shell_tab` | `crates/designos-app/src/terminal.rs:7861` | `uses exited_stub_state which spawns POSIX sh and waits on POSIX PTY master reader_eof` |
| 31 | `non_zero_exit_after_start_window_records_session_failure_and_zero_exit_does_not` | `crates/designos-app/src/terminal.rs:8658` | `uses exited_stub_state which spawns POSIX sh and waits on POSIX PTY master reader_eof` |
| 32 | `a_stale_turn_offset_inside_a_multibyte_char_does_not_panic_the_exit_scan` | `crates/designos-app/src/terminal.rs:11887` | `uses exited_stub_state which spawns POSIX sh and waits on POSIX PTY master reader_eof` |
| 33 | `a_log_trim_inside_an_open_turn_keeps_the_scan_on_that_turns_output` | `crates/designos-app/src/terminal.rs:11903` | `runs POSIX sh script reading /dev/zero via single-quoted path` |
| 34 | `retry_does_nothing_on_an_exited_tab` | `crates/designos-app/src/terminal.rs:11946` | `uses exited_stub_state which spawns POSIX sh and waits on POSIX PTY master reader_eof` |

## 6. `crates/designos-app/src/io.rs` (1 test)
Companion to `io::tests::pen_clone_budget_warning_reaches_the_caller_and_the_toast_text` (`crates/designos-app/src/io.rs:501`, already marked `#[ignore = "allocates the full 2M-node clone budget; run with --ignored"]`): `opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast` constructs an 8,000,000-node `Document` and runs `App::open_path` in unoptimized debug mode (~14 GB RAM), thrashing the Windows CI runner pagefile for >60 minutes.

| # | Test (`io::tests::*`) | File : Line | Reason |
|---|---|---|---|
| 35 | `opening_file_that_hits_clone_budget_surfaces_unexpanded_instances_toast` | `crates/designos-app/src/io.rs:1201` | `allocates the full 8M-node clone budget (~14 GB RAM in debug mode); run with --ignored` |
