//! The scan: reads the non-test Rust source of designos-mcp and says which filesystem calls and which session-file names are not
//! accounted for. It works on text (path, source) pairs, so its own tests can feed it small sources.
//!
//! A line is a filesystem call when it holds one of `FS_TOKENS`. It is fine when it goes through the helper (`VIA_HELPER`); else
//! it must be allowlisted by (file, function) with a reason. A raw call that also names a session file or the session folder
//! (`SESSION_MARKERS`) fails even in an allowlisted function: the allowlist is for the workspace and the library, never for a
//! session file. The helper's own files are exempt. Every mention of a session-file name or of the env var that points at one
//! (`NAME_TOKENS`) must be allowlisted too, so a new way to reach a session file is seen by a reviewer. An allowlist entry that
//! matches nothing is stale and fails.

/// One allowlist entry: where, and why it is not a session-file access. `func: None` covers a whole file and is allowed only
/// when `why` says where the paths come from; `func: Some("-")` covers what is outside any function.
pub struct Allowed {
    pub file: &'static str,
    pub func: Option<&'static str>,
    pub why: &'static str,
}

/// A source file: its path relative to the crate (`src/server.rs`) and its text.
pub struct Source {
    pub path: String,
    pub text: String,
}

/// The helper itself: it opens session files, so it is exempt from the raw-call rule.
const HELPER_FILES: [&str; 3] = ["src/session_file_io.rs", "src/session_file_io/no_follow.rs", "src/session_file_io/path_based.rs"];

const FS_TOKENS: [&str; 16] = [
    "fs::",
    "File::",
    "OpenOptions",
    "read_to_string",
    "read_dir",
    ".metadata(",
    "canonicalize",
    ".exists()",
    ".is_file()",
    ".is_dir()",
    "write_all",
    "create_dir",
    "remove_file",
    "remove_dir",
    "rename(",
    "load_external",
];
/// `designos_library::` reads and writes Studio's library files and the files `designos_open_file` opens.
const LIBRARY_TOKEN: &str = "designos_library::";
const VIA_HELPER: [&str; 2] = ["session_file_io::", "problems."];
const SESSION_MARKERS: [&str; 9] = [
    "session_dir",
    "SESSION_DIR",
    "bridge",
    "live_canvas_path",
    "live_doc_sync_path",
    "live_doc_patch_path",
    "live_spec_path",
    "context_json_path",
    "LEDGER_FILE_NAME",
];
const NAME_TOKENS: [&str; 17] = [
    "\"canvas.designos\"",
    "\"doc-sync.designos\"",
    "\"live-spec.json\"",
    "\"context.json\"",
    "\"doc-patch.jsonl\"",
    "\"session-options.json\"",
    "\"idempotency-ledger.jsonl\"",
    "\"mcp-config.json\"",
    "\"bootstrap.json\"",
    "LEDGER_FILE_NAME",
    "DESIGNOS_SESSION_DIR",
    "DESIGNOS_LIVE_CANVAS",
    "DESIGNOS_LIVE_DOC_SYNC",
    "DESIGNOS_LIVE_DOC_PATCH",
    "DESIGNOS_LIVE_SPEC",
    "DESIGNOS_CONTEXT_JSON",
    "DESIGNOS_MCP_CONFIG",
];

struct Hit {
    file: String,
    line: usize,
    func: Option<String>,
    code: String,
}

/// Whether the file holds only test code: a test file by name, declared under `#[cfg(test)]`; the dev tool in `src/bin`.
fn is_test_file(path: &str, all: &[Source]) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.trim_end_matches(".rs");
    if name != "tests.rs" && !name.ends_with("_tests.rs") && !name.starts_with("test_") {
        return false;
    }
    // A file named like a test is skipped only when some module declaration of it, by name or by `#[path]`, is gated by #[cfg(test)].
    let declaration = format!("mod {stem};");
    let by_path = format!("{name}\"");
    all.iter().any(|source| {
        let lines: Vec<&str> = source.text.lines().collect();
        (0..lines.len()).filter(|&i| lines[i].trim().starts_with("mod ") || lines[i].trim().starts_with("pub mod ")).any(|i| {
            let attributes: Vec<&str> = attributes_above(&lines, i).collect();
            attributes.iter().any(|a| is_test_gate(a))
                && (lines[i].trim() == declaration || attributes.iter().any(|a| a.starts_with("#[path") && a.contains(&by_path)))
        })
    })
}

fn is_test_gate(line: &str) -> bool {
    let line = line.trim();
    (line.starts_with("#[cfg(test") || line.starts_with("#[cfg(all(test")) && !line.contains("not(test")
}

