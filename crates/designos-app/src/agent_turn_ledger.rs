//! Per-tab ledger of agent canvas turns (`#677`).
//!
//! Groups live patches replayed from `doc-patch.jsonl` into per-turn records using byte offsets
//! written by CLI hooks in `turn-marks.jsonl` (or a heuristic silence window for CLIs without
//! hooks), and reverts an entire turn in one undoable step while preserving layers the user
//! edited after the turn.

use crate::agent_turn_marks::{TurnMarkLine, TurnPhase};
use crate::edit::Tx;
use crate::home::node_fingerprint;
use crate::live_patch_apply::relayout_touched;
pub use designos_doc::live_patch::TouchKind;
use designos_doc::{Command, DocError, Document, History, Node, NodeId};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

pub const MAX_TURNS: usize = 10;
pub const MAX_LEDGER_BYTES: usize = 64 << 20; // 64 MiB
pub const HEURISTIC_GRACE_WINDOW: Duration = Duration::from_secs(2);
pub const REVERT_NOTICE_DURATION: Duration = Duration::from_secs(4);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TouchedLayerSummary {
    pub id: NodeId,
    pub name: String,
    pub kind: TouchKind,
    /// Fingerprint of the layer's properties after the agent's last write in this turn,
    /// or `None` when the layer's final state in this turn is `Removed`.
    pub post_fingerprint: Option<u64>,
    /// Children present on this layer after the agent's last write in this turn.
    pub post_children: Vec<NodeId>,
}

