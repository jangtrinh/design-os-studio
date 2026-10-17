//! Prototype interactions (Figma "Prototype" tab): triggers, actions and
//! transitions attached to nodes, plus flow starting points per page.

use crate::{NodeId, Vec2};
use serde::{Deserialize, Serialize};

// The variable expression language (parser, evaluator, limits) lives in its own file.
#[path = "prototype_expr.rs"]
mod expr;
pub use expr::*;

/// Input device of a key/gamepad trigger (Figma's `device`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamepadDevice {
    Xbox,
    PlayStation,
    SwitchPro,
    /// Any other controller (standard mapping).
    #[default]
    Generic,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Trigger {
    OnClick,
    OnDrag,
    WhileHovering,
    WhilePressing,
    MouseEnter {
        delay_ms: u32,
    },
    MouseLeave {
        delay_ms: u32,
    },
    MouseDown {
        delay_ms: u32,
    },
    MouseUp {
        delay_ms: u32,
    },
    AfterDelay {
        delay_ms: u32,
    },
    /// Keyboard press, e.g. "Space", "Ctrl+K" (modifiers joined with `+`, any order).
    KeyPress {
        keys: String,
    },
    /// Gamepad button press: `buttons` joined with `+` ("A", "DPadUp", "LB+A").
    GamepadPress {
        device: GamepadDevice,
        buttons: String,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

/// Figma's 15 `EasingType` kiwi enum values (`0..=14`, REA R5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum EasingType {
    InCubic = 0,
    #[default]
    OutCubic = 1,
    InOutCubic = 2,
    Linear = 3,
    InBackCubic = 4,
    OutBackCubic = 5,
    InOutBackCubic = 6,
    CustomCubic = 7,
    Spring = 8,
    GentleSpring = 9,
    CustomSpring = 10,
    SpringPresetOne = 11,
    SpringPresetTwo = 12,
    SpringPresetThree = 13,
    Hold = 14,
}

impl EasingType {
    /// All 15 `EasingType` variants in kiwi tag order (`0..=14`).
    pub const ALL: [EasingType; 15] = [
        EasingType::InCubic,
        EasingType::OutCubic,
        EasingType::InOutCubic,
        EasingType::Linear,
        EasingType::InBackCubic,
        EasingType::OutBackCubic,
        EasingType::InOutBackCubic,
        EasingType::CustomCubic,
        EasingType::Spring,
        EasingType::GentleSpring,
        EasingType::CustomSpring,
        EasingType::SpringPresetOne,
        EasingType::SpringPresetTwo,
        EasingType::SpringPresetThree,
        EasingType::Hold,
    ];

    pub const fn kiwi_tag(self) -> u32 {
        self as u32
    }

    pub const fn from_kiwi_tag(tag: u32) -> Option<Self> {
        match tag {
            0 => Some(Self::InCubic),
            1 => Some(Self::OutCubic),
            2 => Some(Self::InOutCubic),
            3 => Some(Self::Linear),
            4 => Some(Self::InBackCubic),
            5 => Some(Self::OutBackCubic),
            6 => Some(Self::InOutBackCubic),
            7 => Some(Self::CustomCubic),
            8 => Some(Self::Spring),
            9 => Some(Self::GentleSpring),
            10 => Some(Self::CustomSpring),
            11 => Some(Self::SpringPresetOne),
            12 => Some(Self::SpringPresetTwo),
            13 => Some(Self::SpringPresetThree),
            14 => Some(Self::Hold),
            _ => None,
        }
    }

    pub const fn kiwi_name(self) -> &'static str {
        match self {
            Self::InCubic => "IN_CUBIC",
            Self::OutCubic => "OUT_CUBIC",
            Self::InOutCubic => "INOUT_CUBIC",
            Self::Linear => "LINEAR",
            Self::InBackCubic => "IN_BACK_CUBIC",
            Self::OutBackCubic => "OUT_BACK_CUBIC",
            Self::InOutBackCubic => "INOUT_BACK_CUBIC",
            Self::CustomCubic => "CUSTOM_CUBIC",
            Self::Spring => "SPRING",
            Self::GentleSpring => "GENTLE_SPRING",
            Self::CustomSpring => "CUSTOM_SPRING",
            Self::SpringPresetOne => "SPRING_PRESET_ONE",
            Self::SpringPresetTwo => "SPRING_PRESET_TWO",
            Self::SpringPresetThree => "SPRING_PRESET_THREE",
            Self::Hold => "HOLD",
        }
    }

    pub fn from_kiwi_name(name: &str) -> Option<Self> {
        match name {
            "IN_CUBIC" => Some(Self::InCubic),
            "OUT_CUBIC" => Some(Self::OutCubic),
            "INOUT_CUBIC" => Some(Self::InOutCubic),
            "LINEAR" => Some(Self::Linear),
            "IN_BACK_CUBIC" => Some(Self::InBackCubic),
            "OUT_BACK_CUBIC" => Some(Self::OutBackCubic),
            "INOUT_BACK_CUBIC" => Some(Self::InOutBackCubic),
            "CUSTOM_CUBIC" => Some(Self::CustomCubic),
            "SPRING" => Some(Self::Spring),
            "GENTLE_SPRING" => Some(Self::GentleSpring),
            "CUSTOM_SPRING" => Some(Self::CustomSpring),
            "SPRING_PRESET_ONE" => Some(Self::SpringPresetOne),
            "SPRING_PRESET_TWO" => Some(Self::SpringPresetTwo),
            "SPRING_PRESET_THREE" => Some(Self::SpringPresetThree),
            "HOLD" => Some(Self::Hold),
            _ => None,
        }
    }

    /// Maps an `EasingType` and optional custom parameters into [`Easing`].
    pub fn to_easing(self, custom_bezier: Option<[f32; 4]>, custom_spring: Option<(f32, f32, f32)>) -> Easing {
        match self {
            Self::InCubic => Easing::EaseIn,
            Self::OutCubic => Easing::EaseOut,
            Self::InOutCubic => Easing::EaseInOut,
            Self::Linear | Self::Hold => Easing::Linear,
            Self::InBackCubic => Easing::EaseInBack,
            Self::OutBackCubic => Easing::EaseOutBack,
            Self::InOutBackCubic => Easing::EaseInOutBack,
            Self::CustomCubic => Easing::Bezier(custom_bezier.unwrap_or(DEFAULT_BEZIER)),
            Self::GentleSpring => Easing::Spring { mass: 1.0, stiffness: 100.0, damping: 15.0 },
            Self::Spring | Self::SpringPresetOne => Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 },
            Self::SpringPresetTwo => Easing::Spring { mass: 1.0, stiffness: 600.0, damping: 15.0 },
            Self::SpringPresetThree => Easing::Spring { mass: 1.0, stiffness: 80.0, damping: 20.0 },
            Self::CustomSpring => {
                let (mass, stiffness, damping) = custom_spring.unwrap_or((1.0, 100.0, 15.0));
                Easing::Spring { mass, stiffness, damping }
            }
        }
    }

    pub fn default_easing(self) -> Easing {
        self.to_easing(None, None)
    }

    /// Evaluates this `EasingType` at progress `t` in `0..=1` (`Hold` stays at `0.0` until `t >= 1.0`).
    pub fn ease_at(self, custom_bezier: Option<[f32; 4]>, custom_spring: Option<(f32, f32, f32)>, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if self == Self::Hold {
            return if t >= 1.0 { 1.0 } else { 0.0 };
        }
        ease(&self.to_easing(custom_bezier, custom_spring), t)
    }
}

