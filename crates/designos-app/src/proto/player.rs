//! The prototype player: a UI-free state machine that runs reactions against
//! a scratch copy of the document. The Present view feeds it pointer/key
//! events plus the clock and draws whatever [`Player`] says is on screen.

use super::{easing, geom};
use designos_doc::*;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Most nested actions (conditionals inside conditionals) run in one go.
const MAX_ACTION_DEPTH: usize = 8;
/// Most actions one trigger may run (guards runaway conditional nesting).
const MAX_ACTIONS_PER_TRIGGER: usize = 256;
/// Most error lines kept for the debug panel.
const MAX_ERRORS: usize = 20;

/// Things the host must do outside the player (the player never touches the OS).
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    OpenUrl { url: String, new_tab: bool },
}

/// Event kinds a trigger can match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    Click,
    Drag,
    HoverOn,
    PressOn,
    Enter,
    Leave,
    Down,
    Up,
}

pub fn trigger_matches(t: &Trigger, w: When) -> bool {
    matches!(
        (t, w),
        (Trigger::OnClick, When::Click)
            | (Trigger::OnDrag, When::Drag)
            | (Trigger::WhileHovering, When::HoverOn)
            | (Trigger::WhilePressing, When::PressOn)
            | (Trigger::MouseEnter { .. }, When::Enter)
            | (Trigger::MouseLeave { .. }, When::Leave)
            | (Trigger::MouseDown { .. }, When::Down)
            | (Trigger::MouseUp { .. }, When::Up)
    )
}

fn trigger_delay_ms(t: &Trigger) -> u32 {
    match t {
        Trigger::MouseEnter { delay_ms }
        | Trigger::MouseLeave { delay_ms }
        | Trigger::MouseDown { delay_ms }
        | Trigger::MouseUp { delay_ms }
        | Trigger::AfterDelay { delay_ms } => *delay_ms,
        _ => 0,
    }
}

/// Order-insensitive, case-insensitive comparison of "Ctrl+K" style key strings.
/// Command, Cmd, Meta and Control are all "ctrl" (the host maps the platform
/// command key to Ctrl).
pub fn keys_match(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let mut v: Vec<String> = s
            .split('+')
            .map(|k| k.trim().to_ascii_lowercase())
            .filter(|k| !k.is_empty())
            .map(|k| match k.as_str() {
                "cmd" | "command" | "meta" | "control" | "ctl" => "ctrl".to_string(),
                "esc" => "escape".to_string(),
                "return" => "enter".to_string(),
                _ => k,
            })
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let (x, y) = (norm(a), norm(b));
    !x.is_empty() && x == y
}

/// A screen kept for Back, with the transition that undoes how we left it.
#[derive(Clone, Debug, PartialEq)]
pub struct BackEntry {
    pub frame: NodeId,
    pub via: Option<Transition>,
}

/// Content of a frame as it was before a ChangeTo, kept for the transition.
pub struct Snapshot {
    pub id: u64,
    pub doc: Document,
}

/// A running frame-to-frame transition.
pub struct Anim {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: TransitionKind,
    pub easing: Easing,
    pub start: f64,
    pub dur: f64,
    /// `from` lives in this snapshot instead of the live document.
    pub snapshot: Option<Snapshot>,
}

impl Anim {
    pub fn raw(&self, now: f64) -> f32 {
        if self.dur <= 0.0 {
            1.0
        } else {
            (((now - self.start) / self.dur) as f32).clamp(0.0, 1.0)
        }
    }
    pub fn eased(&self, now: f64) -> f32 {
        easing::ease(&self.easing, self.raw(now))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayAnim {
    pub start: f64,
    pub dur: f64,
    pub opening: bool,
}

#[derive(Clone, Debug)]
pub struct Overlay {
    pub frame: NodeId,
    /// Top-left inside the frame it covers.
    pub origin: Vec2,
    pub background: Option<Color>,
    pub close_outside: bool,
    pub position: OverlayPosition,
    pub offset: Vec2,
    pub transition: Option<Transition>,
    pub anim: Option<OverlayAnim>,
    /// Extra offset while the overlay is dragged (On drag).
    pub drag: Vec2,
}

impl Overlay {
    pub fn closing(&self) -> bool {
        self.anim.is_some_and(|a| !a.opening)
    }
    /// 0 hidden .. 1 shown (eased).
    pub fn shown(&self, now: f64) -> f32 {
        let Some(a) = self.anim else { return 1.0 };
        let raw = if a.dur <= 0.0 { 1.0 } else { (((now - a.start) / a.dur) as f32).clamp(0.0, 1.0) };
        let e = self.transition.map(|t| t.easing).unwrap_or(Easing::EaseOut);
        let v = easing::ease(&e, raw);
        if a.opening {
            v
        } else {
            1.0 - v
        }
    }
}

struct ScrollAnim {
    frame: NodeId,
    from: Vec2,
    to: Vec2,
    easing: Easing,
    start: f64,
    dur: f64,
}

struct Timer {
    at: f64,
    node: NodeId,
    reaction: Reaction,
    /// Only fires while this frame is on screen.
    frame: NodeId,
}

enum Undo {
    /// Revert an instance to its previous variant, animating with the same transition.
    RestoreComponent {
        inst: NodeId,
        comp: NodeId,
        transition: Option<Transition>,
    },
    CloseOverlay {
        frame: NodeId,
    },
}

struct Active {
    node: NodeId,
    when: When,
    undo: Vec<Undo>,
}

/// Playback state of one video layer. Positions are seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaState {
    pub playing: bool,
    pub muted: bool,
    pub looping: bool,
    /// Position at `since` (the player clock).
    pub base: f64,
    pub since: f64,
    /// Length of the video, once the host has opened it.
    pub duration: Option<f64>,
}

impl MediaState {
    /// Current position at clock `now`, wrapped for looping videos and held at
    /// the end otherwise.
    pub fn position(&self, now: f64) -> f64 {
        let raw = if self.playing { self.base + (now - self.since).max(0.0) } else { self.base };
        match self.duration {
            Some(d) if d > 0.0 => {
                if self.looping {
                    raw.rem_euclid(d)
                } else {
                    raw.clamp(0.0, d)
                }
            }
            _ => raw.max(0.0),
        }
    }
}

/// What an On drag gesture does, decided when the drag starts.
enum DragKind {
    /// Drags `overlays[frame]`; releasing far enough runs the reaction (Close overlay).
    Overlay { node: NodeId, frame: NodeId, reaction: Reaction },
    /// Scrolls `frame` with the pointer (the reaction's Scroll to).
    Scroll { frame: NodeId },
}

/// A row of the Present variables panel.
#[derive(Clone, Debug, PartialEq)]
pub struct VarRow {
    pub id: String,
    pub name: String,
    pub collection: String,
    pub mode: String,
    pub value: String,
    /// Changed by an action since the presentation started.
    pub changed: bool,
}

pub struct Player {
    /// Scratch copy of the document; ChangeTo edits this, never the user's file.
    pub doc: Document,
    pub current: NodeId,
    pub back: Vec<BackEntry>,
    pub overlays: Vec<Overlay>,
    pub anim: Option<Anim>,
    pub scroll: BTreeMap<NodeId, Vec2>,
    /// Bumped per top-level frame whenever its content changes (texture cache key).
    pub rev: BTreeMap<NodeId, u64>,
    pub effects: Vec<Effect>,
    pub now: f64,
    /// Variables (ids) changed by actions since the presentation started.
    pub changed_vars: BTreeSet<String>,
    /// Problems met while running actions (bad expressions...), newest last.
    pub errors: Vec<String>,
    /// Playback state of video layers that have been shown.
    pub media: BTreeMap<NodeId, MediaState>,
    scroll_anim: Option<ScrollAnim>,
    timers: Vec<Timer>,
    hover: Vec<NodeId>,
    actives: Vec<Active>,
    press_target: Option<NodeId>,
    drag: Option<DragKind>,
    snapshots: u64,
}

impl Player {
    pub fn new(doc: &Document, start: NodeId) -> Self {
        let mut p = Player {
            doc: doc.clone(),
            current: start,
            back: Vec::new(),
            overlays: Vec::new(),
            anim: None,
            scroll: BTreeMap::new(),
            rev: BTreeMap::new(),
            effects: Vec::new(),
            now: 0.0,
            changed_vars: BTreeSet::new(),
            errors: Vec::new(),
            media: BTreeMap::new(),
            scroll_anim: None,
            timers: Vec::new(),
            hover: Vec::new(),
            actives: Vec::new(),
            press_target: None,
            drag: None,
            snapshots: 0,
        };
        inherit_variant_reactions(&mut p.doc);
        p.current = p.top_frame_of(start).unwrap_or(start);
        p.on_shown(p.current);
        p
    }

    /// Starts over from `start` on a fresh copy of `original`.
    pub fn restart(&mut self, original: &Document, start: NodeId) {
        let now = self.now;
        let rev = std::mem::take(&mut self.rev);
        *self = Player::new(original, start);
        self.now = now;
        // keep revisions growing so cached textures of the old copy are not reused
        self.rev = rev.into_iter().map(|(k, v)| (k, v + 1)).collect();
        self.on_shown(self.current);
    }

    // ------------------------------------------------------------ structure

    /// The page-level frame containing `id` (or `id` itself when top-level).
    pub fn top_frame_of(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            let n = self.doc.get(id)?;
            let p = n.parent?;
            if matches!(self.doc.get(p)?.kind, NodeKind::Page { .. }) {
                return matches!(n.kind, NodeKind::Page { .. }).then_some(id).or(Some(id));
            }
            id = p;
        }
    }

    pub fn size_of(&self, frame: NodeId) -> Vec2 {
        self.doc.get(frame).map(|n| n.size).unwrap_or(Vec2::ZERO)
    }

    pub fn frame_rev(&self, frame: NodeId) -> u64 {
        self.rev.get(&frame).copied().unwrap_or(0)
    }

    fn bump(&mut self, frame: NodeId) {
        *self.rev.entry(frame).or_insert(0) += 1;
    }

    /// The content of `node` changed (a new video frame): redraw the frame it is in.
    pub fn invalidate(&mut self, node: NodeId) {
        if let Some(f) = self.top_frame_of(node) {
            self.bump(f);
        }
    }

    /// Frames currently on screen from bottom to top.
    pub fn visible_frames(&self) -> Vec<NodeId> {
        let mut v = vec![self.current];
        v.extend(self.overlays.iter().map(|o| o.frame));
        v
    }

    fn frame_visible(&self, f: NodeId) -> bool {
        self.current == f || self.overlays.iter().any(|o| o.frame == f && !o.closing())
    }

    pub fn is_animating(&self) -> bool {
        self.anim.is_some()
            || self.scroll_anim.is_some()
            || self.overlays.iter().any(|o| o.anim.is_some())
            || !self.timers.is_empty()
            || self.media.values().any(|m| m.playing)
    }

    /// Whether anything at or above `target` reacts to the pointer (for the hand cursor).
    pub fn is_interactive(&self, target: NodeId) -> bool {
        [When::Click, When::Drag, When::Down, When::Up, When::HoverOn, When::PressOn, When::Enter, When::Leave]
            .iter()
            .any(|w| self.find_reactive(target, *w).is_some())
    }

    fn reactions_on(&self, node: NodeId, w: When) -> Vec<Reaction> {
        self.doc.get(node).map(|n| n.prototype.reactions.iter().filter(|r| trigger_matches(&r.trigger, w)).cloned().collect()).unwrap_or_default()
    }

    /// Deepest node from `target` up to its frame that has a reaction for `w`.
    fn find_reactive(&self, target: NodeId, w: When) -> Option<NodeId> {
        let mut cur = Some(target);
        while let Some(id) = cur {
            if !self.reactions_on(id, w).is_empty() {
                return Some(id);
            }
            cur = self.doc.get(id).and_then(|n| n.parent).filter(|p| !matches!(self.doc.get(*p).map(|n| &n.kind), Some(NodeKind::Page { .. })));
        }
        None
    }

    // ------------------------------------------------------------- clock

    /// Aligns the player's clock with the host's (timers keep their delay).
    pub fn rebase_clock(&mut self, now: f64) {
        let d = now - self.now;
        for t in &mut self.timers {
            t.at += d;
        }
        self.now = now;
    }

    /// Advances the clock: finishes transitions, runs due timers.
    pub fn tick(&mut self, now: f64) {
        self.now = now;
        if let Some(a) = &self.anim {
            if now >= a.start + a.dur {
                self.anim = None;
                self.on_shown(self.current);
            }
        }
        let mut shown = Vec::new();
        self.overlays.retain_mut(|o| {
            if let Some(a) = o.anim {
                if now >= a.start + a.dur {
                    o.anim = None;
                    if a.opening {
                        shown.push(o.frame);
                    } else {
                        return false;
                    }
                }
            }
            true
        });
        for f in shown {
            self.on_shown(f);
        }
        if let Some(s) = &self.scroll_anim {
            let raw = if s.dur <= 0.0 { 1.0 } else { (((now - s.start) / s.dur) as f32).clamp(0.0, 1.0) };
            let e = easing::ease(&s.easing, raw);
            let v = Vec2::new(s.from.x + (s.to.x - s.from.x) * e, s.from.y + (s.to.y - s.from.y) * e);
            let f = s.frame;
            self.scroll.insert(f, v);
            if raw >= 1.0 {
                self.scroll_anim = None;
            }
        }
        // timers
        let mut due = Vec::new();
        let mut i = 0;
        while i < self.timers.len() {
            if self.timers[i].at <= now {
                due.push(self.timers.remove(i));
            } else {
                i += 1;
            }
        }
        for t in due {
            if self.frame_visible(t.frame) && self.doc.get(t.node).is_some() {
                self.run(t.node, &t.reaction, None);
            }
        }
    }

