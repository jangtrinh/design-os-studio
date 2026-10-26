//! How attached skills reach Claude Code through its `UserPromptSubmit` hook (`RD-4` §3.3, `#573`).
//!
//! Claude Code caps every hook `additionalContext` string at 10,000 characters. Past the cap it keeps the text in a file and hands
//! Claude a path and a 2,000 character preview, without asking Claude to read it. Studio lets a run attach up to 8,000 tokens
//! (about 32,000 characters), so the skills are cut into chunks of at most [`HOOK_CHUNK_CHARS`] and each chunk is one reply file
//! with one hook handler of its own: Claude receives every handler's `additionalContext`.
//!
//! - **Chunks**: skills that fit stay whole and share a chunk with their neighbours. A skill that cannot fit in one chunk is split
//!   into parts at line breaks, each part a fenced block headed `designos-skill:<name>@<version> part i/n`, so every chunk is
//!   valid on its own. A single line longer than a chunk is cut inside the line.
//! - **Slots**: the hook is written when a session is provisioned, so the number of handlers is fixed: [`HOOK_SKILL_SLOTS`], sized
//!   for the largest set Studio allows. A handler whose file does not exist prints nothing.
//! - **Per tab**: the file name carries the tab id (`skills-context-reply-<tab>-<slot>.json`) and each handler reads the id from
//!   `DESIGNOS_TAB_ID`, which Studio exports into every agent tab, so a prompt typed in another Claude tab never receives skills
//!   attached for this one.
//! - **Fallback**: a set that does not fit the slots (a huge name, a very long run of backticks, mostly non-BMP text) is not hook
//!   deliverable. [`chunk_skills`] then returns `None` and the caller pastes the fenced blocks like it does for every other CLI.

use super::{longest_backtick_run, CHARS_PER_TOKEN, MAX_COMBINED_TOKENS, MAX_SKILLS_PER_RUN};
use std::path::{Path, PathBuf};

/// Name every skills reply file starts with, so one prefix scan finds them all.
pub const REPLY_FILE_PREFIX: &str = "skills-context-reply";

/// Largest `additionalContext` Studio writes, in UTF-16 code units (what a JavaScript `.length` counts, so never fewer than the
/// characters). Claude Code's cap is 10,000; the rest is margin.
pub const HOOK_CHUNK_CHARS: usize = 9_500;

/// Room a part header reserves for its ` part i/n` suffix (two digits each side).
const PART_SUFFIX_RESERVE: usize = " part 99/99".len();

/// What a fenced block adds around its text in the worst realistic case: two 5-backtick fences, a 240 character info line with its
/// part suffix and 4 line breaks (the 2 inside the block and the 2 that separate it from its neighbour).
const BLOCK_OVERHEAD_ESTIMATE: usize = 2 * 5 + 240 + PART_SUFFIX_RESERVE + 4;

/// Number of skills handlers in the prompt hook, and so of chunks a Claude tab can have armed.
///
/// Sized from the limits the picker enforces: 8,000 tokens at [`CHARS_PER_TOKEN`] characters each, plus the up to 3 characters
/// each skill's token estimate rounds down. Each skill is one block, or parts that are full to within one line, plus the
/// overhead of every block. Chunks are filled in order, and two neighbouring chunks always hold more than one chunk of text
/// together (the second one's first block did not fit into the first), so `n` chunks need more than `n / 2` chunks of text.
/// One slot of margin on top.
pub const HOOK_SKILL_SLOTS: usize = {
    let body = MAX_COMBINED_TOKENS * CHARS_PER_TOKEN + MAX_SKILLS_PER_RUN * (CHARS_PER_TOKEN - 1);
    let blocks = MAX_SKILLS_PER_RUN + 2 * (body / (HOOK_CHUNK_CHARS - BLOCK_OVERHEAD_ESTIMATE)) + 1;
    let text = body + blocks * BLOCK_OVERHEAD_ESTIMATE;
    2 * ((text - 1) / HOOK_CHUNK_CHARS) + 1 + 1
};

/// One selected skill as the hook delivers it.
pub struct SkillText<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub body: &'a str,
}