/// Default cubic-bezier control points (`[0, 0, 0.58, 1]`, REA R5).
pub const DEFAULT_BEZIER: [f32; 4] = [0.0, 0.0, 0.58, 1.0];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    /// CSS-style cubic-bezier control points.
    Bezier([f32; 4]),
    Spring {
        mass: f32,
        stiffness: f32,
        damping: f32,
    },
}

impl Easing {
    /// Returns the matching Figma `EasingType` for this curve.
    pub fn easing_type(&self) -> EasingType {
        match *self {
            Easing::Linear => EasingType::Linear,
            Easing::EaseIn => EasingType::InCubic,
            Easing::EaseOut => EasingType::OutCubic,
            Easing::EaseInOut => EasingType::InOutCubic,
            Easing::EaseInBack => EasingType::InBackCubic,
            Easing::EaseOutBack => EasingType::OutBackCubic,
            Easing::EaseInOutBack => EasingType::InOutBackCubic,
            Easing::Bezier(_) => EasingType::CustomCubic,
            Easing::Spring { mass, stiffness, damping } => {
                if (mass, stiffness, damping) == (1.0, 100.0, 15.0) {
                    EasingType::GentleSpring
                } else if (mass, stiffness, damping) == (1.0, 300.0, 20.0) {
                    EasingType::SpringPresetOne
                } else if (mass, stiffness, damping) == (1.0, 600.0, 15.0) {
                    EasingType::SpringPresetTwo
                } else if (mass, stiffness, damping) == (1.0, 80.0, 20.0) {
                    EasingType::SpringPresetThree
                } else {
                    EasingType::CustomSpring
                }
            }
        }
    }
}

