//! Evaluation suite runner, transcript replay engine, deterministic metrics, and summary aggregation (E15-25 §3, §5).

use crate::agent::execute_agent_profile;
use crate::config::{AgentProfile, CommandVars, EvalConfig};
use crate::hash::sha1_hex;
use crate::jsonl::{sanitize_finite, EvalMode, EvalRecord, HostMeta, Meta, RunArtefacts, RunExit, RunRecord, SuiteSummary, TaskSummary};
use crate::mcp_client::McpClient;
use crate::rng::seed_for_metric;
use crate::sandbox::{prepare_workspace, RunSandbox, SCRUBBED_STUDIO_ENV_VARS};
use crate::split::{build_split_lock, verify_split_lock, Split, SplitLock};
use crate::stats::{bootstrap_ci, ci_method_for_runs, mean, sample_sd};
use crate::task::{SourceRegister, Task, TasksFile};
use crate::transcript::{compute_cost_metrics, had_document_edits, read_transcript, TranscriptLine};
use designos_doc::{Document, NodeId, NodeKind};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Versioned harness preamble embedded from `src/preamble.md`.
pub const HARNESS_PREAMBLE: &str = include_str!("preamble.md");

/// Structured evaluation error carrying the required CLI exit code (E15-25 §5):
/// - `1`: task failures recorded (results still written)
/// - `2`: usage or config error (`agent_not_found`, `agent_not_configured`, `invalid_config`, ...)
/// - `3`: refused (`held_out_refused`, `source_not_approved`, `split_lock_mismatch`, `replay_diverged`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalError {
    pub code: i32,
    pub message: String,
}

impl EvalError {
    pub fn usage(msg: impl Into<String>) -> Self {
        Self { code: 2, message: msg.into() }
    }

    pub fn refused(msg: impl Into<String>) -> Self {
        Self { code: 3, message: msg.into() }
    }
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EvalError {}

/// Options for `designos-eval run`.
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub suites_root: PathBuf,
    pub suites: Vec<String>,
    pub agent: String,
    pub model_override: Option<String>,
    pub runs: u32,
    pub mode: EvalMode,
    pub task_filter: Vec<String>,
    pub out_dir: PathBuf,
    pub release: String,
    pub jobs: usize,
    pub config_path: Option<PathBuf>,
    pub mcp_bin: PathBuf,
    pub repo_root: Option<PathBuf>,
}

/// Deterministic baseline metrics and rendered frame PNG paths produced for a finished run (E15-25 §3.7, §4).
#[derive(Clone, Debug)]
pub struct EvaluatedDocument {
    pub metrics: BTreeMap<String, f64>,
    pub render_rel_paths: Vec<String>,
    /// Artefacts that could not be written (disk full, permissions). A run with any is recorded as `crashed`.
    pub io_errors: Vec<String>,
}

/// Top-level frames (children of a page, in page order) that hold a node which is new or different compared
/// with `start`. Without a start document every node is new.
fn changed_top_level_frames(doc: &Document, start: Option<&Document>) -> Vec<NodeId> {
    let mut frames = Vec::new();
    for &pid in &doc.pages {
        let Some(page) = doc.get(pid) else { continue };
        for &fid in &page.children {
            let mut changed = false;
            doc.walk(fid, |node, _| {
                if start.and_then(|s| s.get(node.id)) != Some(node) {
                    changed = true;
                }
            });
            if changed {
                frames.push(fid);
            }
        }
    }
    frames
}

