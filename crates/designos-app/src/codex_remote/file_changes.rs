//! What a file-change item does, as its card says it (#930): one line per change, the destructive ones first.

use crate::approval_text::reads_as_it_is;
use serde_json::Value;

/// What approving a file-change item would do.
pub enum Changes {
    /// One line per change: `delete <path>` and `move <path> \u{2192} <target>` first, then `add <path>`, then `edit <path>`;
    /// the order of the item within each group.
    Described(Vec<String>, Vec<FileLine>),
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
        lines.sort_by_key(|(rank, ..)| *rank);
        let (texts, files) = lines.into_iter().map(|(_, text, file)| (text, file)).unzip();
        Changes::Described(texts, files)
    }
}

/// One changed file as the card lists it: what happens to the path and how many lines come and go.
#[derive(Debug, Clone, PartialEq)]
pub struct FileLine {
    /// `add`, `edit`, `delete` or `move`.
    pub verb: &'static str,
    /// The path, or `from \u{2192} to` for a move.
    pub path: String,
    /// Added and removed lines, counted from the change's own diff. `None` when the change carries no diff.
    pub counts: Option<(u32, u32)>,
}

/// Added and removed lines of a unified diff, or of a bare `+`/`-` listing. The `---`/`+++` file header of a full diff
/// (it comes before the first hunk) is not counted.
pub fn count_diff_lines(diff: &str) -> (u32, u32) {
    let mut lines = diff.lines().peekable();
    if lines.peek().is_some_and(|first| first.starts_with("--- ")) {
        lines.next();
        if lines.peek().is_some_and(|second| second.starts_with("+++ ")) {
            lines.next();
        }
    }
    lines.fold((0, 0), |(added, removed), line| match line.as_bytes().first() {
        Some(b'+') => (added + 1, removed),
        Some(b'-') => (added, removed + 1),
        _ => (added, removed),
    })
}

/// `(group, line, file)` of one change, or why it cannot be described.
fn describe_change(change: &Value) -> Result<(u8, String, FileLine), Changes> {
    let (Some(path), Some(kind)) = (change["path"].as_str(), change["kind"]["type"].as_str()) else { return Err(Changes::UnknownKind) };
    let target = change["kind"]["move_path"].as_str().filter(|target| !target.is_empty());
    let (group, verb, shown) = match (kind, target) {
        ("add", None) => (ADD, "add", path.to_string()),
        ("update", None) => (EDIT, "edit", path.to_string()),
        ("update", Some(target)) => (DESTRUCTIVE, "move", format!("{path} \u{2192} {target}")),
        ("delete", None) => (DESTRUCTIVE, "delete", path.to_string()),
        _ => return Err(Changes::UnknownKind),
    };
    if shown_as_it_is(path) && target.is_none_or(shown_as_it_is) {
        let counts = change["diff"].as_str().map(count_diff_lines);
        Ok((group, format!("{verb} {shown}"), FileLine { verb, path: shown, counts }))
    } else {
        Err(Changes::PathNotShownExactly)
    }
}

/// A path the card can show as it is: it reads as it is, and it does not contain what the card puts between changes or
/// between a path and its move target.
fn shown_as_it_is(path: &str) -> bool {
    reads_as_it_is(path) && !path.contains('\u{2192}') && !path.contains(", ")
}