/// Length of `text` in UTF-16 code units.
fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Units a fenced block takes: two fences one backtick longer than the longest run in the body, the info line and 2 line breaks.
fn block_units(info_units: usize, body_units: usize, longest_run: usize) -> usize {
    2 * (longest_run.max(2) + 1) + info_units + body_units + 2
}

/// The text of one skill's fenced blocks, cut into chunks of at most [`HOOK_CHUNK_CHARS`]; `Some(vec![])` for no skills. `None`
/// when the set cannot be delivered through the hook: a block that cannot fit a chunk at all, or more chunks than
/// [`HOOK_SKILL_SLOTS`].
pub fn chunk_skills(skills: &[SkillText<'_>]) -> Option<Vec<String>> {
    let mut blocks: Vec<String> = Vec::new();
    for skill in skills {
        let info = super::skill_info(skill.name, skill.version);
        let (info_units, body_units, run) = (units(&info), units(skill.body), longest_backtick_run(skill.body));
        if block_units(info_units, body_units, run) <= HOOK_CHUNK_CHARS {
            blocks.push(super::fenced_block(&info, skill.body));
            continue;
        }
        let parts = split_into_parts(info_units, skill.body)?;
        if parts.len() > 99 {
            return None;
        }
        let count = parts.len();
        blocks.extend(parts.iter().enumerate().map(|(i, part)| super::fenced_block(&format!("{info} part {}/{count}", i + 1), part)));
    }
    let mut chunks: Vec<String> = Vec::new();
    let mut used = 0;
    for block in blocks {
        let len = units(&block);
        match chunks.last_mut() {
            Some(last) if used + 2 + len <= HOOK_CHUNK_CHARS => {
                last.push_str("\n\n");
                last.push_str(&block);
                used += 2 + len;
            }
            _ => {
                chunks.push(block);
                used = len;
            }
        }
    }
    (chunks.len() <= HOOK_SKILL_SLOTS).then_some(chunks)
}

/// Cuts `body` into consecutive pieces that each fit one chunk as a fenced part block, at line breaks. A piece continues where
/// the one before it ended: after a line break, or inside a line when that single line is longer than a chunk. `None` when not even
/// one character fits next to the header.
fn split_into_parts(info_units: usize, body: &str) -> Option<Vec<&str>> {
    let fits = |body_units: usize, run: usize| block_units(info_units + PART_SUFFIX_RESERVE, body_units, run) <= HOOK_CHUNK_CHARS;
    let mut parts = Vec::new();
    let (mut start, mut end) = (0, 0);
    let (mut used, mut run, mut open) = (0, 0, false);
    let mut line_at = 0;
    for line in body.split('\n') {
        let (mut rest, mut rest_at) = (line, line_at);
        line_at += line.len() + 1;
        loop {
            let (rest_units, rest_run) = (units(rest), longest_backtick_run(rest));
            let joined = used + usize::from(open) + rest_units;
            if fits(joined, run.max(rest_run)) {
                if !open {
                    start = rest_at;
                }
                (end, used, run, open) = (rest_at + rest.len(), joined, run.max(rest_run), true);
                break;
            }
            if open {
                parts.push(&body[start..end]);
                (used, run, open) = (0, 0, false);
                continue;
            }
            let cut = longest_fitting_prefix(rest, &fits)?;
            parts.push(&body[rest_at..rest_at + cut]);
            (rest, rest_at) = (&rest[cut..], rest_at + cut);
        }
    }
    if open {
        parts.push(&body[start..end]);
    }
    Some(parts)
}

/// Byte length of the longest prefix of `text` that `fits`; `None` when not even its first character does.
fn longest_fitting_prefix(text: &str, fits: &impl Fn(usize, usize) -> bool) -> Option<usize> {
    let (mut used, mut run, mut longest, mut cut) = (0, 0, 0, 0);
    for (at, c) in text.char_indices() {
        let next_run = if c == '`' { run + 1 } else { 0 };
        if !fits(used + c.len_utf16(), longest.max(next_run)) {
            break;
        }
        (used, run, longest, cut) = (used + c.len_utf16(), next_run, longest.max(next_run), at + c.len_utf8());
    }
    (cut > 0).then_some(cut)
}

/// The reply file of chunk `slot` for agent tab `tab_id` in `session_dir`.
pub fn reply_file(session_dir: &Path, tab_id: usize, slot: usize) -> PathBuf {
    session_dir.join(format!("{REPLY_FILE_PREFIX}-{tab_id}-{slot}.json"))
}

/// The shell commands of the prompt hook's skills handlers, one per slot. Each prints its slot's file for the tab named by the
/// environment (`DESIGNOS_TAB_ID`), and nothing, with exit 0, while that file does not exist.
pub fn handler_commands(session_dir: &Path) -> Vec<String> {
    let prefix = crate::agent_turn_status::sh_single_quote(&format!("{}/{REPLY_FILE_PREFIX}-", session_dir.display()));
    (0..HOOK_SKILL_SLOTS).map(|slot| format!("f={prefix}\"${{DESIGNOS_TAB_ID:-0}}\"'-{slot}.json'; if [ -s \"$f\" ]; then cat \"$f\"; fi")).collect()
}

/// Writes `chunks` as the reply files of `tab_id`, one `additionalContext` each, and deletes the slots from `chunks.len()` up to
/// `previous` that an earlier, longer set left behind. `false` when a file could not be written.
pub fn write_replies(session_dir: &Path, tab_id: usize, chunks: &[String], previous: usize) -> bool {
    let mut all_written = true;
    for (slot, chunk) in chunks.iter().enumerate() {
        let reply = serde_json::json!({ "hookSpecificOutput": { "hookEventName": "UserPromptSubmit", "additionalContext": chunk } });
        let written = serde_json::to_vec_pretty(&reply).map_err(|e| e.to_string()).and_then(|bytes| {
            let path = reply_file(session_dir, tab_id, slot);
            crate::agent_session_files::atomic_write_session_file(&path, &bytes).map_err(|e| crate::agent_session_files::with_next_step(&e))
        });
        all_written &= written.is_ok();
    }
    remove_replies(session_dir, tab_id, chunks.len()..previous);
    all_written
}

/// Deletes the reply files of `tab_id` for the given `slots`.
pub fn remove_replies(session_dir: &Path, tab_id: usize, slots: std::ops::Range<usize>) {
    for slot in slots {
        let _ = crate::agent_session_files::remove_session_file(&reply_file(session_dir, tab_id, slot));
    }
}

/// Deletes every skills reply file in `session_dir`, whoever wrote it: one a run left behind when Studio quit before the prompt
/// hook ran must not reach a later prompt.
pub fn remove_all_replies(session_dir: &Path) {
    for entry in std::fs::read_dir(session_dir).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().starts_with(REPLY_FILE_PREFIX) {
            let _ = crate::agent_session_files::remove_session_file(&entry.path());
        }
    }
}

