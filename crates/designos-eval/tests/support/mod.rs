//! Shared fixtures for the `designos-eval` integration tests: a temp repository with a licence register,
//! suites with split locks, agent profiles, and the real `designos-mcp` binary the harness talks to.

#![allow(dead_code)]

use designos_doc::{Document, NodeSpec};
use designos_eval::jsonl::EvalMode;
use designos_eval::runner::{check_or_write_split_lock, RunOptions};
use designos_eval::split::{assign_split, Split};
use designos_eval::task::{Task, TasksFile};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const SALT: &str = "designos-eval-v1";

pub fn temp_test_dir(tag: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("designos-eval-test-{tag}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The `designos-mcp` binary of this workspace. `cargo test --workspace` has built it next to the test
/// binaries; for `cargo test -p designos-eval` it is built here once.
pub fn mcp_bin() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let exe = std::env::current_exe().expect("test executable path");
        let profile_dir = exe.parent().and_then(Path::parent).expect("target/<profile> directory").to_path_buf();
        let candidate = profile_dir.join(format!("designos-mcp{}", std::env::consts::EXE_SUFFIX));
        if !candidate.is_file() {
            let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
            let mut cmd = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
            cmd.args(["build", "-p", "designos-mcp", "--manifest-path"]).arg(&manifest);
            if profile_dir.file_name().is_some_and(|n| n == "release") {
                cmd.arg("--release");
            }
            assert!(cmd.status().expect("cargo build -p designos-mcp").success(), "building designos-mcp failed");
        }
        assert!(candidate.is_file(), "designos-mcp binary not found at {}", candidate.display());
        candidate
    })
    .clone()
}

/// A minimal E15-23 register: one approved source, one pending, one rejected, plus the ban patterns.
pub fn write_register(repo_root: &Path) {
    let docs = repo_root.join("docs");
    fs::create_dir_all(&docs).unwrap();
    let register = r#"# Register

```toml
[[source]]
id = "shadcn-ui"
review = "approved"
use = "eval-redistribute"

[[source]]
id = "pending-source"
review = "pending"
use = "eval-only-cache"

[[source]]
id = "figma-client-files"
review = "rejected"
use = "excluded"

[ban]
patterns = ['(?i)\bvsf\b', '(?i)\bpcp\b', '^/Users/']
```
"#;
    fs::write(docs.join("eval-licences.md"), register).unwrap();
}

/// A task in suite `suite` with every optional field at its default.
pub fn task(id: &str, suite: &str) -> Task {
    Task {
        id: id.to_string(),
        suite: suite.to_string(),
        source: "shadcn-ui".to_string(),
        surface: "web-app".to_string(),
        brief: "Create a hero card frame.".to_string(),
        inputs: Vec::new(),
        start: "blank".to_string(),
        design_system: "none".to_string(),
        checklist: vec!["Has a Hero Frame".to_string()],
        scope: Vec::new(),
        output: vec!["Hero Frame".to_string()],
        budget: Default::default(),
        stub_script: None,
    }
}

/// Task ids that land in the dev split and one in the held-out split under [`SALT`].
pub fn split_ids(suite: &str) -> (Vec<String>, String) {
    let mut dev_ids = Vec::new();
    let mut held_out_id = String::new();
    for i in 1..200 {
        let candidate = format!("{suite}-{i:04}-shadcn");
        match assign_split(SALT, &candidate) {
            Split::Dev if dev_ids.len() < 2 => dev_ids.push(candidate),
            Split::HeldOut if held_out_id.is_empty() => held_out_id = candidate,
            _ => {}
        }
        if dev_ids.len() == 2 && !held_out_id.is_empty() {
            break;
        }
    }
    (dev_ids, held_out_id)
}

/// JSONL script lines for the stub agent.
pub fn script(lines: &[Value]) -> String {
    lines.iter().map(|l| format!("{}\n", serde_json::to_string(l).unwrap())).collect()
}

pub fn create_hero_frame_call() -> Value {
    json!({
        "tool": "designos_apply_spec",
        "args": { "spec": {
            "name": "Hero Frame", "preset": "easeui-card", "width": 320, "height": 180,
            "children": [{ "kind": "text", "name": "Heading", "text": "Hello Eval", "font_size": 18, "fill": "$easeui.text.primary" }]
        } }
    })
}

pub fn save_output_call() -> Value {
    json!({ "tool": "designos_save_file", "args": { "path": "output.designos", "overwrite": true } })
}

/// Writes `suites/<suite>/` with the given tasks and a fresh split lock, and returns the suites root.
pub fn write_suite(root: &Path, suite: &str, tasks: Vec<Task>, files: &[(&str, String)]) -> PathBuf {
    let suites_root = root.join("suites");
    let suite_dir = suites_root.join(suite);
    fs::create_dir_all(&suite_dir).unwrap();
    for (name, content) in files {
        fs::write(suite_dir.join(name), content).unwrap();
    }
    fs::write(suite_dir.join("tasks.toml"), toml::to_string_pretty(&TasksFile { tasks }).unwrap()).unwrap();
    check_or_write_split_lock(&suites_root, suite, SALT, true, false).unwrap();
    suites_root
}

/// A start document with one frame named "Hero Frame", saved at `path`.
pub fn write_start_document(path: &Path) {
    let mut doc = Document::new("start");
    let page = doc.add_page("Page 1");
    let spec: NodeSpec = serde_json::from_value(json!({
        "name": "Hero Frame", "preset": "easeui-card", "width": 320, "height": 180,
        "children": [{ "kind": "text", "name": "Heading", "text": "Start", "font_size": 18 }]
    }))
    .unwrap();
    doc.apply_spec(page, &spec).unwrap();
    fs::write(path, doc.to_json().unwrap()).unwrap();
}

/// Agent profile file with one `[agent.<name>]` table whose command is the given TOML array literal.
pub fn write_profile(root: &Path, name: &str, command_toml: &str) -> PathBuf {
    let path = root.join("designos-eval.toml");
    fs::write(&path, format!("[agent.{name}]\ncommand = {command_toml}\nmodel = \"test-model\"\nusage = \"none\"\n")).unwrap();
    path
}

/// Options for a release run of `suite` in `root` with the profile `agent`.
pub fn run_options(root: &Path, suites_root: PathBuf, suite: &str, agent: &str, runs: u32) -> RunOptions {
    RunOptions {
        suites_root,
        suites: vec![suite.to_string()],
        agent: agent.to_string(),
        model_override: None,
        runs,
        mode: EvalMode::Release,
        task_filter: Vec::new(),
        out_dir: root.join("results"),
        release: "v0.6.0".to_string(),
        jobs: 1,
        config_path: Some(root.join("designos-eval.toml")),
        mcp_bin: mcp_bin(),
        repo_root: Some(root.to_path_buf()),
    }
}

/// The `run` records of `<out>/v0.6.0/<suite>.jsonl`.
pub fn read_run_records(root: &Path, suite: &str) -> Vec<Value> {
    let raw = fs::read_to_string(root.join("results").join("v0.6.0").join(format!("{suite}.jsonl"))).unwrap();
    raw.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()).filter(|v| v["type"] == "run").collect()
}
