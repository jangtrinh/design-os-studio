//! Busy-frame detection, dismissals, free-spot duplicate placement, and concurrent field-edit
//! tracking when the user selects or edits inside a frame an agent is working on (`#693`).
//!
//! Behaviour rules (`docs/specs/v0.6/693-user-edits-inside-a-frame-an-agent-is-working-on.md`):
//! 1. **Busy frame.** A frame is busy when it has an active (non-stale) `in_progress` flag (`#592`)
//!    or is owned by an agent run that has not ended (`#572`). [`busy_info`] walks the selected
//!    node's ancestors (the node itself and up) and returns the nearest busy frame.
//! 2. **Trigger.** When the user selects a node, or starts an edit on a node (a drag, a text
//!    session, a property scrub), and [`busy_info`] finds a busy frame, a banner shows at the top
//!    of the canvas: `"<agent name> is working here"`. The banner shows once per busy frame until
//!    dismissed or the run ends.
//! 3. **Make a copy.** Duplicates the busy frame into the same parent, selects the copy, and
//!    leaves the original to the agent. The copy is a plain frame: it carries no `in_progress`
//!    flag and no agent ownership, so the agent's later writes land only on the original ids.
//!    Placement in non-auto-layout parents uses [`find_free_spot_to_right`] starting at the
//!    original's right edge plus [`FREE_SPOT_GAP`] (`40 px`).
//! 4. **Edit anyway.** Hides the banner for that frame until the run ends.
//! 5. **No silent drops.** If an agent write and a user edit touch the same field of the same
//!    node, the later one wins and [`AgentBusyFrames::both_changed`] records `(NodeId, field)` for
//!    the per-turn summary (`#677`).
//! 6. **Gone when over.** The banner disappears when the run ends or is stopped, when the busy
//!    frame is deleted, or when the user selects nothing inside it.

use crate::agent_presence::AgentPresence;
use crate::theme;
use designos_doc::live_patch::{LiveOp, LivePatchState};
use designos_doc::{now_ms, Document, Node, NodeId, NodeKind, Rect, Vec2, IN_PROGRESS_DEFAULT_STALE_MS};
use egui::Color32;
use std::collections::{BTreeMap, BTreeSet};

/// Horizontal gap (`40 px`) between a busy frame and its copy placed in free space (`§3.3`, `§13`).
pub const FREE_SPOT_GAP: f32 = theme::CONTROL_ROW;

/// Key identifying an active agent run on a frame (`#572` run id or `#592` in-progress session key).
pub type RunKey = String;

/// Turn counter for per-turn concurrent edit summaries (`#677`).
pub type TurnNo = u64;

/// Information about the nearest busy ancestor frame of a selected or edited node (`§3.1`, `§5`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusyInfo {
    pub frame_id: NodeId,
    pub agent_label: String,
    pub run_key: RunKey,
    pub colour_index: usize,
    /// Additional agents working on the same frame (`Edge case 7`).
    pub other_agents: Vec<String>,
}

impl BusyInfo {
    pub fn color(&self) -> Color32 {
        theme::AGENT_COLORS[self.colour_index % theme::AGENT_COLORS.len()]
    }

    pub fn banner_text(&self) -> String {
        format!("{} is working here", self.agent_label)
    }
}

/// Per-app state for `#693`: dismissed `(frame, run)` pairs, concurrent `(node, field)` edits,
/// and cached active banner (`§4`, `§11`).
#[derive(Clone, Debug, Default)]
pub struct AgentBusyFrames {
    pub dismissed: BTreeSet<(NodeId, RunKey)>,
    pub both_changed: BTreeMap<(NodeId, String), TurnNo>,
    pub active_banner: Option<BusyInfo>,
    pub current_turn: TurnNo,
    /// Fields edited by the user in the current turn (`(node_id, field) -> turn_no`).
    pub user_edited_fields: BTreeMap<(NodeId, String), TurnNo>,
    /// Fields edited by an agent in the current turn (`(node_id, field) -> turn_no`).
    pub agent_edited_fields: BTreeMap<(NodeId, String), TurnNo>,
    /// Active run ownership from `#572` (`frame_id -> Vec<(run_key, agent_label)>`), if any.
    pub frame_owners: BTreeMap<NodeId, Vec<(RunKey, String)>>,
    /// Undo bookmark for `"Make a copy"` so `App::undo` restores the selection that was active
    /// before duplicating (`Edge case 8`).
    pub last_copy_undo: Option<(NodeId, Vec<NodeId>)>,
}

