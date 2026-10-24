//! What the scan of shipped code looks for in a line: a file-system call, the name of a session file inside a string, and a shell command
//! that writes inside a string (a hook).

/// The names Studio gives the files of a session folder, each as a part of the name that is not likely anywhere else.
pub(super) const SESSION_NAMES: &[&str] = &[
    "agent-status.json",
    "agent-events.jsonl",
    "turn-marks.jsonl",
    "live-spec.json",
    "doc-sync.designos",
    "canvas.designos",
    "context.json",
    "doc-patch.jsonl",
    "mcp-config.json",
    "bootstrap.",
    "bootstrap-",
    "owner.json",
    "usage.json",
    "sessions.json",
    "session.lock",
    "run.json",
    "run.unreadable.json",
    "hooks.json",
    "claude-transcript.json",
    "image-jobs",
    ".gemini",
    ".grok",
    ".agents",
    ".claude",
    "AGENTS.md",
    "GEMINI.md",
    "SKILL.md",
    "selection.spec.json",
    "copy-for-ai.md",
    "page-context-reply.json",
    "skills-context-reply",
    // The constant that holds the one above; `derived_names` found a `join(format!("{REPLY_FILE_PREFIX}-..."))` that no name matched.
    "REPLY_FILE_PREFIX",
];

/// The commands of a shell that make, move, copy or remove a file or folder.
const SHELL_WRITERS: &[&str] = &["mkdir ", "rmdir ", "mv ", "cp ", "rm ", "touch ", "tee ", "ln "];

pub(super) fn has_fs_call(shape: &str) -> bool {
    const TOKENS: &[&str] = &[
        "File::open",
        "File::create",
        "File::options",
        "OpenOptions",
        "DirBuilder",
        ".read_to_string(",
        ".read_dir(",
        ".metadata(",
        ".symlink_metadata(",
        ".canonicalize(",
        ".read_link(",
        "write_atomic(",
        "write_owner_only_file(",
        "create_private_dir_all(",
    ];
    if TOKENS.iter().any(|token| shape.contains(token)) {
        return true;
    }
    shape.match_indices("fs::").any(|(at, _)| {
        let (before, after) = (&shape[..at], shape[at + 4..].chars().next());
        match before.chars().next_back() {
            _ if before.ends_with("std::") => true,
            Some(c) if c.is_alphanumeric() || c == '_' => false,
            Some(':') => after.is_some_and(char::is_lowercase),
            _ => true,
        }
    })
}

/// Whether `needle` is in `code` at `at`, with every one of its characters blanked in `shape`: inside a string, not in the code.
fn string_text_at(code: &[char], shape: &[char], at: usize, needle: &[char]) -> bool {
    code.get(at..at + needle.len()) == Some(needle) && shape[at..at + needle.len()].iter().all(|c| *c == ' ')
}

/// The session file names that are inside a string of the line.
pub(super) fn names_in_strings(code: &str, shape: &str) -> Vec<&'static str> {
    let (code, shape): (Vec<char>, Vec<char>) = (code.chars().collect(), shape.chars().collect());
    SESSION_NAMES
        .iter()
        .copied()
        .filter(|name| {
            let name: Vec<char> = name.chars().collect();
            (0..code.len()).any(|at| string_text_at(&code, &shape, at, &name))
        })
        .collect()
}

/// Whether a string of the line holds a shell command that writes: a redirect (`>` or `>>` after a blank), or one of `SHELL_WRITERS`.
/// A hook is such a string, a command the agent's CLI runs in the session folder, so it writes through a link the agent left there.
pub(super) fn shell_writes_in_strings(code: &str, shape: &str) -> bool {
    let (code, shape): (Vec<char>, Vec<char>) = (code.chars().collect(), shape.chars().collect());
    let redirect = (1..code.len()).any(|at| string_text_at(&code, &shape, at, &['>']) && code[at - 1].is_whitespace());
    let command = SHELL_WRITERS.iter().any(|word| {
        let word: Vec<char> = word.chars().collect();
        (0..code.len()).any(|at| string_text_at(&code, &shape, at, &word) && (at == 0 || !code[at - 1].is_alphanumeric()))
    });
    redirect || command
}
