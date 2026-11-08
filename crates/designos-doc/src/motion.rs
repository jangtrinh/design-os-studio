//! The Motion model: timelines, tracks and keyframes that animate the properties of a layer.
//!
//! A [`NodeMotion`] hangs off a [`crate::Node`] (`Node::motion`, absent for almost every layer) and holds three
//! things, mirroring how a Figma file records Motion:
//!
//! * `timelines`: the timelines this layer *owns* (a duration, a playback style, an optional parent timeline for
//!   nesting, snapping and quantize settings);
//! * `assignments`: which timelines play on this layer, with an offset in microseconds (this is how a timeline is
//!   nested inside another one);
//! * `tracks`: the animated properties of *this* layer. Each track belongs to one timeline, names one property,
//!   says how its value combines with the layer's own value ([`KeyframeOperation`]) and holds sorted keyframes.
//!
//! Everything is plain data. Times are signed microseconds exactly as recorded, so a file survives a round trip
//! unchanged. Evaluating a timeline at a time `t` is a separate pure step built on [`Track::keyframes`] and
//! [`interpolate`](crate::motion_value::interpolate); this module never samples.
//!
//! Edits go through the `Command::Update` of the owning node (see [`crate::motion_ops`]), so each edit is one undo
//! step. A document without Motion serializes exactly as before: the field is skipped when `None`.

use crate::{Easing, EasingType};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use crate::motion_value::{interpolate, ColorSpace, KeyValue};

/// An id of a timeline or a track. In a `.fig` file this is the GUID text `"session:local"`; documents made here
/// use any string that is unique in the document.
pub type MotionId = String;

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// How a timeline repeats once it reaches its duration (`PlaybackStyle` in the kiwi schema).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaybackStyle {
    /// Play to the end and hold the last frame.
    #[default]
    Once,
    /// Start again from zero at the end.
    Loop,
    /// Play forward, then backward, then forward again.
    Boomerang,
}

/// How a track's value combines with the layer's own value (`KeyframeOperation`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyframeOperation {
    /// The track value replaces the layer value.
    #[default]
    Set,
    /// The layer value is multiplied by the track value.
    Scale,
    /// The track value is added to the layer value.
    Offset,
}

/// Whether a track repeats on its own inside its timeline (`TrackLoopMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackLoop {
    #[default]
    None,
    /// The keyframes of the track repeat for as long as the timeline runs.
    Loop,
}

/// The curve family of the segment that ends at a keyframe (`InterpolationType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InterpolationType {
    /// Keep the previous value until the keyframe time.
    Hold,
    #[default]
    Bezier,
    Spring,
}

/// A frame rate as a fraction (`FrameRate`), e.g. 30000/1001.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl FrameRate {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self { numerator, denominator }
    }

    /// Microseconds in one frame, rounded to the nearest one. `None` when the rate is zero.
    pub fn frame_us(self) -> Option<i64> {
        if self.numerator == 0 || self.denominator == 0 {
            return None;
        }
        let (n, d) = (i128::from(self.numerator), i128::from(self.denominator));
        // a rate above 2,000,000 fps rounds to a zero-length frame, which cannot snap anything
        Some(((1_000_000 * d + n / 2) / n) as i64).filter(|us| *us > 0)
    }
}

/// A timeline definition (`TimelineData`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    pub id: MotionId,
    #[serde(default, skip_serializing_if = "is_default")]
    pub duration_us: u64,
    /// The timeline a layer plays when nothing else is chosen.
    #[serde(default, skip_serializing_if = "is_default")]
    pub is_default: bool,
    /// The timeline that contains this one (nesting). It must exist and must not lead back to this timeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<MotionId>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub playback: PlaybackStyle,
    #[serde(default, skip_serializing_if = "is_default")]
    pub autoplay: bool,
    /// Keyframe times snap to whole frames of this rate while editing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snap_fps: Option<FrameRate>,
    /// Playback only shows whole frames of `snap_fps`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub quantize_playback: bool,
}