/// Checks whether `node_id` itself is a busy frame (`#592` `in_progress` flag or `#572` `frame_owners`).
fn node_busy_info(
    doc: &Document,
    presence: &AgentPresence,
    frame_owners: &BTreeMap<NodeId, Vec<(RunKey, String)>>,
    node_id: NodeId,
    clock_ms: u64,
) -> Option<BusyInfo> {
    let node = doc.get(node_id)?;
    if matches!(node.kind, NodeKind::Page { .. }) {
        return None;
    }

    // 1. Check explicit frame ownership (`#572`) first if registered.
    if let Some(owners) = frame_owners.get(&node_id) {
        if let Some((first_run, first_agent)) = owners.first() {
            let label = if first_agent.trim().is_empty() { "Agent".to_string() } else { first_agent.trim().to_string() };
            let colour_index = resolve_colour_index(presence, first_run, &label);
            let other_agents = owners.iter().skip(1).map(|(_, a)| a.trim().to_string()).filter(|a| !a.is_empty() && a != &label).collect();
            return Some(BusyInfo { frame_id: node_id, agent_label: label, run_key: first_run.clone(), colour_index, other_agents });
        }
    }

    // 2. Check `#592` transient `in_progress` flag on `doc.in_progress`.
    if let Some(ip) = doc.in_progress.get(&node_id) {
        if !ip.is_stale(clock_ms, IN_PROGRESS_DEFAULT_STALE_MS) {
            let label = ip.agent.as_deref().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("Agent").to_string();
            let run_key = ip
                .run_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("ip:{}:{}:{}", node_id.0, label, ip.since_ms));
            let colour_index = resolve_colour_index(presence, &run_key, &label);
            return Some(BusyInfo { frame_id: node_id, agent_label: label, run_key, colour_index, other_agents: Vec::new() });
        }
    }

    // 3. Also check legacy `node.fig_metadata` keys in case a fixture or plugin sets them.
    if let Some(owner_str) =
        node.fig_metadata.get("agent_in_progress").or_else(|| node.fig_metadata.get("agent_owner")).map(|s| s.trim()).filter(|s| !s.is_empty())
    {
        let label = owner_str.to_string();
        let run_key = node.fig_metadata.get("agent_run_id").cloned().unwrap_or_else(|| format!("meta:{}:{}", node_id.0, label));
        let colour_index = resolve_colour_index(presence, &run_key, &label);
        return Some(BusyInfo { frame_id: node_id, agent_label: label, run_key, colour_index, other_agents: Vec::new() });
    }

    None
}

fn resolve_colour_index(presence: &AgentPresence, run_key: &str, agent_label: &str) -> usize {
    if let Some(sess) = presence.sessions.iter().find(|s| s.id == run_key || s.label.eq_ignore_ascii_case(agent_label)) {
        return sess.colour_index;
    }
    0
}

/// Walks `node_id` and its ancestors up to the page and returns the **nearest** busy frame (`§3.1`, `§3.8`).
///
/// This is the single lookup contract reading `#592` and `#572` state (`§5`).
pub fn busy_info(doc: &Document, presence: &AgentPresence, frame_owners: &BTreeMap<NodeId, Vec<(RunKey, String)>>, node_id: NodeId) -> Option<BusyInfo> {
    let clock_ms = now_ms();
    let mut cur = Some(node_id);
    let mut depth = 0usize;
    while let Some(id) = cur {
        if depth > 256 {
            break;
        }
        depth += 1;
        let node = doc.get(id)?;
        if matches!(node.kind, NodeKind::Page { .. }) {
            break;
        }
        if let Some(info) = node_busy_info(doc, presence, frame_owners, id, clock_ms) {
            return Some(info);
        }
        cur = node.parent;
    }
    None
}

