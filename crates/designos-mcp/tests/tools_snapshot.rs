use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

#[path = "../src/bin/gen-tool-docs.rs"]
mod gen_tool_docs;

use gen_tool_docs::{
    build_tool_snapshots, render_parity_markdown, render_tools_markdown_block, render_tools_snapshot_json, replace_marked_block, repo_root, ParityFile,
    BEGIN_MARKER, DOC_TARGETS, END_MARKER,
};

#[test]
fn parity_markdown_prints_risk_tier() {
    let snapshots = build_tool_snapshots();
    let block = render_tools_markdown_block(&snapshots);
    assert!(block.contains("| Tool ("));
    assert!(block.contains("| Mode | Risk | Required Args | Description |"));
    assert!(block.contains("| `designos_get_document` | read-only | read |"));
    assert!(block.contains("| `designos_create_node` | mutating | additive |"));
    assert!(block.contains("| `designos_delete_node` | mutating | destructive |"));
    assert!(block.contains("| `designos_save_file` | mutating | destructive |"));
    assert!(block.contains("| `designos_update_node` | mutating | modify |"));
    assert_eq!(snapshots.len(), snapshots.iter().filter(|tool| tool.meta.get("risk_tier").and_then(|v| v.as_str()).is_some()).count());
}

#[test]
fn tool_list_matches_snapshot() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshot_path = crate_dir.join("tests/snapshots/tools.json");
    let on_disk = fs::read_to_string(&snapshot_path)
        .unwrap_or_else(|e| panic!("missing {} ({e}); run `cargo run -p designos-mcp --bin gen-tool-docs`", snapshot_path.display()));

    let parsed: Vec<Value> = serde_json::from_str(&on_disk).expect("parse tests/snapshots/tools.json");
    assert!(!parsed.is_empty(), "tool snapshot must not be empty");
    for entry in &parsed {
        let name = entry["name"].as_str().unwrap_or("<unnamed>");
        let schema = entry
            .get("inputSchema")
            .unwrap_or_else(|| panic!("snapshot entry `{name}` is missing canonical `inputSchema`; run `cargo run -p designos-mcp --bin gen-tool-docs`"));
        assert_eq!(schema["type"], json!("object"), "snapshot entry `{name}` inputSchema must have type=object");
        assert!(schema["properties"].is_object(), "snapshot entry `{name}` inputSchema.properties must be an object");
    }

    let snapshots = build_tool_snapshots();
    let expected = render_tools_snapshot_json(&snapshots);
    assert_eq!(on_disk, expected, "tests/snapshots/tools.json is out of date; run `cargo run -p designos-mcp --bin gen-tool-docs`");
}

#[test]
fn every_dispatched_tool_is_declared() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let declared_server: BTreeSet<String> = designos_mcp::server::tools().into_iter().map(|t| t.name.as_ref().to_string()).collect();
    let declared_engine: BTreeSet<String> = designos_mcp::engine::tools().into_iter().map(|t| t["name"].as_str().expect("tool name").to_string()).collect();
    let aliases: BTreeSet<&str> = designos_mcp::engine::ALIASES.iter().copied().collect();
    let dispatched: BTreeSet<&str> = designos_mcp::engine::DISPATCHED.iter().copied().collect();

    let engine_src = fs::read_to_string(crate_dir.join("src/engine.rs")).expect("read src/engine.rs");
    let call_body = engine_src
        .split_once("pub fn call(&mut self, name: &str, args: Value)")
        .and_then(|(_, rest)| rest.split_once("_ => Err(ToolError::new(ErrorCode::UnknownTool, \"\", format!(\"unknown tool: {name}\")))"))
        .map(|(body, _)| body)
        .expect("locate Engine::call match body in src/engine.rs");
    for line in call_body.lines() {
        if line.starts_with("            \"") {
            if let Some((pat, _)) = line.split_once("=>") {
                for seg in pat.split('|') {
                    let arm = seg.trim().trim_matches('"');
                    assert!(dispatched.contains(arm), "Engine::call match arm `{arm}` in src/engine.rs is missing from engine::DISPATCHED");
                }
            }
        }
    }

    let mut probe_engine = designos_mcp::engine::Engine::default();
    let unhandled_err = probe_engine.call("__unhandled_tool_probe__", json!({})).expect_err("unhandled tool probe must fail");
    assert_eq!(
        unhandled_err.message, "unknown tool: __unhandled_tool_probe__",
        "Engine::call default error prefix changed; keep this test aligned with Engine::call"
    );

    for &name in designos_mcp::engine::DISPATCHED {
        assert!(
            declared_server.contains(name) || aliases.contains(name),
            "dispatched tool `{name}` is neither declared in server::tools() nor listed in engine::ALIASES"
        );
        let mut engine = designos_mcp::engine::Engine::default();
        let res = engine.call(name, json!({}));
        if let Err(err) = &res {
            assert!(
                !err.starts_with("unknown tool: ") && err.message != format!("unknown tool: {name}"),
                "DISPATCHED entry `{name}` is not handled by Engine::call (got `{err}`)"
            );
        }
    }

    for name in &declared_engine {
        assert!(dispatched.contains(name.as_str()), "declared engine tool `{name}` is missing from engine::DISPATCHED");
    }

    for &alias in designos_mcp::engine::ALIASES {
        assert!(!alias.starts_with("designos_"), "alias `{alias}` must not be an undocumented `designos_*` tool");
        assert!(dispatched.contains(alias), "alias `{alias}` must be present in engine::DISPATCHED");
    }
}