/// Renders output frames on the CPU path at scale 1.0 (`designos_export::export_png`) and computes
/// deterministic baseline metrics (`render_success`, `token_coverage`, `instance_ratio`).
///
/// Output frames are the task's declared `output` names, otherwise the frames created or changed during the run
/// (compared with `start`). `render_success` is 1 only when the transcript shows a real edit and every output frame
/// rendered; an agent that exits 0 without changing the document scores 0 (E15-25 §8 edge 2).
pub fn evaluate_document_and_render(
    doc: &Document,
    task: &Task,
    start: Option<&Document>,
    transcript: &[TranscriptLine],
    run_output_dir: &Path,
    rel_prefix: &str,
) -> EvaluatedDocument {
    let resolved_doc = designos_doc::apply_bound_variables(doc);
    let changed = changed_top_level_frames(doc, start);

    // Frames to render: the declared output names, or else whatever the run created or changed.
    let mut target_frames: Vec<NodeId> = Vec::new();
    if task.output.is_empty() {
        target_frames = changed.clone();
    } else {
        for wanted_name in &task.output {
            for &pid in &resolved_doc.pages {
                if let Some(page) = resolved_doc.get(pid) {
                    for &cid in &page.children {
                        if resolved_doc.get(cid).is_some_and(|n| n.name == *wanted_name) && !target_frames.contains(&cid) {
                            target_frames.push(cid);
                        }
                    }
                }
            }
        }
    }

    let mut render_rel_paths = Vec::new();
    let mut io_errors = Vec::new();
    let mut rendered: BTreeSet<NodeId> = BTreeSet::new();
    for (idx, &fid) in target_frames.iter().enumerate() {
        let png = designos_export::export_png(&resolved_doc, fid, 1.0);
        if png.is_empty() {
            continue;
        }
        let fname = format!("frame-{}.png", idx + 1);
        let full_png_path = run_output_dir.join(&fname);
        match std::fs::write(&full_png_path, &png) {
            Ok(()) => {
                rendered.insert(fid);
                render_rel_paths.push(if rel_prefix.is_empty() { fname } else { format!("{rel_prefix}/{fname}") });
            }
            Err(e) => io_errors.push(format!("cannot write '{}': {e}", full_png_path.display())),
        }
    }

    let mut metrics = BTreeMap::new();
    let edited = had_document_edits(transcript);
    let output_satisfied = if task.output.is_empty() {
        !rendered.is_empty()
    } else {
        task.output.iter().all(|name| {
            resolved_doc
                .pages
                .iter()
                .filter_map(|&pid| resolved_doc.get(pid))
                .flat_map(|page| page.children.iter())
                .any(|cid| resolved_doc.get(*cid).is_some_and(|n| n.name == *name) && rendered.contains(cid) && changed.contains(cid))
        })
    };
    metrics.insert("render_success".to_string(), if edited && output_satisfied { 1.0 } else { 0.0 });

    // Deterministic token_coverage: fraction of non-page nodes with paints/layout that have at least one
    // variable binding or style ref (0.0 when no non-page nodes exist).
    let mut non_page_nodes = 0usize;
    let mut bound_or_styled_nodes = 0usize;
    let mut instance_nodes = 0usize;
    for node in doc.nodes.values() {
        if matches!(node.kind, NodeKind::Page { .. }) {
            continue;
        }
        non_page_nodes += 1;
        if !node.variables.bindings.is_empty() || !node.style_refs.is_empty() {
            bound_or_styled_nodes += 1;
        }
        if matches!(node.kind, NodeKind::Instance { .. }) {
            instance_nodes += 1;
        }
    }

    let token_coverage = if non_page_nodes > 0 { (bound_or_styled_nodes as f64) / (non_page_nodes as f64) } else { 0.0 };
    let instance_ratio = if non_page_nodes > 0 { (instance_nodes as f64) / (non_page_nodes as f64) } else { 0.0 };
    metrics.insert("token_coverage".to_string(), token_coverage);
    metrics.insert("instance_ratio".to_string(), instance_ratio);

    EvaluatedDocument { metrics, render_rel_paths, io_errors }
}