/// Returns true if rects `a` and `b` overlap with positive area.
#[inline]
pub fn rects_overlap(a: Rect, b: Rect) -> bool {
    let eps = 1e-3;
    a.x + eps < b.x + b.w && a.x + a.w - eps > b.x && a.y + eps < b.y + b.h && a.y + a.h - eps > b.y
}

/// Finds the nearest free spot to the right of `orig` in the parent's space (`§3.3`, `§8.6`):
/// starts at `orig.x + orig.w + gap` (same `y`), and while the candidate rectangle overlaps any
/// sibling's bounds, advances `x` to that sibling's right edge plus `gap`.
///
/// Bounded by `sibling_bounds.len()` iterations (`§11`: O(siblings)); if a crowded row never frees
/// within the cap, places the copy past the right-most overlapping sibling.
pub fn find_free_spot_to_right(orig: Rect, sibling_bounds: &[Rect], gap: f32) -> Vec2 {
    let mut x = orig.x + orig.w + gap;
    let y = orig.y;
    let w = orig.w.max(1.0);
    let h = orig.h.max(1.0);

    let cap = sibling_bounds.len();
    for _ in 0..cap {
        let candidate = Rect::new(x, y, w, h);
        let mut blocking_right: Option<f32> = None;
        for sib in sibling_bounds {
            if rects_overlap(candidate, *sib) {
                let right = sib.x + sib.w + gap;
                blocking_right = Some(blocking_right.map_or(right, |prev| prev.max(right)));
            }
        }
        if let Some(next_x) = blocking_right {
            x = next_x.max(x + crate::theme::BORDER_WIDTH);
        } else {
            return Vec2::new(x, y);
        }
    }

    // Edge case 6: if still overlapping after `cap` steps, place past the right-most sibling in the row.
    let candidate = Rect::new(x, y, w, h);
    if sibling_bounds.iter().any(|sib| rects_overlap(candidate, *sib)) {
        if let Some(max_right) = sibling_bounds.iter().map(|s| s.x + s.w).reduce(f32::max) {
            x = x.max(max_right + gap);
        }
    }
    Vec2::new(x, y)
}

/// Strips any busy / in-progress / agent-owner metadata from `nodes` and from `doc.in_progress`
/// so a duplicated frame is an ordinary user frame (`§3.3`).
pub fn strip_busy_metadata_from_copy(doc: &mut Document, frame_owners: &mut BTreeMap<NodeId, Vec<(RunKey, String)>>, nodes: &mut [Node]) {
    for n in nodes {
        doc.in_progress.remove(&n.id);
        frame_owners.remove(&n.id);
        n.fig_metadata.remove("agent_in_progress");
        n.fig_metadata.remove("agent_owner");
        n.fig_metadata.remove("agent_run_id");
    }
}

/// Computes the top-level serialized field names that differ between `before` and `after` (`§3.5`).
pub fn diff_node_fields(before: &Node, after: &Node) -> Vec<String> {
    let (Ok(serde_json::Value::Object(b_map)), Ok(serde_json::Value::Object(a_map))) = (serde_json::to_value(before), serde_json::to_value(after)) else {
        return Vec::new();
    };
    let mut changed = Vec::new();
    for (k, v_after) in &a_map {
        if matches!(k.as_str(), "id" | "parent" | "children") {
            continue;
        }
        if b_map.get(k) != Some(v_after) {
            changed.push(k.clone());
        }
    }
    for k in b_map.keys() {
        if matches!(k.as_str(), "id" | "parent" | "children") {
            continue;
        }
        if !a_map.contains_key(k) && !changed.contains(k) {
            changed.push(k.clone());
        }
    }
    changed.sort();
    changed
}

