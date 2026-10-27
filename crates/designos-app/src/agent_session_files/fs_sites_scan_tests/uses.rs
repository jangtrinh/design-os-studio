//! `use` statements that name `std::fs`. A call through an alias (`use std::fs as f; f::write(..)`) shows no `fs::`, so the import is
//! what the scan has to see.

use crate::agent_session_files::fs_scan::CodeLine;
use std::collections::HashSet;

/// The text after `pub`, `pub(crate)`, ... when `line` starts a `use` statement.
fn after_use_keyword(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    if let Some(after_pub) = rest.strip_prefix("pub").filter(|after| !after.starts_with(is_word)) {
        rest = after_pub.trim_start();
        if rest.starts_with('(') {
            rest = rest.split_once(')')?.1.trim_start();
        }
    }
    rest.strip_prefix("use ").map(str::trim_start)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The identifier a path or an item of a `{..}` group starts with.
fn first_word(path: &str) -> &str {
    let path = path.trim_start();
    &path[..path.find(|c: char| !is_word(c)).unwrap_or(path.len())]
}

/// Whether the text of a `use` statement (after the keyword) imports `std::fs`, in any form: `std::fs`, `std::fs::{..}`, `std::{fs, ..}`,
/// `std::{self, fs as f}`; or renames `std` itself, which would hide `std::fs` from every later path.
fn names_std_fs(statement: &str) -> bool {
    let mut text = statement.split_whitespace().collect::<Vec<_>>().join(" ");
    for glue in [" ::", ":: "] {
        while text.contains(glue) {
            text = text.replace(glue, "::");
        }
    }
    let path = text.trim_start_matches("::");
    let Some(after_std) = path.strip_prefix("std").filter(|after| !after.starts_with(is_word)) else { return false };
    if after_std.starts_with(" as ") {
        return true;
    }
    let Some(rest) = after_std.strip_prefix("::") else { return false };
    let Some(group) = rest.strip_prefix('{') else { return first_word(rest) == "fs" };
    let (mut depth, mut start, mut items) = (0, 0, Vec::new());
    for (at, c) in group.char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => {
                items.push(&group[start..at]);
                break;
            }
            '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(&group[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    items.iter().any(|item| first_word(item) == "fs")
}

/// The positions in `lines` of every line of a `use` statement that names `std::fs`.
pub(super) fn lines_naming_std_fs(lines: &[CodeLine]) -> HashSet<usize> {
    let mut found = HashSet::new();
    let mut statement: Option<(Vec<usize>, String)> = None;
    for (position, line) in lines.iter().enumerate() {
        if statement.is_none() && after_use_keyword(&line.shape).is_some() {
            statement = Some((Vec::new(), String::new()));
        }
        let Some((positions, text)) = statement.as_mut() else { continue };
        positions.push(position);
        text.push(' ');
        text.push_str(&line.shape);
        if line.shape.contains(';') {
            if after_use_keyword(text).is_some_and(names_std_fs) {
                found.extend(positions.iter().copied());
            }
            statement = None;
        }
    }
    found
}