/// Names of the skills reply files in `session_dir`, sorted.
#[cfg(test)]
pub fn reply_file_names(session_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(session_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(REPLY_FILE_PREFIX))
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill<'a>(name: &'a str, body: &'a str) -> SkillText<'a> {
        SkillText { name, version: "1.0.0", body }
    }

    /// The fenced parts and whole skills of `chunks` as `(info, text)`, in order.
    fn blocks_of(chunks: &[String]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for chunk in chunks {
            let lines: Vec<&str> = chunk.split('\n').collect();
            let mut i = 0;
            while i < lines.len() {
                let fence: String = lines[i].chars().take_while(|c| *c == '`').collect();
                assert!(fence.len() >= 3 && lines[i][fence.len()..].starts_with("designos-skill:"), "a block starts at line {i}: {:?}", lines[i]);
                let close = (i + 1..lines.len()).find(|j| lines[*j] == fence).expect("every block closes its own fence");
                out.push((lines[i][fence.len()..].to_string(), lines[i + 1..close].join("\n")));
                i = close + 1;
                if i < lines.len() {
                    assert_eq!(lines[i], "", "blocks are separated by one blank line");
                    i += 1;
                }
            }
        }
        out
    }

    fn assert_each_chunk_fits(chunks: &[String]) {
        for chunk in chunks {
            assert!(units(chunk) <= HOOK_CHUNK_CHARS, "a chunk has {} units", units(chunk));
        }
    }

    #[test]
    fn small_skills_stay_whole_in_one_chunk_in_the_usual_block_format() {
        let chunks = chunk_skills(&[skill("one", "First body."), skill("two", "Second body.")]).expect("fits");
        assert_eq!(chunks, ["```designos-skill:one@1.0.0\nFirst body.\n```\n\n```designos-skill:two@1.0.0\nSecond body.\n```"]);
        assert_eq!(chunk_skills(&[]), Some(Vec::new()));
    }

    #[test]
    fn a_skill_over_one_chunk_is_split_at_line_breaks_into_labelled_parts_that_rebuild_it() {
        let body = (0..400).map(|i| format!("Line {i:03}: keep spacing on the 8px grid.")).collect::<Vec<_>>().join("\n");
        let chunks = chunk_skills(&[skill("big", &body)]).expect("fits the slots");

        assert!(chunks.len() >= 2);
        assert_each_chunk_fits(&chunks);
        let blocks = blocks_of(&chunks);
        let count = blocks.len();
        for (i, (info, _)) in blocks.iter().enumerate() {
            assert_eq!(info, &format!("designos-skill:big@1.0.0 part {}/{count}", i + 1));
        }
        let rebuilt = blocks.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>().join("\n");
        assert_eq!(rebuilt, body, "every byte of the body, in order");
    }

    #[test]
    fn a_line_longer_than_a_chunk_is_cut_inside_the_line_without_losing_a_character() {
        let long: String = "é😀x".repeat(7_000);
        let body = format!("before\n{long}\nafter");
        let chunks = chunk_skills(&[skill("long-line", &body)]).expect("fits the slots");

        assert_each_chunk_fits(&chunks);
        let blocks = blocks_of(&chunks);
        let rebuilt: String = blocks.iter().map(|(_, text)| text.as_str()).collect::<Vec<_>>().join("\n");
        assert_eq!(rebuilt.replace("\n", ""), body.replace("\n", ""), "no character is lost or invented");
        assert!(rebuilt.starts_with("before\n") && rebuilt.ends_with("\nafter"));
    }

    #[test]
    fn backticks_in_a_part_get_a_fence_that_outgrows_them_and_the_chunk_still_fits() {
        let code = "```rust\nlet a = 1;\n```";
        let filler = "Keep gutters on the 8px grid.\n".repeat(400);
        let body = format!("{filler}{code}\n{filler}");
        let chunks = chunk_skills(&[skill("with-code", &body)]).expect("fits the slots");

        assert_each_chunk_fits(&chunks);
        let rebuilt = blocks_of(&chunks).iter().map(|(_, text)| text.clone()).collect::<Vec<_>>().join("\n");
        assert_eq!(rebuilt, body);
        assert!(chunks.iter().any(|c| c.starts_with("````designos-skill:with-code@1.0.0")), "the part holding the code sample uses a longer fence");
    }

    /// Skill bodies at the picker's limits, shaped to waste as much of each chunk as line-by-line packing can.
    fn worst_case_sets() -> Vec<(String, Vec<String>)> {
        let max_chars = MAX_COMBINED_TOKENS * CHARS_PER_TOKEN;
        let lines_of = |line_len: usize, total: usize| -> String {
            let line = "x".repeat(line_len);
            let n = total / (line_len + 1);
            vec![line; n].join("\n")
        };
        let mut sets = Vec::new();
        for line_len in [1, 60, 2_400, 3_200, 4_700, 4_800, 6_000, 9_000] {
            sets.push((format!("one skill, {line_len} character lines"), vec![lines_of(line_len, max_chars)]));
            let third = max_chars / 3;
            sets.push((format!("three skills, {line_len} character lines"), vec![lines_of(line_len, third); 3]));
        }
        sets.push(("one 32,000 character line".into(), vec!["x".repeat(max_chars)]));
        sets.push(("two skills of 2 and 30 thousand".into(), vec![lines_of(4_700, 2_000), lines_of(4_700, 30_000)]));
        sets
    }

    #[test]
    fn the_slots_hold_the_largest_set_the_picker_allows_however_its_lines_fall() {
        let max_body = MAX_COMBINED_TOKENS * CHARS_PER_TOKEN;
        assert!(HOOK_SKILL_SLOTS > max_body.div_ceil(HOOK_CHUNK_CHARS), "at least one slot of margin over the plain division");
        assert_eq!(HOOK_SKILL_SLOTS, 8, "the handler count written into every session's hooks: change it on purpose");
        for (label, bodies) in worst_case_sets() {
            let names: Vec<String> = (0..bodies.len()).map(|i| format!("skill-{i}")).collect();
            let skills: Vec<SkillText<'_>> = bodies.iter().zip(&names).map(|(body, name)| skill(name, body)).collect();
            assert!(
                bodies.iter().map(|b| b.chars().count() / CHARS_PER_TOKEN).sum::<usize>() <= MAX_COMBINED_TOKENS,
                "{label}: the set is within the picker's limit"
            );
            let chunks = chunk_skills(&skills).unwrap_or_else(|| panic!("{label}: must fit the {HOOK_SKILL_SLOTS} slots"));
            assert!(chunks.len() <= HOOK_SKILL_SLOTS, "{label}: {} chunks", chunks.len());
            assert_each_chunk_fits(&chunks);
            let rebuilt: usize = blocks_of(&chunks).iter().map(|(_, text)| text.replace('\n', "").len()).sum();
            let wanted: usize = bodies.iter().map(|b| b.replace('\n', "").len()).sum();
            assert_eq!(rebuilt, wanted, "{label}: no byte lost");
        }
    }

    #[test]
    fn a_set_the_slots_cannot_hold_is_not_hook_deliverable() {
        // A body of nothing but backticks needs a fence as long as the body, so each part carries about a third of a chunk of
        // text and 32,000 of them need more chunks than there are slots: the caller pastes them instead.
        let backticks = "`".repeat(MAX_COMBINED_TOKENS * CHARS_PER_TOKEN);
        assert_eq!(chunk_skills(&[skill("backticks", &backticks)]), None);
        // A name that leaves no room for a single character in a part.
        let name = "n".repeat(HOOK_CHUNK_CHARS);
        let body = "x\n".repeat(HOOK_CHUNK_CHARS);
        assert_eq!(chunk_skills(&[skill(&name, &body)]), None);
    }

    #[test]
    fn every_slot_has_a_handler_that_prints_only_its_tab_and_slot_file() {
        let dir = std::env::temp_dir().join(format!("designos_hook_delivery_{}", std::process::id()));
        let dir = dir.join("a folder with spaces");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let commands = handler_commands(&dir);
        assert_eq!(commands.len(), HOOK_SKILL_SLOTS);
        let run = |command: &str, tab: Option<&str>| -> String {
            let mut sh = std::process::Command::new("sh");
            sh.arg("-c").arg(command).env_remove("DESIGNOS_TAB_ID");
            if let Some(tab) = tab {
                sh.env("DESIGNOS_TAB_ID", tab);
            }
            let out = sh.output().expect("run handler");
            assert!(out.status.success(), "a handler without a file exits 0: {command}");
            String::from_utf8_lossy(&out.stdout).into_owned()
        };

        assert!(commands.iter().all(|c| run(c, Some("2")).is_empty()), "no file, no output");
        let chunks = vec!["first".to_string(), "second".to_string()];
        assert!(write_replies(&dir, 2, &chunks, 0));
        assert!(run(&commands[0], Some("2")).contains("first") && run(&commands[1], Some("2")).contains("second"));
        assert!(commands[2..].iter().all(|c| run(c, Some("2")).is_empty()), "an unused slot prints nothing");
        assert!(commands.iter().all(|c| run(c, Some("3")).is_empty() && run(c, None).is_empty()), "another tab, or a tab-less process, gets nothing");

        // A shorter set removes the slots the longer one used.
        assert!(write_replies(&dir, 2, &chunks[..1], 2));
        assert_eq!(reply_file_names(&dir), ["skills-context-reply-2-0.json"]);
        remove_all_replies(&dir);
        assert!(reply_file_names(&dir).is_empty());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