#[derive(Clone, Debug)]
pub struct LedgerStep {
    pub patch_offset: u64,
    /// Atomic inverse commands in reverse execution order, each paired with the editor-side
    /// `NodeId` it targets (`None` for document-wide commands like `SetComments`).
    pub atomic_inverses: Vec<(Option<NodeId>, Command)>,
    pub touched: Vec<(NodeId, String, TouchKind)>,
    /// Fingerprint of each affected node immediately before this step was applied.
    pub pre_fingerprints: BTreeMap<NodeId, Option<u64>>,
    /// Fingerprint of each affected node immediately after this step was applied.
    pub post_fingerprints: BTreeMap<NodeId, Option<u64>>,
    pub approx_bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnState {
    Active,
    Completed,
    Kept,
    Reverted,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RevertReport {
    pub reverted_steps: usize,
    /// Layers skipped because the user modified them after the turn ended (`(id, name)`).
    pub kept_user_changed: Vec<(NodeId, String)>,
    /// Layers skipped because they were already absent from the document.
    pub already_deleted: Vec<NodeId>,
    /// Extra children the user added inside an agent-inserted frame that were removed with the frame.
    pub removed_with_frame: Vec<(NodeId, String)>,
}

#[derive(Clone, Debug)]
pub struct TurnRecord {
    pub id: u64,
    pub cli: Option<String>,
    pub start_patch_bytes: u64,
    pub end_patch_bytes: Option<u64>,
    pub steps: Vec<LedgerStep>,
    pub touched_layers: Vec<TouchedLayerSummary>,
    pub inserted_count: usize,
    pub updated_count: usize,
    pub moved_count: usize,
    pub removed_count: usize,
    pub approximate: bool,
    pub state: TurnState,
    pub expanded: bool,
    pub last_revert_report: Option<RevertReport>,
    pub reverted_at: Option<Instant>,
    pub created_at: Instant,
    pub last_patch_at: Instant,
    pub ended_at: Option<Instant>,
    pub approx_bytes: usize,
}

impl TurnRecord {
    fn new(id: u64, cli: Option<String>, start_patch_bytes: u64, approximate: bool, now: Instant) -> Self {
        Self {
            id,
            cli,
            start_patch_bytes,
            end_patch_bytes: None,
            steps: Vec::new(),
            touched_layers: Vec::new(),
            inserted_count: 0,
            updated_count: 0,
            moved_count: 0,
            removed_count: 0,
            approximate,
            state: TurnState::Active,
            expanded: false,
            last_revert_report: None,
            reverted_at: None,
            created_at: now,
            last_patch_at: now,
            ended_at: None,
            approx_bytes: std::mem::size_of::<Self>(),
        }
    }

    pub fn cli_display_name<'a>(&'a self, fallback: &'a str) -> &'a str {
        match self.cli.as_deref() {
            Some("claude") => "Claude",
            Some("codex") => "Codex",
            Some("grok") => "Grok",
            Some("agy" | "antigravity") => "Antigravity",
            Some(other) if !other.is_empty() => other,
            _ => fallback,
        }
    }

    pub fn headline(&self, fallback_cli: &str) -> String {
        format!("{} changed layers", self.cli_display_name(fallback_cli))
    }

    pub fn total_edits(&self) -> usize {
        self.inserted_count + self.updated_count + self.moved_count + self.removed_count
    }

    pub fn has_edits(&self) -> bool {
        !self.steps.is_empty() && self.total_edits() > 0
    }

    /// Formats the header summary:
    /// `N canvas edits — I inserted · U updated · M moved · R removed` (omitting zero buckets).
    pub fn header_summary(&self) -> String {
        let total = self.total_edits();
        let noun = if total == 1 { "canvas edit" } else { "canvas edits" };
        let mut buckets = Vec::with_capacity(4);
        if self.inserted_count > 0 {
            buckets.push(format!("{} inserted", self.inserted_count));
        }
        if self.updated_count > 0 {
            buckets.push(format!("{} updated", self.updated_count));
        }
        if self.moved_count > 0 {
            buckets.push(format!("{} moved", self.moved_count));
        }
        if self.removed_count > 0 {
            buckets.push(format!("{} removed", self.removed_count));
        }
        if buckets.is_empty() {
            format!("{total} {noun}")
        } else {
            format!("{total} {noun} — {}", buckets.join(" · "))
        }
    }
}

#[derive(Debug)]
pub struct AgentTurnLedger {
    pub turns: Vec<TurnRecord>,
    pub evicted_turn_ids: BTreeSet<u64>,
    next_id: u64,
    pub max_turns: usize,
    pub max_bytes: usize,
    /// When the user reverts a still-open hooked turn mid-stream, remembers its start offset so
    /// subsequent patches before `End` open a fresh non-approximate continuation turn.
    open_hook_continuation: Option<u64>,
    last_working: bool,
}

impl Default for AgentTurnLedger {
    fn default() -> Self {
        Self {
            turns: Vec::new(),
            evicted_turn_ids: BTreeSet::new(),
            next_id: 1,
            max_turns: MAX_TURNS,
            max_bytes: MAX_LEDGER_BYTES,
            open_hook_continuation: None,
            last_working: false,
        }
    }
}

impl AgentTurnLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_bytes(&self) -> usize {
        self.turns.iter().map(|t| t.approx_bytes).sum()
    }

    pub fn can_revert(&self, turn_id: u64) -> bool {
        !self.evicted_turn_ids.contains(&turn_id) && self.turns.iter().any(|t| t.id == turn_id && t.has_edits() && t.state != TurnState::Reverted)
    }

    pub fn revert_disabled_reason(&self, turn_id: u64) -> Option<&'static str> {
        if self.evicted_turn_ids.contains(&turn_id) {
            return Some("Too old to revert");
        }
        match self.turns.iter().find(|t| t.id == turn_id) {
            Some(t) if t.state == TurnState::Reverted => Some("Already reverted"),
            Some(t) if !t.has_edits() => Some("No canvas changes in this turn"),
            Some(_) => None,
            None => Some("Too old to revert"),
        }
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn dismiss_completed_turns_with_edits(&mut self) {
        for t in &mut self.turns {
            if t.has_edits() && matches!(t.state, TurnState::Completed | TurnState::Reverted) {
                t.state = TurnState::Kept;
            }
        }
    }

    /// Applies `Start` / `End` marks read from `<session_dir>/turn-marks.jsonl`.
    pub fn ingest_marks(&mut self, marks: impl IntoIterator<Item = TurnMarkLine>, now: Instant) {
        for mark in marks {
            match mark.turn_mark {
                TurnPhase::Start => {
                    self.open_hook_continuation = None;
                    if let Some(last) = self.turns.last_mut() {
                        if last.end_patch_bytes.is_none() {
                            if last.steps.is_empty() && last.start_patch_bytes == mark.patch_bytes {
                                last.cli = mark.cli;
                                last.approximate = false;
                                last.state = TurnState::Active;
                                last.created_at = now;
                                continue;
                            }
                            if last.approximate && last.steps.first().is_some_and(|s| s.patch_offset >= mark.patch_bytes) {
                                last.cli = mark.cli;
                                last.start_patch_bytes = mark.patch_bytes;
                                last.approximate = false;
                                continue;
                            }
                            last.end_patch_bytes = Some(mark.patch_bytes);
                            if last.state == TurnState::Active {
                                last.state = TurnState::Completed;
                            }
                            last.ended_at.get_or_insert(now);
                        }
                    }
                    self.dismiss_completed_turns_with_edits();
                    let id = self.alloc_id();
                    self.turns.push(TurnRecord::new(id, mark.cli, mark.patch_bytes, false, now));
                    self.enforce_bounds();
                }
                TurnPhase::End => {
                    self.open_hook_continuation = None;
                    if let Some(pos) = self.turns.iter().rposition(|t| t.end_patch_bytes.is_none()) {
                        let turn = &mut self.turns[pos];
                        if turn.cli.is_none() {
                            turn.cli = mark.cli;
                        }
                        turn.end_patch_bytes = Some(mark.patch_bytes);
                        turn.ended_at = Some(now);
                        if turn.state == TurnState::Active {
                            turn.state = TurnState::Completed;
                        }
                        // A chat-only turn whose `[start, end)` byte range in `doc-patch.jsonl` is empty
                        // produced zero patches and should not leave an empty record around.
                        if turn.steps.is_empty() && mark.patch_bytes <= turn.start_patch_bytes {
                            self.turns.remove(pos);
                        }
                    }
                }
            }
        }
    }

    /// Handles heuristic status transitions (`Idle`/`Done` <-> `Working`) for CLIs without hooks
    /// (such as Codex), opening an approximate turn when `Working` begins and closing it when the
    /// tab returns to `Done`/`Idle`.
    pub fn on_status_change(&mut self, is_working: bool, current_patch_bytes: u64, now: Instant) {
        let was_working = self.last_working;
        self.last_working = is_working;
        if !was_working && is_working {
            let has_open_turn = self.turns.last().is_some_and(|t| t.end_patch_bytes.is_none());
            if !has_open_turn {
                self.dismiss_completed_turns_with_edits();
                let id = self.alloc_id();
                self.turns.push(TurnRecord::new(id, None, current_patch_bytes, true, now));
                self.enforce_bounds();
            }
        } else if was_working && !is_working {
            if let Some(pos) = self.turns.len().checked_sub(1) {
                if self.turns[pos].approximate && self.turns[pos].end_patch_bytes.is_none() {
                    if self.turns[pos].steps.is_empty() && current_patch_bytes <= self.turns[pos].start_patch_bytes {
                        self.turns.remove(pos);
                    } else {
                        let turn = &mut self.turns[pos];
                        let end_offset = turn.steps.last().map_or(current_patch_bytes, |s| (s.patch_offset + 1).max(current_patch_bytes));
                        turn.end_patch_bytes = Some(end_offset);
                        turn.state = TurnState::Completed;
                        turn.ended_at = Some(now);
                    }
                }
            }
        }
    }

    /// Closes any open turn when the agent tab process exits without writing an end mark (Edge case 7).
    pub fn on_tab_exited(&mut self, current_patch_bytes: u64, now: Instant) {
        self.last_working = false;
        self.open_hook_continuation = None;
        if let Some(pos) = self.turns.len().checked_sub(1) {
            if self.turns[pos].end_patch_bytes.is_none() {
                if self.turns[pos].steps.is_empty() && current_patch_bytes <= self.turns[pos].start_patch_bytes {
                    self.turns.remove(pos);
                } else {
                    let turn = &mut self.turns[pos];
                    let end_offset = turn.steps.last().map_or(current_patch_bytes, |s| (s.patch_offset + 1).max(current_patch_bytes));
                    turn.end_patch_bytes = Some(end_offset);
                    if turn.state == TurnState::Active {
                        turn.state = TurnState::Completed;
                    }
                    turn.ended_at = Some(now);
                }
            }
        }
    }

    /// Closes open heuristic turns once the agent leaves `Working` or after 2 s of patch silence.
    pub fn tick(&mut self, is_working: bool, now: Instant) {
        let approx_bytes = self.turns.last().and_then(|t| t.steps.last().map(|s| s.patch_offset + 1).or(Some(t.start_patch_bytes))).unwrap_or(0);
        self.on_status_change(is_working, approx_bytes, now);
        if let Some(last) = self.turns.last_mut() {
            if last.approximate && last.end_patch_bytes.is_none() && last.state == TurnState::Active && !last.steps.is_empty() {
                let idle = now.saturating_duration_since(last.last_patch_at);
                if (!is_working && idle >= Duration::from_millis(500)) || idle >= HEURISTIC_GRACE_WINDOW {
                    let end_offset = last.steps.last().map_or(last.start_patch_bytes, |s| s.patch_offset + 1);
                    last.end_patch_bytes = Some(end_offset);
                    last.state = TurnState::Completed;
                    last.ended_at = Some(now);
                }
            }
        }
    }

    /// Drops turn steps from a previous document when `DocEpoch` switches documents mid-session.
    pub fn on_doc_epoch(&mut self, now: Instant) {
        let open_hook = self.turns.last().filter(|t| t.end_patch_bytes.is_none() && !t.approximate).map(|t| (t.cli.clone(), t.start_patch_bytes));
        for t in &self.turns {
            self.evicted_turn_ids.insert(t.id);
        }
        self.turns.clear();
        if let Some((cli, start)) = open_hook {
            let id = self.alloc_id();
            self.turns.push(TurnRecord::new(id, cli, start, false, now));
        }
    }

    /// Records one applied patch step into the appropriate [`TurnRecord`].
    #[allow(clippy::too_many_arguments)]
    pub fn record_step(
        &mut self,
        doc_after: &Document,
        patch_offset: u64,
        committed_inverse: Command,
        touched: &[(NodeId, TouchKind)],
        pre_names: &BTreeMap<NodeId, String>,
        pre_fingerprints: BTreeMap<NodeId, Option<u64>>,
        now: Instant,
    ) {
        let mut atomic_inverses = Vec::new();
        flatten_inverses(committed_inverse, &mut atomic_inverses);
        if atomic_inverses.is_empty() && touched.is_empty() {
            return;
        }

        let mut pre_fingerprints = pre_fingerprints;
        for (tid, inv) in &atomic_inverses {
            match inv {
                Command::Remove { id } => {
                    pre_fingerprints.insert(*id, None);
                }
                Command::Update { node } => {
                    pre_fingerprints.entry(node.id).or_insert_with(|| Some(node_fingerprint(node)));
                }
                Command::Insert { nodes, .. } => {
                    if let Some(root) = nodes.first() {
                        pre_fingerprints.entry(root.id).or_insert_with(|| Some(node_fingerprint(root)));
                    }
                }
                _ => {
                    if let Some(id) = *tid {
                        pre_fingerprints.entry(id).or_insert(None);
                    }
                }
            }
        }
        for &(id, kind) in touched {
            if kind == TouchKind::Inserted {
                pre_fingerprints.insert(id, None);
            }
        }

        let mut post_fingerprints = BTreeMap::new();
        for &id in pre_fingerprints.keys() {
            post_fingerprints.insert(id, doc_after.get(id).map(node_fingerprint));
        }
        for &(id, _) in touched {
            post_fingerprints.entry(id).or_insert_with(|| doc_after.get(id).map(node_fingerprint));
        }

        let mut step_touched = Vec::with_capacity(touched.len());
        for &(id, kind) in touched {
            let name = doc_after
                .get(id)
                .map(|n| n.name.clone())
                .or_else(|| pre_names.get(&id).cloned())
                .or_else(|| name_from_inverses(&atomic_inverses, id))
                .unwrap_or_else(|| format!("Layer {}", id.0));
            step_touched.push((id, name, kind));
        }

        let approx_bytes = std::mem::size_of::<LedgerStep>()
            + atomic_inverses.iter().map(|(_, c)| approx_command_bytes(c)).sum::<usize>()
            + step_touched.iter().map(|(_, n, _)| 24 + n.len()).sum::<usize>()
            + (pre_fingerprints.len() + post_fingerprints.len()) * 24;

        let step = LedgerStep { patch_offset, atomic_inverses, touched: step_touched, pre_fingerprints, post_fingerprints, approx_bytes };

        let Some(turn_idx) = self.select_or_open_turn(patch_offset, now) else {
            // Patch arrived after the last closed turn without any open turn or 2 s grace window.
            return;
        };
        // Rule 7: when a newer turn receives edits, dismiss any earlier unreviewed turn as Keep.
        for earlier in &mut self.turns[..turn_idx] {
            if earlier.has_edits() && matches!(earlier.state, TurnState::Active | TurnState::Completed | TurnState::Reverted) {
                earlier.state = TurnState::Kept;
            }
        }
        let turn = &mut self.turns[turn_idx];

        // Edge case 2: if the user edited a layer `id` during the turn (between an earlier agent
        // step in this turn and the current agent step), `pre_fingerprints[id]` before this step
        // will differ from `summary.post_fingerprint` recorded after the earlier step. Drop the
        // earlier steps' inverses for `id` so reverting this turn restores the state right before
        // the agent's latest write (preserving the user's mid-turn edit).
        for &(id, _, _) in &step.touched {
            if let Some(summary) = turn.touched_layers.iter().find(|s| s.id == id) {
                if let Some(&pre_fp) = step.pre_fingerprints.get(&id) {
                    if pre_fp != summary.post_fingerprint {
                        for prev_step in &mut turn.steps {
                            prev_step.atomic_inverses.retain(|(tid, _)| *tid != Some(id));
                        }
                    }
                }
            }
        }

        for (id, name, kind) in &step.touched {
            match kind {
                TouchKind::Inserted => turn.inserted_count += 1,
                TouchKind::Updated => turn.updated_count += 1,
                TouchKind::Moved => turn.moved_count += 1,
                TouchKind::Removed => turn.removed_count += 1,
            }
            let post_fp = if *kind == TouchKind::Removed { None } else { doc_after.get(*id).map(node_fingerprint) };
            let post_children = doc_after.get(*id).map(|n| n.children.clone()).unwrap_or_default();
            if let Some(existing) = turn.touched_layers.iter_mut().find(|s| s.id == *id) {
                existing.name = name.clone();
                existing.kind = merge_touch_kind(existing.kind, *kind);
                existing.post_fingerprint = post_fp;
                existing.post_children = post_children;
            } else {
                turn.touched_layers.push(TouchedLayerSummary { id: *id, name: name.clone(), kind: *kind, post_fingerprint: post_fp, post_children });
            }
        }

        turn.last_patch_at = now;
        turn.approx_bytes += step.approx_bytes;
        turn.steps.push(step);
        self.enforce_bounds();
    }

    fn select_or_open_turn(&mut self, patch_offset: u64, now: Instant) -> Option<usize> {
        // 1. Exact hooked turn whose `[start_patch_bytes, end_patch_bytes)` covers `patch_offset`.
        if let Some(idx) = self.turns.iter().rposition(|t| {
            !t.approximate && t.state != TurnState::Reverted && t.start_patch_bytes <= patch_offset && t.end_patch_bytes.is_none_or(|end| patch_offset < end)
        }) {
            return Some(idx);
        }

        // 2. Continuation of an open hooked turn that the user reverted mid-turn (Edge case 7).
        if self.open_hook_continuation.is_some() {
            let id = self.alloc_id();
            self.turns.push(TurnRecord::new(id, None, patch_offset, false, now));
            return Some(self.turns.len() - 1);
        }

        // 3. Late patch arriving within `HEURISTIC_GRACE_WINDOW` (2 s) after a closed turn:
        //    - For a heuristic turn (`approximate == true`), attach late patches within 2 s (Rule 3).
        //    - For a hooked turn (`approximate == false`), if `patch_offset >= end_patch_bytes` and no
        //      new turn has started yet, it fell outside the closed hooked turn's `[start, end)` range
        //      (Rule 1: `marks_assign_patches_by_byte_offset`), so it belongs to NO turn (`None`).
        if let Some(idx) = self.turns.len().checked_sub(1) {
            if !self.turns[idx].approximate && self.turns[idx].end_patch_bytes.is_some_and(|end| patch_offset >= end) {
                return None;
            }
            let last = &mut self.turns[idx];
            if last.approximate
                && last.state == TurnState::Completed
                && last.ended_at.is_some_and(|ended| now.saturating_duration_since(ended) <= HEURISTIC_GRACE_WINDOW)
            {
                last.end_patch_bytes = Some(patch_offset + 1);
                return Some(idx);
            }
        }

        // 4. Active heuristic turn within the 2 s inactivity window.
        if let Some(idx) = self.turns.len().checked_sub(1) {
            let last = &mut self.turns[idx];
            if last.approximate && last.end_patch_bytes.is_none() && last.state == TurnState::Active {
                if last.steps.is_empty() || now.saturating_duration_since(last.last_patch_at) < HEURISTIC_GRACE_WINDOW {
                    return Some(idx);
                }
                let prev_end = last.steps.last().map_or(patch_offset, |s| s.patch_offset + 1);
                last.end_patch_bytes = Some(prev_end);
                last.state = TurnState::Completed;
                last.ended_at = Some(now);
            }
        }

        // 5. Open a new heuristic turn (`approximate: true`).
        self.dismiss_completed_turns_with_edits();
        let id = self.alloc_id();
        self.turns.push(TurnRecord::new(id, None, patch_offset, true, now));
        Some(self.turns.len() - 1)
    }

    fn enforce_bounds(&mut self) {
        while self.turns.len() > self.max_turns || (self.turns.len() > 1 && self.total_bytes() > self.max_bytes) {
            let dropped = self.turns.remove(0);
            self.evicted_turn_ids.insert(dropped.id);
        }
    }

    /// The turn currently shown in the review card at the bottom of the agent panel, if any.
    /// Returns the most recent turn with ≥ 1 applied edit unless it has been dismissed (`Kept`).
    pub fn visible_turn(&self, _now: Instant) -> Option<&TurnRecord> {
        let turn = self.turns.iter().rev().find(|t| t.has_edits())?;
        match turn.state {
            TurnState::Active | TurnState::Completed | TurnState::Reverted => Some(turn),
            TurnState::Kept => None,
        }
    }

    pub fn visible_turn_mut(&mut self, now: Instant) -> Option<&mut TurnRecord> {
        let id = self.visible_turn(now)?.id;
        self.turns.iter_mut().find(|t| t.id == id)
    }

    /// Marks `turn_id` as kept and dismisses its review card without mutating `doc` or `hist`.
    pub fn keep_turn(&mut self, turn_id: u64) -> bool {
        let Some(turn) = self.turns.iter_mut().find(|t| t.id == turn_id) else {
            return false;
        };
        turn.state = TurnState::Kept;
        true
    }

    /// Reverts all steps of `turn_id` in reverse order inside one [`Tx`] so the entire turn revert
    /// is a single undo step on `hist`. Any layer the user modified after the turn is preserved
    /// and reported in [`RevertReport::kept_user_changed`].
    pub fn revert_turn(&mut self, doc: &mut Document, hist: &mut History, turn_id: u64) -> Result<RevertReport, DocError> {
        let Some(pos) = self.turns.iter().position(|t| t.id == turn_id) else {
            return Ok(RevertReport::default());
        };
        let turn = &self.turns[pos];
        let mut report = RevertReport::default();
        let mut skip_all_steps_for: BTreeSet<NodeId> = BTreeSet::new();
        // Map from `NodeId` to the step index `k` from which steps `k..` were already undone via Cmd+Z.
        let mut already_undone_from_step: BTreeMap<NodeId, usize> = BTreeMap::new();

        for summary in &turn.touched_layers {
            let id = summary.id;
            let cur_fp = doc.get(id).map(node_fingerprint);
            if cur_fp == summary.post_fingerprint {
                if summary.kind == TouchKind::Inserted {
                    if let Some(cur) = doc.get(id) {
                        let expected: BTreeSet<NodeId> = summary.post_children.iter().copied().collect();
                        for &cid in &cur.children {
                            if !expected.contains(&cid) {
                                let cname = doc.get(cid).map(|n| n.name.clone()).unwrap_or_else(|| format!("Layer {}", cid.0));
                                report.removed_with_frame.push((cid, cname));
                            }
                        }
                    }
                }
                continue;
            }

            // Check whether `cur_fp` matches the pre-fingerprint of some step in this turn
            // (meaning the user already pressed Cmd+Z for the trailing steps).
            let mut matched_undo_step = None;
            for (step_idx, step) in turn.steps.iter().enumerate() {
                if let Some(&pre_fp) = step.pre_fingerprints.get(&id) {
                    if cur_fp == pre_fp {
                        matched_undo_step = Some(step_idx);
                        break;
                    }
                }
            }
            if let Some(step_idx) = matched_undo_step {
                already_undone_from_step.insert(id, step_idx);
                continue;
            }

            match cur_fp {
                None => {
                    report.already_deleted.push(id);
                    skip_all_steps_for.insert(id);
                }
                Some(_) => {
                    let cur_name = doc.get(id).map(|n| n.name.clone()).unwrap_or_else(|| summary.name.clone());
                    report.kept_user_changed.push((id, cur_name));
                    skip_all_steps_for.insert(id);
                }
            }
        }

        let touched_set: BTreeSet<NodeId> = turn.touched_layers.iter().map(|s| s.id).collect();
        let mut tx = Tx::new();
        let mut needs_layout = Vec::new();
        let mut applied_any_in_step;

        for (step_idx, step) in turn.steps.iter().enumerate().rev() {
            applied_any_in_step = false;
            for (target_id, inv_cmd) in &step.atomic_inverses {
                if let Some(id) = *target_id {
                    if skip_all_steps_for.contains(&id) {
                        continue;
                    }
                    if let Some(&undone_from) = already_undone_from_step.get(&id) {
                        if step_idx >= undone_from {
                            continue;
                        }
                    }
                    // For relayout updates on containers/siblings not directly in `touched_layers`,
                    // skip the stale update if the user modified that node after the step.
                    if !touched_set.contains(&id) {
                        if let Some(&expected_post) = step.post_fingerprints.get(&id) {
                            let cur_fp = doc.get(id).map(node_fingerprint);
                            if cur_fp != expected_post {
                                continue;
                            }
                        }
                    }
                }
                if !can_apply_inverse(doc, inv_cmd) {
                    continue;
                }
                collect_layout_seeds(doc, inv_cmd, &mut needs_layout);
                tx.cmd(doc, inv_cmd.clone())?;
                applied_any_in_step = true;
            }
            if applied_any_in_step {
                report.reverted_steps += 1;
            }
        }

        let committed = tx.commit(doc, hist)?;
        if committed {
            relayout_touched(doc, &needs_layout);
        }

        let now = Instant::now();
        let turn_mut = &mut self.turns[pos];
        if turn_mut.end_patch_bytes.is_none() {
            if !turn_mut.approximate {
                self.open_hook_continuation = Some(turn_mut.start_patch_bytes);
            }
            let last_end = turn_mut.steps.last().map_or(turn_mut.start_patch_bytes, |s| s.patch_offset + 1);
            turn_mut.end_patch_bytes = Some(last_end);
        }
        turn_mut.state = TurnState::Reverted;
        turn_mut.reverted_at = Some(now);
        turn_mut.last_revert_report = Some(report.clone());
        Ok(report)
    }
}

fn merge_touch_kind(earlier: TouchKind, later: TouchKind) -> TouchKind {
    match (earlier, later) {
        (_, TouchKind::Removed) => TouchKind::Removed,
        (TouchKind::Inserted, TouchKind::Updated | TouchKind::Moved) => TouchKind::Inserted,
        (_, k) => k,
    }
}

/// Captures the pre-replay names and fingerprints for all nodes that `patch` may touch or that
/// exist in `doc` before `replay_live_patch` runs.
pub fn snapshot_pre_step(doc: &Document, patch: &designos_doc::live_patch::LivePatch) -> (BTreeMap<NodeId, String>, BTreeMap<NodeId, Option<u64>>) {
    let mut names = BTreeMap::new();
    let mut fps = BTreeMap::new();
    for op in &patch.ops {
        match op {
            designos_doc::live_patch::LiveOp::Insert { parent, nodes, .. } => {
                if let Some(p) = doc.get(*parent) {
                    names.insert(*parent, p.name.clone());
                    fps.insert(*parent, Some(node_fingerprint(p)));
                }
                if let Some(root) = nodes.first() {
                    names.insert(root.id, root.name.clone());
                    fps.insert(root.id, doc.get(root.id).map(node_fingerprint));
                }
            }
            designos_doc::live_patch::LiveOp::Update { id, .. }
            | designos_doc::live_patch::LiveOp::Reparent { id, .. }
            | designos_doc::live_patch::LiveOp::Remove { id } => {
                if let Some(n) = doc.get(*id) {
                    names.insert(*id, n.name.clone());
                    fps.insert(*id, Some(node_fingerprint(n)));
                    if let Some(pid) = n.parent {
                        if let Some(p) = doc.get(pid) {
                            fps.insert(pid, Some(node_fingerprint(p)));
                        }
                    }
                } else {
                    fps.insert(*id, None);
                }
            }
            designos_doc::live_patch::LiveOp::Comments { .. } => {}
        }
    }
    (names, fps)
}

fn flatten_inverses(cmd: Command, out: &mut Vec<(Option<NodeId>, Command)>) {
    match cmd {
        Command::Batch(cmds) => {
            for c in cmds {
                flatten_inverses(c, out);
            }
        }
        Command::Remove { id } => out.push((Some(id), Command::Remove { id })),
        Command::Insert { parent, index, nodes } => {
            let root = nodes.first().map(|n| n.id);
            out.push((root, Command::Insert { parent, index, nodes }));
        }
        Command::Reparent { id, parent, index } => {
            out.push((Some(id), Command::Reparent { id, parent, index }));
        }
        Command::Update { node } => {
            let id = node.id;
            out.push((Some(id), Command::Update { node }));
        }
        other => out.push((None, other)),
    }
}

fn name_from_inverses(inverses: &[(Option<NodeId>, Command)], target: NodeId) -> Option<String> {
    for (id, cmd) in inverses {
        if *id == Some(target) {
            match cmd {
                Command::Insert { nodes, .. } => {
                    if let Some(root) = nodes.first() {
                        return Some(root.name.clone());
                    }
                }
                Command::Update { node } => return Some(node.name.clone()),
                _ => {}
            }
        }
    }
    None
}

fn can_apply_inverse(doc: &Document, cmd: &Command) -> bool {
    match cmd {
        Command::Remove { id } => doc.get(*id).and_then(|n| n.parent).is_some(),
        Command::Insert { parent, nodes, .. } => {
            let Some(root) = nodes.first() else { return false };
            doc.get(*parent).is_some_and(|p| p.kind.can_hold_children()) && doc.get(root.id).is_none()
        }
        Command::Reparent { id, parent, .. } => {
            doc.get(*id).and_then(|n| n.parent).is_some()
                && doc.get(*parent).is_some_and(|p| p.kind.can_hold_children())
                && *id != *parent
                && !doc.is_ancestor(*id, *parent)
        }
        Command::Update { node } => doc.get(node.id).is_some(),
        _ => true,
    }
}

fn collect_layout_seeds(doc: &Document, cmd: &Command, seeds: &mut Vec<NodeId>) {
    match cmd {
        Command::Remove { id } => {
            if let Some(p) = doc.get(*id).and_then(|n| n.parent) {
                seeds.push(p);
            }
        }
        Command::Insert { parent, .. } => seeds.push(*parent),
        Command::Reparent { id, parent, .. } => {
            if let Some(old_p) = doc.get(*id).and_then(|n| n.parent) {
                seeds.push(old_p);
            }
            seeds.push(*parent);
            seeds.push(*id);
        }
        Command::Update { node } => seeds.push(node.id),
        _ => {}
    }
}

fn approx_node_bytes(n: &Node) -> usize {
    std::mem::size_of::<Node>()
        + n.name.len()
        + n.children.len() * std::mem::size_of::<NodeId>()
        + n.fills.len() * std::mem::size_of::<designos_doc::Paint>()
        + n.effects.len() * std::mem::size_of::<designos_doc::Effect>()
}

fn approx_command_bytes(cmd: &Command) -> usize {
    std::mem::size_of::<Command>()
        + match cmd {
            Command::Insert { nodes, .. } | Command::InsertPage { nodes, .. } => nodes.iter().map(approx_node_bytes).sum(),
            Command::Update { node } => approx_node_bytes(node),
            Command::Batch(cmds) => cmds.iter().map(approx_command_bytes).sum(),
            _ => 0,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_turn_card::{self, TurnCardAction};
    use crate::agent_turn_marks::{turn_mark_hook_command, TurnMarkLine, TurnMarkLog, TurnPhase};
    use crate::app::App;
    use crate::edit::Tx;
    use crate::home::doc_hash;
    use crate::live_patch_log::LivePatchLog;
    use crate::live_sync_tests::unique_tab_id;
    use crate::terminal::TerminalState;
    use designos_doc::live_patch::{LiveLogLine, LiveOp, LivePatch, SiblingHint, LIVE_PATCH_VERSION};
    use designos_doc::{Document, Node, NodeId, Rect};
    use serde_json::json;
    use std::io::Write;

    struct TestRig {
        app: App,
        session: designos_mcp::server::Session,
        dir: std::path::PathBuf,
        workspace: std::path::PathBuf,
        hero: NodeId,
        card: NodeId,
    }

    fn rig() -> TestRig {
        let mut doc = Document::new("Turn Review Spec");
        let page = doc.add_page("Page 1");
        let hero = doc.alloc_id();
        doc.insert(page, Node::frame(hero, Rect::new(0.0, 0.0, 240.0, 120.0))).unwrap();
        doc.get_mut(hero).unwrap().name = "Hero".into();
        let card = doc.alloc_id();
        doc.insert(page, Node::frame(card, Rect::new(280.0, 0.0, 200.0, 120.0))).unwrap();
        doc.get_mut(card).unwrap().name = "Pricing Card".into();

        let tab_id = unique_tab_id();
        let mut app = App::new(doc);
        app.terminal = TerminalState::for_tab(tab_id);
        let dir = app.terminal.session_dir.clone();
        let workspace = std::env::temp_dir().join(format!("designos-677-ws-{tab_id}"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&workspace);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        app.terminal.sync_live_file_context(&app.doc, app.page, &app.selection, None);
        let session = designos_mcp::server::Session::with_session_dir(&workspace, &dir).expect("mcp session");
        TestRig { app, session, dir, workspace, hero, card }
    }

    impl TestRig {
        fn patch_path(&self) -> std::path::PathBuf {
            self.dir.join("doc-patch.jsonl")
        }

        fn marks_path(&self) -> std::path::PathBuf {
            self.dir.join("turn-marks.jsonl")
        }

        fn run_hook(&self, phase: &str, cli: &str) {
            let cmd = turn_mark_hook_command(&self.marks_path(), &self.patch_path(), phase, cli);
            let status = std::process::Command::new("sh").arg("-c").arg(&cmd).status().expect("sh hook");
            assert!(status.success(), "hook command failed: {cmd}");
        }

        fn agent_rename(&mut self, id: NodeId, new_name: &str) {
            self.session.call("designos_update_node", json!({ "node_id": id.0, "name": new_name })).expect("designos_update_node");
        }

        fn agent_create_frame(&mut self, name: &str, x: f32, y: f32) -> NodeId {
            let out = self
                .session
                .call("designos_apply_spec", json!({ "spec": { "kind": "frame", "name": name, "x": x, "y": y, "width": 160.0, "height": 80.0 } }))
                .expect("designos_apply_spec");
            let id = out["root_node_id"].as_u64().or_else(|| out["node_id"].as_u64()).or_else(|| out["id"].as_u64()).expect("created node id");
            NodeId(id)
        }

        fn deliver(&mut self) {
            let actions = self.app.terminal.poll_live_agent_bridge();
            crate::shell::execute_terminal_actions(&mut self.app, actions);
            crate::live_patch_apply::apply_pending_live_patches(&mut self.app);
            crate::live_patch_apply::publish_live_context(&mut self.app);
        }
    }

    impl Drop for TestRig {
        fn drop(&mut self) {
            self.app.terminal.shutdown();
            let _ = std::fs::remove_dir_all(&self.dir);
            let _ = std::fs::remove_dir_all(&self.workspace);
        }
    }

    /// §10 Case 1: a log of two patches, a start mark with `patch_bytes` = 0 and an end mark with
    /// `patch_bytes` = the size after the second patch: both patches belong to the turn; a third
    /// patch after the end belongs to none.
    #[test]
    fn marks_assign_patches_by_byte_offset() {
        let mut r = rig();
        let hero = r.hero;
        let card = r.card;

        r.run_hook("start", "claude");
        r.agent_rename(hero, "Hero v1");
        r.agent_rename(card, "Pricing v1");
        r.run_hook("end", "claude");
        let size_after_two = std::fs::metadata(r.patch_path()).unwrap().len();
        assert!(size_after_two > 0);

        // Third patch written after the end mark without any new start mark.
        r.agent_rename(hero, "Hero Outside Turn");

        r.deliver();

        let ledger = r.app.terminal.active_turn_ledger();
        assert_eq!(ledger.turns.len(), 1, "only the marked turn should exist");
        let turn = &ledger.turns[0];
        assert_eq!(turn.start_patch_bytes, 0);
        assert_eq!(turn.end_patch_bytes, Some(size_after_two));
        assert_eq!(turn.steps.len(), 2, "both patches before end_patch_bytes belong to the turn");
        assert!(turn.steps.iter().all(|s| s.patch_offset < size_after_two), "third patch at >= {size_after_two} must not belong to the turn");
    }

    /// §10 Case 2: a stub writer appends a 1 MiB patch line to `doc-patch.jsonl` while the hook
    /// command runs repeatedly against `turn-marks.jsonl`: every patch line parses and every mark
    /// line parses.
    #[test]
    fn a_large_patch_and_a_mark_never_interleave() {
        let r = rig();
        let patch_path = r.patch_path();
        let marks_path = r.marks_path();

        let mut big_node = Node::frame(NodeId(900), Rect::new(0.0, 0.0, 100.0, 100.0));
        big_node.name = "X".repeat(1_050_000);
        let patch = LivePatch::new(vec![LiveOp::Insert { parent: r.app.page, at: SiblingHint::default(), nodes: vec![big_node] }]);
        let mut line = serde_json::to_vec(&patch).expect("serialize 1 MiB patch");
        line.push(b'\n');

        let patch_path_writer = patch_path.clone();
        let writer = std::thread::spawn(move || {
            for _ in 0..4 {
                let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&patch_path_writer).expect("open doc-patch.jsonl");
                f.write_all(&line).expect("append 1 MiB patch");
            }
        });

        for i in 0..8 {
            let phase = if i % 2 == 0 { "start" } else { "end" };
            r.run_hook(phase, "claude");
        }
        writer.join().expect("writer thread");

        let raw_patches = std::fs::read_to_string(&patch_path).expect("read doc-patch.jsonl");
        let patch_lines: Vec<&str> = raw_patches.lines().collect();
        assert_eq!(patch_lines.len(), 4);
        for line in patch_lines {
            assert!(line.len() > 1_000_000, "expected >= 1 MiB patch line");
            LiveLogLine::parse(line.as_bytes()).expect("every 1 MiB patch line must parse cleanly");
        }

        let raw_marks = std::fs::read_to_string(&marks_path).expect("read turn-marks.jsonl");
        let mark_lines: Vec<&str> = raw_marks.lines().collect();
        assert_eq!(mark_lines.len(), 8);
        for line in mark_lines {
            TurnMarkLine::parse(line.as_bytes()).expect("every mark line must parse cleanly");
        }
    }

    /// §10 Case 3: run the hook command text against a temp log of known size: one valid mark line
    /// with that `patch_bytes`.
    #[test]
    fn hook_command_writes_the_current_log_size() {
        let r = rig();
        let known_bytes = vec![b'a'; 1337];
        std::fs::write(r.patch_path(), &known_bytes).unwrap();

        r.run_hook("end", "claude");

        let mut mark_log = TurnMarkLog::bind(r.marks_path());
        let marks = mark_log.poll();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0], TurnMarkLine { v: LIVE_PATCH_VERSION, turn_mark: TurnPhase::End, cli: Some("claude".to_string()), patch_bytes: 1337 });
    }