impl AgentBusyFrames {
    pub fn clear(&mut self) {
        self.dismissed.clear();
        self.both_changed.clear();
        self.active_banner = None;
        self.user_edited_fields.clear();
        self.agent_edited_fields.clear();
        self.frame_owners.clear();
        self.last_copy_undo = None;
    }

    /// Starts or advances to `turn_no`, clearing per-turn field-edit sets from earlier turns.
    #[allow(dead_code)]
    pub fn start_turn(&mut self, turn_no: TurnNo) {
        self.current_turn = turn_no;
        self.user_edited_fields.retain(|_, t| *t == turn_no);
        self.agent_edited_fields.retain(|_, t| *t == turn_no);
    }

    /// Registers or updates `#572` frame ownership for an active agent run.
    #[allow(dead_code)]
    pub fn set_frame_owner(&mut self, frame_id: NodeId, run_key: impl Into<String>, agent_label: impl Into<String>) {
        let rk = run_key.into();
        let label = agent_label.into();
        let entry = self.frame_owners.entry(frame_id).or_default();
        if let Some(existing) = entry.iter_mut().find(|(r, _)| *r == rk) {
            existing.1 = label;
        } else {
            entry.push((rk, label));
        }
    }

    /// Clears `#572` frame ownership when a run ends or is stopped (`§3.6`).
    #[allow(dead_code)]
    pub fn clear_run(&mut self, run_key: &str) {
        for owners in self.frame_owners.values_mut() {
            owners.retain(|(r, _)| r != run_key);
        }
        self.frame_owners.retain(|_, v| !v.is_empty());
        self.dismissed.retain(|(_, r)| r != run_key);
        if self.active_banner.as_ref().is_some_and(|b| b.run_key == run_key) {
            self.active_banner = None;
        }
    }

    /// Dismisses the banner for `(frame_id, run_key)` until that run ends (`§3.4` "Edit anyway").
    pub fn dismiss_banner(&mut self, frame_id: NodeId, run_key: RunKey) {
        self.dismissed.insert((frame_id, run_key));
        self.active_banner = None;
    }

    /// Refreshes `self.active_banner` from the current `selection` and optional active gesture target (`§3.2`, `§3.6`).
    pub fn sync_banner(&mut self, doc: &Document, presence: &AgentPresence, selection: &[NodeId], gesture_node: Option<NodeId>) {
        let clock_ms = now_ms();

        // Prune dismissals whose `(frame_id, run_key)` is no longer active so a subsequent run on
        // the same frame shows the banner again (`edit_anyway_hides_the_banner_until_the_run_ends`).
        let frame_owners = &self.frame_owners;
        self.dismissed.retain(|(fid, rkey)| node_busy_info(doc, presence, frame_owners, *fid, clock_ms).is_some_and(|info| &info.run_key == rkey));

        // Candidate nodes: current selection plus any node being dragged/edited right now (`Edge case 3`).
        let mut found: Option<BusyInfo> = None;
        for &id in selection.iter().chain(gesture_node.iter()) {
            if let Some(info) = busy_info(doc, presence, &self.frame_owners, id) {
                found = Some(info);
                break;
            }
        }

        match found {
            Some(info) if !self.dismissed.contains(&(info.frame_id, info.run_key.clone())) => {
                self.active_banner = Some(info);
            }
            _ => {
                self.active_banner = None;
            }
        }
    }

    /// Records a user `Command::Update` diff (`before` vs `after`) for concurrent-edit reporting (`§3.5`).
    pub fn record_user_node_update(&mut self, before: &Node, after: &Node) {
        let turn = self.current_turn;
        for field in diff_node_fields(before, after) {
            let key = (after.id, field);
            self.user_edited_fields.insert(key.clone(), turn);
            if self.agent_edited_fields.get(&key) == Some(&turn) {
                self.both_changed.insert(key, turn);
            }
        }
    }