/// The attribute and doc lines directly above `lines[i]`.
fn attributes_above<'a>(lines: &'a [&'a str], i: usize) -> impl Iterator<Item = &'a str> {
    lines[..i].iter().rev().map(|line| line.trim()).take_while(|line| line.starts_with("#[") || line.starts_with("///")).collect::<Vec<_>>().into_iter()
}

/// The lines of `text` that are not inside a `#[cfg(test)]` item; the others are blanked, so line numbers hold.
fn without_test_items(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut keep = vec![true; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if is_test_gate(lines[i]) {
            let (mut j, mut depth, mut started) = (i, 0i64, false);
            while j < lines.len() {
                for ch in lines[j].chars() {
                    match ch {
                        '{' => {
                            depth += 1;
                            started = true;
                        }
                        '}' => depth -= 1,
                        _ => {}
                    }
                }
                keep[j] = false;
                if started && depth <= 0 {
                    break;
                }
                if !started && lines[j].trim_end().ends_with(';') && j > i {
                    break;
                }
                j += 1;
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    lines.iter().zip(keep).map(|(line, keep)| if keep { line.to_string() } else { String::new() }).collect()
}

fn function_of(lines: &[String], index: usize) -> Option<String> {
    let indent = |line: &str| line.len() - line.trim_start().len();
    let at = indent(&lines[index]);
    (0..=index).rev().find_map(|k| {
        let line = lines[k].trim_start();
        let after = line.strip_prefix("pub(crate) ").or_else(|| line.strip_prefix("pub(super) ")).or_else(|| line.strip_prefix("pub ")).unwrap_or(line);
        let after = after.strip_prefix("async ").unwrap_or(after);
        let name = after.strip_prefix("fn ")?;
        (indent(&lines[k]) <= at).then(|| name.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect())
    })
}

/// A line that is a comment or the head of a function definition: neither calls anything.
fn is_comment_or_definition(line: &str) -> bool {
    let line = line.trim_start();
    let item = ["pub(crate) ", "pub(super) ", "pub "].iter().find_map(|p| line.strip_prefix(p)).unwrap_or(line);
    line.is_empty() || line.starts_with("//") || item.starts_with("fn ") || item.starts_with("async fn ")
}

/// The lines of `lines` that are part of a `use` statement, which may span several lines.
fn use_statements(lines: &[String]) -> Vec<bool> {
    let mut inside = false;
    lines
        .iter()
        .map(|line| {
            let line = line.trim();
            inside = inside || line.starts_with("use ") || line.starts_with("pub use ");
            let in_use = inside;
            if line.ends_with(';') {
                inside = false;
            }
            in_use
        })
        .collect()
}

fn hits(sources: &[Source], matching: impl Fn(&str) -> bool) -> Vec<Hit> {
    let mut found = Vec::new();
    for source in sources.iter().filter(|s| !is_test_file(&s.path, sources) && !s.path.ends_with("src/bin/gen-tool-docs.rs")) {
        let lines = without_test_items(&source.text);
        let in_use = use_statements(&lines);
        for (index, line) in lines.iter().enumerate() {
            if !in_use[index] && !is_comment_or_definition(line) && matching(line) {
                found.push(Hit { file: source.path.clone(), line: index + 1, func: function_of(&lines, index), code: line.trim().to_string() });
            }
        }
    }
    found
}

fn is_fs_call(line: &str) -> bool {
    FS_TOKENS.iter().any(|t| line.contains(t)) || line.contains(LIBRARY_TOKEN)
}

/// `func: Some("-")` covers what is outside any function (a constant, a static).
fn covers(entry: &Allowed, hit: &Hit) -> bool {
    entry.file == hit.file && entry.func.is_none_or(|f| hit.func.as_deref().unwrap_or("-") == f)
}

/// What is wrong with `sources`, one sentence each: empty when every filesystem call and name is accounted for.
pub fn problems(sources: &[Source], fs_calls: &[Allowed], names: &[Allowed]) -> Vec<String> {
    let mut problems = Vec::new();
    for (list, kind) in [(fs_calls, "fs-call"), (names, "name")] {
        for entry in list {
            if entry.why.trim().is_empty() || (entry.func.is_none() && !entry.why.contains("paths come from")) {
                problems.push(format!(
                    "{kind} allowlist entry {} {:?}: needs a reason; a whole-file entry's reason says where its paths come from",
                    entry.file, entry.func
                ));
            }
        }
    }
    let calls = hits(sources, is_fs_call);
    for hit in &calls {
        let helper_file = HELPER_FILES.contains(&hit.file.as_str());
        let via_helper = VIA_HELPER.iter().any(|t| hit.code.contains(t));
        let names_a_session_file = SESSION_MARKERS.iter().any(|t| hit.code.contains(t)) || NAME_TOKENS.iter().any(|t| hit.code.contains(t));
        let place = format!("{}:{} (fn {}): {}", hit.file, hit.line, hit.func.as_deref().unwrap_or("-"), hit.code);
        if helper_file || (via_helper && !hit.code.contains("std::fs")) {
            continue;
        }
        if names_a_session_file {
            problems.push(format!("raw filesystem call on a session file or the session folder: {place}"));
        } else if !fs_calls.iter().any(|entry| covers(entry, hit)) {
            problems.push(format!("filesystem call that is not through session_file_io and not allowlisted: {place}"));
        }
    }
    let name_hits = hits(sources, |line| NAME_TOKENS.iter().any(|t| line.contains(t)));
    for hit in &name_hits {
        if !names.iter().any(|entry| covers(entry, hit)) {
            problems.push(format!(
                "session-file name or env var not allowlisted: {}:{} (fn {}): {}",
                hit.file,
                hit.line,
                hit.func.as_deref().unwrap_or("-"),
                hit.code
            ));
        }
    }
    for (list, hit_list, kind) in [(fs_calls, &calls, "fs-call"), (names, &name_hits, "name")] {
        for entry in list {
            if !hit_list.iter().any(|hit| covers(entry, hit)) {
                problems.push(format!("stale {kind} allowlist entry: {} fn {:?} matches nothing", entry.file, entry.func));
            }
        }
    }
    problems
}

/// The non-test sources of the crate at `manifest_dir`: every `.rs` file below `src`.
pub fn read_sources(manifest_dir: &std::path::Path) -> Vec<Source> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<Source>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let relative = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.push(Source { path: relative, text: std::fs::read_to_string(&path).unwrap() });
            }
        }
    }
    let mut out = Vec::new();
    walk(&manifest_dir.join("src"), manifest_dir, &mut out);
    out
}
