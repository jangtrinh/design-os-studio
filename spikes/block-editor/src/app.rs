//! THROWAWAY editor state and the operations both backends share. A backend only reads input and
//! paints; everything that changes the document goes through the `op_*` methods here, so the two
//! approaches are compared on identical editing rules, undo and slash behaviour.

use crate::history::{Edit, History, Snap};
use crate::model::{char_len, Doc, Kind, MergeOutcome};
use crate::script::Script;
use egui::{Context, Event, ImeEvent, Pos2};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// One `egui::TextEdit` per block.
    TextEdit,
    /// Own input handling over egui galleys, one editor surface for all blocks.
    Custom,
}

pub struct SlashState {
    pub start: usize,
    pub query: String,
    pub sel: usize,
}

pub struct App {
    pub mode: Mode,
    pub doc: Doc,
    pub hist: History,
    pub focus: usize,
    pub anchor: usize,
    pub head: usize,
    /// Selection the widget layer must apply on its next paint: `(block, anchor, head)` in chars.
    pub pending: Option<(usize, usize, usize)>,
    /// Custom backend: IME composition, kept OUT of the document until commit.
    pub goal_x: Option<f32>,
    /// egui `CCursor::prefer_next_row`: an offset on a soft-wrap boundary exists on two rows.
    pub prefer_next: bool,
    /// A composition was open when the caret moved away: tell the platform IME to drop it.
    pub interrupt_ime: bool,
    pub preedit: Option<String>,
    /// Both backends: a composition is in flight (seen from raw `Event::Ime`).
    pub preedit_active: bool,
    pub slash: Option<SlashState>,
    pub slash_dismissed: Option<(u64, usize)>,
    pub galleys: HashMap<u64, Arc<egui::Galley>>,
    pub caret_screen: Option<Pos2>,
    pub frame_snap: Snap,
    pub recorded: bool,
    pub now: f64,
    pub script: Option<Script>,
    pub shots_dir: Option<PathBuf>,
    pub pending_shot: Option<String>,
    pub frames: u64,
    /// `--log-ime`: print every raw Ime / Text event (real-IME evidence).
    pub log_ime: bool,
    pub shot_counter: u32,
}

pub fn sample_doc() -> Doc {
    Doc::new(vec![
        (Kind::H1, "Payment methods"),
        (Kind::Text, "How customers pay at checkout, and what happens when a payment fails."),
        (Kind::H2, "Accepted methods"),
        (
            Kind::Text,
            "Customers can pay by card, Apple Pay or bank transfer. Methods appear in the order below, and the list wraps onto a second line so vertical caret movement across wrapped rows can be checked.",
        ),
        (Kind::Bullet, "Card (Visa, Mastercard, Amex)"),
        (Kind::Bullet, "Apple Pay on Safari and iOS"),
        (Kind::Callout, "Card payments use 3-D Secure when the issuer asks for it."),
        (Kind::Check, "Card form validates number, expiry and CVC"),
        (Kind::Text, ""),
    ])
}

impl App {
    pub fn new(mode: Mode) -> Self {
        let doc = sample_doc();
        let frame_snap = Snap { doc: doc.clone(), block: 1, at: 0 };
        App {
            mode,
            doc,
            hist: History::default(),
            focus: 1,
            anchor: 0,
            head: 0,
            pending: Some((1, 0, 0)),
            goal_x: None,
            prefer_next: false,
            interrupt_ime: false,
            preedit: None,
            preedit_active: false,
            slash: None,
            slash_dismissed: None,
            galleys: HashMap::new(),
            caret_screen: None,
            frame_snap,
            recorded: false,
            now: 0.0,
            script: None,
            shots_dir: None,
            pending_shot: None,
            frames: 0,
            log_ime: false,
            shot_counter: 0,
        }
    }

    pub fn snap(&self) -> Snap {
        Snap { doc: self.doc.clone(), block: self.focus, at: self.head }
    }

    pub fn record(&mut self, before: Snap, kind: Edit) {
        self.hist.record(before, kind, self.now);
        self.recorded = true;
    }

    pub fn cursor(&self) -> egui::text::CCursor {
        egui::text::CCursor { index: self.head.into(), prefer_next_row: self.prefer_next }
    }

    pub fn sel(&self) -> std::ops::Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    /// Moves the caret (a navigation, not an edit): ends the typing burst, closes IME state.
    pub fn go(&mut self, block: usize, at: usize) {
        self.focus = block.min(self.doc.blocks.len() - 1);
        let len = char_len(&self.doc.blocks[self.focus].text);
        self.head = at.min(len);
        self.anchor = self.head;
        self.pending = Some((self.focus, self.head, self.head));
        self.interrupt_ime |= self.preedit.take().is_some();
        self.goal_x = None;
        self.prefer_next = false;
        self.hist.break_burst();
    }

