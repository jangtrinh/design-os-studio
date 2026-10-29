//! What the scan of shipped code looks for in a line: a file-system call, the name of a session file inside a string, a shell command
//! that writes inside a string (a hook), and a program that writes files started with `Command::new`.

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
    "leave-asking",
    "close-result.json",
    "close-cancel.json",
    "mcp-config.json",
    "mcp_config.json",
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
    "agent-sessions",
    "input.png",
    "output.png",
    "request.json",
    "mask.png",
    "plugin.json",
    "designos-status.json",
    "trustedFolders.json",
    "config.toml",
    "selection.spec.json",
    ".gemini",
    ".grok",
    ".agents",
    ".claude",
    ".cursor",
    ".mcp.json",
    "AGENTS.md",
    "GEMINI.md",
    "SKILL.md",
    "copy-for-ai.md",
    "page-context-reply.json",
    "skills-context-reply",
    // The constant that holds the one above; `derived_names` found a `join(format!("{REPLY_FILE_PREFIX}-..."))` that no name matched.
    "REPLY_FILE_PREFIX",
];

/// Names inside the folders above (`.agents/plugins/designos/plugin.json`, `.gemini/settings.json`, `.grok/hooks/..`) that are too common to
/// flag a line by themselves, which every line naming them also names a folder above. A `.join` to a session path may use them.
pub(super) const SUBFOLDER_NAMES: &[&str] = &["plugins", "designos", "settings.json", "hooks"];

/// The commands of a shell that make, move, copy, shorten or remove a file or folder.
const SHELL_WRITERS: &[&str] = &["mkdir", "rmdir", "mv", "cp", "rm", "touch", "tee", "ln", "truncate"];

/// The programs that write files, which `Command::new` starts by name. `sh` and `bash` run whatever their `-c` says.
const FILE_WRITERS: &[&str] = &["cp", "mv", "ln", "rm", "tee", "touch", "truncate", "dd", "install", "mkdir", "rmdir", "sh", "bash"];

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

/// A character a shell command word cannot touch: `rm-`, `-mv` and `pop-rm-action` are names, not commands.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-')
}

/// Whether `word` stands in a string of the line on its own, between characters that are not part of a word.
fn word_in_strings(code: &[char], shape: &[char], word: &str) -> bool {
    let word: Vec<char> = word.chars().collect();
    (0..code.len()).any(|at| {
        string_text_at(code, shape, at, &word) && (at == 0 || !is_word_char(code[at - 1])) && code.get(at + word.len()).is_none_or(|next| !is_word_char(*next))
    })
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

/// A `>` in a string that opens a redirect: any `>` that is not part of `->`, `=>`, an angle pair (`<b>`, `Vec<u8>`) or the character `'>'`,
/// and does not write to `/dev/null` or to another descriptor (`2>&1`).
/// That takes in `x>`, `3>`, `&>`, `>>`, `2>>` and a `>` that starts the string.
fn redirect_in_strings(code: &[char], shape: &[char]) -> bool {
    let mut open_angles = 0;
    for at in 0..code.len() {
        if shape[at] != ' ' {
            open_angles = 0; // outside a string: the next string starts afresh
            continue;
        }
        match code[at] {
            '<' => open_angles += 1,
            '>' if open_angles > 0 => open_angles -= 1,
            '>' => {
                let (before, after) = (at.checked_sub(1).map(|b| code[b]), code.get(at + 1).copied());
                let arrow = matches!(before, Some('-' | '='));
                let character = before == Some('\'') && after == Some('\'');
                // What the redirect writes to: `2>/dev/null` and `> /dev/null` write nothing, and `2>&1` only joins two descriptors.
                let target: String = code[at + 1..].iter().skip_while(|c| matches!(**c, '>' | ' ')).take(9).collect();
                let writes_nothing = target == "/dev/null" || (target.starts_with('&') && target[1..].starts_with(|c: char| c.is_ascii_digit()));
                if !arrow && !character && !writes_nothing {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Whether a string of the line holds a shell command that writes: a redirect, or one of `SHELL_WRITERS`.
/// A hook is such a string, a command the agent's CLI runs in the session folder, so it writes through a link the agent left there.
pub(super) fn shell_writes_in_strings(code: &str, shape: &str) -> bool {
    let (code, shape): (Vec<char>, Vec<char>) = (code.chars().collect(), shape.chars().collect());
    redirect_in_strings(&code, &shape) || SHELL_WRITERS.iter().any(|word| word_in_strings(&code, &shape, word))
}

/// Whether the line starts a program that writes files with `Command::new("<program>")` (a path to it, or `.exe`, counts).
/// A program named by a variable is not seen.
pub(super) fn command_of_file_writer(code: &str, shape: &str) -> bool {
    let (code, shape): (Vec<char>, Vec<char>) = (code.chars().collect(), shape.chars().collect());
    ["Command::new(", "CommandBuilder::new("].iter().any(|call| {
        let call: Vec<char> = call.chars().collect();
        (0..shape.len()).filter(|at| shape.get(*at..*at + call.len()) == Some(call.as_slice())).any(|at| {
            let rest: String = code[at + call.len()..].iter().collect();
            let Some(literal) = rest.trim_start().trim_start_matches('&').strip_prefix('"').and_then(|r| r.split('"').next()) else { return false };
            let program = literal.rsplit(['/', '\\']).next().unwrap_or(literal);
            FILE_WRITERS.contains(&program.strip_suffix(".exe").unwrap_or(program))
        })
    })
}