fn line_has_numeric_tool_count(line: &str) -> bool {
    let tokens: Vec<String> = line.split(|c: char| !c.is_ascii_alphanumeric()).filter(|s| !s.is_empty()).map(str::to_ascii_lowercase).collect();
    for (idx, tok) in tokens.iter().enumerate() {
        if !tok.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let rest = &tokens[idx + 1..];
        let is_tool_word = |w: &str| matches!(w, "tool" | "tools");
        let matched = match rest {
            [w, ..] if is_tool_word(w) => true,
            [m, w, ..] if matches!(m.as_str(), "mcp" | "typed") && is_tool_word(w) => true,
            [t, m, w, ..] if t == "typed" && m == "mcp" && is_tool_word(w) => true,
            _ => false,
        };
        if matched {
            return true;
        }
    }
    false
}

fn find_prose_tool_counts_outside_markers(content: &str) -> Vec<(usize, String)> {
    let mut hits = Vec::new();
    let mut in_marked_block = false;
    for (idx, line) in content.lines().enumerate() {
        if line.contains(BEGIN_MARKER) {
            in_marked_block = true;
        }
        if !in_marked_block && line_has_numeric_tool_count(line) {
            hits.push((idx + 1, line.trim().to_string()));
        }
        if line.contains(END_MARKER) {
            in_marked_block = false;
        }
    }
    hits
}