    // ---- structural operations (shared by both backends)
    pub fn op_split(&mut self) {
        let before = self.snap();
        let to = self.doc.split(self.focus, self.head);
        self.record(before, Edit::Other);
        self.go(to, 0);
    }

    pub fn op_backspace_at_start(&mut self) {
        let before = self.snap();
        match self.doc.backspace_at_start(self.focus) {
            MergeOutcome::Nothing => {}
            MergeOutcome::BecameText => {
                self.record(before, Edit::Other);
                self.go(self.focus, 0);
            }
            MergeOutcome::Merged(b, at) => {
                self.record(before, Edit::Other);
                self.go(b, at);
            }
        }
    }

    pub fn op_bold(&mut self) {
        let r = self.sel();
        if r.is_empty() {
            return;
        }
        let before = self.snap();
        self.doc.blocks[self.focus].toggle_bold(r.clone());
        self.record(before, Edit::Other);
        self.pending = Some((self.focus, self.anchor, self.head)); // re-apply the selection after the repaint
    }

    pub fn op_toggle_check(&mut self, idx: usize) {
        let before = self.snap();
        let b = &mut self.doc.blocks[idx];
        b.checked = !b.checked;
        self.record(before, Edit::Other);
    }

    pub fn op_insert_block_below(&mut self, idx: usize) {
        let before = self.snap();
        let at = self.doc.insert_after(idx, Kind::Text);
        self.record(before, Edit::Other);
        self.go(at, 0);
    }

    pub fn op_undo(&mut self, redo: bool) {
        let cur = self.snap();
        let s = if redo { self.hist.redo(cur) } else { self.hist.undo(cur) };
        if let Some(s) = s {
            self.doc = s.doc;
            self.recorded = true; // the undo itself is not an edit
            self.slash = None;
            self.go(s.block, s.at);
        }
    }

    /// Text edit at the caret (custom backend; TextEdit mode edits inside the widget).
    pub fn insert_text(&mut self, s: &str) {
        let s: String = s.chars().filter(|c| !c.is_control() || *c == '\n').collect();
        if s.is_empty() {
            return;
        }
        let before = self.snap();
        let r = self.sel();
        self.doc.blocks[self.focus].replace(r.clone(), &s);
        self.head = r.start + char_len(&s);
        self.anchor = self.head;
        self.record(before, Edit::Typing);
    }

    pub fn delete_range(&mut self, r: std::ops::Range<usize>) {
        if r.is_empty() {
            return;
        }
        let before = self.snap();
        self.doc.blocks[self.focus].replace(r.clone(), "");
        self.head = r.start;
        self.anchor = r.start;
        self.record(before, Edit::Deleting);
    }

    // ---- frame plumbing
    pub fn begin_frame(&mut self, ctx: &Context) {
        self.now = ctx.input(|i| i.time);
        self.frames += 1;
        self.recorded = false;
        self.frame_snap = self.snap();
        let events = ctx.input(|i| i.events.clone());
        for ev in events {
            if self.log_ime {
                match &ev {
                    Event::Ime(i) => println!("EV ime {i:?}"),
                    Event::Text(t) => println!("EV text {t:?}"),
                    Event::Key { key, pressed: true, modifiers, .. } => println!("EV key {key:?} shift={} cmd={}", modifiers.shift, modifiers.command),
                    _ => {}
                }
            }
            match ev {
                Event::Ime(ImeEvent::Preedit { text, .. }) => self.preedit_active = !text.is_empty(),
                Event::Ime(ImeEvent::Commit(_)) => self.preedit_active = false,
                Event::Screenshot { image, .. } => self.save_shot(&image),
                _ => {}
            }
        }
    }

    /// TextEdit mode edits `doc` inside the widget: record those as one undo entry here.
    pub fn end_frame(&mut self) {
        if !self.recorded && self.doc != self.frame_snap.doc {
            let before = self.frame_snap.clone();
            self.record(before, Edit::Typing);
        }
        self.slash_refresh();
    }

    fn save_shot(&mut self, image: &egui::ColorImage) {
        let (Some(name), Some(dir)) = (self.pending_shot.take(), self.shots_dir.clone()) else { return };
        let [w, h] = image.size;
        let mut rgba = Vec::with_capacity(w * h * 4);
        for p in &image.pixels {
            rgba.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
        }
        let path = dir.join(format!("{name}.png"));
        match image::RgbaImage::from_raw(w as u32, h as u32, rgba).map(|i| i.save(&path)) {
            Some(Ok(())) => println!("SHOT {} {}x{}", path.display(), w, h),
            other => println!("SHOT-FAILED {name}: {:?}", other.map(|r| r.map_err(|e| e.to_string()))),
        }
    }

    pub fn markdown(&self) -> String {
        crate::model::to_markdown(&self.doc)
    }
}