    /// §10 Case 4: offsets of three lines equal their positions.
    #[test]
    fn log_reader_reports_line_start_offsets() {
        let mut r = rig();
        let mut reader = LivePatchLog::bind(r.patch_path());
        let hero = r.hero;
        let card = r.card;
        r.agent_rename(hero, "One");
        let pos1 = std::fs::metadata(r.patch_path()).unwrap().len();
        r.agent_rename(card, "Two");
        let pos2 = std::fs::metadata(r.patch_path()).unwrap().len();
        r.agent_rename(hero, "Three");

        let (items, errs) = reader.poll_with_offsets();
        assert!(errs.is_empty(), "unexpected errors: {errs:?}");
        let offsets: Vec<u64> = items.into_iter().map(|(off, _)| off).collect();
        assert_eq!(offsets, vec![0, pos1, pos2]);
    }

    /// §10 Case 5: two turns, three patches: grouped 2 + 1.
    #[test]
    fn ledger_groups_patches_between_marks() {
        let mut r = rig();
        let hero = r.hero;
        let card = r.card;

        // Turn 1: two patches.
        r.run_hook("start", "claude");
        r.agent_rename(hero, "Hero T1");
        r.agent_rename(card, "Card T1");
        r.run_hook("end", "claude");

        // Turn 2: one patch.
        r.run_hook("start", "claude");
        r.agent_rename(hero, "Hero T2");
        r.run_hook("end", "claude");

        r.deliver();

        let turns = &r.app.terminal.active_turn_ledger().turns;
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].steps.len(), 2);
        assert_eq!(turns[1].steps.len(), 1);
        assert_eq!(turns[0].state, TurnState::Kept, "earlier unreviewed turn is dismissed as Keep when next turn edits");
        assert_eq!(turns[1].state, TurnState::Completed);
    }

    /// §10 Case 6: card rows equal the touched layers; click selects the layer.
    #[test]
    fn summary_lists_changed_layers_and_selects_on_click() {
        let mut r = rig();
        let hero = r.hero;
        let card = r.card;

        r.run_hook("start", "claude");
        r.agent_rename(hero, "Hero Banner");
        r.agent_rename(card, "Pro Tier Card");
        r.run_hook("end", "claude");
        r.deliver();

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let panel_rect = egui::Rect::from_min_size(egui::Pos2::new(0.0, 0.0), egui::Vec2::new(300.0, 400.0));

        let run_card_frame = |app: &mut App, events: Vec<egui::Event>| -> egui::FullOutput {
            let raw =
                egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1280.0, 800.0))), events, ..Default::default() };
            ctx.run_ui(raw, |ui| {
                let h = agent_turn_card::desired_card_height(app, 0);
                assert!(h > 0.0, "card must have positive height");
                let rect = egui::Rect::from_min_size(panel_rect.min, egui::Vec2::new(panel_rect.width(), h + 16.0));
                agent_turn_card::show_in_rect(ui, app, 0, rect);
            })
        };

        crate::lint::take_icons();
        let out = run_card_frame(&mut r.app, vec![]);
        let (texts, boxes) = crate::lint::scan_shapes(&out.shapes);
        let painted: Vec<&str> = texts.iter().map(|t| t.text.as_str()).collect();
        assert!(painted.contains(&"Hero Banner"), "missing Hero Banner in {painted:?}");
        assert!(painted.contains(&"Pro Tier Card"), "missing Pro Tier Card in {painted:?}");
        assert!(painted.contains(&"Revert"), "missing Revert in {painted:?}");
        assert!(painted.contains(&"Keep"), "missing Keep in {painted:?}");
        let findings = crate::lint::check_text(&texts, &boxes);
        assert!(findings.is_empty(), "turn review card text lint findings: {findings:?}");

        let row1: egui::Rect =
            ctx.data(|d| d.get_temp(egui::Id::new(("test-rect", format!("agent-turn-layer-{}", card.0))))).expect("second layer row rect recorded");
        let click_at = row1.center();
        let _ = run_card_frame(&mut r.app, vec![egui::Event::PointerMoved(click_at)]);
        for pressed in [true, false] {
            let ev = egui::Event::PointerButton { pos: click_at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            let _ = run_card_frame(&mut r.app, vec![ev]);
        }
        assert_eq!(r.app.selection, vec![card], "clicking a layer row in the card must select that layer");
    }

    /// §10 Case 7: hash of the document equals the hash before the turn; the history grew by one
    /// step; undo reapplies the turn.
    #[test]
    fn revert_restores_the_document_in_one_step() {
        let mut r = rig();
        let hero = r.hero;
        let hash_before = doc_hash(&r.app.doc);

        r.run_hook("start", "claude");
        r.agent_rename(hero, "Hero Renamed");
        let added = r.agent_create_frame("Agent Added Badge", 40.0, 40.0);
        r.run_hook("end", "claude");
        r.deliver();

        let hash_after_turn = doc_hash(&r.app.doc);
        assert_ne!(hash_after_turn, hash_before);
        assert!(r.app.doc.get(added).is_some());
        let hist_before_revert = r.app.hist.undo_depth();

        let turn_id = r.app.terminal.active_turn_ledger().visible_turn(Instant::now()).expect("visible turn").id;
        agent_turn_card::apply_card_action(&mut r.app, 0, TurnCardAction::Revert(turn_id));

        assert_eq!(doc_hash(&r.app.doc), hash_before, "Revert must restore the exact pre-turn document");
        assert_eq!(r.app.hist.undo_depth(), hist_before_revert + 1, "Revert must push a single undo step");

        // Undoing the Revert reapplies the entire turn in one step.
        assert!(r.app.hist.undo(&mut r.app.doc).unwrap());
        assert_eq!(doc_hash(&r.app.doc), hash_after_turn, "Undo after Revert must reapply the turn");
    }

    /// §10 Case 8: the user edits one touched layer after the turn: after Revert that layer keeps
    /// the user's edit and the others are reverted; the note names it.
    #[test]
    fn revert_keeps_layers_the_user_changed_afterwards() {
        let mut r = rig();
        let hero = r.hero;
        let card = r.card;

        r.run_hook("start", "claude");
        r.agent_rename(hero, "Agent Hero");
        r.agent_rename(card, "Agent Card");
        r.run_hook("end", "claude");
        r.deliver();

        // User edits `hero` after the turn finishes.
        let mut tx = Tx::new();
        tx.ed.node(&mut r.app.doc, hero).unwrap().name = "User Kept Hero".into();
        tx.commit(&mut r.app.doc, &mut r.app.hist).unwrap();

        let turn_id = r.app.terminal.active_turn_ledger().visible_turn(Instant::now()).expect("visible turn").id;
        agent_turn_card::apply_card_action(&mut r.app, 0, TurnCardAction::Revert(turn_id));

        assert_eq!(r.app.doc.get(hero).unwrap().name, "User Kept Hero", "user-edited layer must keep user's edit");
        assert_eq!(r.app.doc.get(card).unwrap().name, "Pricing Card", "other touched layer must be reverted");

        let turn = r.app.terminal.active_turn_ledger().turns.last().unwrap();
        let report = turn.last_revert_report.as_ref().expect("revert report recorded");
        assert_eq!(report.kept_user_changed, vec![(hero, "User Kept Hero".to_string())]);
        let note = agent_turn_card::revert_status_text(report);
        assert!(note.contains("User Kept Hero"), "revert note must name the preserved layer: {note}");
    }

    /// §10 Case 9: the user edits an untouched layer during the turn: unchanged by Revert.
    #[test]
    fn revert_keeps_edits_on_other_layers() {
        let mut r = rig();
        let hero = r.hero;
        let card = r.card;

        r.run_hook("start", "claude");
        r.agent_rename(hero, "Agent Hero");
        r.deliver();

        // Mid-turn, the user edits `card` (which the agent does not touch).
        let mut tx = Tx::new();
        tx.ed.node(&mut r.app.doc, card).unwrap().name = "User Card Edit".into();
        tx.ed.node(&mut r.app.doc, card).unwrap().transform.tx = 512.0;
        tx.commit(&mut r.app.doc, &mut r.app.hist).unwrap();

        r.run_hook("end", "claude");
        r.deliver();

        let turn_id = r.app.terminal.active_turn_ledger().visible_turn(Instant::now()).expect("visible turn").id;
        agent_turn_card::apply_card_action(&mut r.app, 0, TurnCardAction::Revert(turn_id));

        assert_eq!(r.app.doc.get(hero).unwrap().name, "Hero", "agent-touched layer must revert");
        assert_eq!(r.app.doc.get(card).unwrap().name, "User Card Edit", "untouched user-edited layer must stay unchanged");
        assert_eq!(r.app.doc.get(card).unwrap().transform.tx, 512.0);
    }

    /// §10 Case 10: Keep: ten single undos still step back through the patches.
    #[test]
    fn keep_leaves_normal_undo_history() {
        let mut r = rig();
        let hero = r.hero;
        let hash_initial = doc_hash(&r.app.doc);

        r.run_hook("start", "claude");
        for i in 1..=10 {
            r.agent_rename(hero, &format!("Hero Step {i}"));
        }
        r.run_hook("end", "claude");
        r.deliver();

        let turn_id = r.app.terminal.active_turn_ledger().visible_turn(Instant::now()).expect("visible turn").id;
        agent_turn_card::apply_card_action(&mut r.app, 0, TurnCardAction::Keep(turn_id));
        assert!(r.app.terminal.active_turn_ledger().visible_turn(Instant::now()).is_none(), "Keep must dismiss the card");

        for expected_step in (1..10).rev() {
            assert!(r.app.hist.undo(&mut r.app.doc).unwrap());
            assert_eq!(r.app.doc.get(hero).unwrap().name, format!("Hero Step {expected_step}"));
        }
        assert!(r.app.hist.undo(&mut r.app.doc).unwrap());
        assert_eq!(r.app.doc.get(hero).unwrap().name, "Hero");
        assert_eq!(doc_hash(&r.app.doc), hash_initial);
    }

    /// §10 Case 11: status transitions only: turn recorded, card says approximate.
    #[test]
    fn codex_heuristic_turns_are_marked_approximate() {
        let mut r = rig();
        let hero = r.hero;
        let now = Instant::now();

        r.app.terminal.active_turn_ledger_mut().on_status_change(true, 0, now);
        r.agent_rename(hero, "Codex Hero");
        r.deliver();
        let patch_bytes = r.app.terminal.live_patch.current_offset();
        r.app.terminal.active_turn_ledger_mut().on_status_change(false, patch_bytes, now + Duration::from_millis(600));

        let turn = r.app.terminal.active_turn_ledger().visible_turn(now + Duration::from_secs(3)).expect("heuristic turn visible");
        assert!(turn.approximate, "heuristic turn must be marked approximate");

        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            agent_turn_card::show(ui, &mut r.app, 0);
        });
        let (texts, _) = crate::lint::scan_shapes(&out.shapes);
        assert!(
            texts.iter().any(|t| t.text.contains("approximate grouping")),
            "card must display 'approximate grouping' for heuristic turns: {:?}",
            texts.iter().map(|t| &t.text).collect::<Vec<_>>()
        );
    }

    /// §10 Case 12: a turn with no patches: no card.
    #[test]
    fn no_card_for_a_turn_without_changes() {
        let mut r = rig();
        r.run_hook("start", "claude");
        r.run_hook("end", "claude");
        r.deliver();

        assert!(r.app.terminal.active_turn_ledger().visible_turn(Instant::now()).is_none());
        assert_eq!(agent_turn_card::desired_card_height(&mut r.app, 0), 0.0);
    }

    /// §10 Case 13: 11 turns: the oldest is dropped and its Revert is disabled.
    #[test]
    fn ledger_is_capped() {
        let mut r = rig();
        let hero = r.hero;

        for i in 1..=11 {
            r.run_hook("start", "claude");
            r.agent_rename(hero, &format!("Turn {i}"));
            r.run_hook("end", "claude");
            r.deliver();
        }

        let ledger = r.app.terminal.active_turn_ledger();
        assert_eq!(ledger.turns.len(), MAX_TURNS);
        let oldest_id = 1;
        assert!(ledger.evicted_turn_ids.contains(&oldest_id), "turn 1 must be recorded as evicted");
        assert!(!ledger.can_revert(oldest_id), "Revert must be disabled for the evicted turn");
        assert!(ledger.revert_disabled_reason(oldest_id).is_some());
    }

    /// §10 Case 14: the generated Claude, Grok and agy hook files contain the start and end append
    /// commands (string pins against the captured event names).
    #[test]
    fn works_for_each_cli_hook_file() {
        let r = rig();
        let claude: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(r.dir.join("hooks.json")).expect("hooks.json")).unwrap();
        let grok: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(r.dir.join(".grok").join("hooks").join("designos-status.json")).expect("grok hooks")).unwrap();
        let agy: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(r.dir.join(".agents").join("hooks.json")).expect("agy hooks")).unwrap();

        for (cli_name, doc) in [("claude", &claude), ("grok", &grok)] {
            let start_cmd = doc["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"].as_str().expect("UserPromptSubmit command");
            assert!(
                start_cmd.contains("\"turn_mark\":\"start\"") && start_cmd.contains(&format!("\"cli\":\"{cli_name}\"")),
                "{cli_name} UserPromptSubmit missing start mark: {start_cmd}"
            );
            for stop_event in ["Stop", "StopCancelled", "StopFailure"] {
                let stop_cmd = doc["hooks"][stop_event][0]["hooks"][0]["command"].as_str().expect("stop command");
                assert!(
                    stop_cmd.contains("\"turn_mark\":\"end\"") && stop_cmd.contains(&format!("\"cli\":\"{cli_name}\"")),
                    "{cli_name} {stop_event} missing end mark: {stop_cmd}"
                );
            }
        }

        let agy_start = agy["designos-status"]["PreInvocation"][0]["command"].as_str().expect("agy PreInvocation");
        assert!(agy_start.contains("\"turn_mark\":\"start\"") && agy_start.contains("\"cli\":\"agy\""), "agy PreInvocation missing start mark: {agy_start}");
        let agy_stop = agy["designos-status"]["Stop"][0]["command"].as_str().expect("agy Stop");
        assert!(agy_stop.contains("\"turn_mark\":\"end\"") && agy_stop.contains("\"cli\":\"agy\""), "agy Stop missing end mark: {agy_stop}");
    }
}
