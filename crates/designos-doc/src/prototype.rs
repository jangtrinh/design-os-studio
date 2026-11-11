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

/// Figma's 16 `EasingType` kiwi enum values (`0..=15`): the 15 of REA R5 plus `EASE_IN` (15), the CSS ease-in curve.
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
    /// Figma's plain "ease in": "an animation curve similar to CSS ease-in" (Figma `EasingType.EASE_IN`), which is
    /// the CSS Easing spec's `cubic-bezier(0.42, 0, 1, 1)`. Not the stronger `InCubic` curve.
    EaseIn = 15,
}

impl EasingType {
    /// All 16 `EasingType` variants in kiwi tag order (`0..=15`).
    pub const ALL: [EasingType; 16] = [
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
        EasingType::EaseIn,
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
            15 => Some(Self::EaseIn),
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
            Self::EaseIn => "EASE_IN",
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
            "EASE_IN" => Some(Self::EaseIn),
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
            Self::EaseIn => Easing::Bezier(CSS_EASE_IN),
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

/// CSS `ease-in` control points (`cubic-bezier(0.42, 0, 1, 1)`, CSS Easing Functions Level 1), the curve of Figma's `EASE_IN`.
pub const CSS_EASE_IN: [f32; 4] = [0.42, 0.0, 1.0, 1.0];

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
            Easing::Bezier(c) if c == CSS_EASE_IN => EasingType::EaseIn,
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

fn normalize_preset_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = false;
    for ch in name.trim().chars() {
        if ch.is_ascii_whitespace() || ch == '_' || ch == '-' {
            if !last_dash && !out.is_empty() {
                out.push('-');
                last_dash = true;
            }
        } else {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

/// Parses an easing preset name into an [`Easing`] curve.
pub fn parse_easing_preset(name: &str) -> Option<Easing> {
    let norm = normalize_preset_name(name);
    match norm.as_str() {
        "linear" => Some(Easing::Linear),
        "ease-in" | "in-cubic" => Some(Easing::EaseIn),
        "css-ease-in" => Some(Easing::Bezier(CSS_EASE_IN)),
        "ease-out" | "out-cubic" => Some(Easing::EaseOut),
        "ease-in-out" | "ease-in-and-out" | "inout-cubic" | "in-out-cubic" => Some(Easing::EaseInOut),
        "ease-in-back" | "in-back" | "in-back-cubic" => Some(Easing::EaseInBack),
        "ease-out-back" | "out-back" | "out-back-cubic" => Some(Easing::EaseOutBack),
        "ease-in-out-back" | "ease-in-and-out-back" | "inout-back" | "in-out-back" | "inout-back-cubic" => Some(Easing::EaseInOutBack),
        "spring" | "quick" | "spring-preset-one" => Some(Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 }),
        "gentle" | "gentle-spring" => Some(Easing::Spring { mass: 1.0, stiffness: 100.0, damping: 15.0 }),
        "bouncy" | "spring-preset-two" => Some(Easing::Spring { mass: 1.0, stiffness: 600.0, damping: 15.0 }),
        "slow" | "spring-preset-three" => Some(Easing::Spring { mass: 1.0, stiffness: 80.0, damping: 20.0 }),
        "hold" => Some(Easing::Linear),
        _ => None,
    }
}

/// Parses an easing preset name into Motion keyframe interpolation and curve parameters.
pub fn parse_key_easing_preset(name: &str) -> Option<(crate::InterpolationType, crate::KeyEasing)> {
    let norm = normalize_preset_name(name);
    match norm.as_str() {
        "linear" => Some((crate::InterpolationType::Bezier, crate::KeyEasing::new(EasingType::Linear))),
        "ease-in" | "in-cubic" => Some((crate::InterpolationType::Bezier, crate::KeyEasing::new(EasingType::InCubic))),
        "css-ease-in" => {
            Some((crate::InterpolationType::Bezier, crate::KeyEasing { easing_type: EasingType::EaseIn, bezier: Some(CSS_EASE_IN), spring: None }))
        }
        "ease-out" | "out-cubic" => Some((crate::InterpolationType::Bezier, crate::KeyEasing::new(EasingType::OutCubic))),
        "ease-in-out" | "ease-in-and-out" | "inout-cubic" | "in-out-cubic" => {
            Some((crate::InterpolationType::Bezier, crate::KeyEasing::new(EasingType::InOutCubic)))
        }
        "ease-in-back" | "in-back" | "in-back-cubic" => Some((crate::InterpolationType::Bezier, crate::KeyEasing::new(EasingType::InBackCubic))),
        "ease-out-back" | "out-back" | "out-back-cubic" => Some((crate::InterpolationType::Bezier, crate::KeyEasing::new(EasingType::OutBackCubic))),
        "ease-in-out-back" | "ease-in-and-out-back" | "inout-back" | "in-out-back" | "inout-back-cubic" => {
            Some((crate::InterpolationType::Bezier, crate::KeyEasing::new(EasingType::InOutBackCubic)))
        }
        "spring" | "quick" | "spring-preset-one" => Some((
            crate::InterpolationType::Spring,
            crate::KeyEasing {
                easing_type: EasingType::SpringPresetOne,
                bezier: None,
                spring: Some(crate::SpringParams { mass: 1.0, stiffness: 300.0, damping: 20.0, normalized: false }),
            },
        )),
        "gentle" | "gentle-spring" => Some((
            crate::InterpolationType::Spring,
            crate::KeyEasing {
                easing_type: EasingType::GentleSpring,
                bezier: None,
                spring: Some(crate::SpringParams { mass: 1.0, stiffness: 100.0, damping: 15.0, normalized: false }),
            },
        )),
        "bouncy" | "spring-preset-two" => Some((
            crate::InterpolationType::Spring,
            crate::KeyEasing {
                easing_type: EasingType::SpringPresetTwo,
                bezier: None,
                spring: Some(crate::SpringParams { mass: 1.0, stiffness: 600.0, damping: 15.0, normalized: false }),
            },
        )),
        "slow" | "spring-preset-three" => Some((
            crate::InterpolationType::Spring,
            crate::KeyEasing {
                easing_type: EasingType::SpringPresetThree,
                bezier: None,
                spring: Some(crate::SpringParams { mass: 1.0, stiffness: 80.0, damping: 20.0, normalized: false }),
            },
        )),
        "hold" => Some((crate::InterpolationType::Hold, crate::KeyEasing::new(EasingType::Hold))),
        _ => None,
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

impl Transition {
    pub fn effective_duration_ms(&self) -> u32 {
        effective_duration_ms(&self.easing, self.duration_ms)
    }

    pub fn duration_us(&self) -> u64 {
        u64::from(self.effective_duration_ms()) * 1000
    }

    pub fn evaluate_at_ms(&self, t_ms: f32) -> f32 {
        let dur = self.effective_duration_ms() as f32;
        if dur <= 0.0 {
            1.0
        } else {
            ease(&self.easing, (t_ms / dur).clamp(0.0, 1.0))
        }
    }

    pub fn evaluate_at_us(&self, t_us: i64) -> f32 {
        self.evaluate_at_ms(t_us as f32 / 1000.0)
    }
}

/// Resolves a transition on a node's prototype reaction, taking active variable bindings into account.
pub fn resolve_transition(doc: &crate::Document, node_id: crate::NodeId, reaction_index: usize) -> Option<Transition> {
    let node = doc.get(node_id)?;
    let resolved = crate::resolve_node(doc, node);
    resolved.prototype.reactions.get(reaction_index)?.transition
}

/// Evaluates progress `0.0..=1.0` of a node's prototype reaction transition at time `t_ms`.
pub fn evaluate_transition(doc: &crate::Document, node_id: crate::NodeId, reaction_index: usize, t_ms: f32) -> Option<f32> {
    let node = doc.get(node_id)?;
    let resolved = crate::resolve_node(doc, node);
    let reaction = resolved.prototype.reactions.get(reaction_index)?;
    Some(match reaction.transition {
        Some(ref tr) => tr.evaluate_at_ms(t_ms),
        None => {
            if t_ms < 0.0 {
                0.0
            } else {
                1.0
            }
        }
    })
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
    fn ease_in_is_the_css_ease_in_curve_not_the_cubic_preset() {
        // Figma documents EASE_IN as "an animation curve similar to CSS ease-in" (figma-api EasingType docs);
        // CSS ease-in is cubic-bezier(0.42, 0, 1, 1) (CSS Easing Functions Level 1).
        let want = |t: f32| cubic_bezier([0.42, 0.0, 1.0, 1.0], t);
        for t in [0.1, 0.25, 0.5, 0.75, 0.9] {
            assert_eq!(EasingType::EaseIn.ease_at(None, None, t), want(t));
            assert!((EasingType::EaseIn.ease_at(None, None, t) - EasingType::InCubic.ease_at(None, None, t)).abs() > 1e-3, "differs from InCubic at {t}");
        }
        assert!((EasingType::EaseIn.ease_at(None, None, 0.5) - 0.3155).abs() < 1e-3);
        assert_eq!(EasingType::EaseIn.to_easing(None, None), Easing::Bezier(CSS_EASE_IN));
        assert_eq!(Easing::Bezier(CSS_EASE_IN).easing_type(), EasingType::EaseIn);
        assert_eq!(Easing::Bezier([0.4, 0.0, 1.0, 1.0]).easing_type(), EasingType::CustomCubic);
        assert_eq!((EasingType::EaseIn.kiwi_tag(), EasingType::from_kiwi_name("EASE_IN")), (15, Some(EasingType::EaseIn)));
    }

    #[test]
    fn all_16_easing_types_roundtrip_tags_and_names() {
        assert_eq!(EasingType::ALL.len(), 16);
        for (idx, kind) in EasingType::ALL.iter().copied().enumerate() {
            assert_eq!(kind.kiwi_tag() as usize, idx);
            assert_eq!(EasingType::from_kiwi_tag(idx as u32), Some(kind));
            assert_eq!(EasingType::from_kiwi_name(kind.kiwi_name()), Some(kind));
            assert!((kind.ease_at(None, None, 0.0) - 0.0).abs() < 1e-3, "{kind:?} at t=0.0 should start at 0");
            assert!((kind.ease_at(None, None, 1.0) - 1.0).abs() < 1e-3, "{kind:?} at t=1.0 should end at 1");
        }
        assert_eq!(EasingType::from_kiwi_tag(16), None);
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

    #[test]
    fn parse_easing_preset_normalizes_and_maps_names() {
        assert_eq!(parse_easing_preset("linear"), Some(Easing::Linear));
        assert_eq!(parse_easing_preset("  EASE_IN "), Some(Easing::EaseIn));
        assert_eq!(parse_easing_preset("in-cubic"), Some(Easing::EaseIn));
        assert_eq!(parse_easing_preset("css-ease-in"), Some(Easing::Bezier(CSS_EASE_IN)));
        assert_eq!(parse_easing_preset("ease-out"), Some(Easing::EaseOut));
        assert_eq!(parse_easing_preset("out-cubic"), Some(Easing::EaseOut));
        assert_eq!(parse_easing_preset("ease-in-out"), Some(Easing::EaseInOut));
        assert_eq!(parse_easing_preset("Ease In And Out"), Some(Easing::EaseInOut));
        assert_eq!(parse_easing_preset("inout-cubic"), Some(Easing::EaseInOut));
        assert_eq!(parse_easing_preset("ease-in-back"), Some(Easing::EaseInBack));
        assert_eq!(parse_easing_preset("ease-out-back"), Some(Easing::EaseOutBack));
        assert_eq!(parse_easing_preset("ease-in-out-back"), Some(Easing::EaseInOutBack));
        assert_eq!(parse_easing_preset("spring"), Some(Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 }));
        assert_eq!(parse_easing_preset("quick"), Some(Easing::Spring { mass: 1.0, stiffness: 300.0, damping: 20.0 }));
        assert_eq!(parse_easing_preset("gentle"), Some(Easing::Spring { mass: 1.0, stiffness: 100.0, damping: 15.0 }));
        assert_eq!(parse_easing_preset("bouncy"), Some(Easing::Spring { mass: 1.0, stiffness: 600.0, damping: 15.0 }));
        assert_eq!(parse_easing_preset("slow"), Some(Easing::Spring { mass: 1.0, stiffness: 80.0, damping: 20.0 }));
        assert_eq!(parse_easing_preset("hold"), Some(Easing::Linear));
        assert_eq!(parse_easing_preset("unknown-preset"), None);
    }

    #[test]
    fn parse_key_easing_preset_supports_springs_and_curves() {
        let (interp, ke) = parse_key_easing_preset("spring").unwrap();
        assert_eq!(interp, crate::InterpolationType::Spring);
        assert_eq!(ke.easing_type, EasingType::SpringPresetOne);
        assert!(ke.spring.is_some());

        let (interp_b, ke_b) = parse_key_easing_preset("ease-in-out").unwrap();
        assert_eq!(interp_b, crate::InterpolationType::Bezier);
        assert_eq!(ke_b.easing_type, EasingType::InOutCubic);

        let (interp_h, ke_h) = parse_key_easing_preset("hold").unwrap();
        assert_eq!(interp_h, crate::InterpolationType::Hold);
        assert_eq!(ke_h.easing_type, EasingType::Hold);
    }

    #[test]
    fn transition_effective_duration_and_evaluate_at() {
        let tr = Transition { kind: TransitionKind::SmartAnimate, easing: Easing::Linear, duration_ms: 500 };
        assert_eq!(tr.effective_duration_ms(), 500);
        assert_eq!(tr.duration_us(), 500_000);
        assert_eq!(tr.evaluate_at_ms(0.0), 0.0);
        assert_eq!(tr.evaluate_at_ms(250.0), 0.5);
        assert_eq!(tr.evaluate_at_ms(500.0), 1.0);
        assert_eq!(tr.evaluate_at_us(250_000), 0.5);
    }
}