/// Cubic-bezier control points of the cubic presets (REA R5).
pub fn control_points(e: &Easing) -> Option<[f32; 4]> {
    Some(match e {
        Easing::Linear | Easing::Spring { .. } => return None,
        Easing::EaseIn => [0.55, 0.055, 0.675, 0.19],
        Easing::EaseOut => [0.215, 0.61, 0.355, 1.0],
        Easing::EaseInOut => [0.645, 0.045, 0.355, 1.0],
        Easing::EaseInBack => [0.3, -0.05, 0.7, -0.5],
        Easing::EaseOutBack => [0.45, 1.45, 0.8, 1.0],
        Easing::EaseInOutBack => [0.7, -0.4, 0.4, 1.4],
        Easing::Bezier(c) => *c,
    })
}

/// y(t) of the unit cubic bezier through (0,0), (x1,y1), (x2,y2), (1,1).
pub fn cubic_bezier(c: [f32; 4], t: f32) -> f32 {
    let [x1, y1, x2, y2] = c;
    let t = t.clamp(0.0, 1.0);
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let sample = |p1: f32, p2: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * p1 + 3.0 * u * s * s * p2 + s * s * s
    };
    let dx = |s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * x1 + 6.0 * u * s * (x2 - x1) + 3.0 * s * s * (1.0 - x2)
    };
    // Newton first, bisection when the slope is flat or Newton leaves [0,1].
    let mut s = t;
    for _ in 0..8 {
        let err = sample(x1, x2, s) - t;
        if err.abs() < 1e-6 {
            return sample(y1, y2, s);
        }
        let d = dx(s);
        if d.abs() < 1e-6 {
            break;
        }
        s -= err / d;
    }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    s = t;
    for _ in 0..40 {
        let x = sample(x1, x2, s);
        if (x - t).abs() < 1e-6 {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    sample(y1, y2, s)
}

/// Seconds a spring needs to settle (envelope below 0.1%), capped at 4s.
pub fn spring_settle_secs(mass: f32, stiffness: f32, damping: f32) -> f32 {
    let (m, k, c) = (mass.max(0.01), stiffness.max(0.01), damping.max(0.0));
    let w0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());
    let rate = if zeta < 1.0 { zeta * w0 } else { w0 * (zeta - (zeta * zeta - 1.0).sqrt()) };
    if rate <= 1e-3 {
        return 4.0;
    }
    (6.9 / rate).clamp(0.1, 4.0)
}

/// Spring step response at time `tau` seconds (starts at 0, settles at 1).
pub fn spring_at(mass: f32, stiffness: f32, damping: f32, tau: f32) -> f32 {
    let (m, k, c) = (mass.max(0.01), stiffness.max(0.01), damping.max(0.0));
    let w0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());
    if zeta < 1.0 - 1e-4 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        let a = zeta * w0;
        1.0 - (-a * tau).exp() * ((wd * tau).cos() + a / wd * (wd * tau).sin())
    } else if zeta <= 1.0 + 1e-4 {
        1.0 - (-w0 * tau).exp() * (1.0 + w0 * tau)
    } else {
        let s = (zeta * zeta - 1.0).sqrt();
        let (r1, r2) = (-w0 * (zeta - s), -w0 * (zeta + s));
        1.0 - (r2 * (r1 * tau).exp() - r1 * (r2 * tau).exp()) / (r2 - r1)
    }
}

/// Eased progress for linear progress `t` in 0..1 (may overshoot for back and spring curves).
pub fn ease(e: &Easing, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => t,
        Easing::Spring { mass, stiffness, damping } => {
            if t >= 1.0 {
                1.0
            } else {
                spring_at(*mass, *stiffness, *damping, t * spring_settle_secs(*mass, *stiffness, *damping))
            }
        }
        other => control_points(other).map(|c| cubic_bezier(c, t)).unwrap_or(t),
    }
}

