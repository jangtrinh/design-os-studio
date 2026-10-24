//! The shipped source of designos-app, line by line: what the scan of file-system calls (`fs_sites_scan_tests`) reads. A line is
//! shipped unless it lies in an item gated by `#[cfg(test)]` (or `#[test]`), or in a file that only such a `mod name;` declares.

mod items;
mod lexer;

use items::{attributes, file_module_name, is_test_attribute, item_end};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// One line of shipped code. `code` has its comments blanked; `shape` has its strings blanked too.
pub(super) struct CodeLine {
    pub(super) number: usize,
    pub(super) code: String,
    pub(super) shape: String,
}

pub(super) struct ShippedFile {
    /// The path below `src`, with `/` as the separator.
    pub(super) rel: String,
    pub(super) lines: Vec<CodeLine>,
}

/// A `mod name;` that puts another file in the build: its name, its `#[path]`, and whether only a test build has it.
struct FileModule {
    name: String,
    path_attribute: Option<String>,
    test_only: bool,
}

struct Parsed {
    shape: Vec<char>,
    code: Vec<char>,
    test_chars: Vec<bool>,
    file_modules: Vec<FileModule>,
    all_test: bool,
}

fn parse(src: &str) -> Parsed {
    let (code, shape) = lexer::lex(src);
    let (mut test_chars, mut file_modules, mut all_test) = (vec![false; shape.len()], Vec::new(), false);
    let attrs = attributes(&shape, &code);
    // The depth of braces before each character: an inner attribute gates a whole file only when it stands outside every block.
    let depth_before: Vec<i32> = shape
        .iter()
        .scan(0i32, |depth, c| {
            let before = *depth;
            *depth += i32::from(*c == '{') - i32::from(*c == '}');
            Some(before)
        })
        .collect();
    let mut k = 0;
    while k < attrs.len() {
        let mut group_end = k;
        while group_end + 1 < attrs.len() && shape[attrs[group_end].end..attrs[group_end + 1].start].iter().all(|c| c.is_whitespace()) {
            group_end += 1;
        }
        let group = &attrs[k..=group_end];
        let test = group.iter().any(|a| is_test_attribute(&a.text));
        all_test |= group.iter().any(|a| a.text.starts_with('!') && is_test_attribute(&a.text[1..]) && depth_before[a.start] == 0);
        let end = item_end(&shape, group[group.len() - 1].end);
        let item: String = shape[group[group.len() - 1].end..end].iter().collect();
        if let Some(name) = file_module_name(&item) {
            let path_attribute = group.iter().find_map(|a| a.text.trim().strip_prefix("path").and_then(|r| r.split('"').nth(1)).map(str::to_string));
            file_modules.push(FileModule { name, path_attribute, test_only: test });
        }
        if test {
            test_chars[group[0].start..end].fill(true);
        }
        k = group_end + 1;
    }
    // A `mod name;` that no attribute stands before declares a file too.
    let text: String = shape.iter().collect();
    for (at, _) in text.match_indices("mod ") {
        let boundary = text[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let name: String = text[at + 4..].trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let declared = text[at + 4..].trim_start().strip_prefix(name.as_str()).is_some_and(|rest| rest.trim_start().starts_with(';'));
        if boundary && !name.is_empty() && declared && !file_modules.iter().any(|m| m.name == name) {
            let in_test_item = test_chars[text[..at].chars().count()];
            file_modules.push(FileModule { name, path_attribute: None, test_only: in_test_item });
        }
    }
    Parsed { shape, code, test_chars, file_modules, all_test }
}

/// The files `parsed_at` declares with `mod name;`, as paths.
fn declared_files(path: &Path, parsed: &Parsed) -> Vec<(PathBuf, bool)> {
    let dir = path.parent().unwrap_or(Path::new(""));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let module_dir = if matches!(stem, "mod" | "main" | "lib") { dir.to_path_buf() } else { dir.join(stem) };
    parsed
        .file_modules
        .iter()
        .map(|m| {
            let file = match &m.path_attribute {
                Some(p) => dir.join(p),
                None => {
                    let flat = module_dir.join(format!("{}.rs", m.name));
                    if flat.exists() {
                        flat
                    } else {
                        module_dir.join(&m.name).join("mod.rs")
                    }
                }
            };
            (file, m.test_only)
        })
        .collect()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// The files only a test build has: those that gate themselves with an inner `#![cfg(test)]` at their top level, those a
/// `#[cfg(test)] mod name;` declares, and those a `mod name;` inside such a file declares. A file that a shipped file declares without
/// a gate is none of them, whatever else names it (a test-gated `#[path]` alias of it, say), unless it gates itself.
fn test_only_files(parsed: &[(PathBuf, Parsed)]) -> HashSet<PathBuf> {
    let gates_itself: HashSet<PathBuf> = parsed.iter().filter(|(_, p)| p.all_test).map(|(path, _)| path.clone()).collect();
    let mut shipped_declared: HashSet<PathBuf> = HashSet::new();
    loop {
        let mut test_only = gates_itself.clone();
        loop {
            let before = test_only.len();
            for (path, p) in parsed {
                let inherited = test_only.contains(path);
                let children = declared_files(path, p).into_iter().filter(|(file, gated)| (*gated || inherited) && !shipped_declared.contains(file));
                test_only.extend(children.map(|(file, _)| file));
            }
            if test_only.len() == before {
                break;
            }
        }
        let declared: HashSet<PathBuf> = parsed
            .iter()
            .filter(|(path, _)| !test_only.contains(path))
            .flat_map(|(path, p)| declared_files(path, p).into_iter().filter(|(_, gated)| !*gated).map(|(file, _)| file))
            .collect();
        if declared.is_subset(&shipped_declared) {
            return test_only;
        }
        shipped_declared.extend(declared);
    }
}

/// Every shipped line of every `.rs` file below `src`.
pub(super) fn shipped_files(src: &Path) -> Vec<ShippedFile> {
    let mut paths = Vec::new();
    rust_files(src, &mut paths);
    let parsed: Vec<(PathBuf, Parsed)> = paths.into_iter().map(|p| (p.clone(), parse(&std::fs::read_to_string(&p).expect("read a source file")))).collect();
    let test_only = test_only_files(&parsed);
    let mut files = Vec::new();
    for (path, p) in parsed {
        if test_only.contains(&path) {
            continue;
        }
        let (mut lines, mut start) = (Vec::new(), 0);
        for (number, text) in p.shape.split(|c| *c == '\n').enumerate() {
            let range = start..start + text.len();
            start = range.end + 1;
            if p.test_chars[range.clone()].iter().any(|t| *t) {
                continue;
            }
            lines.push(CodeLine { number: number + 1, code: p.code[range.clone()].iter().collect(), shape: p.shape[range].iter().collect() });
        }
        files.push(ShippedFile { rel: path.strip_prefix(src).expect("below src").to_string_lossy().replace('\\', "/"), lines });
    }
    files
}
