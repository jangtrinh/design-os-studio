//! What the scan accepts. Nothing here is a session-file access: those go through `session_file_io` and need no entry.
use super::scan::Allowed;

const WORKSPACE_ROOT: &str = "the --workspace root given on the command line";
const WORKSPACE_PATH: &str = "a --workspace path from a tool argument, resolved and checked against the root";
const GUIDELINES: &str = "the guideline files bundled with the crate; no session path";
const LIBRARY_ROOT: &str = "Studio's library root (DESIGNOS_LIBRARY), not a session folder";

/// Filesystem calls that are not through the helper, because their paths are not session files.
pub const FS_CALLS: &[Allowed] = &[
    Allowed {
        file: "src/engine.rs",
        func: Some("call"),
        why: "designos_export_fig_file and import_web_capture on a bare Engine: a --workspace path (Session resolves it first)",
    },
    Allowed { file: "src/engine.rs", func: Some("write_checked_output_file"), why: WORKSPACE_PATH },
    Allowed { file: "src/server.rs", func: Some("new"), why: WORKSPACE_ROOT },
    Allowed { file: "src/server.rs", func: Some("with_session_dir"), why: WORKSPACE_ROOT },
    Allowed { file: "src/server.rs", func: Some("input_path"), why: WORKSPACE_PATH },
    Allowed { file: "src/server.rs", func: Some("output_path_allow_overwrite"), why: WORKSPACE_PATH },
    Allowed { file: "src/server.rs", func: Some("png_output_path"), why: WORKSPACE_PATH },
    Allowed { file: "src/server.rs", func: Some("fig_output_path"), why: WORKSPACE_PATH },
    Allowed {
        file: "src/server.rs",
        func: Some("call_inner"),
        why: "open, save, render, export and import: the paths come from input_path and the *_output_path checks",
    },
    Allowed { file: "src/tools/guidelines.rs", func: Some("call_get_guidelines_with_enabled_tools"), why: GUIDELINES },
    Allowed { file: "src/tools/guidelines.rs", func: Some("list_topics_value"), why: GUIDELINES },
    Allowed { file: "src/tools/guidelines.rs", func: Some("has_toml_files"), why: GUIDELINES },
    Allowed { file: "src/tools/guidelines.rs", func: Some("validate_archetypes_dir"), why: GUIDELINES },
    Allowed { file: "src/tools/libraries.rs", func: Some("get_libraries_at"), why: LIBRARY_ROOT },
    Allowed { file: "src/tools/library_source.rs", func: Some("load_cached_snapshot_at"), why: LIBRARY_ROOT },
    Allowed { file: "src/tools/library_source.rs", func: Some("enabled_snapshots"), why: LIBRARY_ROOT },
];

const PATH_ONLY: &str = "builds a session-file path; every read and write of it goes through session_file_io";
const RULES_TEXT: &str = "text of the generated rules and skill file; no I/O";

/// Where a session-file name or the env var that points at one is written in code.
pub const NAMES: &[Allowed] = &[
    Allowed { file: "src/engine.rs", func: Some("read_session_json_field"), why: "reads context.json and mcp-config.json through session_file_io" },
    Allowed { file: "src/engine.rs", func: Some("resolve_agent_run_id"), why: "names the two files read_session_json_field reads" },
    Allowed { file: "src/engine.rs", func: Some("resolve_agent_name"), why: "names the two files read_session_json_field reads" },
    Allowed { file: "src/rules.rs", func: Some("config_path"), why: PATH_ONLY },
    Allowed { file: "src/rules.rs", func: Some("render_nodespec_schema_section"), why: RULES_TEXT },
    Allowed { file: "src/rules.rs", func: Some("format_session_skill_md"), why: RULES_TEXT },
    Allowed { file: "src/server.rs", func: Some("from_env"), why: "reads the env vars that name the bridge's paths" },
    Allowed { file: "src/server.rs", func: Some("from_env_vars"), why: PATH_ONLY },
    Allowed { file: "src/server.rs", func: Some("for_session_dir"), why: PATH_ONLY },
    Allowed { file: "src/server.rs", func: Some("new"), why: "the ledger's path; Ledger reads and writes it through session_file_io" },
    Allowed { file: "src/server.rs", func: Some("with_session_dir"), why: "the ledger's path; Ledger reads and writes it through session_file_io" },
    Allowed { file: "src/server.rs", func: Some("sync_from_live_canvas_if_newer"), why: "reads mcp-config.json and bootstrap.json through file_problems" },
    Allowed { file: "src/tools/ledger.rs", func: Some("-"), why: "the ledger's file name" },
    Allowed { file: "src/tools/session_options.rs", func: Some("file_path"), why: PATH_ONLY },
];
