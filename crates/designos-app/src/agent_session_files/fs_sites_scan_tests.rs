//! Completeness made mechanical. A session folder is the agent's working folder: it can plant a link or a FIFO at any name in it, so
//! every file-system call that may be given a path in it has to go through `agent_session_files` (`read_session_*`, `open_session_file`,
//! `atomic_write_session_file`, ...). Hand-made scans missed such calls four times, so this test reads all the shipped source of the crate
//! (`fs_scan` skips what only a test build has) and finds three kinds of line:
//!
//! 1. a file-system call: `std::fs::*` (or `fs::*` after `use std::fs`), `File::open/create/options`, `OpenOptions`, `DirBuilder`,
//!    `.read_to_string(`, `.read_dir(`, `.metadata(`, `.symlink_metadata(`, `.canonicalize(`, `.read_link(`, and the write helpers
//!    `write_atomic(`, `write_owner_only_file(`, `create_private_dir_all(`. `Path::exists`/`is_file`/`is_dir` only stat, they neither
//!    block nor write, and are left out;
//! 2. the name of a session file (`SESSION_NAMES`) inside a string, which includes the shell commands of the hooks;
//! 3. a string that holds a shell command that writes (a redirect, `mkdir`, `mv`, ...): a hook runs in the session folder and writes
//!    through a link the agent left there.
//!
//! A line of either kind must be listed in `fs_sites_allowlist.txt` with a reason, by file and text (so a line shifts freely), with
//! the number of times it occurs there. A whole-file entry exempts the file-system calls of a file that never receives a path in a
//! session folder (the reason says where its paths come from); it never exempts a session file name or a shell command. A listed line that is no longer
//! there, or occurs a different number of times, fails the test, so the list cannot go stale.

mod patterns;

use super::fs_scan::{shipped_files, ShippedFile};
use patterns::{has_fs_call, names_in_strings, shell_writes_in_strings, SESSION_NAMES};
use std::collections::BTreeMap;
use std::path::Path;

const ALLOWLIST: &str = include_str!("fs_sites_allowlist.txt");

/// A file under `agent_session_files/` is one of the routed helpers, or a test of them.
fn is_routed_helper(rel: &str) -> bool {
    rel.starts_with("agent_session_files/") || rel == "agent_session_files.rs"
}

/// The names `<something with "session">.join("<name>")` uses, taken from the code itself.
fn derived_names(files: &[ShippedFile]) -> Vec<(String, usize, String)> {
    let mut found = Vec::new();
    for file in files.iter().filter(|f| !is_routed_helper(&f.rel)) {
        for line in &file.lines {
            for (at, _) in line.code.match_indices(".join(") {
                let reversed: String = line.code[..at].chars().rev().take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '.')).collect();
                let receiver: String = reversed.chars().rev().collect();
                let rest = line.code[at + 6..].trim_start_matches("format!(");
                if let (true, Some(literal)) = (receiver.contains("session"), rest.strip_prefix('"')) {
                    found.push((file.rel.clone(), line.number, literal.split('"').next().unwrap_or("").to_string()));
                }
            }
        }
    }
    found
}

struct Allow {
    whole_files: BTreeMap<String, String>,
    lines: BTreeMap<(String, String), (usize, String)>,
}

fn parse_allowlist(text: &str) -> Allow {
    let mut allow = Allow { whole_files: BTreeMap::new(), lines: BTreeMap::new() };
    for (n, row) in text.lines().enumerate().filter(|(_, row)| !row.trim().is_empty() && !row.starts_with('#')) {
        let fields: Vec<&str> = row.split('\t').collect();
        match fields.as_slice() {
            ["FILE", file, reason] if !reason.trim().is_empty() => drop(allow.whole_files.insert(file.to_string(), reason.to_string())),
            ["LINE", file, count, text, reason] if !reason.trim().is_empty() => {
                let count = count.parse().unwrap_or_else(|_| panic!("fs_sites_allowlist.txt:{}: the count is not a number", n + 1));
                allow.lines.insert((file.to_string(), text.to_string()), (count, reason.to_string()));
            }
            _ => panic!("fs_sites_allowlist.txt:{}: a FILE row has 3 tab-separated fields and a LINE row 5, each with a reason: {row}", n + 1),
        }
    }
    allow
}

#[test]
fn every_file_system_call_and_session_file_name_in_shipped_code_is_routed_or_listed() {
    let files = shipped_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    let allow = parse_allowlist(ALLOWLIST);
    // What is in the code: per file and trimmed line, how often it occurs, and whether it holds a file-system call.
    let mut found: BTreeMap<(String, String), (usize, bool)> = BTreeMap::new();
    let mut fs_calls_per_file: BTreeMap<&str, usize> = BTreeMap::new();
    for file in files.iter().filter(|f| !is_routed_helper(&f.rel)) {
        for line in &file.lines {
            let (fs_call, names, shell) =
                (has_fs_call(&line.shape), !names_in_strings(&line.code, &line.shape).is_empty(), shell_writes_in_strings(&line.code, &line.shape));
            if !(fs_call || names || shell) {
                continue;
            }
            *fs_calls_per_file.entry(&file.rel).or_default() += usize::from(fs_call);
            if fs_call && allow.whole_files.contains_key(&file.rel) && !names && !shell {
                continue;
            }
            let entry = found.entry((file.rel.clone(), line.code.trim().to_string())).or_default();
            *entry = (entry.0 + 1, entry.1 || fs_call);
        }
    }
    let mut problems = Vec::new();
    for ((file, text), (count, _)) in &found {
        match allow.lines.get(&(file.clone(), text.clone())) {
            None => problems.push(format!("not listed: LINE\t{file}\t{count}\t{text}\tREASON")),
            Some((listed, _)) if listed != count => problems.push(format!("listed {listed} times, found {count}: {file}: {text}")),
            Some(_) => {}
        }
    }
    for (file, text) in allow.lines.keys().filter(|key| !found.contains_key(*key)) {
        problems.push(format!("stale, no such line any more: {file}: {text}"));
    }
    for file in allow.whole_files.keys().filter(|file| fs_calls_per_file.get(file.as_str()).copied().unwrap_or(0) == 0) {
        problems.push(format!("stale, a whole-file entry for a file with no file-system call (or no such file): {file}"));
    }
    for (file, number, literal) in derived_names(&files) {
        if !SESSION_NAMES.iter().any(|name| literal.contains(name)) {
            problems.push(format!("{file}:{number} joins \"{literal}\" to a session path, and SESSION_NAMES has no name for it"));
        }
    }
    assert!(problems.is_empty(), "{} problem(s) in the scan of shipped code:\n{}", problems.len(), problems.join("\n"));
}
