//! Board (whiteboard) mode: a file whose [`DocumentKind`] is `Board` gets its own bottom toolbar,
//! a floating contextual toolbar instead of the right panel, a dotted canvas, stickies, shapes with
//! text, connectors that follow their nodes, and stamps. The items themselves are document kinds
//! (`designos_doc::board`); everything here is editor behaviour.

pub mod bar;
pub mod canvas;
pub mod connect;
pub mod create;
pub mod demo;
pub mod preview;
pub mod toolbar;

use crate::app::App;
use designos_doc::{Color, DocumentKind, NodeId, NodeKind, ShapeKind};

/// Which popover of the bottom toolbar is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMenu {
    Brush,
    Shape,
    Stamp,
}

/// Which popover of the contextual toolbar is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarMenu {
    Shape,
    StartCap,
    EndCap,
}

/// Local facilitator countdown timer for boards.
#[derive(Clone, Debug)]
pub struct FacilitatorTimer {
    /// Configured countdown duration in seconds (default 300s = 05:00).
    pub duration_secs: f64,
    /// Remaining countdown seconds.
    pub remaining_secs: f64,
    /// Whether the timer is actively ticking down.
    pub running: bool,
    /// Whether the facilitator timer HUD bar is open.
    pub open: bool,
    /// Last frame timestamp (`ui.input(|i| i.time)`) observed while running.
    pub last_tick_time: Option<f64>,
}

impl Default for FacilitatorTimer {
    fn default() -> Self {
        Self { duration_secs: 300.0, remaining_secs: 300.0, running: false, open: false, last_tick_time: None }
    }
}

impl FacilitatorTimer {
    /// Starts or resumes the countdown at timestamp `now`.
    pub fn start(&mut self, now: f64) {
        self.open = true;
        if self.remaining_secs <= 0.0 {
            self.remaining_secs = self.duration_secs.max(1.0);
        }
        self.running = true;
        self.last_tick_time = Some(now);
    }

    /// Pauses the countdown.
    pub fn pause(&mut self) {
        self.running = false;
        self.last_tick_time = None;
    }

    /// Toggles between running and paused states.
    pub fn toggle(&mut self, now: f64) {
        if self.running {
            self.pause();
        } else {
            self.start(now);
        }
    }

    /// Stops and resets the countdown back to `duration_secs`.
    pub fn reset(&mut self) {
        self.running = false;
        self.remaining_secs = self.duration_secs;
        self.last_tick_time = None;
    }

    /// Adds 60 seconds (+1 minute) to the active countdown. The configured duration, which Reset and a
    /// restart after reaching zero go back to, stays as it is.
    pub fn add_minute(&mut self) {
        self.remaining_secs += 60.0;
    }

    ///Advances the timer by elapsed seconds since the previous frame; returns `true` while running.
    pub fn tick(&mut self, now: f64) -> bool {
        if !self.running {
            self.last_tick_time = None;
            return false;
        }
        if let Some(prev) = self.last_tick_time {
            let dt = (now - prev).max(0.0);
            self.remaining_secs = (self.remaining_secs - dt).max(0.0);
            if self.remaining_secs <= 0.0 {
                self.running = false;
                self.last_tick_time = None;
                return false;
            }
        }
        self.last_tick_time = Some(now);
        true
    }

    /// Formats remaining time as `MM:SS`.
    pub fn format_mm_ss(&self) -> String {
        let total = self.remaining_secs.ceil().max(0.0) as u64;
        format!("{:02}:{:02}", total / 60, total % 60)
    }
}

/// Editor state of board mode.
pub struct BoardState {
    /// Colour of the next sticky note.
    pub color: Color,
    /// Shape the Shape tool draws.
    pub shape: ShapeKind,
    /// Stamp the Stamp tool places.
    pub stamp: String,
    /// Marker and connector ink.
    pub ink: Color,
    /// Marker or highlighter: the brush the toolbar button shows.
    pub brush: crate::canvas::tools::Tool,
    /// Open toolbar popover and the button it hangs off.
    pub tool_menu: Option<(ToolMenu, egui::Rect)>,
    /// Open contextual toolbar popover and its anchor.
    pub bar_menu: Option<(BarMenu, egui::Rect)>,
    /// Node whose magnet dots are shown.
    pub dots: Option<NodeId>,
    /// Node a connector being dragged would attach to.
    pub target: Option<NodeId>,
    /// Rendered previews of shapes and stamps for the popovers.
    pub previews: preview::Previews,
    /// Local facilitator timer state.
    pub timer: FacilitatorTimer,
}

impl Default for BoardState {
    fn default() -> Self {
        Self {
            color: designos_doc::STICKY_YELLOW,
            shape: ShapeKind::RoundedRect,
            stamp: "thumbs_up".into(),
            ink: color_of(crate::theme::BOARD_INK[0]),
            brush: crate::canvas::tools::Tool::Marker,
            tool_menu: None,
            bar_menu: None,
            dots: None,
            target: None,
            previews: Default::default(),
            timer: FacilitatorTimer::default(),
        }
    }
}

/// `0xRRGGBB` as a document colour.
pub fn color_of(c: u32) -> Color {
    Color::rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// A document colour as `0xRRGGBB` (alpha dropped).
pub fn rgb_of(c: Color) -> u32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (b(c.r) << 16) | (b(c.g) << 8) | b(c.b)
}

pub fn color32(c: Color) -> egui::Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgb(b(c.r), b(c.g), b(c.b))
}

impl App {
    /// The open file is a board.
    pub fn is_board(&self) -> bool {
        self.doc.kind == DocumentKind::Board
    }
}

/// Kinds a connector can attach to.
pub fn connectable(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Sticky(_) | NodeKind::ShapeWithText(_) | NodeKind::Stamp { .. } | NodeKind::Text(_))
}
