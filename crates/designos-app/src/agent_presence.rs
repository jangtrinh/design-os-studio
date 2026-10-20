//! Per-session agent cursor and write-flash state on the canvas (`#682`).
//!
//! Tracks each active agent session's colour, label, page-space cursor position, exponential
//! follow animation, 200 ms fade-in / 5 s hold / 1.5 s fade-out lifecycle, and per-node write
//! flash outlines (`200 ms` attack, `400 ms` hold, `300 ms` release).
//!
//! Viewport-only invariant: nothing in this module or its canvas overlay ever mutates the
//! document, calls `App::changed()`, or bumps `TileView::rev`.

use crate::comments::settings;
use crate::theme;
use designos_doc::live_patch::{LivePatchReport, TouchKind};
use designos_doc::{Document, NodeId, NodeKind, Rect};
use egui::{Color32, Pos2};
use std::path::Path;

/// Settings key in `settings.json` storing the user's cursor size choice.
pub const CURSOR_SIZE_SETTING_KEY: &str = "agent_cursor_size";

/// Exponential follow rate (1/s): after 1 s the remaining gap is `exp(-2.5)` of the start.
pub const FOLLOW_RATE: f32 = 2.5;

/// Cursor fade-in duration (s), ease-out-cubic.
pub const CURSOR_FADE_IN_S: f64 = 0.2;

/// Seconds after the last write before the cursor begins fading out.
pub const CURSOR_IDLE_HOLD_S: f64 = 5.0;

/// Cursor fade-out duration (s) once idle hold expires.
pub const CURSOR_FADE_OUT_S: f64 = 1.5;

/// Delay (s) added to a write flash when the session cursor had to travel to the node.
pub const FLASH_TRAVEL_DELAY_S: f64 = 0.3;

/// Flash attack, hold, and release durations (s).
pub const FLASH_ATTACK_S: f64 = 0.2;
pub const FLASH_HOLD_S: f64 = 0.4;
pub const FLASH_RELEASE_S: f64 = 0.3;
pub const FLASH_TOTAL_S: f64 = FLASH_ATTACK_S + FLASH_HOLD_S + FLASH_RELEASE_S;

/// Exponential decay rate (1/s) for runtime fade-in of newly inserted top-level nodes (`#682` Rule 7).
pub const TOP_LEVEL_INSERT_DECAY_RATE: f64 = 5.0;

/// Duration (s) after which a top-level insert fade (`1 - exp(-5 * t)`) is >= 99.7% opaque and settled.
pub const TOP_LEVEL_INSERT_SETTLE_S: f64 = 1.2;

/// Maximum concurrent flashes retained per session; oldest drops first.
pub const MAX_FLASHES_PER_SESSION: usize = 64;

/// User setting controlling the size of the on-canvas agent cursor.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentCursorSize {
    Hidden,
    #[default]
    Small,
    Medium,
    Large,
}

#[allow(dead_code)]
impl AgentCursorSize {
    pub fn from_choice(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "hidden" => Self::Hidden,
            "small" => Self::Small,
            "medium" => Self::Medium,
            "large" => Self::Large,
            _ => Self::Small,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    pub fn pixels(self) -> f32 {
        match self {
            Self::Hidden => 0.0,
            Self::Small => 24.0,
            Self::Medium => 40.0,
            Self::Large => 56.0,
        }
    }

    pub fn load(root: &Path) -> Self {
        settings::load_choice(root, CURSOR_SIZE_SETTING_KEY).map(|s| Self::from_choice(&s)).unwrap_or_default()
    }

    pub fn save(self, root: &Path) -> Result<(), String> {
        settings::save_choice(root, CURSOR_SIZE_SETTING_KEY, self.as_str())
    }
}

/// One outline flash on a layer touched by an agent write.
#[derive(Clone, Debug, PartialEq)]
pub struct Flash {
    pub node_id: NodeId,
    pub start_s: f64,
    pub stroke_px: f32,
}

/// Runtime-only fade-in state for a newly inserted top-level node (never written to `Document`).
#[derive(Clone, Debug, PartialEq)]
pub struct TopLevelInsertFade {
    pub node_id: NodeId,
    pub start_s: f64,
}

/// One active agent session on the canvas.
#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub colour_index: usize,
    pub label: String,
    pub page: Option<NodeId>,
    pub has_cursor: bool,
    pub cursor_page: Pos2,
    pub target_page: Pos2,
    pub first_write_s: f64,
    pub last_write_s: f64,
    pub fade: f32,
    pub flashes: Vec<Flash>,
    pub insert_fades: Vec<TopLevelInsertFade>,
}

