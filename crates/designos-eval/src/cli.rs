//! Command-line interface for `designos-eval` (E15-25 §5).

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "designos-eval", about = "Offline reproducible evaluation harness for DESIGN:OS Studio")]
pub struct Cli {
    /// Optional path to `designos-eval.toml`.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Root directory containing `suites/<suite>/tasks.toml` (defaults to `crates/designos-eval/suites` or `./suites`).
    #[arg(long, global = true)]
    pub suites_root: Option<PathBuf>,

    /// `designos-mcp` binary the agent talks to (defaults to `$DESIGNOS_MCP_BIN`, then a sibling of this executable, then `PATH`).
    #[arg(long, global = true)]
    pub mcp_bin: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliMode {
    Tuning,
    Release,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Live evaluation runs through the configured agent CLI.
    Run {
        #[arg(long, value_delimiter = ',')]
        suite: Vec<String>,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, default_value_t = 3)]
        runs: u32,
        #[arg(long, value_enum, default_value_t = CliMode::Tuning)]
        mode: CliMode,
        #[arg(long, value_delimiter = ',')]
        tasks: Vec<String>,
        #[arg(long, default_value = "results")]
        out: PathBuf,
        #[arg(long, default_value = "v0.6.0")]
        release: String,
        #[arg(long, default_value_t = 1)]
        jobs: usize,
    },
    /// Re-run recorded transcripts on the current engine and recompute deterministic metrics.
    Replay {
        #[arg(long)]
        suite: Option<String>,
        #[arg(long)]
        transcript: Option<PathBuf>,
        #[arg(long)]
        transcripts: Option<PathBuf>,
        #[arg(long, default_value = "results/replay")]
        out: PathBuf,
    },
    /// Recompute task and suite summaries (`B = 10,000` bootstrap CIs) for existing JSONL results.
    Stats {
        #[arg(long = "in")]
        in_dir: PathBuf,
    },
    /// Check or regenerate `suites/<suite>/split.lock`.
    Lock {
        #[arg(long)]
        suite: String,
        #[arg(long, default_value = "designos-eval-v1")]
        salt: String,
        #[arg(long)]
        write: bool,
    },
    /// Scripted MCP client used by deterministic harness tests (calls no model).
    StubAgent {
        #[arg(long)]
        script: PathBuf,
        #[arg(long)]
        mcp_config: PathBuf,
    },
    /// Deterministic large-document generator subcommand hook (E15-24).
    Gen {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Checklist & pairwise judge subcommand hook (E15-31).
    Judge {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// CI evaluation regression gate subcommand hook (E15-33).
    Gate {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Human calibration vote-sheet subcommand hook (E15-32).
    VoteSheet {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Human calibration vote-import subcommand hook (E15-32).
    VoteImport {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
