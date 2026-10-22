//! Integration and contract tests for `designos-eval` (E15-25 §10). Runs go through a real `designos-mcp` process.

mod support;

use designos_eval::jsonl::{validate_jsonl_line, EvalMode, EvalRecord};
use designos_eval::rng::seed_for_metric;
use designos_eval::runner::{check_or_write_split_lock, run_harness};
use designos_eval::split::build_split_lock;
use designos_eval::stats::bootstrap_ci;
use designos_eval::task::TasksFile;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use support::*;

const STUB_COMMAND: &str = r#"["stub-agent", "--script", "{workspace}/../create_frame.jsonl", "--mcp-config", "{mcp_config}"]"#;

/// Three tasks (two dev, one held out) whose stub script creates and saves a "Hero Frame".
fn write_stub_suite(root: &Path, suite: &str) -> (std::path::PathBuf, String) {
    write_register(root);
    let (dev_ids, held_out_id) = split_ids(suite);
    let tasks = [dev_ids[0].clone(), dev_ids[1].clone(), held_out_id.clone()]
        .into_iter()
        .map(|id| {
            let mut t = task(&id, suite);
            t.stub_script = Some("create_frame.jsonl".to_string());
            t
        })
        .collect();
    let suites_root = write_suite(root, suite, tasks, &[("create_frame.jsonl", script(&[create_hero_frame_call(), save_output_call()]))]);
    write_profile(root, "stub", STUB_COMMAND);
    (suites_root, held_out_id)
}