impl Timeline {
    pub fn new(id: impl Into<MotionId>, duration_us: u64) -> Self {
        Self {
            id: id.into(),
            duration_us,
            is_default: false,
            parent: None,
            playback: PlaybackStyle::Once,
            autoplay: false,
            snap_fps: None,
            quantize_playback: false,
        }
    }

    /// `time_us` moved onto the nearest frame of `snap_fps`; unchanged when the timeline does not snap.
    pub fn snap(&self, time_us: i64) -> i64 {
        match self.snap_fps.and_then(FrameRate::frame_us) {
            Some(frame) => ((time_us as f64 / frame as f64).round() as i64).saturating_mul(frame),
            None => time_us,
        }
    }
}

/// A timeline played on a layer (`TimelineAssignmentKey` + `TimelineBindingData`). `containing` is the timeline
/// the assigned one is nested in, so the same timeline can sit at a different offset in each container.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineAssignment {
    pub timeline: MotionId,
    pub containing: MotionId,
    #[serde(default, skip_serializing_if = "is_default")]
    pub offset_us: i64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub autoplay: bool,
}

/// A spring in a keyframe easing (`SpringParams`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpringParams {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
    /// `true` when the numbers are the normalized (duration/bounce) form of the kiwi `SpringType`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub normalized: bool,
}

/// The easing of a keyframe (`EasingData`), reusing the [`EasingType`] of the prototype player.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyEasing {
    pub easing_type: EasingType,
    /// Control points of a `CustomCubic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bezier: Option<[f32; 4]>,
    /// Parameters of a `CustomSpring`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spring: Option<SpringParams>,
}

impl KeyEasing {
    pub fn new(easing_type: EasingType) -> Self {
        Self { easing_type, bezier: None, spring: None }
    }

    /// The curve as the prototype player's [`Easing`].
    pub fn to_easing(&self) -> Easing {
        self.easing_type.to_easing(self.bezier, self.spring.map(|s| (s.mass, s.stiffness, s.damping)))
    }

    /// Eased progress for linear progress `u` in `0..=1` (may overshoot for back and spring curves).
    pub fn progress(&self, u: f32) -> f32 {
        self.easing_type.ease_at(self.bezier, self.spring.map(|s| (s.mass, s.stiffness, s.damping)), u)
    }
}

impl Default for KeyEasing {
    fn default() -> Self {
        Self::new(EasingType::default())
    }
}

/// One keyframe of a [`Track`] (`KEYFRAME` node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    /// Microseconds from the start of the track's timeline.
    pub time_us: i64,
    pub value: KeyValue,
    /// The curve family of the segment that ends at this keyframe.
    #[serde(default, skip_serializing_if = "is_default")]
    pub interpolation: InterpolationType,
    /// The easing of that segment. `None` plays the default easing (`OutCubic`), whatever the interpolation type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub easing: Option<KeyEasing>,
    /// Colour space used to blend colour values into this keyframe.
    #[serde(default, skip_serializing_if = "is_default")]
    pub color_space: ColorSpace,
    /// The separate bezier handles a Figma keyframe records next to its easing (`bezierHandles`), kept so a
    /// `.fig` round trip writes them back. `easing` is what plays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bezier_handles: Option<[f32; 4]>,
}

impl Keyframe {
    pub fn new(time_us: i64, value: KeyValue) -> Self {
        Self { time_us, value, interpolation: InterpolationType::Bezier, easing: None, color_space: ColorSpace::Default, bezier_handles: None }
    }

    /// Eased progress of the segment ending at this keyframe, for linear progress `u` in `0..=1`.
    /// `Hold` stays at 0 until `u` reaches 1; a keyframe without an easing uses the default easing.
    pub fn progress(&self, u: f32) -> f32 {
        match (self.interpolation, &self.easing) {
            (InterpolationType::Hold, _) => {
                if u >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
            (_, Some(e)) => e.progress(u),
            (_, None) => KeyEasing::default().progress(u),
        }
    }
}

/// An animated property of a layer, named by the kiwi `VariableField` the Figma file binds the track to.
/// The typed variants are the ones Motion animates; any other field is kept by name in `Other`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TrackProperty {
    Opacity,
    CornerRadius,
    StrokeWeight,
    Width,
    Height,
    TextData,
    TranslationX,
    TranslationY,
    TranslationXy,
    Rotation,
    ScaleX,
    ScaleY,
    ScaleXy,
    Shear,
    TransformOriginX,
    TransformOriginY,
    TransformOriginXy,
    PathTrimStart,
    PathTrimEnd,
    /// Any other kiwi `VariableField` name, verbatim.
    Other(String),
}