#[test]
fn docs_blocks_are_current() {
    assert!(line_has_numeric_tool_count("drives `designos-mcp` (24 tools)"));
    assert!(line_has_numeric_tool_count("22-tool Model Context Protocol server"));
    assert!(line_has_numeric_tool_count("`24` MCP Tools + Native PTY Chat UI"));
    assert!(line_has_numeric_tool_count("Expose 24 typed MCP tools"));
    assert!(!line_has_numeric_tool_count("Scale Tool (`K`) with 1x/1.5x/2x"));

    let root = repo_root();
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let snapshots = build_tool_snapshots();
    let block = render_tools_markdown_block(&snapshots);

    for &rel in DOC_TARGETS {
        let path = root.join(rel);
        let on_disk = fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let expected = replace_marked_block(&on_disk, &block).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(on_disk, expected, "{rel} tool block is out of date; run `cargo run -p designos-mcp --bin gen-tool-docs`");

        let prose_counts = find_prose_tool_counts_outside_markers(&on_disk);
        assert!(
            prose_counts.is_empty(),
            "{rel} contains hardcoded prose tool count(s) outside `{BEGIN_MARKER}`..`{END_MARKER}` that will drift: {prose_counts:?}"
        );
    }

    let parity_raw = fs::read_to_string(crate_dir.join("parity/figma-mcp-parity.json")).expect("read parity/figma-mcp-parity.json");
    let parity: ParityFile = serde_json::from_str(&parity_raw).expect("parse parity/figma-mcp-parity.json");
    let expected_parity_md = render_parity_markdown(&snapshots, &parity);
    let on_disk_parity_md =
        fs::read_to_string(root.join("docs/mcp-parity.md")).expect("read docs/mcp-parity.md; run `cargo run -p designos-mcp --bin gen-tool-docs`");
    assert_eq!(on_disk_parity_md, expected_parity_md, "docs/mcp-parity.md is out of date; run `cargo run -p designos-mcp --bin gen-tool-docs`");

    let expected_rules_md = designos_mcp::rules::generate_default_rules_pack().expect("generate default rules pack");
    let on_disk_generated_rules = fs::read_to_string(crate_dir.join("generated/rules.md"))
        .expect("read crates/designos-mcp/generated/rules.md; run `cargo run -p designos-mcp --bin gen-tool-docs`");
    assert_eq!(
        on_disk_generated_rules, expected_rules_md,
        "crates/designos-mcp/generated/rules.md is out of date; run `cargo run -p designos-mcp --bin gen-tool-docs`"
    );

    let on_disk_skill_rules = fs::read_to_string(root.join("skills/design-os-studio/rules.md"))
        .expect("read skills/design-os-studio/rules.md; run `cargo run -p designos-mcp --bin gen-tool-docs`");
    assert_eq!(on_disk_skill_rules, expected_rules_md, "skills/design-os-studio/rules.md is out of date; run `cargo run -p designos-mcp --bin gen-tool-docs`");
}