impl Session {
    pub fn color(&self) -> Color32 {
        theme::AGENT_COLORS[self.colour_index % theme::AGENT_COLORS.len()]
    }
}

/// All active agent cursors and write flashes for the open document.
#[derive(Clone, Debug, Default)]
pub struct AgentPresence {
    pub sessions: Vec<Session>,
    pub cursor_size: AgentCursorSize,
    next_colour_slot: usize,
    pub bound_doc_runtime_id: u64,
    pub bound_page: Option<NodeId>,
}

/// Exponential follow step in 1D with rate `FOLLOW_RATE` (`2.5 / s`).
pub fn follow_scalar(pos: f32, target: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return pos;
    }
    let factor = 1.0 - (-FOLLOW_RATE * dt).exp();
    pos + (target - pos) * factor
}

/// Exponential follow step in 2D page space with rate `FOLLOW_RATE` (`2.5 / s`).
pub fn follow_pos(pos: Pos2, target: Pos2, dt: f32) -> Pos2 {
    Pos2::new(follow_scalar(pos.x, target.x, dt), follow_scalar(pos.y, target.y, dt))
}

/// Runtime fade-in opacity (`0.0..=1.0`) for a newly inserted top-level node (`1 - exp(-5 * elapsed_s)`).
pub fn top_level_insert_fade_alpha(elapsed_s: f64) -> f32 {
    if elapsed_s <= 0.0 {
        return 0.0;
    }
    if elapsed_s >= TOP_LEVEL_INSERT_SETTLE_S {
        return 1.0;
    }
    (1.0 - (-TOP_LEVEL_INSERT_DECAY_RATE * elapsed_s).exp()).clamp(0.0, 1.0) as f32
}

/// Flash opacity envelope (`0.0..=1.0`) at `elapsed_s` seconds after `flash.start_s`:
/// - Attack: `0..200 ms` linear ramp `0 -> 1`
/// - Hold: `200..600 ms` constant `1.0`
/// - Release: `600..900 ms` linear ramp `1 -> 0`
pub fn flash_envelope(elapsed_s: f64) -> f32 {
    if !(0.0..FLASH_TOTAL_S).contains(&elapsed_s) {
        return 0.0;
    }
    if elapsed_s < FLASH_ATTACK_S {
        (elapsed_s / FLASH_ATTACK_S) as f32
    } else if elapsed_s < FLASH_ATTACK_S + FLASH_HOLD_S {
        1.0
    } else {
        let rel = elapsed_s - (FLASH_ATTACK_S + FLASH_HOLD_S);
        (1.0 - rel / FLASH_RELEASE_S).clamp(0.0, 1.0) as f32
    }
}

/// Cursor opacity (`0.0..=1.0`) given elapsed seconds since the session's first write and last write:
/// - Fade-in: `200 ms` ease-out-cubic (`1 - (1 - t)^3`) starting at `0.0` at `t = 0`
/// - Hold: `5.0 s` after last write at `1.0`
/// - Fade-out: `1.5 s` linear ramp to `0.0` from `5.0 s` to `6.5 s` after last write
pub fn cursor_fade_alpha(since_first_write_s: f64, since_last_write_s: f64) -> f32 {
    if since_last_write_s >= CURSOR_IDLE_HOLD_S + CURSOR_FADE_OUT_S {
        return 0.0;
    }
    let fade_in = if since_first_write_s < CURSOR_FADE_IN_S {
        let u = (since_first_write_s / CURSOR_FADE_IN_S).clamp(0.0, 1.0) as f32;
        1.0 - (1.0 - u).powi(3)
    } else {
        1.0
    };
    let fade_out = if since_last_write_s <= CURSOR_IDLE_HOLD_S {
        1.0
    } else {
        let u = ((since_last_write_s - CURSOR_IDLE_HOLD_S) / CURSOR_FADE_OUT_S).clamp(0.0, 1.0) as f32;
        1.0 - u
    };
    fade_in * fade_out
}

/// True when `id` is a top-level frame on a page (Rule 4: top-level frames do not flash).
pub fn is_top_level_frame(doc: &Document, id: NodeId) -> bool {
    let Some(node) = doc.get(id) else {
        return false;
    };
    if node.kind.frame_props().is_none() {
        return false;
    }
    node.parent.and_then(|p| doc.get(p)).is_some_and(|pn| matches!(pn.kind, NodeKind::Page { .. }))
}