const PROPERTY_NAMES: [(TrackProperty, &str); 19] = [
    (TrackProperty::Opacity, "OPACITY"),
    (TrackProperty::CornerRadius, "CORNER_RADIUS"),
    (TrackProperty::StrokeWeight, "STROKE_WEIGHT"),
    (TrackProperty::Width, "WIDTH"),
    (TrackProperty::Height, "HEIGHT"),
    (TrackProperty::TextData, "TEXT_DATA"),
    (TrackProperty::TranslationX, "MOTION_TRANSLATION_X"),
    (TrackProperty::TranslationY, "MOTION_TRANSLATION_Y"),
    (TrackProperty::TranslationXy, "MOTION_TRANSLATION_XY"),
    (TrackProperty::Rotation, "MOTION_ROTATION"),
    (TrackProperty::ScaleX, "MOTION_SCALE_X"),
    (TrackProperty::ScaleY, "MOTION_SCALE_Y"),
    (TrackProperty::ScaleXy, "MOTION_SCALE_XY"),
    (TrackProperty::Shear, "MOTION_SHEAR"),
    (TrackProperty::TransformOriginX, "MOTION_TRANSFORM_ORIGIN_X"),
    (TrackProperty::TransformOriginY, "MOTION_TRANSFORM_ORIGIN_Y"),
    (TrackProperty::TransformOriginXy, "MOTION_TRANSFORM_ORIGIN_XY"),
    (TrackProperty::PathTrimStart, "PATH_TRIM_START"),
    (TrackProperty::PathTrimEnd, "PATH_TRIM_END"),
];

impl TrackProperty {
    /// The kiwi `VariableField` name.
    pub fn kiwi_name(&self) -> &str {
        match self {
            TrackProperty::Other(name) => name,
            typed => PROPERTY_NAMES.iter().find(|(p, _)| p == typed).map(|(_, n)| *n).unwrap_or("MISSING"),
        }
    }

    /// The property for a kiwi `VariableField` name; an unknown name becomes `Other`.
    pub fn from_kiwi_name(name: &str) -> Self {
        PROPERTY_NAMES.iter().find(|(_, n)| *n == name).map(|(p, _)| p.clone()).unwrap_or_else(|| TrackProperty::Other(name.to_string()))
    }
}

impl Serialize for TrackProperty {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.kiwi_name())
    }
}

impl<'de> Deserialize<'de> for TrackProperty {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        // An empty name is read as `Other("")` so a file never fails to open over it; the load repair drops the
        // track and counts it, and the writer never writes one.
        Ok(Self::from_kiwi_name(&String::deserialize(d)?))
    }
}

fn sorted_keyframes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Keyframe>, D::Error> {
    let mut track = Track::new("", "", TrackProperty::Opacity);
    track.keyframes = Vec::<Keyframe>::deserialize(d)?;
    track.normalize();
    Ok(track.keyframes)
}

/// Writes the tracks that have a property; a track with an empty property name is never written.
fn serialize_tracks<S: Serializer>(tracks: &[Track], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(tracks.iter().filter(|t| !t.property.kiwi_name().is_empty()))
}

/// A sequence of keyframes for one property of the layer that owns it (`KEYFRAME_TRACK` node).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: MotionId,
    /// The timeline whose clock the keyframe times are measured on.
    pub timeline: MotionId,
    pub property: TrackProperty,
    #[serde(default, skip_serializing_if = "is_default")]
    pub operation: KeyframeOperation,
    #[serde(default, skip_serializing_if = "is_default")]
    pub loop_mode: TrackLoop,
    /// Sorted by `time_us` (stable). A file may hold several keyframes at one time (a step), and all of them are kept.
    /// The editing calls never add a second keyframe at a time that has one: set replaces, and set, remove and
    /// move act on the last keyframe at a time. A file written elsewhere may hold them in any order; they are
    /// sorted when the file is read.
    #[serde(default, deserialize_with = "sorted_keyframes")]
    pub keyframes: Vec<Keyframe>,
}

