//! Binary entry point for `designos-eval`.

use clap::Parser;
use designos_eval::agent::{resolve_binary_path, run_stub_agent};
use designos_eval::cli::{Cli, CliMode, Commands};
use designos_eval::jsonl::EvalMode;
use designos_eval::runner::{
    check_or_write_split_lock, load_suite_tasks, recompute_stats_in_dir, replay_context, replay_directory, replay_transcript_file, run_harness, EvalError,
    RunOptions,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(false) => ExitCode::from(0),
        Ok(true) => ExitCode::from(1),
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(err.code.clamp(1, 255) as u8)
        }
    }
}

fn resolve_suites_root(override_root: Option<PathBuf>) -> PathBuf {
    if let Some(p) = override_root {
        return p;
    }
    let candidate = PathBuf::from("crates/designos-eval/suites");
    if candidate.exists() {
        return candidate;
    }
    PathBuf::from("suites")
}

/// The `designos-mcp` the agent is wired to: `--mcp-bin`, `$DESIGNOS_MCP_BIN`, a sibling of this executable, or `PATH`.
/// It is never this executable, which is not an MCP server.
fn resolve_mcp_bin(explicit: Option<PathBuf>) -> Result<PathBuf, EvalError> {
    let named = explicit.or_else(|| std::env::var_os("DESIGNOS_MCP_BIN").filter(|v| !v.is_empty()).map(PathBuf::from));
    if let Some(path) = named {
        return if path.is_file() {
            Ok(path)
        } else {
            Err(EvalError::usage(format!("mcp_not_found: '{}' is not a file; pass the designos-mcp binary to --mcp-bin", path.display())))
        };
    }
    resolve_binary_path("designos-mcp").ok_or_else(|| {
        EvalError::usage("mcp_not_found: designos-mcp is not next to designos-eval or on PATH; build it (cargo build -p designos-mcp) or pass --mcp-bin")
    })
}

fn not_implemented(command: &str, task: &str) -> EvalError {
    EvalError::usage(format!("not_implemented: `designos-eval {command}` is delivered by {task}; this build does not include it yet"))
}

fn dispatch(cli: Cli) -> Result<bool, EvalError> {
    let suites_root = resolve_suites_root(cli.suites_root);
    let mcp_override = cli.mcp_bin;
    match cli.command {
        Commands::Run { suite, agent, model, runs, mode, tasks, out, release, jobs } => {
            let eval_mode = match mode {
                CliMode::Tuning => EvalMode::Tuning,
                CliMode::Release => EvalMode::Release,
            };
            let opts = RunOptions {
                suites_root,
                suites: suite,
                agent,
                model_override: model,
                runs,
                mode: eval_mode,
                task_filter: tasks,
                out_dir: out,
                release,
                jobs,
                config_path: cli.config,
                mcp_bin: resolve_mcp_bin(mcp_override)?,
                repo_root: std::env::current_dir().ok(),
            };
            run_harness(&opts)
        }
        Commands::Replay { suite, transcript, transcripts, out } => {
            let mcp_bin = resolve_mcp_bin(mcp_override)?;
            let suite_tasks = suite.map(|name| load_suite_tasks(&suites_root, &name)).transpose()?;
            let suite_ctx = suite_tasks.as_ref().map(|(tasks, dir)| (tasks, dir.as_path()));
            if let Some(t_file) = transcript {
                let res = replay_transcript_file(&mcp_bin, &t_file, replay_context(&t_file, suite_ctx), &out)?;
                println!("{}", serde_json::to_string_pretty(&res.metrics).unwrap_or_default());
                return Ok(false);
            }
            if let Some(t_dir) = transcripts {
                if !t_dir.exists() {
                    return Err(EvalError::usage(format!("transcript_not_found: '{}' does not exist", t_dir.display())));
                }
                let replayed = replay_directory(&mcp_bin, &t_dir, &out, suite_ctx)?;
                println!("replayed {replayed} transcript(s) into {}", out.display());
                return Ok(false);
            }
            Err(EvalError::usage("missing_transcript: pass --transcript <file> or --transcripts <dir>"))
        }
        Commands::Stats { in_dir } => {
            recompute_stats_in_dir(&in_dir)?;
            Ok(false)
        }
        Commands::Lock { suite, salt, write } => {
            let lock = check_or_write_split_lock(&suites_root, &suite, &salt, write, false)?;
            println!("suite={} tasks={} held_out_fraction={:.3}", lock.suite, lock.tasks.len(), lock.held_out_fraction);
            Ok(false)
        }
        Commands::StubAgent { script, mcp_config } => {
            run_stub_agent(&script, &mcp_config).map_err(EvalError::usage)?;
            Ok(false)
        }
        Commands::Gen { .. } => Err(not_implemented("gen", "E15-24")),
        Commands::Judge { .. } => Err(not_implemented("judge", "E15-31")),
        Commands::Gate { .. } => Err(not_implemented("gate", "E15-33")),
        Commands::VoteSheet { .. } => Err(not_implemented("vote-sheet", "E15-32")),
        Commands::VoteImport { .. } => Err(not_implemented("vote-import", "E15-32")),
    }
}
