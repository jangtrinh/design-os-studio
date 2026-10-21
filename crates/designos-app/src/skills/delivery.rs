//! Which skills a send attaches to an agent tab, and when (`RD-4` §3.3: "per run", "on run start").
//!
//! A run is one agent tab. Its first composer send attaches the selected skills; later sends attach nothing, so a long session
//! does not repeat up to 8,000 tokens in every prompt. Choosing a different set (a skill added, removed or edited) starts a new
//! run: the whole new set attaches again with the next send and a new entry is appended to `run.json`. A send that attaches no
//! skill at all starts no run and leaves `run.json` alone.

use super::SkillsAttachmentState;
use crate::terminal::TerminalTab;
use std::path::Path;

/// The skills one send attaches: their fenced blocks, how they are delivered, and the fingerprint of the set they came from.
pub(crate) struct SkillsAttachment {
    fingerprint: u64,
    /// Fenced `designos-skill:<name>@<version>` blocks; empty when the selected set is empty.
    block: String,
    /// The chunks a Claude Code tab reads from its prompt hook, one reply file each (`hook_delivery`). Empty for every other CLI,
    /// and for a set the hook cannot carry, which is pasted instead.
    pub hook_chunks: Vec<String>,
}

impl SkillsAttachment {
    /// The fenced blocks to paste in front of the prompt: none when the Claude hook carries them.
    pub fn paste_block(&self) -> &str {
        if self.hook_chunks.is_empty() {
            &self.block
        } else {
            ""
        }
    }
}

/// What a composer send attaches to `tab`: the selected skills when it is the tab's first send or the selected set changed
/// since the last attach, nothing otherwise. Call after the selected files were re-read and the limit was checked.
pub(crate) fn plan_attachment(skills: &SkillsAttachmentState, tab: &TerminalTab) -> Option<SkillsAttachment> {
    let fingerprint = skills.selection_fingerprint();
    (tab.composer.skills_sent != Some(fingerprint)).then(|| SkillsAttachment {
        fingerprint,
        block: skills.format_fenced_blocks(),
        hook_chunks: if tab.reads_skills_from_hook() { skills.hook_chunks().unwrap_or_default() } else { Vec::new() },
    })
}

/// Records that `plan` was written to `tab`: the set is delivered, a Claude Code tab holds its chunks for the skills hook replies
/// until its first prompt is submitted, and the run is appended to `run.json`. A send that attaches no skill is no run and
/// leaves `run.json` alone. Only call once the send reached the PTY.
pub(crate) fn commit_attachment(
    skills: &SkillsAttachmentState,
    tab: &mut TerminalTab,
    plan: &SkillsAttachment,
    session_dir: &Path,
    session_id: &str,
) -> Result<(), String> {
    tab.composer.skills_sent = Some(plan.fingerprint);
    tab.composer.skills_hook_chunks = plan.hook_chunks.clone();
    tab.composer.skills_hook_submitted_at = None;
    if plan.block.is_empty() {
        return Ok(());
    }
    skills.write_run_record(session_dir, session_id, tab.id, tab.skills_runtime_label()).map(|_| ())
}