fn collect_known_roadmap_tasks(root: &Path) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();

    let specs_dir = root.join("docs/specs/v0.6");
    if let Ok(entries) = fs::read_dir(&specs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some((prefix, _)) = stem.split_once('-') {
                        if prefix.starts_with('T') || prefix.starts_with('E') {
                            ids.insert(prefix.to_string());
                        }
                    }
                }
                if let Ok(text) = fs::read_to_string(&path) {
                    for token in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-') {
                        if (token.starts_with("T7.") || token.starts_with("T8.") || token.starts_with("E9-")) && token.len() >= 4 {
                            ids.insert(token.trim_end_matches('.').to_string());
                        }
                    }
                }
            }
        }
    }

    let plans_dir = root.join("plans/261017-epics-full-scope");
    if let Ok(entries) = fs::read_dir(&plans_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(raw) = fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                        if let Some(tasks) = val.get("tasks").and_then(Value::as_array) {
                            for task in tasks {
                                if let Some(id) = task.get("id").and_then(Value::as_str) {
                                    ids.insert(id.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ids
}

fn validate_parity_rows(parity: &ParityFile, declared_tools: &BTreeSet<String>, known_tasks: &BTreeSet<String>) -> Result<(), String> {
    if parity.core_tools == 0 {
        return Err("parity.core_tools must be > 0".into());
    }
    if parity.rows.len() != parity.core_tools + parity.weave_tools {
        return Err(format!("parity matrix rows ({}) must equal core_tools ({}) + weave_tools ({})", parity.rows.len(), parity.core_tools, parity.weave_tools));
    }

    let mut seen_figma_tools = BTreeSet::new();
    for row in &parity.rows {
        if !seen_figma_tools.insert(row.figma_tool.clone()) {
            return Err(format!("duplicate figma_tool `{}` in parity matrix", row.figma_tool));
        }

        match row.status.as_str() {
            "implemented" => {
                let studio_tool = row.studio_tool.as_deref().ok_or_else(|| format!("implemented row `{}` must specify `studio_tool`", row.figma_tool))?;
                if !declared_tools.contains(studio_tool) {
                    return Err(format!("implemented row `{}` references unknown studio_tool `{studio_tool}`", row.figma_tool));
                }
            }
            "task" => {
                let task_id = row.task_id.as_deref().ok_or_else(|| format!("task row `{}` must specify `task_id`", row.figma_tool))?;
                if !(task_id.starts_with("T7.") || task_id.starts_with("T8.") || task_id.starts_with("E9-")) {
                    return Err(format!("task row `{}` has unexpected task_id prefix `{task_id}`", row.figma_tool));
                }
                if !known_tasks.contains(task_id) {
                    return Err(format!(
                        "task row `{}` references task_id `{task_id}` not found in docs/specs/v0.6/ or plans/261017-epics-full-scope/",
                        row.figma_tool
                    ));
                }
                let studio_tool = row.studio_tool.as_deref().ok_or_else(|| {
                    format!("task row `{}` ({task_id}) must specify its planned `studio_tool` so shipping the tool forces status update", row.figma_tool)
                })?;
                if !studio_tool.starts_with("designos_") {
                    return Err(format!("task row `{}` planned studio_tool `{studio_tool}` must start with `designos_`", row.figma_tool));
                }
                let implied_studio_tool = format!("designos_{}", row.figma_tool);
                if declared_tools.contains(&implied_studio_tool) {
                    return Err(format!(
                        "task row `{}` still says `task` ({task_id}) even though `{implied_studio_tool}` is now declared in server::tools()",
                        row.figma_tool
                    ));
                }
                if declared_tools.contains(studio_tool) {
                    return Err(format!(
                        "task row `{}` still says `task` ({task_id}) even though `{studio_tool}` is now declared in server::tools()",
                        row.figma_tool
                    ));
                }
            }
            "excluded" => {
                let reason = row.reason.as_deref().ok_or_else(|| format!("excluded row `{}` must specify `reason`", row.figma_tool))?;
                if !(reason.contains("R4.13") || reason.contains("R4.14")) {
                    return Err(format!("excluded row `{}` reason `{reason}` must cite owner ruling R4.13 or R4.14", row.figma_tool));
                }
            }
            other => {
                return Err(format!("row `{}` has invalid status `{other}`; expected implemented | task | excluded", row.figma_tool));
            }
        }
    }
    Ok(())
}

#[test]
fn parity_rows_valid() {
    let root = repo_root();
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parity_raw = fs::read_to_string(crate_dir.join("parity/figma-mcp-parity.json")).expect("read parity/figma-mcp-parity.json");
    let parity: ParityFile = serde_json::from_str(&parity_raw).expect("parse parity/figma-mcp-parity.json");

    let declared_tools: BTreeSet<String> = designos_mcp::server::tools().into_iter().map(|t| t.name.as_ref().to_string()).collect();
    let known_tasks = collect_known_roadmap_tasks(&root);
    validate_parity_rows(&parity, &declared_tools, &known_tasks).unwrap_or_else(|e| panic!("{e}"));

    // Edge case 3: shipping a non-homonym planned tool (e.g. `designos_get_board` for `get_figjam`
    // or `designos_run_plugin_script` for `use_figma`) must fail while the row still says `task`.
    for shipped in ["designos_get_board", "designos_run_plugin_script"] {
        let mut with_shipped = declared_tools.clone();
        with_shipped.insert(shipped.to_string());
        let err =
            validate_parity_rows(&parity, &with_shipped, &known_tasks).expect_err("shipping a task row's planned studio_tool must reject stale `task` status");
        assert!(err.contains(shipped), "expected error mentioning `{shipped}`, got `{err}`");
    }
}

#[test]
fn stdio_count_follows_snapshot() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdio_src = fs::read_to_string(crate_dir.join("tests/stdio.rs")).expect("read tests/stdio.rs");
    assert!(stdio_src.contains("tests/snapshots/tools.json"), "tests/stdio.rs must read tests/snapshots/tools.json");
    assert!(!stdio_src.contains("assert_eq!(tools.len(), 22)"), "tests/stdio.rs must not hardcode tool count 22");

    let snapshot_raw = fs::read_to_string(crate_dir.join("tests/snapshots/tools.json")).expect("read tests/snapshots/tools.json");
    let snapshot: Vec<Value> = serde_json::from_str(&snapshot_raw).expect("parse tools.json");
    assert_eq!(designos_mcp::server::tools().len(), snapshot.len());
}