#[test]
fn stub_run_writes_valid_jsonl() {
    let root = temp_test_dir("stub-jsonl");
    let (suites_root, _) = write_stub_suite(&root, "s2");
    let opts = run_options(&root, suites_root, "s2", "stub", 2);
    let failed = run_harness(&opts).expect("stub release run succeeds");
    assert!(!failed, "all stub tasks should exit Ok");

    let raw = fs::read_to_string(root.join("results/v0.6.0/s2.jsonl")).expect("results JSONL written");
    let (mut meta_count, mut run_count, mut task_count, mut suite_count) = (0usize, 0usize, 0usize, 0usize);
    for line in raw.lines() {
        match validate_jsonl_line(line).expect("every JSONL line validates against results-v1.json") {
            EvalRecord::Meta(m) => {
                assert_eq!(m.schema, 1);
                assert_eq!(m.runs, 2);
                assert!(
                    m.started != "2026-10-19T00:00:00Z" && m.started.ends_with('Z') && m.started.len() == 20,
                    "started must be the run time: {}",
                    m.started
                );
                meta_count += 1;
            }
            EvalRecord::Run(r) => {
                assert_eq!(r.metrics.get("render_success").copied().flatten(), Some(1.0));
                assert!(!r.artefacts.renders.is_empty());
                assert!(r.tool_calls >= 2, "the real server logged the stub's calls: {}", r.tool_calls);
                run_count += 1;
            }
            EvalRecord::Task(t) => {
                assert_eq!(t.runs, 2);
                task_count += 1;
            }
            EvalRecord::Suite(s) => {
                assert_eq!(s.n_tasks, 3);
                assert_eq!(s.ci_method, "bootstrap-two-stage");
                assert_eq!(s.b, 10_000);
                suite_count += 1;
            }
        }
    }
    assert_eq!((meta_count, run_count, task_count), (1, 6, 3));
    assert!(suite_count >= 1);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn held_out_refused_in_tuning() {
    let root = temp_test_dir("held-out-refused");
    let (suites_root, held_out_id) = write_stub_suite(&root, "s2");
    let mut opts = run_options(&root, suites_root, "s2", "stub", 1);
    opts.mode = EvalMode::Tuning;
    opts.task_filter = vec![held_out_id.clone()];

    let err = run_harness(&opts).expect_err("held-out task in tuning mode must be refused");
    assert_eq!(err.code, 3);
    assert!(err.message.starts_with("held_out_refused:"), "unexpected message: {}", err.message);
    assert!(err.message.contains(&held_out_id));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn split_fraction_within_bounds() {
    let root = temp_test_dir("split-bounds");
    let suite_dir = root.join("s2");
    fs::create_dir_all(&suite_dir).unwrap();
    let tasks: Vec<_> = (1..=100)
        .map(|i| {
            let mut t = task(&format!("s2-{i:04}-shadcn"), "s2");
            t.brief = format!("Synthetic task {i}");
            t.output = Vec::new();
            t.checklist = Vec::new();
            t
        })
        .collect();
    let lock = build_split_lock("s2", SALT, &tasks, &suite_dir, true).expect("100 synthetic tasks lock within 0.30 ± 0.05 bounds");
    let held_out_count = lock.tasks.iter().filter(|e| e.split == designos_eval::split::Split::HeldOut).count();
    assert!((25..=35).contains(&held_out_count), "expected held_out_count in 25..=35, got {held_out_count}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn split_lock_mismatch_refused() {
    let root = temp_test_dir("lock-mismatch");
    let (suites_root, held_out_id) = write_stub_suite(&root, "s2");
    let tasks_path = suites_root.join("s2").join("tasks.toml");
    let mut tf: TasksFile = toml::from_str(&fs::read_to_string(&tasks_path).unwrap()).unwrap();
    for t in &mut tf.tasks {
        if t.id == held_out_id {
            t.brief = "Tampered held-out brief text!".to_string();
        }
    }
    fs::write(&tasks_path, toml::to_string_pretty(&tf).unwrap()).unwrap();

    let err = run_harness(&run_options(&root, suites_root, "s2", "stub", 1)).expect_err("tampered held-out task must fail lock check");
    assert_eq!(err.code, 3);
    assert!(err.message.starts_with("split_lock_mismatch:"), "unexpected message: {}", err.message);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn bootstrap_is_reproducible() {
    let per_task: Vec<Vec<f64>> = (0..100)
        .map(|i| {
            let base = (i as f64) / 100.0;
            vec![base, (base + 0.03).min(1.0), (base * 0.95).max(0.0)]
        })
        .collect();
    let seed = seed_for_metric("v0.6.0", "s2", "token_coverage");
    // Judge the time budget on the best of up to five runs so a loaded CI runner does not flake it.
    let mut best_ms = u128::MAX;
    let mut runs = 0;
    let mut first = None;
    while runs < 5 {
        let t0 = std::time::Instant::now();
        let out = bootstrap_ci(&per_task, 10_000, seed);
        best_ms = best_ms.min(t0.elapsed().as_millis());
        runs += 1;
        first.get_or_insert(out);
        if best_ms <= 200 {
            break;
        }
    }
    let (m1, lo1, hi1) = first.unwrap();
    let (m2, lo2, hi2) = bootstrap_ci(&per_task, 10_000, seed);
    assert_eq!(m1.to_bits(), m2.to_bits());
    assert_eq!(lo1.to_bits(), lo2.to_bits());
    assert_eq!(hi1.to_bits(), hi2.to_bits());
    assert!(lo1 <= m1 && m1 <= hi1);
    assert!(best_ms <= 200, "10,000 resamples over 100x3 runs took {best_ms}ms best of {runs} runs (budget <= 200ms)");
}

#[test]
fn bootstrap_known_case() {
    let per_task = vec![vec![1.0, 1.0, 1.0]; 4];
    let (m, lo, hi) = bootstrap_ci(&per_task, 10_000, 42);
    assert_eq!((m, lo, hi), (1.0, 1.0, 1.0));
}

#[test]
fn unapproved_source_refused() {
    let root = temp_test_dir("unapproved-source");
    let (suites_root, _) = write_stub_suite(&root, "s2");
    let tasks_path = suites_root.join("s2").join("tasks.toml");
    let mut tf: TasksFile = toml::from_str(&fs::read_to_string(&tasks_path).unwrap()).unwrap();
    tf.tasks[0].source = "pending-source".to_string();
    fs::write(&tasks_path, toml::to_string_pretty(&tf).unwrap()).unwrap();
    check_or_write_split_lock(&suites_root, "s2", SALT, true, false).unwrap();

    let err = run_harness(&run_options(&root, suites_root, "s2", "stub", 1)).expect_err("task with pending source must be refused");
    assert_eq!(err.code, 3);
    assert!(err.message.starts_with("source_not_approved:"), "unexpected message: {}", err.message);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_register_refuses_the_run() {
    let root = temp_test_dir("no-register");
    let (suites_root, _) = write_stub_suite(&root, "s2");
    fs::remove_file(root.join("docs/eval-licences.md")).unwrap();

    let err = run_harness(&run_options(&root, suites_root.clone(), "s2", "stub", 1)).expect_err("no register, no run: there is no built-in approved list");
    assert!(err.message.starts_with("register_missing:"), "unexpected message: {}", err.message);

    let mut no_root = run_options(&root, suites_root, "s2", "stub", 1);
    no_root.repo_root = None;
    let err = run_harness(&no_root).expect_err("without a repository root the register cannot be read");
    assert!(err.message.starts_with("register_missing:"), "unexpected message: {}", err.message);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn noop_agent_is_not_a_render_success() {
    let root = temp_test_dir("noop-agent");
    write_register(&root);
    let mut declared = task("s2-noop-declared", "s2");
    declared.start = "start.designos".to_string();
    declared.stub_script = Some("noop.jsonl".to_string());
    let mut undeclared = task("s2-noop-undeclared", "s2");
    undeclared.start = "start.designos".to_string();
    undeclared.output = Vec::new();
    undeclared.stub_script = Some("noop.jsonl".to_string());
    let noop = script(&[json!({"tool": "designos_open_file", "args": {"path": "start.designos"}}), json!({"tool": "designos_get_document", "args": {}})]);
    let suites_root = write_suite(&root, "s2", vec![declared, undeclared], &[("noop.jsonl", noop)]);
    write_start_document(&suites_root.join("s2").join("start.designos"));
    write_profile(&root, "stub", STUB_COMMAND);
    // The lock was written before the start document existed; refresh it so the inputs hash.
    check_or_write_split_lock(&suites_root, "s2", SALT, true, false).unwrap();

    run_harness(&run_options(&root, suites_root, "s2", "stub", 1)).expect("the run itself succeeds and is recorded");
    let runs = read_run_records(&root, "s2");
    assert_eq!(runs.len(), 2);
    for run in &runs {
        assert_eq!(run["exit"], "ok", "{run}");
        assert_eq!(run["metrics"]["render_success"], json!(0.0), "an agent that only opens and reads the start document did nothing: {run}");
    }
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn chatty_agent_is_not_recorded_as_a_timeout() {
    let root = temp_test_dir("chatty-agent");
    write_register(&root);
    let mut t = task("s2-chatty", "s2");
    t.budget.timeout_s = 5;
    let suites_root = write_suite(&root, "s2", vec![t], &[]);
    write_profile(&root, "chatty", r#"["sh", "-c", "yes xxxxxxxxxxxxxxxx | head -c 3000000"]"#);

    run_harness(&run_options(&root, suites_root, "s2", "chatty", 1)).unwrap();
    let runs = read_run_records(&root, "s2");
    assert_eq!(runs[0]["exit"], "ok", "3 MB on stdout must be drained while the agent runs: {}", runs[0]);
    assert!(runs[0]["wall_ms"].as_u64().unwrap() < 4_000, "{}", runs[0]);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn timed_out_agent_gets_sigterm_before_sigkill() {
    let root = temp_test_dir("sigterm-agent");
    write_register(&root);
    let mut t = task("s2-sigterm", "s2");
    t.budget.timeout_s = 1;
    let suites_root = write_suite(&root, "s2", vec![t], &[]);
    write_profile(&root, "slow", r#"["sh", "-c", "trap 'touch \"$1/got-term\"; exit 0' TERM; while true; do sleep 0.2; done", "sh", "{output_dir}"]"#);

    run_harness(&run_options(&root, suites_root, "s2", "slow", 1)).unwrap();
    let runs = read_run_records(&root, "s2");
    assert_eq!(runs[0]["exit"], "timeout", "{}", runs[0]);
    assert!(root.join("results/v0.6.0/s2-sigterm/1/got-term").is_file(), "the agent must be told to stop (SIGTERM) so it can flush its work");
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn unsaved_session_is_rebuilt_from_the_transcript() {
    let root = temp_test_dir("unsaved-session");
    write_register(&root);
    let mut t = task("s2-unsaved", "s2");
    t.budget.timeout_s = 30;
    let suites_root = write_suite(&root, "s2", vec![t], &[]);
    // The "agent" edits through the transcript only and never saves output.designos.
    let apply = create_hero_frame_call();
    let line = json!({"seq": 1, "t_ms": 1, "tool": apply["tool"], "args": apply["args"], "result": {}, "is_error": false, "undo_depth": 1});
    let recorded = root.join("recorded.jsonl");
    fs::write(&recorded, format!("{}\n", serde_json::to_string(&line).unwrap())).unwrap();
    write_profile(&root, "silent", &format!(r#"["sh", "-c", "cat \"$1\" >> \"$DESIGNOS_MCP_TRANSCRIPT\"", "sh", "{}"]"#, recorded.display()));

    run_harness(&run_options(&root, suites_root, "s2", "silent", 1)).unwrap();
    let runs = read_run_records(&root, "s2");
    assert_eq!(runs[0]["exit"], "ok", "{}", runs[0]);
    assert_eq!(
        runs[0]["metrics"]["render_success"],
        json!(1.0),
        "the unsaved session must be saved through the server, not replaced by a blank page: {}",
        runs[0]
    );
    let saved = fs::read_to_string(root.join("results/v0.6.0/s2-unsaved/1/output.designos")).unwrap();
    assert!(saved.contains("Hero Frame"), "output.designos holds the agent's frame");
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn jobs_run_tasks_side_by_side_and_keep_record_order() {
    let root = temp_test_dir("jobs");
    write_register(&root);
    let ids: Vec<String> = (1..=4).map(|i| format!("s2-job-{i}")).collect();
    let tasks = ids.iter().map(|id| task(id, "s2")).collect();
    let suites_root = write_suite(&root, "s2", tasks, &[]);
    write_profile(&root, "sleepy", r#"["sh", "-c", "sleep 1"]"#);
    let mut opts = run_options(&root, suites_root, "s2", "sleepy", 1);
    opts.jobs = 4;

    let t0 = std::time::Instant::now();
    run_harness(&opts).unwrap();
    let elapsed = t0.elapsed();
    assert!(elapsed.as_millis() < 3_200, "four 1 s agents with --jobs 4 took {elapsed:?}: tasks ran one after another");
    let order: Vec<String> = read_run_records(&root, "s2").iter().map(|r| r["task"].as_str().unwrap().to_string()).collect();
    assert_eq!(order, ids, "records follow the task order whatever the finishing order");
    let _ = fs::remove_dir_all(&root);
}

fn eval_cli(root: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_designos-eval"));
    cmd.current_dir(root).env_remove("DESIGNOS_MCP_BIN");
    cmd
}

#[test]
fn unimplemented_subcommands_fail_instead_of_pretending() {
    let root = temp_test_dir("cli-unimplemented");
    for (command, task) in [("gen", "E15-24"), ("judge", "E15-31"), ("gate", "E15-33"), ("vote-sheet", "E15-32"), ("vote-import", "E15-32")] {
        let out = eval_cli(&root).arg(command).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{command} must not exit 0 without doing anything");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("not_implemented") && stderr.contains(task), "{command}: {stderr}");
    }
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn live_runs_wire_the_agent_to_designos_mcp_not_to_the_eval_binary() {
    let root = temp_test_dir("mcp-wiring");
    write_register(&root);
    let suites_root = write_suite(&root, "s2", vec![task("s2-wiring", "s2")], &[]);
    write_profile(&root, "peek", r#"["sh", "-c", "cp \"$1\" \"$2/seen-mcp-config.json\"", "sh", "{mcp_config}", "{output_dir}"]"#);
    mcp_bin(); // make sure designos-mcp sits next to the test binaries, where a sibling lookup finds it

    let out = eval_cli(&root)
        .args(["--suites-root"])
        .arg(&suites_root)
        .arg("--config")
        .arg(root.join("designos-eval.toml"))
        .args(["run", "--suite", "s2", "--agent", "peek", "--runs", "1", "--mode", "release", "--out"])
        .arg(root.join("results"))
        .output()
        .unwrap();
    assert!(out.status.code().is_some_and(|c| c <= 1), "{}", String::from_utf8_lossy(&out.stderr));
    let seen: Value = serde_json::from_str(&fs::read_to_string(root.join("results/v0.6.0/s2-wiring/1/seen-mcp-config.json")).unwrap()).unwrap();
    let command = Path::new(seen["mcpServers"]["designos"]["command"].as_str().unwrap()).file_stem().unwrap().to_string_lossy().to_string();
    assert_eq!(command, "designos-mcp", "the MCP server command must be designos-mcp, got {seen}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn replay_cli_uses_the_current_engine_for_every_tool() {
    let root = temp_test_dir("cli-replay");
    let recorded = root.join("transcript.jsonl");
    let line = json!({"seq": 1, "t_ms": 1, "tool": "designos_list_comments", "args": {}, "result": {"comments": []}, "is_error": false, "undo_depth": 0});
    fs::write(&recorded, format!("{}\n", serde_json::to_string(&line).unwrap())).unwrap();

    let out = eval_cli(&root)
        .env("DESIGNOS_MCP_BIN", mcp_bin())
        .arg("replay")
        .arg("--transcript")
        .arg(&recorded)
        .arg("--out")
        .arg(root.join("replayed"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "a tool the real server knows must replay: {}", String::from_utf8_lossy(&out.stderr));
    let _ = fs::remove_dir_all(&root);
}