fn node_center_page(doc: &Document, index: Option<&designos_render::SceneIndex>, id: NodeId) -> Option<Pos2> {
    let node = doc.get(id)?;
    if matches!(node.kind, NodeKind::Page { .. }) {
        return None;
    }
    let bb: Rect = crate::canvas::fast_world_aabb(doc, index, id);
    Some(Pos2::new(bb.x + bb.w * 0.5, bb.y + bb.h * 0.5))
}

impl AgentPresence {
    /// Clears all sessions (called on `DocEpoch` document switch, tab switch, or page change).
    pub fn reset(&mut self) {
        self.sessions.clear();
        self.next_colour_slot = 0;
        self.bound_doc_runtime_id = 0;
        self.bound_page = None;
    }

    /// Runtime-only opacity multiplier (`0.0..=1.0`) for a newly inserted top-level node at `now_s`.
    /// Returns `1.0` once the fade settles, when `cursor_size == AgentCursorSize::Hidden`, or for any node that is not currently fading in.
    pub fn top_level_insert_opacity(&self, node_id: NodeId, now_s: f64) -> f32 {
        if self.cursor_size == AgentCursorSize::Hidden {
            return 1.0;
        }
        for session in self.sessions.iter().rev() {
            if let Some(fade) = session.insert_fades.iter().find(|f| f.node_id == node_id) {
                return top_level_insert_fade_alpha(now_s - fade.start_s);
            }
        }
        1.0
    }

    /// Active runtime-only opacity multipliers (`0.0..1.0`) for newly inserted top-level nodes on `page` at `now_s`.
    /// Empty when `cursor_size == AgentCursorSize::Hidden` or once all insert fades have settled to `1.0`.
    pub fn active_top_level_insert_opacities(&self, doc: &Document, page: NodeId, now_s: f64) -> Vec<(NodeId, f32)> {
        if self.cursor_size == AgentCursorSize::Hidden {
            return Vec::new();
        }
        let mut out = Vec::new();
        for session in &self.sessions {
            if session.page != Some(page) {
                continue;
            }
            for fade in &session.insert_fades {
                if doc.page_of(fade.node_id) != Some(page) {
                    continue;
                }
                let alpha = self.top_level_insert_opacity(fade.node_id, now_s);
                if alpha < 1.0 {
                    if let Some(existing) = out.iter_mut().find(|(id, _)| *id == fade.node_id) {
                        existing.1 = alpha;
                    } else {
                        out.push((fade.node_id, alpha));
                    }
                }
            }
        }
        out
    }

    fn alloc_colour_index(&mut self) -> usize {
        // Assign unused colour in 0..4 first while fewer than 4 sessions are active.
        if self.sessions.len() < theme::AGENT_COLORS.len() {
            for candidate in 0..theme::AGENT_COLORS.len() {
                let candidate_idx = (self.next_colour_slot + candidate) % theme::AGENT_COLORS.len();
                if !self.sessions.iter().any(|s| s.colour_index == candidate_idx) {
                    self.next_colour_slot = (candidate_idx + 1) % theme::AGENT_COLORS.len();
                    return candidate_idx;
                }
            }
        }
        // 5th+ concurrent agent wraps around and reuses `agent-1` (`0`), told apart by its label.
        let idx = self.next_colour_slot % theme::AGENT_COLORS.len();
        self.next_colour_slot = (self.next_colour_slot + 1) % theme::AGENT_COLORS.len();
        idx
    }