/// Duration actually used: springs run for their settle time, others as requested.
pub fn effective_duration_ms(e: &Easing, requested: u32) -> u32 {
    match e {
        Easing::Spring { mass, stiffness, damping } => (spring_settle_secs(*mass, *stiffness, *damping) * 1000.0) as u32,
        _ => requested,
    }
}

/// Preset names shown in the easing picker, with the curve each stands for (REA R5).
pub const PRESETS: &[(&str, Easing)] = &[
    ("Linear", Easing::Linear),
    ("Ease in", Easing::EaseIn),
    ("Ease out", Easing::EaseOut),
    ("Ease in and out", Easing::EaseInOut),
    ("Ease in back", Easing::EaseInBack),
    ("Ease out back", Easing::EaseOutBack),
    ("Ease in and out back", Easing::EaseInOutBack),
    ("Gentle", Easing::Spring { mass: 1.0, stiffness: 100.0, damping: 15.0 }),
    ("Quick", Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 }),
    ("Bouncy", Easing::Spring { mass: 1.0, stiffness: 600.0, damping: 15.0 }),
    ("Slow", Easing::Spring { mass: 1.0, stiffness: 80.0, damping: 20.0 }),
];

pub fn preset_name(e: &Easing) -> String {
    if let Some((n, _)) = PRESETS.iter().find(|(_, p)| p == e) {
        return (*n).into();
    }
    match e {
        Easing::Bezier(_) => "Custom bezier".into(),
        Easing::Spring { .. } => "Custom spring".into(),
        _ => "Custom".into(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    Dissolve,
    SmartAnimate,
    MoveIn(Direction),
    MoveOut(Direction),
    Push(Direction),
    SlideIn(Direction),
    SlideOut(Direction),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub kind: TransitionKind,
    pub easing: Easing,
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayPosition {
    #[default]
    Center,
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    /// Relative to the triggering layer, offset by `Overlay::offset`.
    Manual,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// Go to another top-level frame. `preserve_scroll` keeps scroll position.
    Navigate {
        dest: NodeId,
        preserve_scroll: bool,
    },
    OpenOverlay {
        dest: NodeId,
        position: OverlayPosition,
        offset: Vec2,
        close_on_click_outside: bool,
        background: Option<crate::Color>,
    },
    SwapOverlay {
        dest: NodeId,
    },
    CloseOverlay,
    Back,
    /// Scroll a scrollable ancestor so `dest` is visible.
    ScrollTo {
        dest: NodeId,
        offset: Vec2,
    },
    OpenUrl {
        url: String,
        new_tab: bool,
    },
    /// Change an instance to another variant of its component set.
    ChangeTo {
        dest: NodeId,
    },
    /// Set a variable (by id) to the value of an expression. Present only:
    /// the document is never changed.
    SetVariable {
        variable: String,
        value: Expression,
    },
    /// Switch a variable collection (by id) to one of its modes (by id) for the
    /// rest of the Present session.
    SetVariableMode {
        collection: String,
        mode: String,
    },
    /// Run the actions of the first branch whose condition holds, else the
    /// `otherwise` actions ("If / Else if / Else").
    Conditional {
        branches: Vec<(Expression, Vec<Action>)>,
        otherwise: Vec<Action>,
    },
    /// Control video playback on `dest` (a layer with a [`VideoFill`]).
    MediaControl {
        dest: NodeId,
        op: MediaOp,
    },
}

/// Playback control of a video layer (Figma's media actions plus loop).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MediaOp {
    Play,
    Pause,
    TogglePlay,
    Mute,
    Unmute,
    ToggleMute,
    /// Skip forward (positive) or backward (negative) by seconds.
    SkipBy(f32),
    SeekTo(f32),
    SetLoop(bool),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    pub trigger: Trigger,
    pub actions: Vec<Action>,
    /// `None` means instant.
    pub transition: Option<Transition>,
}

/// A prototype flow: a named starting frame on a page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flow {
    pub name: String,
    pub page: NodeId,
    pub start: NodeId,
    pub description: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Overflow {
    #[default]
    None,
    Horizontal,
    Vertical,
    Both,
}

/// A video fill, stored as a sidecar of an ordinary image paint.
///
/// Decision: no new `PaintKind` variant. Adding one would touch every renderer
/// (CPU, GPU, SVG/CSS export, .fig export), and a video only plays in Present.
/// Instead the node keeps a normal `PaintKind::Image` paint at `paint` whose
/// image is the poster frame (so every renderer and exporter shows it), and
/// this record says that paint is really a video. The video bytes live in
/// `Document::images` under `source` (the images map is persisted, copied by
/// clipboard/library and embedded in files like any other blob).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoFill {
    /// Index into the node's `fills` of the poster image paint.
    pub paint: usize,
    /// Key of the video file bytes in `Document::images`.
    pub source: String,
    pub autoplay: bool,
    #[serde(rename = "loop")]
    pub looping: bool,
    pub muted: bool,
}

/// Container of video bytes by magic number, without decoding anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoContainer {
    Mp4,
    WebM,
}