/// Runs the evaluation harness for the requested suites and writes JSONL results to
/// `<out_dir>/<release>/<suite>.jsonl` (or `<out_dir>/<suite>.jsonl` if `out_dir` already ends with `release`).
/// Returns `Ok(has_task_failures)` where `true` maps to exit code `1` and `false` to exit code `0`.
pub fn run_harness(opts: &RunOptions) -> Result<bool, EvalError> {
    if opts.runs == 0 || opts.runs > 10 {
        return Err(EvalError::usage("invalid_runs: --runs must be between 1 and 10; pass e.g. --runs 3"));
    }

    let cfg = load_eval_config(opts.config_path.as_deref())?;
    let Some(profile) = cfg.agent.get(&opts.agent).cloned() else {
        return Err(EvalError::usage(format!(
            "agent_not_configured: no [agent.{}] profile found in designos-eval.toml; configure it or pass --config",
            opts.agent
        )));
    };
    profile.validate(&opts.agent).map_err(EvalError::usage)?;

    let register = SourceRegister::load(opts.repo_root.as_deref()).map_err(EvalError::usage)?;
    if !opts.mcp_bin.is_file() {
        return Err(EvalError::usage(format!(
            "mcp_not_found: '{}' is not a file; build designos-mcp (cargo build -p designos-mcp) or pass --mcp-bin",
            opts.mcp_bin.display()
        )));
    }

    let preamble_sha1 = sha1_hex(HARNESS_PREAMBLE.as_bytes());
    let model_name = opts.model_override.clone().unwrap_or_else(|| profile.model.clone());

    let mut any_failure = false;

    for suite in &opts.suites {
        let suite_dir = opts.suites_root.join(suite);
        let tasks_path = suite_dir.join("tasks.toml");
        if !tasks_path.exists() {
            return Err(EvalError::usage(format!("suite_not_found: '{}' does not exist", tasks_path.display())));
        }
        let raw_tasks = std::fs::read_to_string(&tasks_path).map_err(|e| EvalError::usage(format!("cannot read '{}': {e}", tasks_path.display())))?;
        let tasks_file: TasksFile = toml::from_str(&raw_tasks).map_err(|e| EvalError::usage(format!("invalid TOML in '{}': {e}", tasks_path.display())))?;

        let lock_path = suite_dir.join("split.lock");
        if !lock_path.exists() {
            return Err(EvalError::refused(format!(
                "split_lock_mismatch: missing '{}'; generate it with `designos-eval lock --suite {suite} --write`",
                lock_path.display()
            )));
        }
        let raw_lock = std::fs::read_to_string(&lock_path).map_err(|e| EvalError::usage(format!("cannot read '{}': {e}", lock_path.display())))?;
        let split_lock: SplitLock =
            toml::from_str(&raw_lock).map_err(|e| EvalError::refused(format!("split_lock_mismatch: invalid TOML in '{}': {e}", lock_path.display())))?;

        // Verify split.lock against all tasks in the suite BEFORE running any task (E15-25 §3.2)
        let splits_by_id = verify_split_lock(&split_lock, &tasks_file.tasks, &suite_dir).map_err(EvalError::refused)?;

        // Filter tasks if --tasks was given
        let selected_tasks: Vec<Task> = if opts.task_filter.is_empty() {
            tasks_file
                .tasks
                .into_iter()
                .filter(|t| if opts.mode == EvalMode::Tuning { splits_by_id.get(&t.id).copied() != Some(Split::HeldOut) } else { true })
                .collect()
        } else {
            let mut picked = Vec::new();
            for tid in &opts.task_filter {
                let Some(t) = tasks_file.tasks.iter().find(|t| &t.id == tid) else {
                    continue;
                };
                let sp = splits_by_id.get(&t.id).copied().unwrap_or(Split::Dev);
                if opts.mode == EvalMode::Tuning && sp == Split::HeldOut {
                    return Err(EvalError::refused(format!("held_out_refused: task {} is held out; run with --mode release or remove it from --tasks", t.id)));
                }
                picked.push(t.clone());
            }
            picked
        };

        // Check source register gate on every selected task before running (E15-25 §3.12)
        for task in &selected_tasks {
            register.verify_task(task).map_err(EvalError::refused)?;
        }

        let suite_out_dir = resolve_suite_out_dir(&opts.out_dir, &opts.release);
        std::fs::create_dir_all(&suite_out_dir).map_err(|e| EvalError::usage(format!("cannot create output directory '{}': {e}", suite_out_dir.display())))?;

        let meta = EvalRecord::Meta(Meta {
            schema: 1,
            release: opts.release.clone(),
            git: detect_git_sha(opts.repo_root.as_deref()),
            preamble_sha1: preamble_sha1.clone(),
            started: utc_now_rfc3339(),
            agent: opts.agent.clone(),
            model: model_name.clone(),
            judge_model: None,
            mode: opts.mode,
            runs: opts.runs,
            host: HostMeta::default(),
        });

        let mut records: Vec<EvalRecord> = vec![meta];
        let mut run_records: Vec<RunRecord> = Vec::new();

        // Every (task, run) pair gets its own sandbox and MCP process, so `--jobs` can run them side by side.
        let work: Vec<(&Task, Split, u32)> = selected_tasks
            .iter()
            .flat_map(|task| {
                let split = splits_by_id.get(&task.id).copied().unwrap_or(Split::Dev);
                (1..=opts.runs).map(move |run_idx| (task, split, run_idx))
            })
            .collect();
        let outcomes = run_jobs(opts.jobs, work.len(), |i| {
            let (task, split, run_idx) = work[i];
            execute_single_task_run(task, split, run_idx, &suite_dir, &suite_out_dir, &profile, &model_name, &opts.mcp_bin)
        });
        for outcome in outcomes {
            let run_rec = outcome?;
            if run_rec.exit != RunExit::Ok {
                any_failure = true;
            }
            records.push(EvalRecord::Run(run_rec.clone()));
            run_records.push(run_rec);
        }

        let (task_summaries, suite_summaries) = build_summaries(suite, &opts.release, &selected_tasks, &splits_by_id, &run_records);
        for ts in task_summaries {
            records.push(EvalRecord::Task(ts));
        }
        for ss in suite_summaries {
            records.push(EvalRecord::Suite(ss));
        }

        let jsonl_path = suite_out_dir.join(format!("{suite}.jsonl"));
        write_jsonl_records(&jsonl_path, &records).map_err(EvalError::usage)?;
    }

    Ok(any_failure)
}