    /// Records the touched layers of a committed [`LivePatchReport`] for `session_id`.
    pub fn record_patch(
        &mut self,
        session_id: &str,
        label: &str,
        doc: &Document,
        index: Option<&designos_render::SceneIndex>,
        report: &LivePatchReport,
        now_s: f64,
    ) {
        if report.touched.is_empty() {
            return;
        }
        if self.bound_doc_runtime_id != 0 && self.bound_doc_runtime_id != doc.runtime_id() {
            self.reset();
        }
        self.bound_doc_runtime_id = doc.runtime_id();

        let clean_id = if session_id.trim().is_empty() { "default" } else { session_id.trim() };
        let clean_label = if label.trim().is_empty() { "Claude" } else { label.trim() };

        // Prune expired sessions before allocating a colour or looking up `clean_id`.
        self.sessions.retain(|s| (now_s - s.last_write_s) < CURSOR_IDLE_HOLD_S + CURSOR_FADE_OUT_S || !s.flashes.is_empty() || !s.insert_fades.is_empty());

        let session_idx = match self.sessions.iter().position(|s| s.id == clean_id) {
            Some(idx) => {
                self.sessions[idx].label = clean_label.to_string();
                idx
            }
            None => {
                let colour_index = self.alloc_colour_index();
                self.sessions.push(Session {
                    id: clean_id.to_string(),
                    colour_index,
                    label: clean_label.to_string(),
                    page: None,
                    has_cursor: false,
                    cursor_page: Pos2::ZERO,
                    target_page: Pos2::ZERO,
                    first_write_s: now_s,
                    last_write_s: now_s,
                    fade: 0.0,
                    flashes: Vec::new(),
                    insert_fades: Vec::new(),
                });
                self.sessions.len() - 1
            }
        };

        let session = &mut self.sessions[session_idx];
        for &(id, kind) in &report.touched {
            let target_node_id = match kind {
                TouchKind::Removed => report.removed_parents.get(&id).copied().unwrap_or(id),
                _ => id,
            };
            let Some(center) = node_center_page(doc, index, target_node_id) else {
                continue;
            };
            session.page = doc.page_of(target_node_id);
            let had_to_travel = session.has_cursor && (center - session.cursor_page).length() > 1.0;
            if !session.has_cursor {
                session.cursor_page = center;
                session.has_cursor = true;
            }
            session.target_page = center;
            session.last_write_s = now_s;

            if kind == TouchKind::Inserted && is_top_level_frame(doc, target_node_id) {
                if let Some(existing) = session.insert_fades.iter_mut().find(|f| f.node_id == target_node_id) {
                    existing.start_s = now_s;
                } else {
                    session.insert_fades.push(TopLevelInsertFade { node_id: target_node_id, start_s: now_s });
                }
            }

            // Rule 4: top-level frames do not flash; a removed node flashes its parent instead.
            let should_flash = match kind {
                TouchKind::Removed => doc.get(target_node_id).is_some_and(|n| !matches!(n.kind, NodeKind::Page { .. })),
                TouchKind::Inserted | TouchKind::Updated | TouchKind::Moved => {
                    !is_top_level_frame(doc, target_node_id) && doc.get(target_node_id).is_some_and(|n| !matches!(n.kind, NodeKind::Page { .. }))
                }
            };
            if should_flash {
                let stroke_px = match kind {
                    TouchKind::Inserted => 1.0,
                    TouchKind::Updated | TouchKind::Moved | TouchKind::Removed => 2.0,
                };
                let start_s = if had_to_travel { now_s + FLASH_TRAVEL_DELAY_S } else { now_s };
                if let Some(existing) = session.flashes.iter_mut().find(|f| f.node_id == target_node_id) {
                    existing.start_s = start_s;
                    existing.stroke_px = stroke_px;
                } else {
                    if session.flashes.len() >= MAX_FLASHES_PER_SESSION {
                        session.flashes.remove(0);
                    }
                    session.flashes.push(Flash { node_id: target_node_id, start_s, stroke_px });
                }
            }
        }
    }