impl VideoContainer {
    /// Sniffs MP4 (`ftyp` box) and WebM/Matroska (EBML header).
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
            Some(VideoContainer::Mp4)
        } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            Some(VideoContainer::WebM)
        } else {
            None
        }
    }
}

/// Prototype settings for a frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrototypeProps {
    pub reactions: Vec<Reaction>,
    /// Scroll behaviour when content overflows (frames only).
    pub overflow: Overflow,
    /// Children that stay fixed while the frame scrolls.
    pub fixed_children: Vec<NodeId>,
    /// Video fills of this layer (prototype-only playback).
    #[serde(default)]
    pub videos: Vec<VideoFill>,
}

impl PrototypeProps {
    pub fn is_empty(&self) -> bool {
        self.reactions.is_empty() && self.overflow == Overflow::None && self.fixed_children.is_empty() && self.videos.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_15_easing_types_roundtrip_tags_and_names() {
        assert_eq!(EasingType::ALL.len(), 15);
        for (idx, kind) in EasingType::ALL.iter().copied().enumerate() {
            assert_eq!(kind.kiwi_tag() as usize, idx);
            assert_eq!(EasingType::from_kiwi_tag(idx as u32), Some(kind));
            assert_eq!(EasingType::from_kiwi_name(kind.kiwi_name()), Some(kind));
            assert!((kind.ease_at(None, None, 0.0) - 0.0).abs() < 1e-3, "{kind:?} at t=0.0 should start at 0");
            assert!((kind.ease_at(None, None, 1.0) - 1.0).abs() < 1e-3, "{kind:?} at t=1.0 should end at 1");
        }
        assert_eq!(EasingType::from_kiwi_tag(15), None);
        assert_eq!(EasingType::from_kiwi_name("UNKNOWN"), None);
    }

    #[test]
    fn rea_r5_presets_and_hold_step() {
        assert_eq!(DEFAULT_BEZIER, [0.0, 0.0, 0.58, 1.0]);
        assert_eq!(EasingType::InCubic.default_easing(), Easing::EaseIn);
        assert_eq!(control_points(&Easing::EaseIn), Some([0.55, 0.055, 0.675, 0.19]));
        assert_eq!(control_points(&Easing::EaseOut), Some([0.215, 0.61, 0.355, 1.0]));
        assert_eq!(control_points(&Easing::EaseInOut), Some([0.645, 0.045, 0.355, 1.0]));
        assert_eq!(control_points(&Easing::EaseInBack), Some([0.3, -0.05, 0.7, -0.5]));
        assert_eq!(control_points(&Easing::EaseOutBack), Some([0.45, 1.45, 0.8, 1.0]));
        assert_eq!(control_points(&Easing::EaseInOutBack), Some([0.7, -0.4, 0.4, 1.4]));
        assert_eq!(EasingType::GentleSpring.default_easing(), Easing::Spring { mass: 1.0, stiffness: 100.0, damping: 15.0 });
        assert_eq!(EasingType::SpringPresetOne.default_easing(), Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 });
        assert_eq!(EasingType::SpringPresetTwo.default_easing(), Easing::Spring { mass: 1.0, stiffness: 600.0, damping: 15.0 });
        assert_eq!(EasingType::SpringPresetThree.default_easing(), Easing::Spring { mass: 1.0, stiffness: 80.0, damping: 20.0 });
        assert_eq!(EasingType::Hold.ease_at(None, None, 0.0), 0.0);
        assert_eq!(EasingType::Hold.ease_at(None, None, 0.99), 0.0);
        assert_eq!(EasingType::Hold.ease_at(None, None, 1.0), 1.0);
    }
}