    /// Records agent `LiveOp::Update` field writes for concurrent-edit reporting (`§3.5`).
    pub fn record_agent_patch_ops(&mut self, ops: &[LiveOp], _state: &LivePatchState) {
        let turn = self.current_turn;
        for op in ops {
            if let LiveOp::Update { id, set, unset } = op {
                let resolved_id = *id;
                for field in set.keys().chain(unset.iter()) {
                    if matches!(field.as_str(), "id" | "parent" | "children") {
                        continue;
                    }
                    let key = (resolved_id, field.clone());
                    self.agent_edited_fields.insert(key.clone(), turn);
                    if self.user_edited_fields.get(&key) == Some(&turn) {
                        self.both_changed.insert(key, turn);
                    }
                }
            }
        }
    }

    /// Returns all `(NodeId, field_name)` pairs touched by both the user and the agent in `turn_no` (`§5`).
    pub fn both_changed_for_turn(&self, turn_no: TurnNo) -> Vec<(NodeId, String)> {
        self.both_changed.iter().filter(|(_, &t)| t == turn_no).map(|(k, _)| k.clone()).collect()
    }

    /// Formats the one-line summary for `#677`'s turn card when `both_changed` is non-empty (`§3.5`, `§5`).
    #[allow(dead_code)]
    pub fn format_both_changed_line(&self, turn_no: TurnNo) -> Option<String> {
        let items = self.both_changed_for_turn(turn_no);
        if items.is_empty() {
            return None;
        }
        let mut fields: Vec<&str> = items.iter().map(|(_, f)| f.as_str()).collect();
        fields.dedup();
        Some(format!("{} changed by you and the agent", fields.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use designos_doc::live_patch::LivePatch;
    use designos_doc::{Affine, AutoLayout, Document, FrameProps, InProgress, LayoutDirection, Paint, Stroke};

    fn make_frame(id: u64, name: &str, x: f32, y: f32, w: f32, h: f32) -> Node {
        Node {
            id: NodeId(id),
            source_id: None,
            name: name.to_string(),
            kind: NodeKind::Frame(FrameProps::default()),
            parent: None,
            children: Vec::new(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: designos_doc::BlendMode::PassThrough,
            transform: Affine::translate(x, y),
            size: Vec2::new(w, h),
            fills: vec![Paint::solid(designos_doc::Color::rgb(50, 50, 64))],
            stroke: Stroke::default(),
            effects: Vec::new(),
            corner_radii: [0.0; 4],
            corner_smoothing: 0.0,
            constraints: designos_doc::Constraints::default(),
            layout_child: designos_doc::LayoutChild::default(),
            is_mask: false,
            style_refs: BTreeMap::new(),
            fill_geometry: None,
            stroke_geometry: None,
            component: designos_doc::ComponentLink::default(),
            prototype: designos_doc::PrototypeProps::default(),
            variables: designos_doc::NodeVariables::default(),
            export_settings: Vec::new(),
            dev: designos_doc::DevInfo::default(),
            slide: None,
            lock_aspect: false,
            hyperlink: None,
            plugin_data: BTreeMap::new(),
            detached_symbol_id: None,
            fig_metadata: BTreeMap::new(),
        }
    }

    fn setup_app_with_busy_frame() -> (App, NodeId, NodeId, NodeId) {
        designos_doc::set_mock_now_ms(Some(1_000_000));
        let mut doc = Document::new("Busy Test");
        let page = doc.add_page("Page 1");
        let frame_id = NodeId(10);
        let child_id = NodeId(11);
        let grandchild_id = NodeId(12);

        let frame = make_frame(10, "Hero", 100.0, 100.0, 300.0, 200.0);
        let child = make_frame(11, "Card", 20.0, 20.0, 120.0, 80.0);
        let grandchild = make_frame(12, "CTA", 10.0, 10.0, 60.0, 30.0);

        doc.insert(page, frame).unwrap();
        doc.insert(frame_id, child).unwrap();
        doc.insert(child_id, grandchild).unwrap();

        let mut ip = InProgress::new(Some("Claude".into()), Some("Building hero".into()), 1_000_000);
        ip.run_id = Some("run-claude-1".into());
        doc.in_progress.insert(frame_id, ip);

        let app = App::new(doc);
        (app, frame_id, child_id, grandchild_id)
    }

    #[test]
    fn selecting_a_node_in_a_busy_frame_shows_the_banner_with_the_agent_name() {
        let (mut app, frame_id, _child_id, grandchild_id) = setup_app_with_busy_frame();

        // Edge case 1: selecting the busy frame itself shows the banner.
        app.select_only(frame_id);
        app.sync_busy_frame_banner();
        let banner = app.agent_busy_frames.active_banner.as_ref().expect("banner shown for busy frame");
        assert_eq!(banner.frame_id, frame_id);
        assert_eq!(banner.banner_text(), "Claude is working here");

        // Edge case 2: selecting a child two levels deep finds the busy ancestor frame.
        app.select_only(grandchild_id);
        app.sync_busy_frame_banner();
        let banner2 = app.agent_busy_frames.active_banner.as_ref().expect("banner shown for grandchild");
        assert_eq!(banner2.frame_id, frame_id);
        assert_eq!(banner2.banner_text(), "Claude is working here");
    }

    #[test]
    fn make_a_copy_creates_one_duplicate_in_free_space_selected() {
        let (mut app, frame_id, _child_id, grandchild_id) = setup_app_with_busy_frame();
        app.select_only(grandchild_id);
        app.sync_busy_frame_banner();

        let hist_before = app.hist.undo_depth();
        let copy_id = app.duplicate_into_free_spot(frame_id).expect("copy created");

        assert_ne!(copy_id, frame_id);
        assert_eq!(app.selection, vec![copy_id], "copy is selected");
        assert_eq!(app.hist.undo_depth(), hist_before + 1, "one undo step committed");

        let orig = app.doc.get(frame_id).unwrap();
        let copy = app.doc.get(copy_id).unwrap();
        let orig_rect = Rect::new(orig.transform.tx, orig.transform.ty, orig.size.x, orig.size.y);
        let copy_rect = Rect::new(copy.transform.tx, copy.transform.ty, copy.size.x, copy.size.y);
        assert!(!rects_overlap(orig_rect, copy_rect), "copy does not overlap original");
        assert!((copy_rect.x - (orig_rect.x + orig_rect.w + FREE_SPOT_GAP)).abs() < 1e-3);

        // Edge case 8: Undo removes the copy and restores the previous selection.
        app.undo();
        assert!(app.doc.get(copy_id).is_none(), "undo removes the copy");
        assert_eq!(app.selection, vec![grandchild_id], "undo restores previous selection");
    }

    #[test]
    fn later_agent_writes_land_only_on_the_original() {
        let (mut app, frame_id, child_id, _grandchild_id) = setup_app_with_busy_frame();
        app.select_only(child_id);
        let copy_id = app.duplicate_into_free_spot(frame_id).expect("copy created");
        let copy_before = serde_json::to_string(app.doc.get(copy_id).unwrap()).unwrap();
        let copy_children_before: Vec<String> =
            app.doc.get(copy_id).unwrap().children.iter().map(|cid| serde_json::to_string(app.doc.get(*cid).unwrap()).unwrap()).collect();

        // Apply a stub live patch touching the original frame and child.
        let mut set = serde_json::Map::new();
        set.insert("name".into(), serde_json::Value::String("Hero Updated By Agent".into()));
        let patch = LivePatch::new(vec![LiveOp::Update { id: frame_id, set, unset: Vec::new() }]);
        crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);

        assert_eq!(app.doc.get(frame_id).unwrap().name, "Hero Updated By Agent");
        let copy_after = serde_json::to_string(app.doc.get(copy_id).unwrap()).unwrap();
        let copy_children_after: Vec<String> =
            app.doc.get(copy_id).unwrap().children.iter().map(|cid| serde_json::to_string(app.doc.get(*cid).unwrap()).unwrap()).collect();
        assert_eq!(copy_before, copy_after, "copy root unchanged by later agent patch");
        assert_eq!(copy_children_before, copy_children_after, "copy subtree unchanged");
    }

    #[test]
    fn the_copy_carries_no_busy_flag_or_owner() {
        let (mut app, frame_id, _child_id, _grandchild_id) = setup_app_with_busy_frame();
        app.agent_busy_frames.set_frame_owner(frame_id, "run-claude-1", "Claude");
        app.doc.get_mut(frame_id).unwrap().fig_metadata.insert("agent_owner".into(), "Claude".into());

        let copy_id = app.duplicate_into_free_spot(frame_id).expect("copy created");
        assert!(!app.doc.in_progress.contains_key(&copy_id));
        assert!(!app.agent_busy_frames.frame_owners.contains_key(&copy_id));
        assert!(busy_info(&app.doc, &app.agent_presence, &app.agent_busy_frames.frame_owners, copy_id).is_none());
    }

    #[test]
    fn edit_anyway_hides_the_banner_until_the_run_ends() {
        let (mut app, frame_id, child_id, _grandchild_id) = setup_app_with_busy_frame();
        app.select_only(child_id);
        app.sync_busy_frame_banner();
        let info = app.agent_busy_frames.active_banner.clone().expect("banner shown");

        // User clicks "Edit anyway".
        app.agent_busy_frames.dismiss_banner(info.frame_id, info.run_key.clone());
        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_none(), "hidden during current run");

        // Run ends -> dismissal is pruned; next run on the same frame shows the banner again.
        app.doc.in_progress.remove(&frame_id);
        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_none());

        let mut next_ip = InProgress::new(Some("Claude".into()), None, 2_000_000);
        next_ip.run_id = Some("run-claude-2".into());
        app.doc.in_progress.insert(frame_id, next_ip);
        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_some(), "banner shown again for the next run");
    }