    fn finish_anim(&mut self) {
        if self.anim.take().is_some() {
            self.on_shown(self.current);
        }
        let mut shown = Vec::new();
        self.overlays.retain_mut(|o| match o.anim.take() {
            Some(a) if a.opening => {
                shown.push(o.frame);
                true
            }
            Some(_) => false,
            None => true,
        });
        for f in shown {
            self.on_shown(f);
        }
        if let Some(s) = self.scroll_anim.take() {
            self.scroll.insert(s.frame, s.to);
        }
    }

    /// Schedules the AfterDelay reactions of everything in `frame`.
    fn on_shown(&mut self, frame: NodeId) {
        self.timers.retain(|t| t.frame != frame);
        let mut stack = vec![frame];
        let mut found = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(n) = self.doc.get(id) else { continue };
            for r in &n.prototype.reactions {
                if let Trigger::AfterDelay { delay_ms } = r.trigger {
                    found.push((id, r.clone(), delay_ms));
                }
            }
            stack.extend(n.children.iter().copied());
        }
        for (node, reaction, ms) in found {
            self.timers.push(Timer { at: self.now + ms as f64 / 1000.0, node, reaction, frame });
        }
        self.init_media(frame);
    }

    /// Creates the playback state of video layers in `frame` the first time it is shown.
    fn init_media(&mut self, frame: NodeId) {
        let mut stack = vec![frame];
        let mut found = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(n) = self.doc.get(id) else { continue };
            if let Some(v) = n.prototype.videos.first() {
                found.push((id, v.autoplay, v.muted, v.looping));
            }
            stack.extend(n.children.iter().copied());
        }
        for (id, autoplay, muted, looping) in found {
            let now = self.now;
            self.media.entry(id).or_insert(MediaState { playing: autoplay, muted, looping, base: 0.0, since: now, duration: None });
        }
    }

    /// Sets the length of a video once the host has opened it.
    pub fn set_media_duration(&mut self, node: NodeId, seconds: f64) {
        if let Some(m) = self.media.get_mut(&node) {
            m.duration = Some(seconds).filter(|d| *d > 0.0);
        }
    }

    /// Applies a media action to the video layer `dest`.
    pub(crate) fn media_control(&mut self, dest: NodeId, op: MediaOp) {
        if self.doc.get(dest).is_none_or(|n| n.prototype.videos.is_empty()) {
            return;
        }
        if !self.media.contains_key(&dest) {
            let v = self.doc.get(dest).and_then(|n| n.prototype.videos.first().cloned());
            let Some(v) = v else { return };
            self.media.insert(dest, MediaState { playing: false, muted: v.muted, looping: v.looping, base: 0.0, since: self.now, duration: None });
        }
        let now = self.now;
        let Some(m) = self.media.get_mut(&dest) else { return };
        let pos = m.position(now);
        match op {
            MediaOp::Play => {
                if !m.playing {
                    m.base = if m.duration.is_some_and(|d| !m.looping && pos >= d) { 0.0 } else { pos };
                    m.playing = true;
                    m.since = now;
                }
            }
            MediaOp::Pause => {
                m.base = pos;
                m.playing = false;
            }
            MediaOp::TogglePlay => {
                let next = if m.playing { MediaOp::Pause } else { MediaOp::Play };
                self.media_control(dest, next);
            }
            MediaOp::Mute => m.muted = true,
            MediaOp::Unmute => m.muted = false,
            MediaOp::ToggleMute => m.muted = !m.muted,
            MediaOp::SkipBy(s) => {
                m.base = (pos + s as f64).max(0.0);
                m.since = now;
            }
            MediaOp::SeekTo(s) => {
                m.base = (s as f64).max(0.0);
                m.since = now;
            }
            MediaOp::SetLoop(l) => {
                m.base = pos;
                m.since = now;
                m.looping = l;
            }
        }
    }

    fn schedule(&mut self, node: NodeId, r: &Reaction) {
        let frame = self.top_frame_of(node).unwrap_or(self.current);
        self.timers.push(Timer { at: self.now + trigger_delay_ms(&r.trigger) as f64 / 1000.0, node, reaction: r.clone(), frame });
    }

    // ------------------------------------------------------------ pointer

    fn fire(&mut self, node: NodeId, w: When, undo: Option<&mut Vec<Undo>>) {
        let rs = self.reactions_on(node, w);
        let mut undo = undo;
        for r in rs {
            if trigger_delay_ms(&r.trigger) > 0 {
                self.schedule(node, &r);
            } else {
                self.run(node, &r, undo.as_deref_mut());
            }
        }
    }

    /// Pointer moved; `target` is the deepest layer under it (None over nothing).
    pub fn pointer_move(&mut self, target: Option<NodeId>) {
        let mut chain = Vec::new();
        let mut cur = target;
        while let Some(id) = cur {
            if self.doc.get(id).is_none() {
                break;
            }
            // A node whose While hovering is active stays hovered while the pointer is over it, even
            // when the state it switched to (another variant) has no hover reaction of its own.
            let hovered_now = self.actives.iter().any(|a| a.node == id && a.when == When::HoverOn);
            let reactive = hovered_now || [When::Enter, When::Leave, When::HoverOn].iter().any(|w| !self.reactions_on(id, *w).is_empty());
            if reactive {
                chain.push(id);
            }
            cur = self.doc.get(id).and_then(|n| n.parent).filter(|p| !matches!(self.doc.get(*p).map(|n| &n.kind), Some(NodeKind::Page { .. })));
        }
        if chain == self.hover || self.anim.is_some() {
            return;
        }
        let old = std::mem::replace(&mut self.hover, chain.clone());
        for n in old.iter().filter(|n| !chain.contains(n)) {
            // hover ended: undo while-hovering effects, then Leave
            if let Some(i) = self.actives.iter().position(|a| a.node == *n && a.when == When::HoverOn) {
                let a = self.actives.remove(i);
                self.undo(a.undo);
            }
            self.fire(*n, When::Leave, None);
        }
        for n in chain.iter().filter(|n| !old.contains(n)).rev() {
            self.fire(*n, When::Enter, None);
            if !self.reactions_on(*n, When::HoverOn).is_empty() {
                let mut u = Vec::new();
                self.fire(*n, When::HoverOn, Some(&mut u));
                self.actives.push(Active { node: *n, when: When::HoverOn, undo: u });
            }
        }
    }

    pub fn pointer_down(&mut self, target: Option<NodeId>) {
        self.finish_anim();
        self.press_target = target;
        let Some(t) = target else { return };
        if let Some(n) = self.find_reactive(t, When::Down) {
            self.fire(n, When::Down, None);
        }
        if let Some(n) = self.find_reactive(t, When::PressOn) {
            let mut u = Vec::new();
            self.fire(n, When::PressOn, Some(&mut u));
            self.actives.push(Active { node: n, when: When::PressOn, undo: u });
        }
    }

    /// Pointer released. `dragged` is true when the press turned into a drag/scroll.
    pub fn pointer_up(&mut self, target: Option<NodeId>, dragged: bool) {
        self.finish_anim();
        self.drag_end();
        while let Some(i) = self.actives.iter().position(|a| a.when == When::PressOn) {
            let a = self.actives.remove(i);
            self.undo(a.undo);
        }
        self.press_target = None;
        if let Some(t) = target {
            if let Some(n) = self.find_reactive(t, When::Up) {
                self.fire(n, When::Up, None);
            }
        }
        if !dragged {
            self.click(target);
        }
    }

    /// A drag gesture started on `target`. Returns whether an On drag reaction took it.
    ///
    /// - On drag -> Close overlay on a layer inside an overlay: the overlay follows the pointer
    ///   ([`Player::drag_by`]); releasing it far enough ([`Player::drag_end`]) closes it, else it
    ///   snaps back (swipe to dismiss).
    /// - On drag -> Scroll to: the frame holding the destination scrolls with the pointer.
    /// - Anything else fires once, right away.
    pub fn drag(&mut self, target: Option<NodeId>) -> bool {
        let Some(n) = target.and_then(|t| self.find_reactive(t, When::Drag)) else { return false };
        self.finish_anim();
        let reaction = self.reactions_on(n, When::Drag).into_iter().next();
        if let Some(r) = reaction.filter(|r| !r.actions.is_empty() && trigger_delay_ms(&r.trigger) == 0) {
            if r.actions.iter().all(|a| matches!(a, Action::CloseOverlay)) {
                let owner = self.top_frame_of(n);
                if let Some(frame) = owner.filter(|f| self.overlays.iter().any(|o| o.frame == *f && !o.closing())) {
                    self.drag = Some(DragKind::Overlay { node: n, frame, reaction: r });
                    return true;
                }
            }
            if let [Action::ScrollTo { dest, .. }] = r.actions.as_slice() {
                if let Some(frame) = self.top_frame_of(*dest).filter(|f| self.frame_visible(*f)) {
                    self.drag = Some(DragKind::Scroll { frame });
                    return true;
                }
            }
        }
        self.fire(n, When::Drag, None);
        true
    }

    /// The pointer moved `delta` (frame units, `last - now` like the wheel: positive = content
    /// moves up/left) during a drag started with [`Player::drag`].
    pub fn drag_by(&mut self, delta: Vec2) {
        match &self.drag {
            Some(DragKind::Scroll { frame }) => {
                let f = *frame;
                self.scroll_by(f, delta);
            }
            Some(DragKind::Overlay { frame, .. }) => {
                let f = *frame;
                if let Some(o) = self.overlays.iter_mut().find(|o| o.frame == f) {
                    o.drag = Vec2::new(o.drag.x - delta.x, o.drag.y - delta.y);
                }
            }
            None => {}
        }
    }

    /// The drag ended: a dragged overlay that moved far enough runs its reaction.
    pub fn drag_end(&mut self) {
        let Some(kind) = self.drag.take() else { return };
        if let DragKind::Overlay { node, frame, reaction } = kind {
            let Some(i) = self.overlays.iter().position(|o| o.frame == frame) else { return };
            let size = self.size_of(frame);
            let moved = self.overlays[i].drag;
            let threshold = (size.x.min(size.y) * 0.25).max(40.0);
            if (moved.x * moved.x + moved.y * moved.y).sqrt() >= threshold {
                self.run(node, &reaction, None);
            } else {
                self.overlays[i].drag = Vec2::ZERO;
            }
        }
    }

    fn click(&mut self, target: Option<NodeId>) {
        // Close-on-click-outside: a click that is not inside the topmost overlay closes it.
        if let Some(top) = self.overlays.iter().rposition(|o| !o.closing()) {
            let o = &self.overlays[top];
            let inside = target.and_then(|t| self.top_frame_of(t)) == Some(o.frame);
            if o.close_outside && !inside {
                self.close_overlay_at(top);
                return;
            }
        }
        if let Some(n) = target.and_then(|t| self.find_reactive(t, When::Click)) {
            self.fire(n, When::Click, None);
        }
    }

    /// Keyboard shortcut like "Space" or "Ctrl+K".
    pub fn key(&mut self, pressed: &str) {
        self.finish_anim();
        let mut hits = Vec::new();
        for f in self.visible_frames() {
            let mut stack = vec![f];
            while let Some(id) = stack.pop() {
                let Some(n) = self.doc.get(id) else { continue };
                for r in &n.prototype.reactions {
                    if matches!(&r.trigger, Trigger::KeyPress { keys } if keys_match(keys, pressed)) {
                        hits.push((id, r.clone()));
                    }
                }
                stack.extend(n.children.iter().rev().copied());
            }
        }
        for (id, r) in hits {
            self.run(id, &r, None);
        }
    }

    /// Gamepad button chord like "A" or "DPadUp+A" from `device`. A trigger set to the generic
    /// device matches any controller; a specific one only matches its own.
    pub fn gamepad(&mut self, device: GamepadDevice, pressed: &str) {
        self.finish_anim();
        let mut hits = Vec::new();
        for f in self.visible_frames() {
            let mut stack = vec![f];
            while let Some(id) = stack.pop() {
                let Some(n) = self.doc.get(id) else { continue };
                for r in &n.prototype.reactions {
                    if matches!(&r.trigger, Trigger::GamepadPress { device: d, buttons }
                        if (*d == device || *d == GamepadDevice::Generic || device == GamepadDevice::Generic) && keys_match(buttons, pressed))
                    {
                        hits.push((id, r.clone()));
                    }
                }
                stack.extend(n.children.iter().rev().copied());
            }
        }
        for (id, r) in hits {
            self.run(id, &r, None);
        }
    }

    fn undo(&mut self, list: Vec<Undo>) {
        for u in list {
            match u {
                Undo::RestoreComponent { inst, comp, transition } => self.change_to(inst, comp, transition, None),
                Undo::CloseOverlay { frame } => {
                    if let Some(i) = self.overlays.iter().rposition(|o| o.frame == frame && !o.closing()) {
                        self.close_overlay_at(i);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------- actions

    fn run(&mut self, source: NodeId, r: &Reaction, undo: Option<&mut Vec<Undo>>) {
        let mut budget = MAX_ACTIONS_PER_TRIGGER;
        self.run_actions(source, &r.actions, r.transition, undo, 0, &mut budget);
    }

    /// Runs `actions` in order. Conditionals pick a branch and run its actions in place.
    fn run_actions(
        &mut self,
        source: NodeId,
        actions: &[Action],
        transition: Option<Transition>,
        mut undo: Option<&mut Vec<Undo>>,
        depth: usize,
        budget: &mut usize,
    ) {
        for a in actions {
            if *budget == 0 {
                self.error("Too many actions in one trigger; the rest were skipped");
                return;
            }
            *budget -= 1;
            match a {
                Action::Navigate { dest, preserve_scroll } => self.navigate(*dest, transition, *preserve_scroll, true),
                Action::Back => self.go_back(),
                Action::OpenOverlay { dest, position, offset, close_on_click_outside, background } => {
                    if let Some(frame) = self.open_overlay(source, *dest, *position, *offset, *close_on_click_outside, *background, transition) {
                        if let Some(u) = undo.as_deref_mut() {
                            u.push(Undo::CloseOverlay { frame });
                        }
                    }
                }
                Action::SwapOverlay { dest } => self.swap_overlay(source, *dest, transition),
                Action::CloseOverlay => self.close_overlay(source),
                Action::ScrollTo { dest, offset } => self.scroll_to(*dest, *offset, transition),
                Action::OpenUrl { url, new_tab } => self.effects.push(Effect::OpenUrl { url: url.clone(), new_tab: *new_tab }),
                Action::ChangeTo { dest } => self.change_to(source, *dest, transition, undo.as_deref_mut()),
                Action::SetVariable { variable, value } => self.set_variable(source, variable, value),
                Action::SetVariableMode { collection, mode } => self.set_variable_mode(collection, mode),
                Action::MediaControl { dest, op } => self.media_control(*dest, *op),
                Action::Conditional { branches, otherwise } => {
                    if depth >= MAX_ACTION_DEPTH {
                        self.error("Conditionals are nested too deeply");
                        continue;
                    }
                    let mut chosen: Option<&[Action]> = None;
                    for (cond, acts) in branches {
                        let verdict = cond.eval_bool(&self.env(source));
                        match verdict {
                            Ok(true) => {
                                chosen = Some(acts);
                                break;
                            }
                            Ok(false) => {}
                            Err(e) => self.error(format!("Condition: {e}")),
                        }
                    }
                    let acts = chosen.unwrap_or(otherwise);
                    self.run_actions(source, acts, transition, undo.as_deref_mut(), depth + 1, budget);
                }
            }
        }
    }

    fn error(&mut self, m: impl Into<String>) {
        if self.errors.len() >= MAX_ERRORS {
            self.errors.remove(0);
        }
        self.errors.push(m.into());
    }

    // ----------------------------------------------------------- variables

    /// Variable lookup for expressions: the value `node` sees (its mode, aliases followed).
    fn env(&self, node: NodeId) -> impl Fn(&str) -> Option<ExprValue> + '_ {
        move |id| self.doc.resolve_variable(node, id).as_ref().map(ExprValue::from)
    }

    /// The runtime value of a variable as seen from the current frame.
    pub fn variable_value(&self, id: &str) -> Option<VariableValue> {
        self.doc.resolve_variable(self.current, id)
    }

    /// Set variable: evaluates `value` and stores it for the mode `source` sees. The scratch
    /// document changes, the user's never does. Bound layers are re-resolved and redrawn.
    fn set_variable(&mut self, source: NodeId, variable: &str, value: &Expression) {
        let Some(v) = self.doc.variables.get(variable) else {
            self.error(format!("Set variable: unknown variable {variable}"));
            return;
        };
        let (kind, collection, name) = (v.kind, v.collection.clone(), v.name.clone());
        let evaluated = value.eval(&self.env(source));
        let result = match evaluated {
            Ok(r) => r,
            Err(e) => {
                self.error(format!("Set {name}: {e}"));
                return;
            }
        };
        let Some(new) = result.to_variable_value(kind) else {
            self.error(format!("Set {name}: cannot store a {} in a {} variable", result.ty().name(), ExprType::from(kind).name()));
            return;
        };
        let Some(mode) = self.doc.variable_mode(source, &collection) else { return };
        if let Some(v) = self.doc.variables.get_mut(variable) {
            v.values.insert(mode, new);
        }
        self.changed_vars.insert(variable.to_string());
        self.refresh_bound(&[variable.to_string()]);
    }

    /// Set variable mode: switches `collection` to `mode` everywhere (explicit per-layer choices
    /// of that collection are dropped) for the rest of the session.
    fn set_variable_mode(&mut self, collection: &str, mode: &str) {
        let valid = self.doc.variable_collections.get(collection).is_some_and(|c| c.modes.iter().any(|m| m.id == mode));
        if !valid {
            self.error(format!("Set mode: unknown mode {mode} in {collection}"));
            return;
        }
        if let Some(c) = self.doc.variable_collections.get_mut(collection) {
            c.default_mode = mode.to_string();
        }
        for n in self.doc.nodes.values_mut() {
            n.variables.modes.remove(collection);
        }
        let vars: Vec<String> = self.doc.variable_collections.get(collection).map(|c| c.variables.clone()).unwrap_or_default();
        self.refresh_bound(&vars);
    }

    /// Re-resolves the layers bound to `vars` (and to variables aliasing them), re-fits auto-sized
    /// text, re-runs auto layout and invalidates the frames' textures.
    fn refresh_bound(&mut self, vars: &[String]) {
        let mut set: HashSet<String> = vars.iter().cloned().collect();
        loop {
            let more: Vec<String> = self
                .doc
                .variables
                .iter()
                .filter(|(id, v)| !set.contains(*id) && v.values.values().any(|x| matches!(x, VariableValue::Alias(a) if set.contains(a))))
                .map(|(id, _)| id.clone())
                .collect();
            if more.is_empty() {
                break;
            }
            set.extend(more);
        }
        let nodes = crate::variables::bound_nodes(&self.doc, &set);
        if nodes.is_empty() {
            return;
        }
        crate::variables::reflow_bound(&mut self.doc, &set, &nodes);
        let mut frames: Vec<NodeId> = nodes.iter().filter_map(|n| self.top_frame_of(*n)).collect();
        frames.sort();
        frames.dedup();
        for f in frames {
            self.bump(f);
        }
    }

    /// Rows for the variables panel: every variable with its current value.
    pub fn variable_rows(&self) -> Vec<VarRow> {
        let mut rows = Vec::new();
        for c in self.doc.variable_collections.values() {
            for id in &c.variables {
                let Some(v) = self.doc.variables.get(id) else { continue };
                let mode =
                    self.doc.variable_mode(self.current, &c.id).and_then(|m| c.modes.iter().find(|x| x.id == m).map(|x| x.name.clone())).unwrap_or_default();
                let value = match self.variable_value(id) {
                    Some(VariableValue::Number(n)) => format_number(n as f64),
                    Some(VariableValue::String(s)) => format!("\"{s}\""),
                    Some(VariableValue::Boolean(b)) => b.to_string(),
                    Some(VariableValue::Color(col)) => format!("#{}", col.to_hex()),
                    Some(VariableValue::Alias(a)) => format!("-> {a}"),
                    None => "-".into(),
                };
                rows.push(VarRow { id: id.clone(), name: v.name.clone(), collection: c.name.clone(), mode, value, changed: self.changed_vars.contains(id) });
            }
        }
        rows
    }

    fn start_anim(&mut self, from: NodeId, to: NodeId, t: Transition, snapshot: Option<Snapshot>) {
        let dur = easing::effective_duration_ms(&t.easing, t.duration_ms) as f64 / 1000.0;
        self.anim = Some(Anim { from, to, kind: t.kind, easing: t.easing, start: self.now, dur, snapshot });
    }

    /// Moves a deck presenter to a page-level slide without a prototype back-stack entry.
    pub fn navigate_slide(&mut self, target: NodeId, now: f64) -> bool {
        if self.doc.kind != DocumentKind::Deck
            || target == self.current
            || self.top_frame_of(target) != Some(target)
            || !self.doc.get(target).is_some_and(|node| node.slide.is_some() && node.kind.frame_props().is_some())
        {
            return false;
        }
        self.now = now;
        self.press_target = None;
        self.drag = None;
        self.scroll_anim = None;
        self.back.clear();
        self.navigate(target, None, false, false);
        true
    }

    fn navigate(&mut self, dest: NodeId, transition: Option<Transition>, preserve_scroll: bool, push: bool) {
        let Some(frame) = self.top_frame_of(dest) else { return };
        self.finish_anim();
        self.clear_overlays();
        let from = self.current;
        if push {
            self.back.push(BackEntry { frame: from, via: transition.map(|t| Transition { kind: geom::reverse(t.kind), ..t }) });
        }
        self.timers.retain(|t| t.frame != from);
        self.current = frame;
        if !preserve_scroll {
            self.scroll.remove(&frame);
        }
        self.hover.clear();
        self.actives.clear();
        match transition.filter(|t| easing::effective_duration_ms(&t.easing, t.duration_ms) > 0) {
            Some(t) => self.start_anim(from, frame, t, None),
            None => self.on_shown(frame),
        }
    }

    fn go_back(&mut self) {
        let Some(e) = self.back.pop() else { return };
        self.navigate(e.frame, e.via, false, false);
    }

    fn clear_overlays(&mut self) {
        for o in std::mem::take(&mut self.overlays) {
            self.timers.retain(|t| t.frame != o.frame);
        }
    }

    /// The overlay whose frame contains `node`, else the topmost open overlay.
    fn overlay_of(&self, node: NodeId) -> Option<usize> {
        let owner = self.top_frame_of(node);
        self.overlays.iter().position(|o| Some(o.frame) == owner && !o.closing()).or_else(|| self.overlays.iter().rposition(|o| !o.closing()))
    }

    /// Rect of `node` in the coordinates of the frame it is shown over.
    fn rect_over_base(&self, node: NodeId) -> Rect {
        let Some(owner) = self.top_frame_of(node) else { return Rect::default() };
        let r = rect_in_frame(&self.doc, node, owner);
        match self.overlays.iter().find(|o| o.frame == owner) {
            Some(o) => Rect::new(r.x + o.origin.x, r.y + o.origin.y, r.w, r.h),
            None => r,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open_overlay(
        &mut self,
        source: NodeId,
        dest: NodeId,
        position: OverlayPosition,
        offset: Vec2,
        close_outside: bool,
        background: Option<Color>,
        transition: Option<Transition>,
    ) -> Option<NodeId> {
        let frame = self.top_frame_of(dest)?;
        if frame == self.current || self.overlays.iter().any(|o| o.frame == frame && !o.closing()) {
            return None;
        }
        self.finish_anim();
        let trigger = self.rect_over_base(source);
        let base = self.size_of(self.current);
        let origin = geom::overlay_origin(position, offset, self.size_of(frame), base, trigger);
        let t = transition.filter(|t| easing::effective_duration_ms(&t.easing, t.duration_ms) > 0);
        let anim = t.map(|t| OverlayAnim { start: self.now, dur: easing::effective_duration_ms(&t.easing, t.duration_ms) as f64 / 1000.0, opening: true });
        self.overlays.push(Overlay { frame, origin, background, close_outside, position, offset, transition: t, anim, drag: Vec2::ZERO });
        if anim.is_none() {
            self.on_shown(frame);
        }
        Some(frame)
    }

    fn swap_overlay(&mut self, source: NodeId, dest: NodeId, transition: Option<Transition>) {
        let Some(i) = self.overlay_of(source) else {
            self.open_overlay(source, dest, OverlayPosition::Center, Vec2::ZERO, false, None, transition);
            return;
        };
        let old = self.overlays.remove(i);
        self.timers.retain(|t| t.frame != old.frame);
        let Some(frame) = self.top_frame_of(dest) else { return };
        let origin = geom::overlay_origin(old.position, old.offset, self.size_of(frame), self.size_of(self.current), self.rect_over_base(source));
        let t = transition.filter(|t| easing::effective_duration_ms(&t.easing, t.duration_ms) > 0);
        let anim = t.map(|t| OverlayAnim { start: self.now, dur: easing::effective_duration_ms(&t.easing, t.duration_ms) as f64 / 1000.0, opening: true });
        self.overlays.insert(
            i,
            Overlay {
                frame,
                origin,
                background: old.background,
                close_outside: old.close_outside,
                position: old.position,
                offset: old.offset,
                transition: t,
                anim,
                drag: Vec2::ZERO,
            },
        );
        if anim.is_none() {
            self.on_shown(frame);
        }
    }

    fn close_overlay(&mut self, source: NodeId) {
        if let Some(i) = self.overlay_of(source) {
            self.close_overlay_at(i);
        }
    }

    fn close_overlay_at(&mut self, i: usize) {
        let Some(o) = self.overlays.get_mut(i) else { return };
        self.timers.retain(|t| t.frame != o.frame);
        match o.transition {
            Some(t) => {
                o.anim = Some(OverlayAnim { start: self.now, dur: easing::effective_duration_ms(&t.easing, t.duration_ms) as f64 / 1000.0, opening: false });
            }
            None => {
                self.overlays.remove(i);
            }
        }
    }

    /// Valid scroll range per axis for a top-level frame.
    pub fn scroll_range(&self, frame: NodeId) -> Vec2 {
        let Some(n) = self.doc.get(frame) else { return Vec2::ZERO };
        let ext = content_extent(&self.doc, frame);
        let (ox, oy) = match n.prototype.overflow {
            Overflow::None => (false, false),
            Overflow::Horizontal => (true, false),
            Overflow::Vertical => (false, true),
            Overflow::Both => (true, true),
        };
        Vec2::new(if ox { (ext.x - n.size.x).max(0.0) } else { 0.0 }, if oy { (ext.y - n.size.y).max(0.0) } else { 0.0 })
    }

    pub fn scroll_of(&self, frame: NodeId) -> Vec2 {
        let r = self.scroll_range(frame);
        let s = self.scroll.get(&frame).copied().unwrap_or(Vec2::ZERO);
        Vec2::new(s.x.clamp(0.0, r.x), s.y.clamp(0.0, r.y))
    }

    /// Scrolls `frame` by a delta in frame units (positive = content moves up/left).
    pub fn scroll_by(&mut self, frame: NodeId, delta: Vec2) {
        let r = self.scroll_range(frame);
        let s = self.scroll_of(frame);
        self.scroll_anim = None;
        self.scroll.insert(frame, Vec2::new((s.x + delta.x).clamp(0.0, r.x), (s.y + delta.y).clamp(0.0, r.y)));
    }

    fn scroll_to(&mut self, dest: NodeId, offset: Vec2, transition: Option<Transition>) {
        let Some(frame) = self.top_frame_of(dest) else { return };
        if !self.frame_visible(frame) {
            return;
        }
        let r = self.scroll_range(frame);
        let pos = rect_in_frame(&self.doc, dest, frame);
        let to = Vec2::new((pos.x - offset.x).clamp(0.0, r.x), (pos.y - offset.y).clamp(0.0, r.y));
        let from = self.scroll_of(frame);
        match transition.map(|t| (t, easing::effective_duration_ms(&t.easing, t.duration_ms))).filter(|(_, d)| *d > 0) {
            Some((t, d)) => self.scroll_anim = Some(ScrollAnim { frame, from, to, easing: t.easing, start: self.now, dur: d as f64 / 1000.0 }),
            None => {
                self.scroll_anim = None;
                self.scroll.insert(frame, to);
            }
        }
    }

    fn change_to(&mut self, source: NodeId, dest: NodeId, transition: Option<Transition>, undo: Option<&mut Vec<Undo>>) {
        // nearest instance at or above the source layer
        let mut cur = Some(source);
        let mut inst = None;
        while let Some(id) = cur {
            let Some(n) = self.doc.get(id) else { break };
            if matches!(n.kind, NodeKind::Instance { .. }) {
                inst = Some(id);
                break;
            }
            cur = n.parent;
        }
        let Some(inst) = inst else { return };
        let Some(prev) = self.doc.get(inst).and_then(instance_component) else { return };
        let Some(frame) = self.top_frame_of(inst) else { return };
        let snapshot = transition
            .filter(|t| {
                matches!(t.kind, TransitionKind::SmartAnimate | TransitionKind::Dissolve) && easing::effective_duration_ms(&t.easing, t.duration_ms) > 0
            })
            .and_then(|t| {
                let doc = super::smart::subtree_doc(&self.doc, frame)?;
                self.snapshots += 1;
                Some((t, Snapshot { id: self.snapshots, doc }))
            });
        // Interactions live in the variants: when the instance still carries its variant's own
        // reactions (no override of its own), it takes the new variant's.
        let old_own = self.doc.get(prev).map(|c| c.prototype.reactions.clone());
        let inst_own = self.doc.get(inst).map(|i| i.prototype.reactions.clone());
        let cmd = swap_instance(&mut self.doc, inst, dest);
        if cmd.apply(&mut self.doc).is_err() {
            return;
        }
        if self.doc.get(inst).and_then(instance_component) == Some(prev) {
            return;
        }
        if old_own == inst_own {
            if let Some(new_rs) = self.doc.get(dest).map(|c| c.prototype.reactions.clone()) {
                if let Some(i) = self.doc.get_mut(inst) {
                    i.prototype.reactions = new_rs;
                }
            }
        }
        self.bump(frame);
        if let Some(u) = undo {
            u.push(Undo::RestoreComponent { inst, comp: prev, transition: transition.map(|t| Transition { kind: geom::reverse(t.kind), ..t }) });
        }
        if let Some((t, snap)) = snapshot {
            if self.anim.is_none() && frame == self.current {
                self.start_anim(frame, frame, t, Some(snap));
            }
        }
    }
}

// ------------------------------------------------------------- free helpers

/// Interactive components: interactions are defined on the variants of a component, and every
/// instance (and layer inside one) runs those of the variant it shows. Instances made before an
/// interaction was added to the variant have none of their own, so they take the variant's here
/// (on the presentation's scratch copy only). A layer with reactions of its own keeps them.
fn inherit_variant_reactions(doc: &mut Document) {
    let todo: Vec<(NodeId, Vec<Reaction>)> = doc
        .nodes
        .values()
        .filter(|n| n.prototype.reactions.is_empty())
        .filter_map(|n| {
            let rs = &doc.get(n.component.source?)?.prototype.reactions;
            (!rs.is_empty()).then(|| (n.id, rs.clone()))
        })
        .collect();
    for (id, rs) in todo {
        if let Some(n) = doc.get_mut(id) {
            n.prototype.reactions = rs;
        }
    }
}

/// Whether the layer is shown, with a bound `visible` variable applied.
fn is_visible(doc: &Document, n: &Node) -> bool {
    resolve_node(doc, n).visible
}

/// Bounding box of `node` in the coordinate space of its ancestor `frame`.
pub fn rect_in_frame(doc: &Document, node: NodeId, frame: NodeId) -> Rect {
    let inv = doc.world_transform(frame).invert().unwrap_or(Affine::IDENTITY);
    let w = inv * doc.world_transform(node);
    let Some(n) = doc.get(node) else { return Rect::default() };
    let pts = [Vec2::new(0.0, 0.0), Vec2::new(n.size.x, 0.0), Vec2::new(n.size.x, n.size.y), Vec2::new(0.0, n.size.y)].map(|p| w.apply(p));
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Right/bottom extent of the scrolling (non-fixed) children of a frame.
pub fn content_extent(doc: &Document, frame: NodeId) -> Vec2 {
    let Some(f) = doc.get(frame) else { return Vec2::ZERO };
    let mut ext = f.size;
    for c in &f.children {
        if f.prototype.fixed_children.contains(c) {
            continue;
        }
        let Some(n) = doc.get(*c) else { continue };
        if !is_visible(doc, n) {
            continue;
        }
        let r = rect_in_frame(doc, *c, frame);
        ext.x = ext.x.max(r.x + r.w);
        ext.y = ext.y.max(r.y + r.h);
    }
    ext
}

/// Deepest visible layer at `p` (frame coordinates, before scrolling).
/// Fixed children are tested at `p`, everything else at `p + scroll`.
pub fn hit_layer(doc: &Document, frame: NodeId, p: Vec2, scroll: Vec2) -> Option<NodeId> {
    let f = doc.get(frame)?;
    if !Rect::new(0.0, 0.0, f.size.x, f.size.y).contains(p) {
        return None;
    }
    for c in f.children_front_to_back() {
        let fixed = f.prototype.fixed_children.contains(c);
        let q = if fixed { p } else { Vec2::new(p.x + scroll.x, p.y + scroll.y) };
        if let Some(h) = hit_in(doc, *c, q) {
            return Some(h);
        }
    }
    Some(frame)
}

fn hit_in(doc: &Document, id: NodeId, p: Vec2) -> Option<NodeId> {
    let n = doc.get(id)?;
    if !is_visible(doc, n) {
        return None;
    }
    let local = n.transform.invert()?.apply(p);
    let inside = Rect::new(0.0, 0.0, n.size.x, n.size.y).contains(local);
    let clips = n.kind.frame_props().is_some_and(|f| f.clips_content);
    if n.kind.is_container() && (inside || !clips) {
        for c in n.children_front_to_back() {
            if let Some(h) = hit_in(doc, *c, local) {
                return Some(h);
            }
        }
    }
    inside.then_some(id)
}

/// Top-level frames of a page that can be prototype destinations.
pub fn top_level_frames(doc: &Document, page: NodeId) -> Vec<NodeId> {
    doc.get(page)
        .map(|p| {
            p.children
                .iter()
                .copied()
                .filter(|c| doc.get(*c).is_some_and(|n| n.visible && matches!(n.kind, NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::Section)))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fx {
        doc: Document,
        a: NodeId,
        b: NodeId,
        c: NodeId,
        btn: NodeId,
        btn_b: NodeId,
    }

    fn tr(kind: TransitionKind, ms: u32) -> Option<Transition> {
        Some(Transition { kind, easing: Easing::Linear, duration_ms: ms })
    }

    fn react(trigger: Trigger, action: Action, transition: Option<Transition>) -> Reaction {
        Reaction { trigger, actions: vec![action], transition }
    }

    fn fixture() -> Fx {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let mut frames = Vec::new();
        for (i, name) in ["A", "B", "C"].iter().enumerate() {
            let id = doc.alloc_id();
            let mut f = Node::frame(id, Rect::new(i as f32 * 500.0, 0.0, 400.0, 800.0));
            f.name = (*name).into();
            frames.push(doc.insert(page, f).unwrap());
        }
        let (a, b, c) = (frames[0], frames[1], frames[2]);
        let id = doc.alloc_id();
        let btn = doc.insert(a, Node::rectangle(id, Rect::new(20.0, 700.0, 360.0, 48.0))).unwrap();
        let id = doc.alloc_id();
        let btn_b = doc.insert(b, Node::rectangle(id, Rect::new(20.0, 700.0, 360.0, 48.0))).unwrap();
        Fx { doc, a, b, c, btn, btn_b }
    }

    fn add(doc: &mut Document, node: NodeId, r: Reaction) {
        doc.get_mut(node).unwrap().prototype.reactions.push(r);
    }

    #[test]
    fn deck_navigation_validates_slide_roots_and_uses_instant_navigation() {
        let mut f = fixture();
        f.doc.kind = DocumentKind::Deck;
        for id in [f.a, f.b] {
            f.doc.get_mut(id).unwrap().slide = Some(SlideProps::default());
        }
        // A nested layer with slide metadata must still never become a slide root.
        f.doc.get_mut(f.btn).unwrap().slide = Some(SlideProps::default());
        let mut player = Player::new(&f.doc, f.a);
        assert!(!player.navigate_slide(f.btn, 1.0));
        assert!(!player.navigate_slide(f.c, 1.0));
        assert_eq!(player.current, f.a);
        assert!(player.navigate_slide(f.b, 2.0));
        assert_eq!(player.current, f.b);
        assert!(player.anim.is_none());
        assert!(player.back.is_empty());
        assert!(!player.navigate_slide(f.b, 3.0));
        assert!(player.navigate_slide(f.a, 4.0));
        assert_eq!(player.current, f.a);
        assert_eq!(f.doc.get(f.b).unwrap().name, "B", "presentation leaves the original document unchanged");
    }

    #[test]
    fn click_navigates_instantly_and_back_returns() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::Back, None));
        let mut p = Player::new(&f.doc, f.a);
        assert_eq!(p.current, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.current, f.b);
        assert!(p.anim.is_none());
        assert_eq!(p.back.len(), 1);
        p.pointer_up(Some(f.btn_b), false);
        assert_eq!(p.current, f.a);
        assert!(p.back.is_empty(), "back pops, it does not push");
        p.pointer_up(Some(f.btn_b), false);
        assert_eq!(p.current, f.a, "back on an empty stack is a no-op");
    }

    #[test]
    fn dragged_release_is_not_a_click() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), true);
        assert_eq!(p.current, f.a);
    }

    #[test]
    fn click_bubbles_to_the_nearest_ancestor_with_a_reaction() {
        let mut f = fixture();
        add(&mut f.doc, f.a, react(Trigger::OnClick, Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.current, f.c);
    }

    #[test]
    fn back_stack_unwinds_in_order_with_reversed_transitions() {
        let mut f = fixture();
        let t = tr(TransitionKind::MoveIn(Direction::Left), 300);
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, t));
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::Navigate { dest: f.c, preserve_scroll: false }, t));
        let back_btn = {
            let id = f.doc.alloc_id();
            f.doc.insert(f.c, Node::rectangle(id, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap()
        };
        add(&mut f.doc, back_btn, react(Trigger::OnClick, Action::Back, None));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(1.0);
        p.pointer_up(Some(f.btn), false);
        p.tick(1.4);
        p.pointer_up(Some(f.btn_b), false);
        p.tick(2.0);
        assert_eq!(p.current, f.c);
        assert_eq!(p.back.iter().map(|e| e.frame).collect::<Vec<_>>(), vec![f.a, f.b]);
        p.pointer_up(Some(back_btn), false);
        assert_eq!(p.current, f.b);
        let a = p.anim.as_ref().expect("back plays the reversed transition");
        assert_eq!(a.kind, TransitionKind::MoveOut(Direction::Right));
        assert_eq!((a.from, a.to), (f.c, f.b));
    }

    #[test]
    fn transition_runs_for_its_duration_then_finishes() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, tr(TransitionKind::Dissolve, 400)));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(10.0);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.current, f.b);
        let a = p.anim.as_ref().unwrap();
        assert_eq!((a.from, a.to), (f.a, f.b));
        assert_eq!(a.raw(10.0), 0.0);
        p.tick(10.2);
        assert!((p.anim.as_ref().unwrap().raw(10.2) - 0.5).abs() < 1e-3);
        assert!(p.is_animating());
        p.tick(10.4);
        assert!(p.anim.is_none());
        p.tick(10.5);
        assert!(!p.is_animating());
    }

    #[test]
    fn a_new_event_during_a_transition_completes_it_first() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, tr(TransitionKind::Dissolve, 1000)));
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert!(p.anim.is_some());
        p.pointer_up(Some(f.btn_b), false);
        assert_eq!(p.current, f.c);
        assert!(p.anim.is_none());
        assert_eq!(p.back.len(), 2);
    }

    #[test]
    fn spring_transitions_use_their_settle_time() {
        let mut f = fixture();
        let spring = Some(Transition {
            kind: TransitionKind::MoveIn(Direction::Left),
            easing: Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 },
            duration_ms: 10,
        });
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, spring));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert!(p.anim.as_ref().unwrap().dur > 0.3);
    }

    #[test]
    fn after_delay_fires_once_the_frame_has_been_shown_long_enough() {
        let mut f = fixture();
        add(&mut f.doc, f.a, react(Trigger::AfterDelay { delay_ms: 1500 }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(0.0);
        p.restart(&f.doc, f.a);
        p.tick(1.0);
        assert_eq!(p.current, f.a);
        p.tick(1.6);
        assert_eq!(p.current, f.b);
        p.tick(10.0);
        assert_eq!(p.current, f.b, "timers fire once");
    }

    #[test]
    fn after_delay_is_cancelled_when_the_frame_is_left() {
        let mut f = fixture();
        add(&mut f.doc, f.a, react(Trigger::AfterDelay { delay_ms: 2000 }, Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(0.0);
        p.pointer_up(Some(f.btn), false);
        p.tick(5.0);
        assert_eq!(p.current, f.b);
    }

    #[test]
    fn after_delay_restarts_when_returning_to_a_frame() {
        let mut f = fixture();
        add(&mut f.doc, f.b, react(Trigger::AfterDelay { delay_ms: 1000 }, Action::Back, None));
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(0.0);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.current, f.b);
        p.tick(1.1);
        assert_eq!(p.current, f.a);
        p.pointer_up(Some(f.btn), false);
        p.tick(1.5);
        assert_eq!(p.current, f.b, "fresh timer, not yet due");
        p.tick(2.3);
        assert_eq!(p.current, f.a);
    }

    fn overlay_fixture() -> (Fx, NodeId) {
        let mut f = fixture();
        let id = f.doc.alloc_id();
        let page = f.doc.pages[0];
        let mut o = Node::frame(id, Rect::new(2000.0, 0.0, 200.0, 100.0));
        o.name = "Modal".into();
        let modal = f.doc.insert(page, o).unwrap();
        let id = f.doc.alloc_id();
        let close = f.doc.insert(modal, Node::rectangle(id, Rect::new(0.0, 0.0, 30.0, 30.0))).unwrap();
        add(&mut f.doc, close, react(Trigger::OnClick, Action::CloseOverlay, None));
        add(
            &mut f.doc,
            f.btn,
            react(
                Trigger::OnClick,
                Action::OpenOverlay { dest: modal, position: OverlayPosition::Center, offset: Vec2::ZERO, close_on_click_outside: true, background: None },
                None,
            ),
        );
        (f, close)
    }

    #[test]
    fn overlay_opens_centered_and_closes_by_its_own_button() {
        let (f, close) = overlay_fixture();
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.overlays.len(), 1);
        assert_eq!(p.overlays[0].origin, Vec2::new(100.0, 350.0));
        assert_eq!(p.current, f.a, "the base stays");
        p.pointer_up(Some(close), false);
        assert!(p.overlays.is_empty());
    }

    #[test]
    fn click_outside_closes_overlay_and_swallows_the_click() {
        let (mut f, _) = overlay_fixture();
        add(&mut f.doc, f.a, react(Trigger::OnClick, Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.overlays.len(), 1);
        // click on the underlying frame: closes, does not navigate
        p.pointer_up(Some(f.a), false);
        assert!(p.overlays.is_empty());
        assert_eq!(p.current, f.a);
        p.pointer_up(Some(f.a), false);
        assert_eq!(p.current, f.c);
    }

    #[test]
    fn overlay_without_close_outside_stays_open() {
        let (mut f, _) = overlay_fixture();
        for a in &mut f.doc.get_mut(f.btn).unwrap().prototype.reactions[0].actions {
            if let Action::OpenOverlay { close_on_click_outside, .. } = a {
                *close_on_click_outside = false;
            }
        }
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        p.pointer_up(None, false);
        assert_eq!(p.overlays.len(), 1);
    }

    #[test]
    fn same_overlay_is_not_opened_twice_and_navigation_clears_overlays() {
        let (mut f, _) = overlay_fixture();
        let nav = f.doc.alloc_id();
        let nav = f.doc.insert(f.a, Node::rectangle(nav, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        add(&mut f.doc, nav, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        p.overlays[0].close_outside = false;
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.overlays.len(), 1);
        p.pointer_up(Some(nav), false);
        assert!(p.overlays.is_empty());
        assert_eq!(p.current, f.b);
    }

    #[test]
    fn swap_overlay_replaces_in_place_keeping_position() {
        let (mut f, _) = overlay_fixture();
        let other = {
            let page = f.doc.pages[0];
            let id = f.doc.alloc_id();
            f.doc.insert(page, Node::frame(id, Rect::new(3000.0, 0.0, 100.0, 100.0))).unwrap()
        };
        let sw = f.doc.alloc_id();
        let modal = f.doc.get(f.btn).unwrap().prototype.reactions[0].actions[0].clone();
        let Action::OpenOverlay { dest: modal, .. } = modal else { panic!() };
        let sw = f.doc.insert(modal, Node::rectangle(sw, Rect::new(50.0, 50.0, 20.0, 20.0))).unwrap();
        add(&mut f.doc, sw, react(Trigger::OnClick, Action::SwapOverlay { dest: other }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        p.pointer_up(Some(sw), false);
        assert_eq!(p.overlays.len(), 1);
        assert_eq!(p.overlays[0].frame, other);
        assert_eq!(p.overlays[0].origin, Vec2::new(150.0, 350.0), "centered with the new size");
    }

    #[test]
    fn overlay_close_animation_keeps_it_until_done() {
        let (mut f, close) = overlay_fixture();
        f.doc.get_mut(close).unwrap().prototype.reactions[0].transition = None;
        let modal = f.doc.get(close).unwrap().parent.unwrap();
        // give the overlay an opening transition
        f.doc.get_mut(f.btn).unwrap().prototype.reactions[0].transition = tr(TransitionKind::Dissolve, 200);
        let mut p = Player::new(&f.doc, f.a);
        p.tick(1.0);
        p.pointer_up(Some(f.btn), false);
        assert!(p.overlays[0].anim.is_some());
        assert_eq!(p.overlays[0].shown(1.0), 0.0);
        p.tick(1.2);
        assert!(p.overlays[0].anim.is_none());
        assert_eq!(p.overlays[0].shown(1.2), 1.0);
        p.pointer_up(Some(close), false);
        assert_eq!(p.overlays.len(), 1);
        assert!(p.overlays[0].closing());
        assert_eq!(p.overlays[0].frame, modal);
        p.tick(1.3);
        assert!((p.overlays[0].shown(1.3) - 0.5).abs() < 1e-3);
        p.tick(1.5);
        assert!(p.overlays.is_empty());
    }

    #[test]
    fn hover_trigger_opens_overlay_and_closing_on_leave() {
        let (mut f, _) = overlay_fixture();
        let modal = match &f.doc.get(f.btn).unwrap().prototype.reactions[0].actions[0] {
            Action::OpenOverlay { dest, .. } => *dest,
            _ => unreachable!(),
        };
        f.doc.get_mut(f.btn).unwrap().prototype.reactions[0].trigger = Trigger::WhileHovering;
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_move(Some(f.btn));
        assert_eq!(p.overlays.len(), 1);
        assert_eq!(p.overlays[0].frame, modal);
        p.pointer_move(Some(f.btn));
        assert_eq!(p.overlays.len(), 1, "still hovering: nothing new");
        p.pointer_move(None);
        assert!(p.overlays.is_empty());
    }

    #[test]
    fn mouse_enter_with_delay_is_scheduled() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::MouseEnter { delay_ms: 500 }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(0.0);
        p.pointer_move(Some(f.btn));
        assert_eq!(p.current, f.a);
        p.tick(0.4);
        assert_eq!(p.current, f.a);
        p.tick(0.6);
        assert_eq!(p.current, f.b);
    }

    #[test]
    fn mouse_leave_fires_when_the_pointer_exits() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::MouseLeave { delay_ms: 0 }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_move(Some(f.btn));
        assert_eq!(p.current, f.a);
        p.pointer_move(None);
        assert_eq!(p.current, f.b);
    }

    #[test]
    fn mouse_down_and_up_and_pressing() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::MouseDown { delay_ms: 0 }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        add(&mut f.doc, f.btn_b, react(Trigger::MouseUp { delay_ms: 0 }, Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_down(Some(f.btn));
        assert_eq!(p.current, f.b);
        p.pointer_up(Some(f.btn_b), true);
        assert_eq!(p.current, f.c);
    }

    #[test]
    fn key_press_matches_any_order_and_case() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::KeyPress { keys: "Ctrl+K".into() }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.key("Space");
        assert_eq!(p.current, f.a);
        p.key("k+ctrl");
        assert_eq!(p.current, f.b);
        assert!(keys_match("Shift+A", "a+shift"));
        assert!(!keys_match("", ""));
        assert!(!keys_match("A", "B"));
    }

    #[test]
    fn open_url_becomes_an_effect() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::OpenUrl { url: "https://example.com".into(), new_tab: true }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.effects, vec![Effect::OpenUrl { url: "https://example.com".into(), new_tab: true }]);
    }

    #[test]
    fn navigating_to_a_nested_layer_goes_to_its_frame() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.btn_b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.current, f.b);
        let mut g = fixture();
        add(&mut g.doc, g.btn, react(Trigger::OnClick, Action::Navigate { dest: NodeId(9999), preserve_scroll: false }, None));
        let mut p = Player::new(&g.doc, g.a);
        p.pointer_up(Some(g.btn), false);
        assert_eq!(p.current, g.a, "unknown destination is ignored");
    }

    fn scroll_fixture() -> (Fx, NodeId, NodeId) {
        let mut f = fixture();
        f.doc.get_mut(f.b).unwrap().prototype.overflow = Overflow::Vertical;
        let id = f.doc.alloc_id();
        let tall = f.doc.insert(f.b, Node::rectangle(id, Rect::new(0.0, 0.0, 400.0, 2000.0))).unwrap();
        let id = f.doc.alloc_id();
        let header = f.doc.insert(f.b, Node::rectangle(id, Rect::new(0.0, 0.0, 400.0, 60.0))).unwrap();
        f.doc.get_mut(f.b).unwrap().prototype.fixed_children.push(header);
        (f, tall, header)
    }

    #[test]
    fn scroll_range_ignores_fixed_children_and_non_scrollable_axes() {
        let (f, _tall, _) = scroll_fixture();
        let p = Player::new(&f.doc, f.b);
        assert_eq!(p.scroll_range(f.b), Vec2::new(0.0, 1200.0));
        assert_eq!(p.scroll_range(f.a), Vec2::ZERO, "overflow none never scrolls");
    }

    #[test]
    fn scroll_by_clamps_and_navigation_resets_it() {
        let (mut f, _tall, _) = scroll_fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.b);
        p.scroll_by(f.b, Vec2::new(50.0, 500.0));
        assert_eq!(p.scroll_of(f.b), Vec2::new(0.0, 500.0));
        p.scroll_by(f.b, Vec2::new(0.0, 5000.0));
        assert_eq!(p.scroll_of(f.b).y, 1200.0);
        p.scroll_by(f.b, Vec2::new(0.0, -9999.0));
        assert_eq!(p.scroll_of(f.b).y, 0.0);
        p.scroll_by(f.b, Vec2::new(0.0, 300.0));
        // navigate away and back without preserve: reset
        p.current = f.a;
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.scroll_of(f.b).y, 0.0);
    }

    #[test]
    fn preserve_scroll_keeps_the_offset() {
        let (mut f, _tall, _) = scroll_fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: true }, None));
        let mut p = Player::new(&f.doc, f.b);
        p.scroll_by(f.b, Vec2::new(0.0, 400.0));
        p.current = f.a;
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.scroll_of(f.b).y, 400.0);
    }

    #[test]
    fn scroll_to_moves_the_frame_so_the_target_is_at_the_offset() {
        let (mut f, tall, _) = scroll_fixture();
        let id = f.doc.alloc_id();
        f.doc.get_mut(tall).unwrap().kind = NodeKind::Frame(FrameProps::default());
        let target = f.doc.insert(tall, Node::rectangle(id, Rect::new(0.0, 900.0, 50.0, 50.0))).unwrap();
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::ScrollTo { dest: target, offset: Vec2::new(0.0, 100.0) }, None));
        let mut p = Player::new(&f.doc, f.b);
        p.pointer_up(Some(f.btn_b), false);
        assert_eq!(p.scroll_of(f.b).y, 800.0);
    }

    #[test]
    fn animated_scroll_to_interpolates() {
        let (mut f, tall, _) = scroll_fixture();
        let id = f.doc.alloc_id();
        f.doc.get_mut(tall).unwrap().kind = NodeKind::Frame(FrameProps::default());
        let target = f.doc.insert(tall, Node::rectangle(id, Rect::new(0.0, 1000.0, 50.0, 50.0))).unwrap();
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::ScrollTo { dest: target, offset: Vec2::ZERO }, tr(TransitionKind::Dissolve, 1000)));
        let mut p = Player::new(&f.doc, f.b);
        p.tick(0.0);
        p.pointer_up(Some(f.btn_b), false);
        p.tick(0.5);
        assert!((p.scroll_of(f.b).y - 500.0).abs() < 1.0);
        p.tick(1.0);
        assert_eq!(p.scroll_of(f.b).y, 1000.0);
        assert!(p.scroll_anim.is_none());
    }

    #[test]
    fn hit_layer_respects_scroll_and_fixed_children() {
        let (mut f, tall, header) = scroll_fixture();
        let id = f.doc.alloc_id();
        f.doc.get_mut(tall).unwrap().kind = NodeKind::Frame(FrameProps::default());
        let item = f.doc.insert(tall, Node::rectangle(id, Rect::new(10.0, 900.0, 50.0, 50.0))).unwrap();
        // not scrolled: header on top at y=30, the item is far below the frame
        assert_eq!(hit_layer(&f.doc, f.b, Vec2::new(20.0, 30.0), Vec2::ZERO), Some(header));
        // scrolled by 800: the item is at y=100 on screen, the header still at y=30
        assert_eq!(hit_layer(&f.doc, f.b, Vec2::new(20.0, 120.0), Vec2::new(0.0, 800.0)), Some(item));
        assert_eq!(hit_layer(&f.doc, f.b, Vec2::new(20.0, 30.0), Vec2::new(0.0, 800.0)), Some(header));
        assert_eq!(hit_layer(&f.doc, f.b, Vec2::new(-5.0, 30.0), Vec2::ZERO), None, "outside the frame");
    }

    #[test]
    fn hit_layer_follows_paint_order_in_reverse_z_frames() {
        let mut f = fixture();
        let id = f.doc.alloc_id();
        let later = f.doc.insert(f.a, Node::rectangle(id, Rect::new(20.0, 700.0, 360.0, 48.0))).unwrap();
        let p = Vec2::new(30.0, 710.0);
        assert_eq!(hit_layer(&f.doc, f.a, p, Vec2::ZERO), Some(later), "last child paints on top by default");
        if let NodeKind::Frame(fp) = &mut f.doc.get_mut(f.a).unwrap().kind {
            fp.auto_layout = Some(AutoLayout { reverse_z: true, ..Default::default() });
        }
        assert_eq!(hit_layer(&f.doc, f.a, p, Vec2::ZERO), Some(f.btn), "first child paints on top with reverse_z");
    }

    #[test]
    fn hit_layer_ignores_hidden_layers() {
        let mut f = fixture();
        f.doc.get_mut(f.btn).unwrap().visible = false;
        assert_eq!(hit_layer(&f.doc, f.a, Vec2::new(30.0, 710.0), Vec2::ZERO), Some(f.a));
    }

    #[test]
    fn restart_resets_everything() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnClick, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.current, f.b);
        p.restart(&f.doc, f.a);
        assert_eq!(p.current, f.a);
        assert!(p.back.is_empty() && p.overlays.is_empty() && p.anim.is_none());
    }

    #[test]
    fn rebasing_the_clock_keeps_timer_delays() {
        let mut f = fixture();
        add(&mut f.doc, f.a, react(Trigger::AfterDelay { delay_ms: 1000 }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.rebase_clock(500.0);
        p.tick(500.5);
        assert_eq!(p.current, f.a, "not due 0.5s after the rebase");
        p.tick(501.1);
        assert_eq!(p.current, f.b);
    }

    fn variant_fixture() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let screen = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 400.0, 800.0))).unwrap();
        let mut comps = Vec::new();
        for (i, c) in [Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)].into_iter().enumerate() {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(1000.0 + i as f32 * 200.0, 0.0, 100.0, 40.0));
            f.name = format!("State={i}");
            f.fills = vec![Paint::solid(c)];
            let f = d.insert(page, f).unwrap();
            create_component(&mut d, f).apply(&mut d).unwrap();
            comps.push(f);
        }
        combine_as_variants(&mut d, &comps).apply(&mut d).unwrap();
        let cmd = create_instance(&mut d, comps[0], screen, Vec2::new(10.0, 10.0));
        let Command::Insert { nodes, .. } = &cmd else { panic!() };
        let inst = nodes[0].id;
        cmd.apply(&mut d).unwrap();
        (d, screen, inst, comps[0], comps[1])
    }

    #[test]
    fn change_to_swaps_the_variant_on_a_scratch_copy_only() {
        let (mut d, screen, inst, c1, c2) = variant_fixture();
        add(&mut d, inst, react(Trigger::OnClick, Action::ChangeTo { dest: c2 }, None));
        let mut p = Player::new(&d, screen);
        assert_eq!(p.doc.get(inst).and_then(instance_component), Some(c1));
        p.pointer_up(Some(inst), false);
        assert_eq!(p.doc.get(inst).and_then(instance_component), Some(c2));
        assert_eq!(d.get(inst).and_then(instance_component), Some(c1), "the user's document is untouched");
        assert_eq!(p.frame_rev(screen), 1, "textures of that frame are invalidated");
        // clicking a child of the instance finds the instance too
        let child = p.doc.get(inst).unwrap().children.first().copied();
        if let Some(child) = child {
            let mut q = Player::new(&d, screen);
            q.pointer_up(Some(child), false);
            assert_eq!(q.doc.get(inst).and_then(instance_component), Some(c2));
        }
    }

    #[test]
    fn change_to_while_hovering_reverts_on_leave() {
        let (mut d, screen, inst, c1, c2) = variant_fixture();
        add(&mut d, inst, react(Trigger::WhileHovering, Action::ChangeTo { dest: c2 }, None));
        let mut p = Player::new(&d, screen);
        p.pointer_move(Some(inst));
        assert_eq!(p.doc.get(inst).and_then(instance_component), Some(c2));
        p.pointer_move(None);
        assert_eq!(p.doc.get(inst).and_then(instance_component), Some(c1));
    }

    #[test]
    fn change_to_with_smart_animate_keeps_a_snapshot_of_the_old_look() {
        let (mut d, screen, inst, _c1, c2) = variant_fixture();
        add(&mut d, inst, react(Trigger::OnClick, Action::ChangeTo { dest: c2 }, tr(TransitionKind::SmartAnimate, 300)));
        let mut p = Player::new(&d, screen);
        p.pointer_up(Some(inst), false);
        let a = p.anim.as_ref().expect("animates in place");
        assert_eq!((a.from, a.to), (screen, screen));
        assert!(a.snapshot.is_some());
        p.tick(1.0);
        assert!(p.anim.is_none());
    }

    #[test]
    fn change_to_on_something_that_is_not_an_instance_does_nothing() {
        let (mut d, screen, _inst, _c1, c2) = variant_fixture();
        let id = d.alloc_id();
        let plain = d.insert(screen, Node::rectangle(id, Rect::new(0.0, 100.0, 10.0, 10.0))).unwrap();
        add(&mut d, plain, react(Trigger::OnClick, Action::ChangeTo { dest: c2 }, None));
        let mut p = Player::new(&d, screen);
        p.pointer_up(Some(plain), false);
        assert_eq!(p.frame_rev(screen), 0);
    }

    #[test]
    fn top_level_frames_lists_visible_frames() {
        let mut f = fixture();
        f.doc.get_mut(f.c).unwrap().visible = false;
        let page = f.doc.pages[0];
        assert_eq!(top_level_frames(&f.doc, page), vec![f.a, f.b]);
    }

    // ------------------------------------------------ variables in Present

    struct Vars {
        count: String,
        label: String,
        flag: String,
        color: String,
        coll: String,
        dark: String,
        light: String,
    }

    /// Adds a collection "State" (Light/Dark) with count, label, flag and color variables.
    fn add_vars(doc: &mut Document) -> Vars {
        let mut s = VariableStore::from_doc(doc);
        let coll = s.add_collection("State");
        let light = s.collections[&coll].default_mode.clone();
        let dark = s.add_mode(&coll, "Dark").unwrap();
        let count = s.add_variable(&coll, "count", VariableKind::Number).unwrap();
        let label = s.add_variable(&coll, "label", VariableKind::String).unwrap();
        let flag = s.add_variable(&coll, "flag", VariableKind::Boolean).unwrap();
        let color = s.add_variable(&coll, "color", VariableKind::Color).unwrap();
        s.set_value(&label, &light, VariableValue::String("zero".into()));
        s.set_value(&label, &dark, VariableValue::String("zero".into()));
        s.set_value(&color, &light, VariableValue::Color(Color::rgb(255, 0, 0)));
        s.set_value(&color, &dark, VariableValue::Color(Color::rgb(0, 0, 255)));
        let _ = s.command().apply(doc);
        Vars { count, label, flag, color, coll, dark, light }
    }

    fn var(id: &str) -> Expression {
        Expression::var(id)
    }

    fn num_of(p: &Player, id: &str) -> f32 {
        match p.variable_value(id) {
            Some(VariableValue::Number(n)) => n,
            other => panic!("{other:?}"),
        }
    }

    fn text_node(doc: &mut Document, parent: NodeId, s: &str) -> NodeId {
        let id = doc.alloc_id();
        doc.insert(parent, Node::text(id, Rect::new(10.0, 10.0, 100.0, 20.0), s, TextStyle::default())).unwrap()
    }

    fn content_of(p: &Player, id: NodeId) -> String {
        match &resolve_node(&p.doc, p.doc.get(id).unwrap()).kind {
            NodeKind::Text(t) => t.content.clone(),
            _ => panic!("not text"),
        }
    }

    fn counter() -> (Fx, Vars) {
        let mut f = fixture();
        let v = add_vars(&mut f.doc);
        let cond = Action::Conditional {
            branches: vec![(
                Expression::binary(BinaryOp::Ge, var(&v.count), Expression::number(3.0)),
                vec![Action::Navigate { dest: f.b, preserve_scroll: false }],
            )],
            otherwise: vec![Action::SetVariable {
                variable: v.label.clone(),
                value: Expression::binary(BinaryOp::Add, Expression::text("count="), var(&v.count)),
            }],
        };
        let inc = Action::SetVariable { variable: v.count.clone(), value: Expression::binary(BinaryOp::Add, var(&v.count), Expression::number(1.0)) };
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.push(Reaction { trigger: Trigger::OnClick, actions: vec![inc, cond], transition: None });
        (f, v)
    }

    #[test]
    fn set_variable_then_conditional_navigates_on_the_third_click() {
        let (f, v) = counter();
        let mut p = Player::new(&f.doc, f.a);
        assert_eq!(num_of(&p, &v.count), 0.0, "starts from the document value");
        p.pointer_up(Some(f.btn), false);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(num_of(&p, &v.count), 2.0);
        assert_eq!(p.current, f.a, "the else branch ran: no navigation yet");
        assert_eq!(p.variable_value(&v.label), Some(VariableValue::String("count=2".into())));
        p.pointer_up(Some(f.btn), false);
        assert_eq!(num_of(&p, &v.count), 3.0);
        assert_eq!(p.current, f.b, "count >= 3 took the first branch");
        assert_eq!(p.variable_value(&v.label), Some(VariableValue::String("count=2".into())), "the else branch did not run");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
    }

    #[test]
    fn variable_state_is_never_written_back_to_the_document() {
        let (f, v) = counter();
        let before = f.doc.variables.clone();
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(f.doc.variables, before);
        // a restart goes back to the document's values
        p.restart(&f.doc, f.a);
        assert_eq!(num_of(&p, &v.count), 0.0);
        assert!(p.changed_vars.is_empty());
    }

    #[test]
    fn bound_text_and_visibility_follow_the_variables() {
        let (mut f, v) = counter();
        let t = text_node(&mut f.doc, f.a, "zero");
        f.doc.get_mut(t).unwrap().variables.bindings.insert("characters".into(), v.label.clone());
        let hidden = f.doc.alloc_id();
        let flagged = f.doc.insert(f.a, Node::rectangle(hidden, Rect::new(0.0, 0.0, 300.0, 300.0))).unwrap();
        f.doc.get_mut(flagged).unwrap().variables.bindings.insert("visible".into(), v.flag.clone());
        f.doc.get_mut(flagged).unwrap().visible = false;
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::SetVariable { variable: v.flag.clone(), value: Expression::logical_not(var(&v.flag)) }, None));
        f.doc.get_mut(f.a).unwrap().prototype.reactions.clear();
        let mut p = Player::new(&f.doc, f.a);
        assert_eq!(content_of(&p, t), "zero");
        let rev = p.frame_rev(f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(content_of(&p, t), "count=1", "text characters re-resolve");
        assert!(p.frame_rev(f.a) > rev, "the frame's texture is invalidated");
        assert_eq!(f.doc.get(t).and_then(|n| if let NodeKind::Text(t) = &n.kind { Some(t.content.clone()) } else { None }).as_deref(), Some("zero"));
        // visibility binding: hidden layers are not hit, shown ones are
        assert_eq!(hit_layer(&p.doc, f.a, Vec2::new(150.0, 150.0), Vec2::ZERO), Some(f.a));
        let mut q = Player::new(&f.doc, f.b);
        q.pointer_up(Some(f.btn_b), false);
        assert_eq!(q.variable_value(&v.flag), Some(VariableValue::Boolean(true)));
        q.current = f.a;
        assert_eq!(hit_layer(&q.doc, f.a, Vec2::new(150.0, 150.0), Vec2::ZERO), Some(flagged));
    }

    #[test]
    fn set_variable_mode_switches_modes_for_every_layer() {
        let (mut f, v) = counter();
        let id = f.doc.alloc_id();
        let swatch = f.doc.insert(f.a, Node::rectangle(id, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        f.doc.get_mut(swatch).unwrap().variables.bindings.insert("fills/0".into(), v.color.clone());
        f.doc.get_mut(swatch).unwrap().fills = vec![Paint::solid(Color::WHITE)];
        // an explicit per-layer choice must not stop the switch
        f.doc.get_mut(swatch).unwrap().variables.modes.insert(v.coll.clone(), v.light.clone());
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::SetVariableMode { collection: v.coll.clone(), mode: v.dark.clone() }, None));
        let mut p = Player::new(&f.doc, f.b);
        let fill = |p: &Player| match &resolve_node(&p.doc, p.doc.get(swatch).unwrap()).fills[0].kind {
            PaintKind::Solid(c) => c.to_hex(),
            _ => panic!(),
        };
        assert_eq!(fill(&p), "FF0000");
        p.pointer_up(Some(f.btn_b), false);
        assert_eq!(fill(&p), "0000FF");
        assert!(p.errors.is_empty());
        assert_eq!(f.doc.variable_collections[&v.coll].default_mode, v.light, "the document keeps its mode");
        // unknown modes are reported, not applied
        add(&mut f.doc, f.btn_b, react(Trigger::OnClick, Action::SetVariableMode { collection: v.coll.clone(), mode: "nope".into() }, None));
        let mut q = Player::new(&f.doc, f.b);
        q.pointer_up(Some(f.btn_b), false);
        assert_eq!(q.errors.len(), 1);
        assert!(q.errors[0].contains("unknown mode"));
    }

    #[test]
    fn bad_expressions_are_reported_and_leave_state_alone() {
        let (mut f, v) = counter();
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.clear();
        let bad = vec![
            Action::SetVariable { variable: v.count.clone(), value: Expression::binary(BinaryOp::Div, Expression::number(1.0), Expression::number(0.0)) },
            Action::SetVariable { variable: v.count.clone(), value: Expression::text("seven") },
            Action::SetVariable { variable: "VariableID:missing".into(), value: Expression::number(1.0) },
            Action::SetVariable { variable: v.count.clone(), value: var("also-missing") },
            Action::Conditional { branches: vec![(Expression::number(1.0), vec![Action::Back])], otherwise: vec![] },
            Action::SetVariable { variable: v.count.clone(), value: Expression::number(9.0) },
        ];
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.push(Reaction { trigger: Trigger::OnClick, actions: bad, transition: None });
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.errors.len(), 5, "{:?}", p.errors);
        assert_eq!(num_of(&p, &v.count), 9.0, "the actions after the failures still run");
        assert!(p.errors[0].contains("Division by zero"));
        assert!(p.errors[1].contains("cannot store"));
    }

    #[test]
    fn actions_run_in_order_and_nested_conditionals_are_bounded() {
        let (mut f, v) = counter();
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.clear();
        let set = |e: Expression| Action::SetVariable { variable: v.count.clone(), value: e };
        let mut deep = vec![set(Expression::number(-1.0))];
        for _ in 0..20 {
            deep = vec![Action::Conditional { branches: vec![(Expression::boolean(true), deep)], otherwise: vec![] }];
        }
        let mut actions = vec![
            set(Expression::number(1.0)),
            set(Expression::binary(BinaryOp::Add, Expression::binary(BinaryOp::Mul, var(&v.count), Expression::number(10.0)), Expression::number(2.0))),
        ];
        actions.extend(deep);
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.push(Reaction { trigger: Trigger::OnClick, actions, transition: None });
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(num_of(&p, &v.count), 12.0, "1, then 1*10+2; the 20-deep conditional never reaches its leaf");
        assert!(p.errors.iter().any(|e| e.contains("nested too deeply")));
    }

    #[test]
    fn variable_rows_mark_changed_values() {
        let (f, v) = counter();
        let mut p = Player::new(&f.doc, f.a);
        let rows = p.variable_rows();
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| !r.changed && r.collection == "State" && r.mode == "Mode 1" || r.mode == "Light" || !r.mode.is_empty()));
        p.pointer_up(Some(f.btn), false);
        let rows = p.variable_rows();
        let count = rows.iter().find(|r| r.id == v.count).unwrap();
        assert_eq!((count.value.as_str(), count.changed), ("1", true));
        let label = rows.iter().find(|r| r.id == v.label).unwrap();
        assert_eq!((label.value.as_str(), label.changed), ("\"count=1\"", true));
        assert!(!rows.iter().find(|r| r.id == v.flag).unwrap().changed);
        assert_eq!(rows.iter().find(|r| r.id == v.color).unwrap().value, "#FF0000");
    }

    // ------------------------------------------- interactive components

    /// A set with Default (red) and Hover (blue) variants, two instances on a screen. The
    /// interactions live on the Default variant only, as in Figma.
    fn interactive() -> (Document, NodeId, NodeId, NodeId, NodeId, NodeId) {
        let (mut d, screen, inst1, c1, c2) = variant_fixture();
        let cmd = create_instance(&mut d, c1, screen, Vec2::new(10.0, 200.0));
        let Command::Insert { nodes, .. } = &cmd else { panic!() };
        let inst2 = nodes[0].id;
        cmd.apply(&mut d).unwrap();
        // interactions defined on the variant AFTER the instances were made
        add(&mut d, c1, react(Trigger::WhileHovering, Action::ChangeTo { dest: c2 }, tr(TransitionKind::SmartAnimate, 200)));
        add(&mut d, c1, react(Trigger::OnClick, Action::Navigate { dest: screen, preserve_scroll: false }, None));
        (d, screen, inst1, inst2, c1, c2)
    }

    fn comp_of(p: &Player, inst: NodeId) -> Option<NodeId> {
        p.doc.get(inst).and_then(instance_component)
    }

    #[test]
    fn instances_run_the_interactions_of_their_variant() {
        let (d, screen, inst1, _inst2, c1, c2) = interactive();
        assert!(d.get(inst1).unwrap().prototype.reactions.is_empty(), "instances made before the interaction have none stored");
        let mut p = Player::new(&d, screen);
        assert!(!p.doc.get(inst1).unwrap().prototype.reactions.is_empty(), "the player inherits the variant's");
        p.tick(0.0);
        p.pointer_move(Some(inst1));
        assert_eq!(comp_of(&p, inst1), Some(c2));
        p.tick(0.3);
        p.pointer_move(None);
        assert_eq!(comp_of(&p, inst1), Some(c1));
        assert!(d.get(inst1).unwrap().prototype.reactions.is_empty(), "the user's document is untouched");
    }

    #[test]
    fn while_hovering_stays_in_the_hover_variant_until_the_pointer_leaves() {
        let (d, screen, inst1, _inst2, c1, c2) = interactive();
        let mut p = Player::new(&d, screen);
        p.tick(0.0);
        p.pointer_move(Some(inst1));
        assert_eq!(comp_of(&p, inst1), Some(c2));
        // the Hover variant has no hover reaction of its own; staying over it must not flicker back
        for t in [0.05, 0.1, 0.5, 1.0, 1.1] {
            p.tick(t);
            p.pointer_move(Some(inst1));
            assert_eq!(comp_of(&p, inst1), Some(c2), "t={t}");
        }
        p.pointer_move(None);
        assert_eq!(comp_of(&p, inst1), Some(c1));
        // and it works again the next time
        p.tick(2.0);
        p.pointer_move(Some(inst1));
        assert_eq!(comp_of(&p, inst1), Some(c2));
    }

    #[test]
    fn leaving_during_the_change_animation_takes_effect_once_it_ends() {
        let (d, screen, inst1, _inst2, c1, c2) = interactive();
        let mut p = Player::new(&d, screen);
        p.tick(0.0);
        p.pointer_move(Some(inst1));
        p.tick(0.1);
        p.pointer_move(None);
        assert_eq!(comp_of(&p, inst1), Some(c2), "deferred while the animation runs");
        p.tick(0.3);
        p.pointer_move(None);
        assert_eq!(comp_of(&p, inst1), Some(c1));
    }

    #[test]
    fn each_instance_keeps_its_own_variant_state() {
        let (d, screen, inst1, inst2, c1, c2) = interactive();
        let mut p = Player::new(&d, screen);
        p.tick(0.0);
        p.pointer_move(Some(inst1));
        assert_eq!((comp_of(&p, inst1), comp_of(&p, inst2)), (Some(c2), Some(c1)));
        p.tick(0.3);
        p.pointer_move(Some(inst2));
        assert_eq!((comp_of(&p, inst1), comp_of(&p, inst2)), (Some(c1), Some(c2)), "moving to the second reverts the first");
        p.tick(0.6);
        let rev = p.frame_rev(screen);
        p.pointer_move(None);
        assert_eq!((comp_of(&p, inst1), comp_of(&p, inst2)), (Some(c1), Some(c1)));
        assert!(p.frame_rev(screen) > rev);
    }

    #[test]
    fn variant_changes_smart_animate_both_ways() {
        let (d, screen, inst1, _inst2, c1, c2) = interactive();
        let mut p = Player::new(&d, screen);
        p.tick(0.0);
        p.pointer_move(Some(inst1));
        let a = p.anim.as_ref().expect("changing to the hover variant animates");
        assert_eq!((a.kind, a.from, a.to), (TransitionKind::SmartAnimate, screen, screen));
        assert!(a.snapshot.is_some());
        p.tick(0.3);
        assert!(p.anim.is_none());
        assert_eq!(comp_of(&p, inst1), Some(c2));
        p.pointer_move(None);
        let a = p.anim.as_ref().expect("the automatic revert animates too");
        assert_eq!(a.kind, TransitionKind::SmartAnimate);
        assert_eq!(comp_of(&p, inst1), Some(c1));
    }

    #[test]
    fn while_pressing_changes_and_reverts_on_release() {
        let (mut d, screen, inst, c1, c2) = variant_fixture();
        add(&mut d, inst, react(Trigger::WhilePressing, Action::ChangeTo { dest: c2 }, None));
        let mut p = Player::new(&d, screen);
        p.pointer_down(Some(inst));
        assert_eq!(comp_of(&p, inst), Some(c2));
        p.pointer_up(Some(inst), false);
        assert_eq!(comp_of(&p, inst), Some(c1));
    }

    #[test]
    fn click_on_an_instance_swaps_it_and_stays_swapped() {
        let (mut d, screen, inst, c1, c2) = variant_fixture();
        add(&mut d, c1, react(Trigger::OnClick, Action::ChangeTo { dest: c2 }, None));
        let mut p = Player::new(&d, screen);
        p.pointer_up(Some(inst), false);
        assert_eq!(comp_of(&p, inst), Some(c2));
        p.pointer_move(Some(inst));
        p.pointer_move(None);
        assert_eq!(comp_of(&p, inst), Some(c2), "On click is not undone by moving away");
    }

    // --------------------------------------------------------- triggers

    #[test]
    fn key_triggers_need_the_exact_modifiers() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::KeyPress { keys: "Ctrl+K".into() }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        add(&mut f.doc, f.btn_b, react(Trigger::KeyPress { keys: "K".into() }, Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.key("K");
        assert_eq!(p.current, f.a, "plain K is not Ctrl+K, and B is not on screen");
        p.key("Shift+Ctrl+K");
        assert_eq!(p.current, f.a, "an extra modifier does not match");
        p.key("Ctrl+K");
        assert_eq!(p.current, f.b);
        p.key("K");
        assert_eq!(p.current, f.c, "the new screen's own key trigger runs");
        assert!(keys_match("Cmd+K", "Ctrl+K") && keys_match("Command+Shift+S", "shift+ctrl+s"));
        assert!(keys_match("Esc", "Escape") && keys_match("Return", "Enter"));
    }

    #[test]
    fn key_triggers_run_all_their_actions_in_order() {
        let (mut f, v) = counter();
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.clear();
        let set = |e: Expression| Action::SetVariable { variable: v.count.clone(), value: e };
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.push(Reaction {
            trigger: Trigger::KeyPress { keys: "Space".into() },
            actions: vec![
                set(Expression::number(4.0)),
                Action::Navigate { dest: f.b, preserve_scroll: false },
                set(Expression::binary(BinaryOp::Add, var(&v.count), Expression::number(1.0))),
            ],
            transition: None,
        });
        let mut p = Player::new(&f.doc, f.a);
        p.key("Space");
        assert_eq!(p.current, f.b);
        assert_eq!(num_of(&p, &v.count), 5.0);
    }

    #[test]
    fn gamepad_triggers_match_their_device() {
        let mut f = fixture();
        let pad = |device, buttons: &str| Trigger::GamepadPress { device, buttons: buttons.into() };
        add(&mut f.doc, f.btn, react(pad(GamepadDevice::Xbox, "A"), Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        add(&mut f.doc, f.btn_b, react(pad(GamepadDevice::Generic, "LB+A"), Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.gamepad(GamepadDevice::PlayStation, "A");
        assert_eq!(p.current, f.a, "an Xbox trigger ignores a PlayStation pad");
        p.key("A");
        assert_eq!(p.current, f.a, "keyboard keys are not gamepad buttons");
        p.gamepad(GamepadDevice::Xbox, "a");
        assert_eq!(p.current, f.b);
        p.gamepad(GamepadDevice::PlayStation, "A+LB");
        assert_eq!(p.current, f.c, "a generic trigger takes any pad, in any button order");
    }

    #[test]
    fn after_delay_chains_across_screens() {
        let mut f = fixture();
        add(&mut f.doc, f.a, react(Trigger::AfterDelay { delay_ms: 1000 }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        add(&mut f.doc, f.b, react(Trigger::AfterDelay { delay_ms: 500 }, Action::Navigate { dest: f.c, preserve_scroll: false }, None));
        add(&mut f.doc, f.c, react(Trigger::AfterDelay { delay_ms: 250 }, Action::Back, None));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(0.0);
        p.tick(0.9);
        assert_eq!(p.current, f.a);
        p.tick(1.0);
        assert_eq!(p.current, f.b);
        p.tick(1.4);
        assert_eq!(p.current, f.b, "the second delay counts from when B appeared");
        p.tick(1.5);
        assert_eq!(p.current, f.c);
        p.tick(1.7);
        assert_eq!(p.current, f.c);
        p.tick(1.8);
        assert_eq!(p.current, f.b, "Back returns, and B's timer starts over");
        assert!(p.is_animating());
    }

    #[test]
    fn several_delays_on_one_layer_each_fire_once() {
        let mut f = fixture();
        let (v_count, label) = {
            let v = add_vars(&mut f.doc);
            (v.count, v.label)
        };
        for (ms, n) in [(300u32, 1.0), (100, 2.0), (200, 3.0)] {
            add(
                &mut f.doc,
                f.a,
                react(Trigger::AfterDelay { delay_ms: ms }, Action::SetVariable { variable: v_count.clone(), value: Expression::number(n) }, None),
            );
        }
        let mut p = Player::new(&f.doc, f.a);
        p.tick(0.0);
        p.tick(0.15);
        assert_eq!(num_of(&p, &v_count), 2.0);
        p.tick(0.25);
        assert_eq!(num_of(&p, &v_count), 3.0);
        p.tick(0.35);
        assert_eq!(num_of(&p, &v_count), 1.0, "fired in time order, not list order");
        assert!(p.variable_value(&label).is_some());
    }

    #[test]
    fn mouse_leave_delay_waits_after_the_pointer_left() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::MouseLeave { delay_ms: 400 }, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        p.tick(0.0);
        p.pointer_move(Some(f.btn));
        p.tick(0.2);
        p.pointer_move(None);
        p.tick(0.5);
        assert_eq!(p.current, f.a, "400ms after leaving at t=0.2");
        p.tick(0.7);
        assert_eq!(p.current, f.b);
    }

    // ------------------------------------------------- overlays and drag

    #[test]
    fn manual_overlays_open_relative_to_the_trigger_layer() {
        let (mut f, _) = overlay_fixture();
        for a in &mut f.doc.get_mut(f.btn).unwrap().prototype.reactions[0].actions {
            if let Action::OpenOverlay { position, offset, .. } = a {
                *position = OverlayPosition::Manual;
                *offset = Vec2::new(10.0, -120.0);
            }
        }
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        // the button sits at (20, 700): the overlay is offset from its top-left
        assert_eq!(p.overlays[0].origin, Vec2::new(30.0, 580.0));
        assert_eq!(p.overlays[0].position, OverlayPosition::Manual);
    }

    fn drag_overlay_fixture() -> (Fx, NodeId, NodeId) {
        let (mut f, close) = overlay_fixture();
        let modal = f.doc.get(close).unwrap().parent.unwrap();
        let id = f.doc.alloc_id();
        let handle = f.doc.insert(modal, Node::rectangle(id, Rect::new(60.0, 0.0, 80.0, 20.0))).unwrap();
        add(&mut f.doc, handle, react(Trigger::OnDrag, Action::CloseOverlay, None));
        (f, handle, modal)
    }

    #[test]
    fn dragging_an_overlay_far_enough_dismisses_it() {
        let (f, handle, modal) = drag_overlay_fixture();
        let mut p = Player::new(&f.doc, f.a);
        p.pointer_up(Some(f.btn), false);
        assert_eq!(p.overlays.len(), 1);
        assert!(p.drag(Some(handle)), "On drag took the gesture");
        assert_eq!(p.overlays.len(), 1, "closing waits for the release");
        p.drag_by(Vec2::new(0.0, -30.0));
        assert_eq!(p.overlays[0].drag, Vec2::new(0.0, 30.0), "the overlay follows the pointer (drag down 30)");
        p.pointer_up(None, true);
        assert_eq!(p.overlays[0].drag, Vec2::ZERO, "30 is less than the threshold: it snaps back");
        assert_eq!(p.overlays[0].frame, modal);
        p.drag(Some(handle));
        p.drag_by(Vec2::new(0.0, -80.0));
        p.pointer_up(None, true);
        assert!(p.overlays.is_empty(), "dragged past the threshold: dismissed");
    }

    #[test]
    fn on_drag_scroll_to_moves_the_frame_with_the_pointer() {
        let (mut f, tall, header) = scroll_fixture();
        let id = f.doc.alloc_id();
        f.doc.get_mut(tall).unwrap().kind = NodeKind::Frame(FrameProps::default());
        let target = f.doc.insert(tall, Node::rectangle(id, Rect::new(0.0, 900.0, 50.0, 50.0))).unwrap();
        add(&mut f.doc, header, react(Trigger::OnDrag, Action::ScrollTo { dest: target, offset: Vec2::ZERO }, None));
        let mut p = Player::new(&f.doc, f.b);
        assert!(p.drag(Some(header)));
        assert_eq!(p.scroll_of(f.b).y, 0.0, "nothing jumps when the drag starts");
        p.drag_by(Vec2::new(0.0, 120.0));
        assert_eq!(p.scroll_of(f.b).y, 120.0);
        p.drag_by(Vec2::new(0.0, 5000.0));
        assert_eq!(p.scroll_of(f.b).y, 1200.0, "clamped to the scroll range");
        p.pointer_up(None, true);
        p.drag_by(Vec2::new(0.0, 50.0));
        assert_eq!(p.scroll_of(f.b).y, 1200.0, "the gesture is over");
    }

    #[test]
    fn other_on_drag_actions_fire_at_the_start() {
        let mut f = fixture();
        add(&mut f.doc, f.btn, react(Trigger::OnDrag, Action::Navigate { dest: f.b, preserve_scroll: false }, None));
        let mut p = Player::new(&f.doc, f.a);
        assert!(p.drag(Some(f.btn)));
        assert_eq!(p.current, f.b);
        assert!(!p.drag(Some(f.btn_b)), "nothing on B reacts to a drag");
    }

    // ------------------------------------------------------------ media

    fn video_fixture() -> (Fx, NodeId) {
        let mut f = fixture();
        let id = f.doc.alloc_id();
        let clip = f.doc.insert(f.a, Node::rectangle(id, Rect::new(0.0, 0.0, 200.0, 120.0))).unwrap();
        f.doc.get_mut(clip).unwrap().prototype.videos = vec![VideoFill { paint: 0, source: "video-1".into(), autoplay: false, looping: false, muted: false }];
        (f, clip)
    }

    #[test]
    fn media_actions_drive_playback_state() {
        let (mut f, clip) = video_fixture();
        let op = |o: MediaOp| Action::MediaControl { dest: clip, op: o };
        f.doc.get_mut(f.btn).unwrap().prototype.reactions.push(Reaction {
            trigger: Trigger::OnClick,
            actions: vec![op(MediaOp::Play), op(MediaOp::Mute)],
            transition: None,
        });
        let mut p = Player::new(&f.doc, f.a);
        p.tick(10.0);
        assert!(!p.media[&clip].playing, "autoplay off");
        p.pointer_up(Some(f.btn), false);
        let m = &p.media[&clip];
        assert!(m.playing && m.muted);
        assert!((m.position(12.5) - 2.5).abs() < 1e-9, "plays in real time");
        p.tick(12.0);
        p.media_control(clip, MediaOp::Pause);
        assert!((p.media[&clip].position(99.0) - 2.0).abs() < 1e-9, "paused at 2s");
        p.media_control(clip, MediaOp::TogglePlay);
        assert!(p.media[&clip].playing);
        p.media_control(clip, MediaOp::ToggleMute);
        assert!(!p.media[&clip].muted);
        p.media_control(clip, MediaOp::SeekTo(30.0));
        assert!((p.media[&clip].position(12.0) - 30.0).abs() < 1e-9);
        p.media_control(clip, MediaOp::SkipBy(-10.0));
        assert!((p.media[&clip].position(12.0) - 20.0).abs() < 1e-9);
        p.media_control(clip, MediaOp::SkipBy(-100.0));
        assert_eq!(p.media[&clip].position(12.0), 0.0, "never before the start");
        assert!(p.is_animating(), "a playing video keeps the clock running");
        // not a video layer: ignored
        p.media_control(f.btn, MediaOp::Play);
        assert!(!p.media.contains_key(&f.btn));
    }

    #[test]
    fn video_length_loops_or_stops() {
        let (f, clip) = video_fixture();
        let mut p = Player::new(&f.doc, f.a);
        p.media_control(clip, MediaOp::Play);
        p.set_media_duration(clip, 4.0);
        assert_eq!(p.media[&clip].position(3.0), 3.0);
        assert_eq!(p.media[&clip].position(9.0), 4.0, "holds the last frame");
        p.media_control(clip, MediaOp::SetLoop(true));
        assert!((p.media[&clip].position(9.0) - 1.0).abs() < 1e-9, "wraps around");
        p.media_control(clip, MediaOp::SetLoop(false));
        p.tick(100.0);
        p.media_control(clip, MediaOp::Pause);
        p.media_control(clip, MediaOp::Play);
        assert_eq!(p.media[&clip].position(100.0), 0.0, "playing from the end restarts");
    }

    #[test]
    fn autoplay_starts_when_the_screen_is_shown() {
        let (mut f, clip) = video_fixture();
        f.doc.get_mut(clip).unwrap().prototype.videos[0].autoplay = true;
        f.doc.get_mut(clip).unwrap().prototype.videos[0].looping = true;
        let mut p = Player::new(&f.doc, f.a);
        let m = &p.media[&clip];
        assert!(m.playing && m.looping && !m.muted);
        p.restart(&f.doc, f.a);
        assert!(p.media[&clip].playing);
    }
}