    /// Advances cursor positions, fades, and flash lifetimes to `now_s` with frame delta `dt`.
    pub fn step(&mut self, now_s: f64, dt: f32) {
        for session in &mut self.sessions {
            session.cursor_page = follow_pos(session.cursor_page, session.target_page, dt);
            let since_first = (now_s - session.first_write_s).max(0.0);
            let since_last = (now_s - session.last_write_s).max(0.0);
            session.fade = cursor_fade_alpha(since_first, since_last);
            session.flashes.retain(|f| now_s < f.start_s + FLASH_TOTAL_S);
            session.insert_fades.retain(|f| now_s < f.start_s + TOP_LEVEL_INSERT_SETTLE_S);
        }
        self.sessions.retain(|s| {
            let since_last = (now_s - s.last_write_s).max(0.0);
            since_last < CURSOR_IDLE_HOLD_S + CURSOR_FADE_OUT_S || !s.flashes.is_empty() || !s.insert_fades.is_empty()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::live_patch_apply::apply_live_patches;
    use designos_doc::live_patch::{LiveOp, LivePatch, SiblingHint};
    use designos_doc::{Affine, Color, FrameProps, Node, Paint, Vec2 as DV};
    use egui::{Id, Pos2, RawInput, Rect as ERect, Vec2};
    use serde_json::{Map, Value};
    use std::time::Duration;

    fn update_name_op(id: NodeId, new_name: &str) -> LiveOp {
        let mut set = Map::new();
        set.insert("name".into(), Value::String(new_name.into()));
        LiveOp::Update { id, set, unset: Vec::new() }
    }

    fn make_doc_with_frame_and_three_children() -> (Document, NodeId, [NodeId; 3]) {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let frame_id = doc.alloc_id();
        let mut frame = Node::new(frame_id, NodeKind::Frame(FrameProps::default()));
        frame.name = "Card".into();
        frame.size = DV::new(400.0, 300.0);
        frame.transform = Affine::translate(40.0, 40.0);
        doc.insert(page, frame).unwrap();

        let mut children = [NodeId(0); 3];
        for (i, slot) in children.iter_mut().enumerate() {
            let cid = doc.alloc_id();
            let mut child = Node::new(cid, NodeKind::Rectangle);
            child.name = format!("Item {i}");
            child.size = DV::new(80.0, 40.0);
            child.transform = Affine::translate(20.0 + (i as f32) * 100.0, 30.0);
            child.fills.push(Paint::solid(Color::rgb(200, 100, 80)));
            doc.insert(frame_id, child).unwrap();
            *slot = cid;
        }
        (doc, frame_id, children)
    }

    fn recorded_rect(ctx: &egui::Context, name: &str) -> Option<ERect> {
        ctx.data_mut(|d| d.get_temp::<ERect>(Id::new(("test-rect", name.to_string()))))
    }

    #[test]
    fn follow_reaches_target_with_rate_2_5() {
        let start = 0.0_f32;
        let target = 100.0_f32;
        let after_1s = follow_scalar(start, target, 1.0);
        let remaining_ratio = (target - after_1s) / (target - start);
        let expected = (-2.5_f32).exp();
        assert!((remaining_ratio - expected).abs() < 1e-5, "expected remaining ratio exp(-2.5) = {expected}, got {remaining_ratio}");
    }

    #[test]
    fn flash_envelope_is_200_400_300() {
        let a_100 = flash_envelope(0.100);
        let a_400 = flash_envelope(0.400);
        let a_700 = flash_envelope(0.700);
        let a_950 = flash_envelope(0.950);
        assert!((a_100 - 0.5).abs() < 1e-5, "100 ms should be 0.5, got {a_100}");
        assert!((a_400 - 1.0).abs() < 1e-5, "400 ms should be 1.0, got {a_400}");
        let expected_700 = (1.0 - (0.700 - 0.600) / 0.300) as f32;
        assert!((a_700 - expected_700).abs() < 1e-5, "700 ms should be {expected_700}, got {a_700}");
        assert_eq!(a_950, 0.0, "950 ms should be 0.0");
    }

    #[test]
    fn three_writes_show_one_cursor_and_three_flashes() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);
        app.now = 1.0;

        let patches: Vec<LivePatch> =
            children.iter().enumerate().map(|(idx, &cid)| LivePatch::new(vec![update_name_op(cid, &format!("Updated {idx}"))])).collect();
        apply_live_patches(&mut app, &patches);

        assert_eq!(app.agent_presence.sessions.len(), 1);
        assert_eq!(app.agent_presence.sessions[0].color(), theme::AGENT_1);
        assert_eq!(app.agent_presence.sessions[0].flashes.len(), 3);

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        // At t = 1.35 s, flash 0 is in its hold phase and flashes 1 & 2 have passed their 300ms travel delay.
        let raw = RawInput { screen_rect: Some(canvas_rect), time: Some(1.35), ..Default::default() };
        app.now = 1.35;
        let _ = ctx.run_ui(raw, |ui| {
            crate::canvas::agent_overlay::paint(&mut app, ui, canvas_rect);
        });

        assert!(recorded_rect(&ctx, "agent-cursor-0").is_some(), "expected one agent cursor rect");
        assert!(recorded_rect(&ctx, "agent-cursor-1").is_none(), "expected only one cursor");
        assert!(recorded_rect(&ctx, "agent-flash-0").is_some(), "expected flash 0");
        assert!(recorded_rect(&ctx, "agent-flash-1").is_some(), "expected flash 1");
        assert!(recorded_rect(&ctx, "agent-flash-2").is_some(), "expected flash 2");
    }

    #[test]
    fn flash_waits_300_ms_when_the_cursor_travels() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);