    #[test]
    fn no_banner_for_frames_no_agent_works_on() {
        let (mut app, frame_id, _child_id, _grandchild_id) = setup_app_with_busy_frame();
        let page = app.page;
        let idle_frame_id = NodeId(99);
        let idle_frame = make_frame(99, "Idle Frame", 600.0, 100.0, 200.0, 200.0);
        app.doc.insert(page, idle_frame).unwrap();

        // Busy frame exists on page, but user selects nothing inside it.
        app.clear_selection();
        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_none());

        // User selects the idle frame.
        app.select_only(idle_frame_id);
        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_none());
        let _ = frame_id;
    }

    #[test]
    fn banner_clears_when_the_run_ends_or_stops() {
        let (mut app, frame_id, child_id, _grandchild_id) = setup_app_with_busy_frame();
        app.select_only(child_id);
        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_some());

        // Agent finishes or stops -> `in_progress` flag cleared.
        app.doc.in_progress.remove(&frame_id);
        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_none());
    }

    #[test]
    fn free_spot_skips_overlapping_siblings() {
        let orig = Rect::new(100.0, 100.0, 200.0, 150.0);
        let gap = FREE_SPOT_GAP; // 40.0

        // 1. Empty row: lands right at orig.x + orig.w + gap = 340.0.
        let p_empty = find_free_spot_to_right(orig, &[], gap);
        assert_eq!(p_empty, Vec2::new(340.0, 100.0));

        // 2. One blocker at 340..540: skips past blocker to 540 + 40 = 580.0.
        let blocker = Rect::new(340.0, 100.0, 200.0, 150.0);
        let p_one = find_free_spot_to_right(orig, &[blocker], gap);
        assert_eq!(p_one, Vec2::new(580.0, 100.0));

        // 3. Crowded row (out-of-order overlapping siblings): lands past the right-most sibling.
        let crowded = vec![Rect::new(500.0, 100.0, 200.0, 150.0), Rect::new(330.0, 100.0, 200.0, 150.0), Rect::new(680.0, 100.0, 200.0, 150.0)];
        let p_crowded = find_free_spot_to_right(orig, &crowded, gap);
        let placed_rect = Rect::new(p_crowded.x, p_crowded.y, orig.w, orig.h);
        for sib in &crowded {
            assert!(!rects_overlap(placed_rect, *sib), "placed rect {placed_rect:?} must not overlap {sib:?}");
        }
        assert_eq!(p_crowded, Vec2::new(920.0, 100.0));
    }

    #[test]
    fn auto_layout_parent_inserts_the_copy_as_the_next_sibling() {
        let (mut app, _frame_id, _child_id, _grandchild_id) = setup_app_with_busy_frame();
        let page = app.page;

        let al_parent_id = NodeId(50);
        let mut al_parent = make_frame(50, "Row", 0.0, 500.0, 800.0, 200.0);
        if let NodeKind::Frame(fp) = &mut al_parent.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 16.0, ..AutoLayout::default() });
        }
        let busy_child_id = NodeId(51);
        let busy_child = make_frame(51, "Busy Card", 0.0, 0.0, 160.0, 120.0);
        let trailing_child_id = NodeId(52);
        let trailing_child = make_frame(52, "Trailing Card", 176.0, 0.0, 160.0, 120.0);

        app.doc.insert(page, al_parent).unwrap();
        app.doc.insert(al_parent_id, busy_child).unwrap();
        app.doc.insert(al_parent_id, trailing_child).unwrap();
        app.doc.in_progress.insert(busy_child_id, InProgress::new(Some("Codex".into()), None, 1_000_000));

        let copy_id = app.duplicate_into_free_spot(busy_child_id).expect("copy created in AL parent");
        let children = &app.doc.get(al_parent_id).unwrap().children;
        assert_eq!(children, &vec![busy_child_id, copy_id, trailing_child_id], "inserted as immediate next sibling in auto-layout parent");
    }

    #[test]
    fn same_field_edits_are_listed_as_changed_by_both() {
        let (mut app, frame_id, _child_id, _grandchild_id) = setup_app_with_busy_frame();
        app.agent_busy_frames.start_turn(1);

        let before = app.doc.get(frame_id).unwrap().clone();
        app.edit_nodes(&[frame_id], |n| {
            n.opacity = 0.5;
        });
        let after = app.doc.get(frame_id).unwrap().clone();
        app.agent_busy_frames.record_user_node_update(&before, &after);

        // Agent also updates `opacity` on the same node in turn 1.
        let mut set = serde_json::Map::new();
        set.insert("opacity".into(), serde_json::json!(0.8));
        let patch = LivePatch::new(vec![LiveOp::Update { id: frame_id, set, unset: Vec::new() }]);
        crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);

        assert!((app.doc.get(frame_id).unwrap().opacity - 0.8).abs() < 1e-4, "later write wins");
        let both = app.agent_busy_frames.both_changed_for_turn(1);
        assert_eq!(both, vec![(frame_id, "opacity".to_string())]);
        assert_eq!(app.agent_busy_frames.format_both_changed_line(1).as_deref(), Some("opacity changed by you and the agent"));
    }

    #[test]
    fn banner_never_moves_the_view_or_the_selection() {
        let (mut app, _frame_id, child_id, _grandchild_id) = setup_app_with_busy_frame();
        app.view = crate::app::View { offset: Vec2::new(42.0, 84.0), zoom: 1.25 };
        app.select_only(child_id);
        let view_before = app.view;
        let sel_before = app.selection.clone();

        app.sync_busy_frame_banner();
        assert!(app.agent_busy_frames.active_banner.is_some());
        assert_eq!(app.view.offset, view_before.offset);
        assert_eq!(app.view.zoom, view_before.zoom);
        assert_eq!(app.selection, sel_before);
    }
}
