//! Offline reproducible evaluation harness for DESIGN:OS Studio (E15-25).

pub mod agent;
pub mod cli;
pub mod config;
pub mod hash;
pub mod jsonl;
pub mod mcp_client;
pub mod rng;
pub mod runner;
pub mod sandbox;
pub mod split;
pub mod stats;
pub mod task;
pub mod transcript;

pub use config::{AgentProfile, EvalConfig, UsageFormat};
pub use jsonl::{validate_jsonl_line, EvalMode, EvalRecord, HostMeta, Meta, RunArtefacts, RunExit, RunRecord, SuiteSummary, TaskSummary};
pub use runner::{EvalError, RunOptions};
pub use split::{assign_split, build_split_lock, verify_split_lock, Split, SplitEntry, SplitLock};
pub use stats::bootstrap_ci;
pub use task::{SourceRegister, Task, TaskBudget, TasksFile};
