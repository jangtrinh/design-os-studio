//! Completeness made mechanical. A session folder is the agent's working folder: it can plant a link or a FIFO at any name in it, so
//! every file-system call that may be given a path in it has to go through `agent_session_files` (`read_session_*`, `open_session_file`,
//! `atomic_write_session_file`, ...). Hand-made scans missed such calls four times, so this test reads all the shipped source of the crate
//! (`fs_scan` skips what only a test build has) and finds five kinds of line:
//!
//! 1. a file-system call: `std::fs::*` (or `fs::*` after `use std::fs`), `File::open/create/options`, `OpenOptions`, `DirBuilder`,
//!    `.read_to_string(`, `.read_dir(`, `.metadata(`, `.symlink_metadata(`, `.canonicalize(`, `.read_link(`, and the write helpers
//!    `write_atomic(`, `write_owner_only_file(`, `create_private_dir_all(`. `Path::exists`/`is_file`/`is_dir` only stat, they neither
//!    block nor write, and are left out;
//! 2. a `use` that names `std::fs` in any form (`use std::fs as f;`, `use std::{fs as f2}`, `use std::fs::*`) or renames `std`: calls made
//!    through the alias show no `fs::`, so the import is what is read;
//! 3. the name of a session file (`SESSION_NAMES`) inside a string, which includes the shell commands of the hooks;
//! 4. a string that holds a shell command that writes: a redirect in any form (`>`, `>>`, `x>`, `3>`, `&>`, a leading `>`; not `->`,
//!    `=>`, an angle pair, `2>&1` or a write to `/dev/null`), or `mkdir`, `mv`, `cp`, `rm`, `touch`, `tee`, `ln`, `truncate`: a hook runs
//!    in the session folder and writes through a link the agent left there;
//! 5. `Command::new("<program>")` of a program that writes files (`cp`, `mv`, `ln`, `rm`, `tee`, `touch`, `truncate`, `dd`, `install`,
//!    `mkdir`, `rmdir`, `sh`, `bash`).
//!
//! A line of any kind must be listed in `fs_sites_allowlist.txt` with a reason, by file and text (so a line shifts freely), with the
//! number of times it occurs there. A whole-file entry exempts the file-system calls of a file that never receives a path in a
//! session folder (the reason says where its paths come from); it never exempts a session file name, a shell command or a program. A
//! listed line that is no longer there, or occurs a different number of times, fails the test, so the list cannot go stale. A name
//! joined to a session path (`session_dir.join("x")`, also `.join(&format!(..))` and a chain of joins) must be in `SESSION_NAMES`.
//!
//! What the scan cannot see:
//! - file APIs of other crates: `image::open`, `.save(`, the methods of `designos-library` other than `write_atomic`;
//! - `libc` calls, and `Command::new` of a program that a variable names;
//! - a path built from a session folder under a variable whose name is not in `SESSION_PATHS`, and a name built at run time;
//! - what a FILE row's helpers do: each file in them was read to be sure no caller gives it a session path, and that has to be done again
//!   when a caller of one of its public functions changes;
//! - code another crate or a macro generates, and any file outside `designos-app/src`.
//!
//! `scanner_tests` runs the scan on source made for the test: each way it was blind (a field-level `#[cfg(test)]`, an inner
//! `#![cfg(test)]` in a module, a `use` alias, a `Command::new("cp")`, ...) is a test that fails without its fix.

mod bypass_tests;
mod patterns;
mod scanner_tests;
mod uses;

use super::fs_scan::{shipped_files, ShippedFile};
use patterns::{command_of_file_writer, has_fs_call, names_in_strings, shell_writes_in_strings, SESSION_NAMES, SUBFOLDER_NAMES};
use std::collections::BTreeMap;
use std::path::Path;
use uses::lines_naming_std_fs;

const ALLOWLIST: &str = include_str!("fs_sites_allowlist.txt");

/// A file under `agent_session_files/` is one of the routed helpers, or a test of them.
fn is_routed_helper(rel: &str) -> bool {
    rel.starts_with("agent_session_files/") || rel == "agent_session_files.rs"
}

/// Names of the variables that hold a session folder, or a folder made in one: a `.join(..)` after any of them (on the same line, so
/// `session_dir.join(".gemini").join("settings.json")` counts for both joins) names a file or folder of a session.
const SESSION_PATHS: &[&str] = &["session", "job_dir", "plugin_dir", "stable_dir", "fallback_dir"];

/// The names that `.join("<name>")`, `.join(&format!("<name>..."))`, `.join(format!(..))` and `.with_file_name("<name>")` give to a session
/// path, taken from the code.
fn derived_names(files: &[ShippedFile]) -> Vec<(String, usize, String)> {
    let mut found = Vec::new();
    for file in files.iter().filter(|f| !is_routed_helper(&f.rel)) {
        for line in &file.lines {
            for call in [".join(", ".with_file_name("] {
                for (at, _) in line.code.match_indices(call) {
                    let rest = line.code[at + call.len()..].trim_start_matches('&').trim_start_matches("format!(");
                    let before = &line.code[..at];
                    // The private channel folder is beside the session folder, not in it, whatever the line also says about the session.
                    let in_session = SESSION_PATHS.iter().any(|path| before.contains(path)) && !before.contains("channel");
                    if let (true, Some(literal)) = (in_session, rest.strip_prefix('"')) {
                        found.push((file.rel.clone(), line.number, literal.split('"').next().unwrap_or("").to_string()));
                    }
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

/// Every problem in `files` against `allow`: a line that is neither routed nor listed, a listed line that is gone or counted wrong, a
/// whole-file entry for a file without a file-system call, and a joined name that `SESSION_NAMES` does not know.
fn problems(files: &[ShippedFile], allow: &Allow) -> Vec<String> {
    // What is in the code: per file and trimmed line, how often it occurs, and whether it holds a file-system call.
    let mut found: BTreeMap<(String, String), (usize, bool)> = BTreeMap::new();
    let mut fs_calls_per_file: BTreeMap<&str, usize> = BTreeMap::new();
    for file in files.iter().filter(|f| !is_routed_helper(&f.rel)) {
        let std_fs_uses = lines_naming_std_fs(&file.lines);
        for (position, line) in file.lines.iter().enumerate() {
            let fs_call = has_fs_call(&line.shape) || std_fs_uses.contains(&position);
            let names = !names_in_strings(&line.code, &line.shape).is_empty();
            let shell = shell_writes_in_strings(&line.code, &line.shape) || command_of_file_writer(&line.code, &line.shape);
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
    for (file, number, literal) in derived_names(files) {
        if !SESSION_NAMES.iter().chain(SUBFOLDER_NAMES).any(|name| literal.contains(name)) {
            problems.push(format!("{file}:{number} joins \"{literal}\" to a session path, and SESSION_NAMES has no name for it"));
        }
    }
    problems
}

#[test]
fn every_file_system_call_and_session_file_name_in_shipped_code_is_routed_or_listed() {
    let files = shipped_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    let problems = problems(&files, &parse_allowlist(ALLOWLIST));
    assert!(problems.is_empty(), "{} problem(s) in the scan of shipped code:\n{}", problems.len(), problems.join("\n"));
}
