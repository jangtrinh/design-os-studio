//! What a file-change item does, as its card says it (#930): one line per change, the destructive ones first.

use crate::approval_text::reads_as_it_is;
use serde_json::Value;

/// What approving a file-change item would do.
pub enum Changes {
    /// One line per change: `delete <path>` and `move <path> \u{2192} <target>` first, then `add <path>`, then `edit <path>`;
    /// the order of the item within each group.
    Described(Vec<String>),
    /// A change of a kind Studio does not describe (or without a path): such a card would not say what it does.
    UnknownKind,
    /// A path that would not read on the card as it is: see [`reads_as_it_is`]; or one that contains the card's own
    /// separators (`\u{2192}`, `, `), which can make one change read as two.
    PathNotShownExactly,
}

const DESTRUCTIVE: u8 = 0;
const ADD: u8 = 1;
const EDIT: u8 = 2;

impl Changes {
    /// Describes the `changes` of a file-change item.
    pub fn describe(changes: &[Value]) -> Changes {
        let mut lines = Vec::with_capacity(changes.len());
        for change in changes {
            match describe_change(change) {
                Ok(line) => lines.push(line),
                Err(failed) => return failed,
            }
        }
        lines.sort_by_key(|(rank, _)| *rank);
        Changes::Described(lines.into_iter().map(|(_, line)| line).collect())
    }
}

/// `(group, line)` of one change, or why it cannot be described.
fn describe_change(change: &Value) -> Result<(u8, String), Changes> {
    let (Some(path), Some(kind)) = (change["path"].as_str(), change["kind"]["type"].as_str()) else { return Err(Changes::UnknownKind) };
    let target = change["kind"]["move_path"].as_str().filter(|target| !target.is_empty());
    let (group, line) = match (kind, target) {
        ("add", None) => (ADD, format!("add {path}")),
        ("update", None) => (EDIT, format!("edit {path}")),
        ("update", Some(target)) => (DESTRUCTIVE, format!("move {path} \u{2192} {target}")),
        ("delete", None) => (DESTRUCTIVE, format!("delete {path}")),
        _ => return Err(Changes::UnknownKind),
    };
    if shown_as_it_is(path) && target.is_none_or(shown_as_it_is) {
        Ok((group, line))
    } else {
        Err(Changes::PathNotShownExactly)
    }
}

/// A path the card can show as it is: it reads as it is, and it does not contain what the card puts between changes or
/// between a path and its move target.
fn shown_as_it_is(path: &str) -> bool {
    reads_as_it_is(path) && !path.contains('\u{2192}') && !path.contains(", ")
}