impl Track {
    pub fn new(id: impl Into<MotionId>, timeline: impl Into<MotionId>, property: TrackProperty) -> Self {
        Self { id: id.into(), timeline: timeline.into(), property, operation: KeyframeOperation::Set, loop_mode: TrackLoop::None, keyframes: Vec::new() }
    }

    /// Index just past the last keyframe at or before `time_us`.
    fn after(&self, time_us: i64) -> usize {
        self.keyframes.partition_point(|k| k.time_us <= time_us)
    }

    /// Index of the last keyframe at `time_us`, if any. A file may hold several keyframes at one time (a step);
    /// the editing calls act on the last of them.
    fn last_at(&self, time_us: i64) -> Option<usize> {
        self.after(time_us).checked_sub(1).filter(|i| self.keyframes[*i].time_us == time_us)
    }

    /// Puts `key` in time order. A keyframe already at that time is replaced (the last one, when a file holds
    /// several) and returned.
    pub fn set_keyframe(&mut self, key: Keyframe) -> Option<Keyframe> {
        match self.last_at(key.time_us) {
            Some(i) => Some(std::mem::replace(&mut self.keyframes[i], key)),
            None => {
                let at = self.after(key.time_us);
                self.keyframes.insert(at, key);
                None
            }
        }
    }

    /// Removes the keyframe at `time_us` (the last one, when a file holds several).
    pub fn remove_keyframe(&mut self, time_us: i64) -> Option<Keyframe> {
        let i = self.last_at(time_us)?;
        Some(self.keyframes.remove(i))
    }

    /// The keyframe at `time_us` (the last one, when a file holds several).
    pub fn keyframe(&self, time_us: i64) -> Option<&Keyframe> {
        self.last_at(time_us).map(|i| &self.keyframes[i])
    }

    /// Sorts by time. The sort is stable and drops nothing: keyframes that share a time are all kept, in the order the
    /// file has them. Files made elsewhere may be unsorted.
    pub fn normalize(&mut self) {
        self.keyframes.sort_by_key(|k| k.time_us);
    }
}

/// Linear progress `0..=1` of `t_us` in the segment from `from` to `to`. A segment that has no length (the two share
/// a time) is a step: progress is 1 at and after that time, so the value jumps to `to`.
pub fn segment_progress(from: &Keyframe, to: &Keyframe, t_us: i64) -> f32 {
    if to.time_us <= from.time_us {
        return if t_us >= to.time_us { 1.0 } else { 0.0 };
    }
    (((t_us - from.time_us) as f64) / ((to.time_us - from.time_us) as f64)).clamp(0.0, 1.0) as f32
}

/// The Motion data of one layer. Empty means the layer has no Motion and the field is not written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeMotion {
    /// Timelines this layer owns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timelines: Vec<Timeline>,
    /// Timelines that play on this layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignments: Vec<TimelineAssignment>,
    /// Animated properties of this layer.
    #[serde(default, skip_serializing_if = "Vec::is_empty", serialize_with = "serialize_tracks")]
    pub tracks: Vec<Track>,
}

impl NodeMotion {
    pub fn is_empty(&self) -> bool {
        self.timelines.is_empty() && self.assignments.is_empty() && self.tracks.is_empty()
    }

    pub fn timeline(&self, id: &str) -> Option<&Timeline> {
        self.timelines.iter().find(|t| t.id == id)
    }

    pub fn track(&self, id: &str) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: &str) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// The track of `property` on `timeline`, if any.
    pub fn track_for(&self, timeline: &str, property: &TrackProperty) -> Option<&Track> {
        self.tracks.iter().find(|t| t.timeline == timeline && &t.property == property)
    }
}