        // First write at t = 1.0 on child 0.
        app.now = 1.0;
        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(children[0], "First")])]);
        assert_eq!(app.agent_presence.sessions[0].flashes[0].start_s, 1.0);

        // Second write at t = 2.0 on child 2 (200 px away -> cursor must travel).
        app.now = 2.0;
        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(children[2], "Second")])]);
        let flash2 = app.agent_presence.sessions[0].flashes.iter().find(|f| f.node_id == children[2]).expect("flash for child 2");
        assert!(
            (flash2.start_s - (2.0 + FLASH_TRAVEL_DELAY_S)).abs() < 1e-6,
            "expected delayed flash start at {}, got {}",
            2.0 + FLASH_TRAVEL_DELAY_S,
            flash2.start_s
        );
    }

    #[test]
    fn top_level_frames_do_not_flash() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.now = 1.0;

        let frame_id = NodeId(500);
        let mut frame = Node::new(frame_id, NodeKind::Frame(FrameProps::default()));
        frame.name = "Hero Frame".into();
        frame.size = DV::new(800.0, 600.0);
        frame.parent = Some(page);
        let patch = LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![frame] }]);
        apply_live_patches(&mut app, &[patch]);

        assert_eq!(app.agent_presence.sessions.len(), 1, "cursor still targets top-level frame");
        assert!(app.agent_presence.sessions[0].flashes.is_empty(), "top-level frames must not produce an outline flash");
    }

    #[test]
    fn two_agents_two_colours() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);
        app.now = 1.0;

        let p1 = LivePatch::new(vec![update_name_op(children[0], "By Claude")]).with_agent("session-claude", "Claude");
        let p2 = LivePatch::new(vec![update_name_op(children[1], "By Codex")]).with_agent("session-codex", "Codex");

        apply_live_patches(&mut app, &[p1, p2]);

        assert_eq!(app.agent_presence.sessions.len(), 2);
        let c0 = app.agent_presence.sessions[0].color();
        let c1 = app.agent_presence.sessions[1].color();
        assert_ne!(c0, c1, "two active agent sessions must receive distinct colours");
        assert_eq!(c0, theme::AGENT_1);
        assert_eq!(c1, theme::AGENT_2);
    }

    #[test]
    fn cursor_is_gone_five_seconds_after_the_last_write() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);
        app.now = 1.0;

        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(children[0], "Touched")])]);
        assert_eq!(app.agent_presence.sessions.len(), 1);

        // At 1.0 + 5.0 + 1.5 = 7.5 s, the 5 s hold + 1.5 s fade-out has completed.
        app.agent_presence.step(1.0 + CURSOR_IDLE_HOLD_S + CURSOR_FADE_OUT_S, 0.016);
        assert!(app.agent_presence.sessions.is_empty(), "cursor session must be gone 5s + 1.5s after the last write");
    }

    #[test]
    fn hidden_draws_nothing_and_asks_for_no_frame() {
        let tmp_dir = std::env::temp_dir().join(format!("designos_cursor_size_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp_dir);
        for mode in [AgentCursorSize::Small, AgentCursorSize::Medium, AgentCursorSize::Large, AgentCursorSize::Hidden] {
            mode.save(&tmp_dir).expect("save cursor size choice");
            assert_eq!(AgentCursorSize::load(&tmp_dir), mode);
        }
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);
        app.agent_presence.cursor_size = AgentCursorSize::Hidden;
        app.now = 1.0;

        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(children[0], "Hidden Edit")])]);

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        // Flush the one-shot font-atlas upload repaint from `install_fonts` before testing `agent_overlay::paint`.
        let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(1.0), ..Default::default() }, |_| {});
        let raw = RawInput { screen_rect: Some(canvas_rect), time: Some(1.05), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            crate::canvas::agent_overlay::paint(&mut app, ui, canvas_rect);
        });

        assert!(recorded_rect(&ctx, "agent-cursor-0").is_none(), "Hidden must draw no cursor");
        assert!(recorded_rect(&ctx, "agent-flash-0").is_none(), "Hidden must draw no flash");
        let delay = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).unwrap_or(Duration::MAX);
        assert!(delay >= Duration::from_secs(60), "Hidden must not request a repaint, got delay {delay:?}");
    }

    #[test]
    fn overlay_never_bumps_the_tile_revision() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);
        app.now = 1.0;

        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(children[0], "Step 1")])]);

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));

        // Render tiles once so `canvas.dirty` from the document edit is settled.
        let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(1.0), ..Default::default() }, |ui| {
            crate::canvas::tiles::paint(&mut app, ui, canvas_rect);
        });
        let rev_before = app.canvas.tiles.rev();

        // Run the overlay across the entire cursor & flash animation and verify `tiles.rev()` never changes.
        for &t in &[1.05, 1.2, 1.5, 2.0, 6.2, 7.6] {
            app.now = t;
            let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(t), ..Default::default() }, |ui| {
                crate::canvas::agent_overlay::paint(&mut app, ui, canvas_rect);
            });
            assert_eq!(app.canvas.tiles.rev(), rev_before, "agent_overlay::paint must never bump TileView::rev (at t={t})");
            assert!(!app.canvas.dirty, "agent_overlay::paint must never set app.canvas.dirty");
        }
    }

    #[test]
    fn cpu_export_is_pixel_identical_with_and_without_an_active_agent() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);

        let png_without_agent = designos_export::export_png(&app.doc, app.page, 1.0);

        // Record active agent presence on the same document state without mutating any node.
        let mut fake_report = LivePatchReport::default();
        fake_report.touched.push((children[0], TouchKind::Updated));
        fake_report.touched.push((children[1], TouchKind::Inserted));
        app.agent_presence.record_patch("session-1", "Claude", &app.doc, None, &fake_report, 1.0);
        assert!(!app.agent_presence.sessions.is_empty());

        let png_with_agent = designos_export::export_png(&app.doc, app.page, 1.0);
        assert_eq!(png_without_agent, png_with_agent, "CPU export must be pixel-identical with and without an active agent");
    }

    #[test]
    fn batch_writes_in_same_frame_keep_cursor_at_first_node_and_delay_later_flashes() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);
        app.now = 1.0;

        let patches: Vec<LivePatch> =
            children.iter().enumerate().map(|(idx, &cid)| LivePatch::new(vec![update_name_op(cid, &format!("Batch {idx}"))])).collect();
        apply_live_patches(&mut app, &patches);

        let s = &app.agent_presence.sessions[0];
        let c0_center = super::node_center_page(&app.doc, None, children[0]).unwrap();
        let c2_center = super::node_center_page(&app.doc, None, children[2]).unwrap();
        assert_eq!(s.cursor_page, c0_center, "cursor must start on the first touched node, not jump to the last");
        assert_eq!(s.target_page, c2_center, "target_page must point to the last touched node");
        assert_eq!(s.flashes[0].start_s, 1.0, "first node flash starts immediately");
        assert!((s.flashes[1].start_s - (1.0 + FLASH_TRAVEL_DELAY_S)).abs() < 1e-6, "second node in same-frame batch must delay flash by 300ms");
        assert!((s.flashes[2].start_s - (1.0 + FLASH_TRAVEL_DELAY_S)).abs() < 1e-6, "third node in same-frame batch must delay flash by 300ms");
    }

    #[test]
    fn hold_frames_keep_rearming_repaint_after_until_five_seconds() {
        let (doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let mut app = App::new(doc);
        app.now = 1.0;
        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(children[0], "HoldWake")])]);

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        // Flush font upload frame.
        let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(1.0), ..Default::default() }, |_| {});

        // At t = 2.5 s (flash finished at 1.9 s, fade-in finished at 1.2 s), first hold frame schedules wake in 3.5 s.
        app.now = 2.5;
        let out1 = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(2.5), ..Default::default() }, |ui| {
            crate::canvas::agent_overlay::paint(&mut app, ui, canvas_rect);
        });
        let delay1 = out1.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).unwrap_or(Duration::MAX);
        assert!((delay1.as_secs_f64() - 3.5).abs() < 0.05, "expected ~3.5s wake delay at t=2.5s, got {delay1:?}");

        // An unrelated hover/toast frame arrives at t = 3.0 s during the hold window; it MUST re-arm the wake for the remaining 3.0 s.
        app.now = 3.0;
        let out2 = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(3.0), ..Default::default() }, |ui| {
            crate::canvas::agent_overlay::paint(&mut app, ui, canvas_rect);
        });
        let delay2 = out2.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).unwrap_or(Duration::MAX);
        assert!((delay2.as_secs_f64() - 3.0).abs() < 0.05, "expected ~3.0s wake delay at t=3.0s, got {delay2:?}");
    }

    #[test]
    fn switching_page_or_tab_resets_presence_and_ignores_other_pages() {
        let (mut doc, _frame_id, children) = make_doc_with_frame_and_three_children();
        let page2 = doc.add_page("Page 2");
        let mut app = App::new(doc);
        app.now = 1.0;

        apply_live_patches(&mut app, &[LivePatch::new(vec![update_name_op(children[0], "Page1Edit")])]);
        assert_eq!(app.agent_presence.sessions.len(), 1);

        // Switching to Page 2 clears presence on the next overlay paint so Page 1's cursor/flashes never bleed onto Page 2.
        app.page = page2;
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let canvas_rect = ERect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        app.now = 1.1;
        let _ = ctx.run_ui(RawInput { screen_rect: Some(canvas_rect), time: Some(1.1), ..Default::default() }, |ui| {
            crate::canvas::agent_overlay::paint(&mut app, ui, canvas_rect);
        });
        assert!(recorded_rect(&ctx, "agent-cursor-0").is_none(), "cursor from Page 1 must not draw on Page 2");
        assert!(recorded_rect(&ctx, "agent-flash-0").is_none(), "flash from Page 1 must not draw on Page 2");
    }

    #[test]
    fn cursor_fade_in_starts_at_zero_and_top_level_insert_fades_with_rate_5() {
        assert_eq!(cursor_fade_alpha(0.0, 0.0), 0.0, "cursor fade-in must start at 0.0 at t=0 on ease-out-cubic");
        let mid_fade = cursor_fade_alpha(0.1, 0.1);
        assert!((mid_fade - 0.875).abs() < 1e-4, "at 100ms (u=0.5), ease-out-cubic is 0.875, got {mid_fade}");

        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.now = 1.0;

        let frame_id = NodeId(500);
        let mut frame = Node::new(frame_id, NodeKind::Frame(FrameProps::default()));
        frame.name = "Hero Frame".into();
        frame.size = DV::new(800.0, 600.0);
        frame.fills.push(Paint::solid(Color::rgba(255, 0, 0, 255)));
        frame.parent = Some(page);
        let patch = LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![frame] }]);
        apply_live_patches(&mut app, &[patch]);

        // Document opacity must remain 1.0 (never mutated in document).
        let inserted_id = app.selection[0];
        assert_eq!(app.doc.get(inserted_id).unwrap().opacity, 1.0);
        // Runtime insert fade starts at 0.0 at t=1.0 and reaches 1 - exp(-5 * 0.2) at t=1.2.
        assert_eq!(app.agent_presence.top_level_insert_opacity(inserted_id, 1.0), 0.0);
        let at_200ms = app.agent_presence.top_level_insert_opacity(inserted_id, 1.2);
        let expected = 1.0 - (-5.0_f32 * 0.2).exp();
        assert!((at_200ms - expected).abs() < 1e-5, "expected {expected}, got {at_200ms}");

        // Verify CPU tile path and GPU compile both multiply draw opacity without bumping TileView::rev.
        let vp = designos_render::Viewport { offset: DV::ZERO, zoom: 1.0, dpr: 1.0 };
        let sample_tile_red = |app: &mut App| -> u8 {
            let opacities = app.agent_presence.active_top_level_insert_opacities(&app.doc, app.page, app.now);
            let rev = crate::canvas::tiles::take_rev(app);
            app.canvas.tiles.cache.set_runtime_opacities(&app.doc, app.page, opacities);
            let tf = app.canvas.tiles.cache.render_tiles(&app.doc, app.page, &vp, (512, 512), rev, designos_render::TileBudget::unlimited());
            let tile = tf.tiles.iter().find(|t| t.key.x == 0 && t.key.y == 0).expect("tile (0,0)");
            let idx = (100 * (designos_render::TILE_SIZE as usize) + 100) * 4;
            tile.pixmap.data()[idx]
        };

        let r_t0 = sample_tile_red(&mut app);
        let rev_initial = app.canvas.tiles.rev();
        assert_eq!(r_t0, 0x5A, "at t=1.0 (fade alpha 0.0), rendered CPU tile red channel must equal page background 0x5A (90)");

        app.canvas.gpu.compiler.opts.runtime_node_opacities = app.agent_presence.active_top_level_insert_opacities(&app.doc, app.page, 1.0);
        let gpu_t0 = app.canvas.gpu.compiler.compile(&app.doc, app.page, rev_initial, &vp, (512, 512));
        assert_eq!(gpu_t0.stats().draws, 0, "at t=1.0 (fade alpha 0.0), GPU compile must skip drawing the 0-opacity frame");

        app.now = 1.2;
        let r_mid = sample_tile_red(&mut app);
        assert_eq!(app.canvas.tiles.rev(), rev_initial, "runtime fade must not bump TileView::rev");
        assert!(r_mid > 180 && r_mid < 230, "at t=1.2 (fade alpha ~0.632), CPU tile red channel must be blended above 0x5A and below 255, got {r_mid}");

        app.canvas.gpu.compiler.opts.runtime_node_opacities = app.agent_presence.active_top_level_insert_opacities(&app.doc, app.page, 1.2);
        let gpu_mid = app.canvas.gpu.compiler.compile(&app.doc, app.page, rev_initial, &vp, (512, 512));
        assert!(gpu_mid.stats().draws > 0, "at t=1.2, GPU compile must emit draws for the fading top-level frame");

        app.now = 2.5;
        app.agent_presence.step(2.5, 0.016);
        let r_settled = sample_tile_red(&mut app);
        assert_eq!(app.canvas.tiles.rev(), rev_initial, "settling runtime fade must not bump TileView::rev");
        assert_eq!(r_settled, 255, "at t=2.5 (settled), CPU tile red channel must be 255");
        assert_eq!(app.doc.get(inserted_id).unwrap().opacity, 1.0, "Node.opacity in Document must never be mutated");
    }
}