/// Runs `work(0..count)` on up to `jobs` threads and returns the results in index order. After the first
/// `Err` no further item is started, so a configuration error stops the harness as early as a serial loop would.
fn run_jobs<T: Send>(jobs: usize, count: usize, work: impl Fn(usize) -> Result<T, EvalError> + Sync) -> Vec<Result<T, EvalError>> {
    let next = AtomicUsize::new(0);
    let failed = std::sync::atomic::AtomicBool::new(false);
    let slots: Mutex<Vec<Option<Result<T, EvalError>>>> = Mutex::new((0..count).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..jobs.clamp(1, count.max(1)) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= count || failed.load(Ordering::SeqCst) {
                    break;
                }
                let outcome = work(i);
                if outcome.is_err() {
                    failed.store(true, Ordering::SeqCst);
                }
                slots.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(outcome);
            });
        }
    });
    let mut out = Vec::with_capacity(count);
    for slot in slots.into_inner().unwrap_or_else(|e| e.into_inner()) {
        match slot {
            Some(outcome) => {
                let stop = outcome.is_err();
                out.push(outcome);
                if stop {
                    break;
                }
            }
            None => break,
        }
    }
    out
}

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ` (the schema's `meta.started`).
pub fn utc_now_rfc3339() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    rfc3339_utc(secs)
}

/// Formats Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (proleptic Gregorian calendar, no leap seconds).
pub fn rfc3339_utc(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3_600, rem % 3_600 / 60, rem % 60)
}

/// The task's start document as authored in the suite (not the workspace copy, which the agent may overwrite).
fn load_start_document(task: &Task, suite_dir: &Path) -> Option<Document> {
    if task.start == "blank" || task.start.is_empty() || task.start.starts_with("gen:") {
        return None;
    }
    designos_library::load_external(&suite_dir.join(&task.start)).ok()
}

#[allow(clippy::too_many_arguments)]
fn execute_single_task_run(
    task: &Task,
    split: Split,
    run_idx: u32,
    suite_dir: &Path,
    suite_out_dir: &Path,
    profile: &AgentProfile,
    model_name: &str,
    mcp_bin: &Path,
) -> Result<RunRecord, EvalError> {
    let full_prompt = format!("{}\n\n## Task Brief ({})\n{}\n", HARNESS_PREAMBLE.trim(), task.id, task.brief.trim());
    let sandbox = RunSandbox::prepare(task, run_idx, suite_dir, suite_out_dir, mcp_bin, &full_prompt).map_err(EvalError::usage)?;

    let stub_script_override = task.stub_script.as_ref().map(|rel| suite_dir.join(rel));

    let vars = CommandVars {
        prompt: &full_prompt,
        prompt_file: &sandbox.prompt_file_path,
        mcp_config: &sandbox.mcp_config_path,
        mcp_bin,
        workspace: &sandbox.workspace_dir,
        model: model_name,
        output_dir: &sandbox.output_dir,
    };

    let outcome = execute_agent_profile(profile, &vars, &sandbox, task.budget.timeout_s, stub_script_override.as_deref()).map_err(EvalError::usage)?;

    let mut notes = Vec::new();
    let mut exit = outcome.exit;
    if let Some(stderr_note) = outcome.stderr_note {
        notes.push(stderr_note);
    }
    let transcript = match read_transcript(&sandbox.transcript_path) {
        Ok(lines) => lines,
        Err(e) => {
            notes.push(format!("transcript_unreadable: {e}"));
            if exit == RunExit::Ok {
                exit = RunExit::Crashed;
            }
            Vec::new()
        }
    };
    let cost = compute_cost_metrics(&transcript, 0);
    if cost.tool_calls > (task.budget.max_tool_calls as u64) && exit == RunExit::Ok {
        exit = RunExit::Budget;
    }

    // The agent was told to save `output.designos`. When it did not (or was killed first), the session is rebuilt
    // from its transcript on a fresh server and saved, so the run is scored on what the agent actually did.
    let final_doc = match finalize_output_document(mcp_bin, task, suite_dir, &sandbox, &transcript) {
        Ok(doc) => Some(doc),
        Err(e) => {
            notes.push(format!("output_unavailable: {e}"));
            if exit == RunExit::Ok || exit == RunExit::Budget {
                exit = RunExit::Crashed;
            }
            None
        }
    };

    let rel_prefix = format!("{}/{}", task.id, run_idx);
    let (metrics, render_rel_paths) = match &final_doc {
        Some(doc) => {
            let start_doc = load_start_document(task, suite_dir);
            let evaluated = evaluate_document_and_render(doc, task, start_doc.as_ref(), &transcript, &sandbox.output_dir, &rel_prefix);
            if !evaluated.io_errors.is_empty() {
                notes.extend(evaluated.io_errors.iter().map(|e| format!("artefact_io_error: {e}")));
                if exit == RunExit::Ok || exit == RunExit::Budget {
                    exit = RunExit::Crashed;
                }
            }
            (evaluated.metrics, evaluated.render_rel_paths)
        }
        None => (BTreeMap::from([("render_success".to_string(), 0.0)]), Vec::new()),
    };
    if task.checklist.is_empty() {
        notes.push("zero_applicable_checklist_items: judge metrics omitted".to_string());
    }

    let error_rate = sanitize_finite(cost.error_rate, "error_rate", &mut notes);
    let retry_rate = sanitize_finite(cost.retry_rate, "retry_rate", &mut notes);
    let mut sanitized_metrics = BTreeMap::new();
    for (k, v) in metrics {
        sanitized_metrics.insert(k.clone(), sanitize_finite(v, &k, &mut notes));
    }

    Ok(RunRecord {
        task: task.id.clone(),
        split,
        run: run_idx,
        exit,
        wall_ms: outcome.wall_ms,
        tool_calls: cost.tool_calls,
        tool_errors: cost.tool_errors,
        error_rate,
        retries: cost.retries,
        retry_rate,
        undo_groups: cost.undo_groups,
        tokens_in: outcome.tokens_in,
        tokens_out: outcome.tokens_out,
        metrics: sanitized_metrics,
        artefacts: RunArtefacts {
            doc: format!("{rel_prefix}/output.designos"),
            renders: render_rel_paths,
            transcript: format!("{rel_prefix}/transcript.jsonl"),
        },
        notes,
    })
}

/// Puts the run's final document at `sandbox.output_doc_path` and loads it. A saved `output.designos` is copied;
/// otherwise the transcript is replayed on a fresh `designos-mcp` and the session is saved there (E15-25 §3.7).
fn finalize_output_document(mcp_bin: &Path, task: &Task, suite_dir: &Path, sandbox: &RunSandbox, transcript: &[TranscriptLine]) -> Result<Document, String> {
    let ws_output = sandbox.workspace_dir.join("output.designos");
    if ws_output.is_file() {
        std::fs::copy(&ws_output, &sandbox.output_doc_path)
            .map_err(|e| format!("cannot copy '{}' to '{}': {e}", ws_output.display(), sandbox.output_doc_path.display()))?;
    } else {
        let saved =
            replay_into_saved_document(mcp_bin, Some((task, suite_dir)), &sandbox.output_dir.join("fallback-session"), transcript).map_err(|e| e.message)?;
        std::fs::copy(&saved, &sandbox.output_doc_path)
            .map_err(|e| format!("cannot copy '{}' to '{}': {e}", saved.display(), sandbox.output_doc_path.display()))?;
    }
    designos_library::load_external(&sandbox.output_doc_path).map_err(|e| format!("cannot load '{}': {e}", sandbox.output_doc_path.display()))
}

/// Computes task and suite summaries (`TaskSummary`, `SuiteSummary` with `B = 10_000` bootstrap CIs)
/// from a collection of [`RunRecord`]s.
pub fn build_summaries(
    suite: &str,
    release: &str,
    tasks: &[Task],
    splits_by_id: &BTreeMap<String, Split>,
    run_records: &[RunRecord],
) -> (Vec<TaskSummary>, Vec<SuiteSummary>) {
    let mut metric_names = BTreeSet::new();
    for r in run_records {
        for k in r.metrics.keys() {
            metric_names.insert(k.clone());
        }
    }

    let mut task_order: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();
    if task_order.is_empty() {
        for r in run_records {
            if !task_order.contains(&r.task) {
                task_order.push(r.task.clone());
            }
        }
    }

    let mut task_summaries = Vec::new();
    // Map metric -> Vec<Vec<f64>> (outer = tasks, inner = runs for that task)
    let mut per_metric_task_runs: BTreeMap<String, Vec<Vec<f64>>> = BTreeMap::new();

    for tid in &task_order {
        let t_runs: Vec<&RunRecord> = run_records.iter().filter(|r| &r.task == tid).collect();
        if t_runs.is_empty() {
            continue;
        }
        let split = splits_by_id.get(tid).copied().unwrap_or(t_runs[0].split);
        let mut mean_map = BTreeMap::new();
        let mut sd_map = BTreeMap::new();
        let mut notes = Vec::new();

        for m in &metric_names {
            let vals: Vec<f64> = t_runs.iter().filter_map(|r| r.metrics.get(m).and_then(|v| *v)).filter(|v| v.is_finite()).collect();
            if vals.is_empty() {
                mean_map.insert(m.clone(), None);
                sd_map.insert(m.clone(), None);
            } else {
                let m_val = mean(&vals);
                let sd_val = sample_sd(&vals);
                mean_map.insert(m.clone(), sanitize_finite(m_val, m, &mut notes));
                sd_map.insert(m.clone(), sanitize_finite(sd_val, m, &mut notes));
                per_metric_task_runs.entry(m.clone()).or_default().push(vals);
            }
        }

        task_summaries.push(TaskSummary { task: tid.clone(), split, runs: t_runs.len() as u32, mean: mean_map, sd: sd_map, notes });
    }

    let mut suite_summaries = Vec::new();
    for (m, per_task_runs) in per_metric_task_runs {
        if per_task_runs.is_empty() {
            continue;
        }
        let seed = seed_for_metric(release, suite, &m);
        let (suite_mean, ci_lo, ci_hi) = bootstrap_ci(&per_task_runs, 10_000, seed);
        let ci_method = ci_method_for_runs(&per_task_runs).to_string();
        let mut notes = Vec::new();
        let mean_opt = sanitize_finite(suite_mean, &m, &mut notes);
        let lo_opt = sanitize_finite(ci_lo, &format!("{m}.ci95[0]"), &mut notes);
        let hi_opt = sanitize_finite(ci_hi, &format!("{m}.ci95[1]"), &mut notes);
        suite_summaries.push(SuiteSummary {
            suite: suite.to_string(),
            split: "all".to_string(),
            n_tasks: per_task_runs.len(),
            metric: m,
            mean: mean_opt,
            ci95: [lo_opt, hi_opt],
            ci_method,
            b: 10_000,
            notes,
        });
    }

    (task_summaries, suite_summaries)
}

/// Replays `lines` against a fresh `designos-mcp` (the current engine) in `work_dir/workspace`, prepared like a
/// run (start document and inputs of `task_ctx`), then saves the session to `workspace/output.designos` and returns
/// that path. The replay's own transcript is written to `work_dir/replay_transcript.jsonl`.
///
/// Node ids that differ on replay are followed through an old-to-new map built from the recorded and the new results.
/// A call that succeeded when recorded but fails now stops the replay with `replay_diverged` at its `seq`.
fn replay_into_saved_document(mcp_bin: &Path, task_ctx: Option<(&Task, &Path)>, work_dir: &Path, lines: &[TranscriptLine]) -> Result<PathBuf, EvalError> {
    let ws_dir = work_dir.join("workspace");
    if ws_dir.exists() {
        std::fs::remove_dir_all(&ws_dir).map_err(|e| EvalError::usage(format!("cannot reset replay workspace '{}': {e}", ws_dir.display())))?;
    }
    std::fs::create_dir_all(&ws_dir).map_err(|e| EvalError::usage(format!("cannot create replay dir '{}': {e}", ws_dir.display())))?;
    if let Some((task, suite_dir)) = task_ctx {
        prepare_workspace(task, suite_dir, &ws_dir).map_err(EvalError::usage)?;
    }
    let replay_transcript = work_dir.join("replay_transcript.jsonl");
    match std::fs::remove_file(&replay_transcript) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(EvalError::usage(format!("cannot reset '{}': {e}", replay_transcript.display()))),
    }

    let env = BTreeMap::from([("DESIGNOS_MCP_TRANSCRIPT".to_string(), replay_transcript.to_string_lossy().to_string())]);
    let mut client = McpClient::spawn(mcp_bin, &["--workspace".to_string(), ws_dir.to_string_lossy().to_string()], &env, SCRUBBED_STUDIO_ENV_VARS)
        .map_err(EvalError::usage)?;

    // old node id -> new node id, for results where an id came out different from the recording
    let mut id_map: BTreeMap<u64, u64> = BTreeMap::new();
    for line in lines {
        let mut mapped_args = line.args.clone();
        remap_node_ids(&mut mapped_args, &id_map);
        let out = client.call_tool(&line.tool, &mapped_args).map_err(EvalError::usage)?;
        if out.is_error && !line.is_error {
            return Err(EvalError::refused(format!(
                "replay_diverged: seq {} ({}) succeeded in the transcript but failed on replay: {}",
                line.seq, line.tool, out.value
            )));
        }
        if !out.is_error && !line.is_error {
            if let Some(old_result) = &line.result {
                collect_id_changes(old_result, &out.value, &mut id_map);
            }
        }
    }

    let saved = client.call_tool("designos_save_file", &json!({ "path": "output.designos", "overwrite": true })).map_err(EvalError::usage)?;
    if saved.is_error {
        return Err(EvalError::usage(format!("output_save_failed: designos_save_file output.designos failed: {}", saved.value)));
    }
    client.shutdown();
    Ok(ws_dir.join("output.designos"))
}

/// Re-executes a recorded `transcript.jsonl` file in order against a fresh `designos-mcp` and recomputes the
/// deterministic metrics (E15-25 §3.11, AC3). With `task` (and its suite directory) the replay sandbox gets the same
/// start document and inputs as the run, and the task's declared output frames are the ones rendered.
pub fn replay_transcript_file(mcp_bin: &Path, transcript_path: &Path, task: Option<(&Task, &Path)>, out_dir: &Path) -> Result<EvaluatedDocument, EvalError> {
    if !transcript_path.exists() {
        return Err(EvalError::usage(format!("transcript_not_found: '{}' does not exist", transcript_path.display())));
    }
    let lines = read_transcript(transcript_path).map_err(EvalError::usage)?;
    std::fs::create_dir_all(out_dir).map_err(|e| EvalError::usage(format!("cannot create replay dir '{}': {e}", out_dir.display())))?;
    let saved = replay_into_saved_document(mcp_bin, task, out_dir, &lines)?;
    let doc = designos_library::load_external(&saved).map_err(|e| EvalError::usage(format!("cannot load replayed document '{}': {e}", saved.display())))?;

    let default_task = Task {
        id: "replay".to_string(),
        suite: "replay".to_string(),
        source: "replay".to_string(),
        surface: "none".to_string(),
        brief: String::new(),
        inputs: Vec::new(),
        start: "blank".to_string(),
        design_system: "none".to_string(),
        checklist: Vec::new(),
        scope: Vec::new(),
        output: Vec::new(),
        budget: Default::default(),
        stub_script: None,
    };
    let task_ref = task.map_or(&default_task, |(t, _)| t);
    let start_doc = task.and_then(|(t, suite_dir)| load_start_document(t, suite_dir));
    let replay_lines = read_transcript(&out_dir.join("replay_transcript.jsonl")).map_err(EvalError::usage)?;
    Ok(evaluate_document_and_render(&doc, task_ref, start_doc.as_ref(), &replay_lines, out_dir, ""))
}

/// Task and suite folder for a `<task-id>/<run>/transcript.jsonl` path of a results folder, when `suite` knows the task.
pub fn replay_context<'a>(transcript: &Path, suite: Option<(&'a BTreeMap<String, Task>, &'a Path)>) -> Option<(&'a Task, &'a Path)> {
    let task_id = transcript.parent()?.parent()?.file_name()?.to_str()?;
    let (tasks, suite_dir) = suite?;
    tasks.get(task_id).map(|t| (t, suite_dir))
}

/// Loads `suites/<suite>/tasks.toml` as an id-indexed map together with the suite folder.
pub fn load_suite_tasks(suites_root: &Path, suite: &str) -> Result<(BTreeMap<String, Task>, PathBuf), EvalError> {
    let suite_dir = suites_root.join(suite);
    let tasks_path = suite_dir.join("tasks.toml");
    let raw = std::fs::read_to_string(&tasks_path).map_err(|e| EvalError::usage(format!("suite_not_found: cannot read '{}': {e}", tasks_path.display())))?;
    let file: TasksFile = toml::from_str(&raw).map_err(|e| EvalError::usage(format!("invalid TOML in '{}': {e}", tasks_path.display())))?;
    Ok((file.tasks.into_iter().map(|t| (t.id.clone(), t)).collect(), suite_dir))
}

/// Replays every `transcript.jsonl` under `dir` into `out` (same sub-folder layout). When `suite` is given, the
/// `<task-id>/<run>/transcript.jsonl` layout of a results folder selects the task that the replay sandbox is built from.
pub fn replay_directory(mcp_bin: &Path, dir: &Path, out: &Path, suite: Option<(&BTreeMap<String, Task>, &Path)>) -> Result<usize, EvalError> {
    let entries = std::fs::read_dir(dir).map_err(|e| EvalError::usage(format!("cannot read '{}': {e}", dir.display())))?;
    let mut replayed = 0usize;
    let mut sorted: Vec<_> = entries.flatten().collect();
    sorted.sort_by_key(|e| e.file_name());
    for entry in sorted {
        let p = entry.path();
        if p.is_dir() {
            replayed += replay_directory(mcp_bin, &p, &out.join(entry.file_name()), suite)?;
        } else if p.file_name().and_then(|s| s.to_str()) == Some("transcript.jsonl") {
            replay_transcript_file(mcp_bin, &p, replay_context(&p, suite), out)?;
            replayed += 1;
        }
    }
    Ok(replayed)
}

/// Keys whose numeric values are node ids in tool arguments.
const NODE_ID_KEYS: &[&str] = &["node_id", "parent_id", "id", "target_id", "root_id"];
/// Keys whose arrays hold node ids.
const NODE_ID_LIST_KEYS: &[&str] = &["node_ids", "ids", "children"];

/// Rewrites node ids in `value` through `id_map`, wherever a known id key holds a number (or a list of numbers).
fn remap_node_ids(value: &mut Value, id_map: &BTreeMap<u64, u64>) {
    match value {
        Value::Object(obj) => {
            for (key, v) in obj.iter_mut() {
                if NODE_ID_KEYS.contains(&key.as_str()) {
                    if let Some(&new_id) = v.as_u64().and_then(|old| id_map.get(&old)) {
                        *v = json!(new_id);
                        continue;
                    }
                }
                if NODE_ID_LIST_KEYS.contains(&key.as_str()) {
                    if let Value::Array(items) = v {
                        for item in items.iter_mut() {
                            if let Some(&new_id) = item.as_u64().and_then(|old| id_map.get(&old)) {
                                *item = json!(new_id);
                            }
                        }
                        continue;
                    }
                }
                remap_node_ids(v, id_map);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| remap_node_ids(v, id_map)),
        _ => {}
    }
}

/// Walks a recorded result and the replayed result side by side and records `old id -> new id` wherever the same
/// id key holds different numbers.
fn collect_id_changes(old: &Value, new: &Value, id_map: &mut BTreeMap<u64, u64>) {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, old_v) in a {
                let Some(new_v) = b.get(key) else { continue };
                if NODE_ID_KEYS.contains(&key.as_str()) || key == "root_node_id" {
                    if let (Some(o), Some(n)) = (old_v.as_u64(), new_v.as_u64()) {
                        if o != n {
                            id_map.insert(o, n);
                        }
                        continue;
                    }
                }
                collect_id_changes(old_v, new_v, id_map);
            }
        }
        (Value::Array(a), Value::Array(b)) => a.iter().zip(b).for_each(|(x, y)| collect_id_changes(x, y, id_map)),
        _ => {}
    }
}

/// Checks or writes `suites/<suite>/split.lock` (`designos-eval lock --suite <s> [--write]`).
pub fn check_or_write_split_lock(suites_root: &Path, suite: &str, salt: &str, write: bool, enforce_bounds: bool) -> Result<SplitLock, EvalError> {
    let suite_dir = suites_root.join(suite);
    let tasks_path = suite_dir.join("tasks.toml");
    let raw_tasks = std::fs::read_to_string(&tasks_path).map_err(|e| EvalError::usage(format!("cannot read '{}': {e}", tasks_path.display())))?;
    let tasks_file: TasksFile = toml::from_str(&raw_tasks).map_err(|e| EvalError::usage(format!("invalid TOML in '{}': {e}", tasks_path.display())))?;

    let lock_path = suite_dir.join("split.lock");
    if write {
        let lock = build_split_lock(suite, salt, &tasks_file.tasks, &suite_dir, enforce_bounds).map_err(EvalError::refused)?;
        let serialized = toml::to_string_pretty(&lock).map_err(|e| EvalError::usage(format!("cannot serialize split.lock: {e}")))?;
        std::fs::write(&lock_path, serialized).map_err(|e| EvalError::usage(format!("cannot write '{}': {e}", lock_path.display())))?;
        Ok(lock)
    } else {
        let raw_lock =
            std::fs::read_to_string(&lock_path).map_err(|e| EvalError::refused(format!("split_lock_mismatch: cannot read '{}': {e}", lock_path.display())))?;
        let lock: SplitLock =
            toml::from_str(&raw_lock).map_err(|e| EvalError::refused(format!("split_lock_mismatch: invalid TOML in '{}': {e}", lock_path.display())))?;
        verify_split_lock(&lock, &tasks_file.tasks, &suite_dir).map_err(EvalError::refused)?;
        Ok(lock)
    }
}

/// Recomputes `task` and `suite` summary lines for every `.jsonl` file inside `in_dir` (`designos-eval stats`).
pub fn recompute_stats_in_dir(in_dir: &Path) -> Result<usize, EvalError> {
    if !in_dir.exists() {
        return Err(EvalError::usage(format!("stats_input_not_found: '{}' does not exist", in_dir.display())));
    }
    let mut files = Vec::new();
    if in_dir.is_file() {
        files.push(in_dir.to_path_buf());
    } else {
        let entries = std::fs::read_dir(in_dir).map_err(|e| EvalError::usage(format!("cannot read '{}': {e}", in_dir.display())))?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                files.push(p);
            }
        }
    }

    let mut updated = 0usize;
    for path in files {
        let suite_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("s1").to_string();
        let raw = std::fs::read_to_string(&path).map_err(|e| EvalError::usage(format!("cannot read '{}': {e}", path.display())))?;
        let mut meta_rec: Option<Meta> = None;
        let mut runs: Vec<RunRecord> = Vec::new();
        for line in raw.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let rec: EvalRecord = serde_json::from_str(line).map_err(|e| EvalError::usage(format!("invalid JSONL in '{}': {e}", path.display())))?;
            match rec {
                EvalRecord::Meta(m) => meta_rec = Some(m),
                EvalRecord::Run(r) => runs.push(r),
                _ => {}
            }
        }
        let release = meta_rec.as_ref().map(|m| m.release.clone()).unwrap_or_else(|| "v0.6.0".to_string());
        let (task_summaries, suite_summaries) = build_summaries(&suite_name, &release, &[], &BTreeMap::new(), &runs);
        let mut out_records = Vec::new();
        if let Some(m) = meta_rec {
            out_records.push(EvalRecord::Meta(m));
        }
        for r in runs {
            out_records.push(EvalRecord::Run(r));
        }
        for ts in task_summaries {
            out_records.push(EvalRecord::Task(ts));
        }
        for ss in suite_summaries {
            out_records.push(EvalRecord::Suite(ss));
        }
        write_jsonl_records(&path, &out_records).map_err(EvalError::usage)?;
        updated += 1;
    }
    Ok(updated)
}

pub fn write_jsonl_records(path: &Path, records: &[EvalRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create '{}': {e}", parent.display()))?;
    }
    let mut f = std::fs::File::create(path).map_err(|e| format!("cannot create '{}': {e}", path.display()))?;
    for rec in records {
        let mut line = serde_json::to_string(rec).map_err(|e| format!("JSONL serialize error: {e}"))?;
        line.push('\n');
        f.write_all(line.as_bytes()).map_err(|e| format!("cannot write '{}': {e}", path.display()))?;
    }
    f.sync_all().map_err(|e| format!("cannot sync '{}': {e}", path.display()))?;
    Ok(())
}

fn load_eval_config(config_path: Option<&Path>) -> Result<EvalConfig, EvalError> {
    if let Some(p) = config_path {
        return EvalConfig::load_from_path(p).map_err(EvalError::usage);
    }
    if let Some(default_p) = EvalConfig::default_config_path() {
        if default_p.exists() {
            return EvalConfig::load_from_path(&default_p).map_err(EvalError::usage);
        }
    }
    Ok(EvalConfig::default())
}

fn resolve_suite_out_dir(out_dir: &Path, release: &str) -> PathBuf {
    if out_dir.file_name().and_then(|s| s.to_str()).is_some_and(|last| last == release) {
        out_dir.to_path_buf()
    } else {
        out_dir.join(release)
    }
}

fn detect_git_sha(repo_root: Option<&Path>) -> String {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["rev-parse", "--short=7", "HEAD"]);
    if let Some(r) = repo_root {
        cmd.current_dir(r);
    }
    if let Ok(out) = cmd.output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "unknown".to_string()
}
