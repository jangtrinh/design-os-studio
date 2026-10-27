//! Spec-Driven Declarative UI Builder, EaseUI BrainWave 2.0 Token/Preset Engine,
//! Auto-Layout 5.0 Subtree Solver, and Canvas Comment Pin Target Resolver.
#![allow(clippy::result_large_err)]

use crate::*;
use serde::{Deserialize, Serialize};

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}
fn default_one_num_or_var() -> NumberOrVar {
    NumberOrVar::Num(1.0)
}

/// Auto-Layout 5.0 configuration for a container frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct LayoutSpec {
    /// `"row"` / `"horizontal"` or `"column"` / `"vertical"`.
    #[serde(default, alias = "dir")]
    pub direction: String,
    /// Wrap overflowing children onto additional lines when `true`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrap: bool,
    /// Primary-axis child gap in px (or `"var:<collection>/<name>"`).
    #[serde(default, skip_serializing_if = "NumberOrVar::is_zero")]
    pub gap: NumberOrVar,
    /// Cross-axis spacing between wrapped lines in px (or `"var:..."`).
    #[serde(default, skip_serializing_if = "NumberOrVar::is_zero")]
    pub counter_gap: NumberOrVar,
    /// `[top, right, bottom, left]` padding in px or `"var:..."` references.
    #[serde(default)]
    pub padding: [NumberOrVar; 4],
    /// `"start"`, `"center"`, `"end"`, `"space-between"`, `"space-around"`, `"space-evenly"`, or `"baseline"` (alias: `"justify"`).
    #[serde(default, alias = "justify", skip_serializing_if = "String::is_empty")]
    pub primary_align: String,
    /// `"start"`, `"center"`, `"end"`, or `"baseline"` (alias: `"align"`).
    #[serde(default, alias = "align", skip_serializing_if = "String::is_empty")]
    pub counter_align: String,
    /// Grid column count when using grid layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_columns: Option<u32>,
    /// Grid row count when using grid layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_rows: Option<u32>,
}

/// Declarative node specification for `apply_spec` and `export_spec`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NodeSpec {
    /// Node ID (assigned on export; optional on input).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// `"frame"`, `"rectangle"`, `"ellipse"`, `"text"`, `"section"`, `"component"`, or `"instance"`.
    #[serde(default)]
    pub kind: String,
    /// Layer name (avoid default names like `"Frame 1"`).
    #[serde(default)]
    pub name: String,
    /// Component ref for `"instance"` (`"node:<id>"`, key, or `"<Library>/<Component>"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Variant selection `{property: value}` for `"instance"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variant: BTreeMap<String, String>,
    /// Component property overrides `{prop: value}` on an `"instance"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, serde_json::Value>,
    /// Descendant overrides inside an `"instance"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<OverrideSpec>,
    /// Slot fills `{slot: [NodeSpec...] | false}` on an `"instance"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, SlotFillSpec>,
    /// EaseUI preset (`"easeui-screen"`, `"easeui-panel"`, `"easeui-card"`, `"easeui-button-primary"`, `"easeui-button-secondary"`, `"easeui-input"`, `"easeui-badge"`, `"easeui-row"`, `"easeui-column"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// Parent-relative X in px.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub x: f32,
    /// Parent-relative Y in px.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub y: f32,
    /// Width in px (or `"var:..."` Number variable reference).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<NumberOrVar>,
    /// Height in px (or `"var:..."` Number variable reference).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<NumberOrVar>,
    /// `"fixed"`, `"hug"`, or `"fill"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_sizing: Option<String>,
    /// `"fixed"`, `"hug"`, or `"fill"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_sizing: Option<String>,
    /// Min width in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f32>,
    /// Max width in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f32>,
    /// Min height in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f32>,
    /// Max height in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f32>,
    /// Grid column span (`>= 1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col_span: Option<u16>,
    /// Grid row span (`>= 1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_span: Option<u16>,
    /// Fill (`"#18181B"`, `"var:<collection>/<name>"`, `"style:<name>"`, or `"$easeui.bg.panel"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Stroke (`"#RRGGBB"`, `"var:<collection>/<name>"`, `"style:<name>"`, or EaseUI token).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// Stroke weight in px (or `"var:..."`).
    #[serde(default, alias = "stroke_width", skip_serializing_if = "Option::is_none")]
    pub stroke_weight: Option<NumberOrVar>,
    /// Uniform corner radius in px (or `"var:..."`).
    #[serde(default, alias = "radius", skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<NumberOrVar>,
    /// Per-corner radii `[tl, tr, br, bl]` in px (or `"var:..."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radii: Option<[NumberOrVar; 4]>,
    /// Opacity (`0.0..=1.0`, default `1.0`, or `"var:..."`).
    #[serde(default = "default_one_num_or_var", skip_serializing_if = "NumberOrVar::is_one")]
    pub opacity: NumberOrVar,
    /// Clip overflowing children when `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clips_content: Option<bool>,
    /// `false` hides the node; a `"var:..."` string binds `visible` to a Boolean variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<BoolOrVar>,
    /// Text characters (for `"text"`, or auto-creates a child label on a button/badge frame).
    /// A `"var:..."` string binds `characters` to a String variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Font family name (default `"Inter"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schema", schemars(skip))]
    pub font_family: Option<String>,
    /// Font size in px (or `"var:..."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<NumberOrVar>,
    /// Line height in px (or `"var:..."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<NumberOrVar>,
    /// Font weight (`100..=900`, e.g. `400`, `500`, `600`, `700`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    /// Text colour (`"#RRGGBB"`, `"var:<collection>/<name>"`, or `"$easeui.text.primary"`).
    #[serde(default, alias = "color", skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    /// Shared text style (`"style:<name>"` or style name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_style: Option<String>,
    /// Shared effect style (`"style:<name>"` or style name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_style: Option<String>,
    /// Layout grid style name or `"style:<key>"` reference (frame-like nodes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_style: Option<String>,
    /// Variable mode overrides `{collection: mode}` on this container.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub mode: std::collections::BTreeMap<String, String>,
    /// Auto-Layout 5.0 configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutSpec>,
    /// `"ready"` (`ReadyForDev`) or `"completed"` (`Completed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev_status: Option<String>,
    /// Single handoff annotation string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation: Option<String>,
    /// Handoff annotations attached to this node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<String>,
    /// Agent `in_progress` flag: `true`, `false`, or `{run_id, agent, status, stale_after_s}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_progress: Option<InProgressSpec>,
    /// Resolved canvas comment threads on export.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<ResolvedCommentThread>,
    /// Child `NodeSpec` items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeSpec>,
}

/// Literal `false` used by [`SlotFillSpec::Disabled`] (`slots: {"Actions": false}`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FalseLiteral;

#[cfg(feature = "schema")]
impl schemars::JsonSchema for FalseLiteral {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("FalseLiteral")
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "boolean",
            "enum": [false],
            "description": "Literal false to disable an optional component slot."
        })
    }
}

impl Serialize for FalseLiteral {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(false)
    }
}

impl<'de> Deserialize<'de> for FalseLiteral {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let b = bool::deserialize(deserializer)?;
        if !b {
            Ok(FalseLiteral)
        } else {
            Err(serde::de::Error::custom("INVALID_SLOT_VALUE: slot value must be false or [NodeSpec...]"))
        }
    }
}

/// Slot fill or disable value in `slots` (`[NodeSpec...] | false`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum SlotFillSpec {
    /// Disable and hide the slot (`false`).
    Disabled(FalseLiteral),
    /// Populate the slot with child `NodeSpec` items (`[NodeSpec...]`).
    Fill(Vec<NodeSpec>),
}

impl<'de> Deserialize<'de> for SlotFillSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(deserializer)?;
        match v {
            serde_json::Value::Bool(false) => Ok(SlotFillSpec::Disabled(FalseLiteral)),
            serde_json::Value::Array(_) => serde_json::from_value::<Vec<NodeSpec>>(v).map(SlotFillSpec::Fill).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("INVALID_SLOT_VALUE: slot value must be false or [NodeSpec...]")),
        }
    }
}

/// Descendant override inside an `"instance"` node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OverrideSpec {
    /// Descendant path inside the instance (`"Header/Title"` or `"Card[1]/Label"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Numeric descendant node ID (alternative to `path`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// Override text on a descendant `"text"` layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Override fill (`"#RRGGBB"`, `"var:<collection>/<name>"`, or `"style:<name>"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Override stroke (`"#RRGGBB"`, `"var:<collection>/<name>"`, or `"style:<name>"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// Toggle descendant visibility (`true` | `false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Override descendant opacity (`0.0..=1.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    /// Swap nested instance component (`"<Component>"` or `"<Library>/<Component>"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<String>,
    /// Variant properties `{property: value}` on a nested instance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variant: BTreeMap<String, String>,
    /// Component properties `{prop: value}` on a nested instance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, serde_json::Value>,
    /// Slot fills `{slot: [NodeSpec...] | false}` on a nested instance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, SlotFillSpec>,
}

fn spec_levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = b_chars.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for (i, &ca) in a_chars.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b_chars.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Parses a `serde` `unknown field` error string into `(received_field, closest_5_valid_fields)`.
pub fn extract_unknown_field_options(err_msg: &str) -> Option<(String, Vec<String>)> {
    let (_, after_prefix) = err_msg.split_once("unknown field `")?;
    let (bad, rest) = after_prefix.split_once('`')?;
    let mut raw_fields = Vec::new();
    if let Some((_, expected_part)) = rest.split_once("expected ") {
        let mut rem = expected_part;
        while let Some((_, after_bt)) = rem.split_once('`') {
            if let Some((tok, tail)) = after_bt.split_once('`') {
                if !tok.is_empty() && !raw_fields.iter().any(|x: &String| x == tok) {
                    raw_fields.push(tok.to_string());
                }
                rem = tail;
            } else {
                break;
            }
        }
    }
    // Filter out serde aliases when their canonical field name is present.
    let alias_pairs = [
        ("dir", "direction"),
        ("justify", "primary_align"),
        ("align", "counter_align"),
        ("stroke_width", "stroke_weight"),
        ("radius", "corner_radius"),
        ("color", "text_color"),
    ];
    let filtered: Vec<String> = raw_fields
        .iter()
        .filter(|cand| !alias_pairs.iter().any(|(alias, canon)| cand.as_str() == *alias && raw_fields.iter().any(|x| x == canon)))
        .cloned()
        .collect();

    let bad_lower = bad.to_ascii_lowercase();
    let mut scored: Vec<(usize, usize, String)> = filtered
        .into_iter()
        .take(10_000)
        .enumerate()
        .map(|(idx, cand)| {
            let dist = spec_levenshtein(&bad_lower, &cand.to_ascii_lowercase());
            (dist, idx, cand)
        })
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let valid_options: Vec<String> = scored.into_iter().take(5).map(|(_, _, c)| c).collect();
    Some((bad.to_string(), valid_options))
}

/// Formats a `NodeSpec` / `LayoutSpec` / `OverrideSpec` `serde_json::Error` with `UNKNOWN_FIELD`
/// and the 5 closest valid field names so Studio's live-spec toast and CLI/MCP share the same diagnostic.
pub fn format_spec_serde_error(err: &serde_json::Error) -> String {
    let msg = err.to_string();
    if let Some((bad, valid)) = extract_unknown_field_options(&msg) {
        if valid.is_empty() {
            format!("UNKNOWN_FIELD: unknown field `{bad}`")
        } else {
            format!("UNKNOWN_FIELD: unknown field `{bad}` (valid_options: {})", valid.join(", "))
        }
    } else {
        msg
    }
}

/// Declarative `in_progress` flag on a [`NodeSpec`] (`true`/`false` or a detailed [`InProgress`] object).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum InProgressSpec {
    Bool(bool),
    Detailed(InProgress),
}

impl InProgressSpec {
    pub fn into_state(self) -> Option<InProgress> {
        match self {
            InProgressSpec::Bool(true) => Some(InProgress {
                run_id: std::env::var("DESIGNOS_AGENT_RUN_ID").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                agent: std::env::var("DESIGNOS_AGENT_RUNTIME").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                since_ms: now_ms(),
                stale_after_s: Some(crate::default_in_progress_stale_s()),
                status: None,
            }),
            InProgressSpec::Bool(false) => None,
            InProgressSpec::Detailed(mut ip) => {
                if ip.since_ms == 0 {
                    ip.since_ms = now_ms();
                }
                if ip.stale_after_s.is_none() {
                    ip.stale_after_s = Some(crate::default_in_progress_stale_s());
                }
                Some(ip)
            }
        }
    }
}

/// A canvas comment thread enriched with its resolved world position and the
/// exact target node and top-level screen/frame under the pin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResolvedCommentThread {
    pub id: String,
    pub resolved: bool,
    pub page_id: u64,
    pub page_name: String,
    pub world_x: f32,
    pub world_y: f32,
    pub target_node_id: Option<u64>,
    pub target_node_name: Option<String>,
    pub target_node_kind: Option<String>,
    pub target_frame_id: Option<u64>,
    pub target_frame_name: Option<String>,
    pub messages: Vec<Comment>,
}

/// Resolves an EaseUI BrainWave 2.0 design token (`$easeui.*`) or `#RRGGBB[AA]` hex string.
pub fn resolve_easeui_color(spec: &str) -> Option<Color> {
    let s = spec.trim();
    if let Some((_, c)) = crate::lookup_easeui_token(s) {
        return Some(c);
    }
    match s {
        "transparent" | "none" => Some(Color::TRANSPARENT),
        _ => Color::from_hex(s),
    }
}

pub(crate) fn parse_sizing(s: &str) -> Sizing {
    match s.trim().to_ascii_lowercase().as_str() {
        "hug" | "auto" => Sizing::Hug,
        "fill" | "stretch" | "100%" => Sizing::Fill,
        _ => Sizing::Fixed,
    }
}

pub(crate) fn parse_align(s: &str) -> LayoutAlign {
    match s.trim().to_ascii_lowercase().as_str() {
        "center" | "middle" => LayoutAlign::Center,
        "end" | "right" | "bottom" => LayoutAlign::End,
        "space-between" | "between" | "space_between" => LayoutAlign::SpaceBetween,
        "space-around" | "around" | "space_around" => LayoutAlign::SpaceAround,
        "space-evenly" | "evenly" | "space_evenly" => LayoutAlign::SpaceEvenly,
        "baseline" | "first-baseline" => LayoutAlign::Baseline,
        _ => LayoutAlign::Start,
    }
}

pub(crate) fn parse_direction(s: &str) -> LayoutDirection {
    match s.trim().to_ascii_lowercase().as_str() {
        "column" | "col" | "vertical" | "v" => LayoutDirection::Vertical,
        "grid" | "g" => LayoutDirection::Grid,
        _ => LayoutDirection::Horizontal,
    }
}

/// Applies an EaseUI BrainWave 2.0 preset to `node` before explicit `NodeSpec` overrides run.
pub fn apply_easeui_preset(node: &mut Node, preset: &str) {
    match preset.trim().to_ascii_lowercase().as_str() {
        "easeui-screen" => {
            node.size = Vec2::new(1440.0, 900.0);
            node.fills = vec![Paint::solid(Color::rgb(0x12, 0x12, 0x15))];
            if let Some(f) = node.kind.frame_props_mut() {
                f.clips_content = true;
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Vertical,
                    item_spacing: 24.0,
                    padding: [24.0, 24.0, 24.0, 24.0],
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-panel" => {
            node.fills = vec![Paint::solid(Color::rgb(0x18, 0x18, 0x1B))];
            node.stroke.paints = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
            node.stroke.weight = 1.0;
            node.corner_radii = [12.0; 4];
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Vertical,
                    item_spacing: 12.0,
                    padding: [16.0, 16.0, 16.0, 16.0],
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Hug,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-card" => {
            node.fills = vec![Paint::solid(Color::rgb(0x18, 0x18, 0x1B))];
            node.stroke.paints = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
            node.stroke.weight = 1.0;
            node.corner_radii = [12.0; 4];
            node.effects = vec![Effect::DropShadow {
                color: Color::rgba(0, 0, 0, 90),
                offset: Vec2::new(0.0, 6.0),
                radius: 16.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
                blend: Default::default(),
            }];
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Vertical,
                    item_spacing: 12.0,
                    padding: [20.0, 20.0, 20.0, 20.0],
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Hug,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-button-primary" => {
            node.fills = vec![Paint::solid(Color::rgb(0xE3, 0x63, 0x23))];
            node.corner_radii = [8.0; 4];
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: 8.0,
                    padding: [10.0, 16.0, 10.0, 16.0],
                    primary_align: LayoutAlign::Center,
                    counter_align: LayoutAlign::Center,
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Hug,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-button-secondary" => {
            node.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
            node.stroke.paints = vec![Paint::solid(Color::rgb(0x3F, 0x3F, 0x46))];
            node.stroke.weight = 1.0;
            node.corner_radii = [8.0; 4];
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: 8.0,
                    padding: [10.0, 16.0, 10.0, 16.0],
                    primary_align: LayoutAlign::Center,
                    counter_align: LayoutAlign::Center,
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Hug,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-input" => {
            node.size = Vec2::new(280.0, 40.0);
            node.fills = vec![Paint::solid(Color::rgb(0x12, 0x12, 0x15))];
            node.stroke.paints = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
            node.stroke.weight = 1.0;
            node.corner_radii = [8.0; 4];
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: 8.0,
                    padding: [10.0, 12.0, 10.0, 12.0],
                    counter_align: LayoutAlign::Center,
                    primary_sizing: Sizing::Fixed,
                    counter_sizing: Sizing::Fixed,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-badge" => {
            node.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
            node.stroke.paints = vec![Paint::solid(Color::rgb(0x3F, 0x3F, 0x46))];
            node.stroke.weight = 1.0;
            node.corner_radii = [999.0; 4];
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: 6.0,
                    padding: [4.0, 10.0, 4.0, 10.0],
                    primary_align: LayoutAlign::Center,
                    counter_align: LayoutAlign::Center,
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Hug,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-row" => {
            node.fills.clear();
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Horizontal,
                    item_spacing: 12.0,
                    counter_align: LayoutAlign::Center,
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Hug,
                    ..AutoLayout::v1()
                });
            }
        }
        "easeui-column" => {
            node.fills.clear();
            if let Some(f) = node.kind.frame_props_mut() {
                f.auto_layout = Some(AutoLayout {
                    direction: LayoutDirection::Vertical,
                    item_spacing: 12.0,
                    primary_sizing: Sizing::Hug,
                    counter_sizing: Sizing::Hug,
                    ..AutoLayout::v1()
                });
            }
        }
        _ => {}
    }
}

impl Document {
    /// Materializes a declarative [`NodeSpec`] subtree under `parent` and runs
    /// Auto-Layout 5.0 reflow across the created hierarchy.
    pub fn apply_spec(&mut self, parent: NodeId, spec: &NodeSpec) -> Result<NodeId, String> {
        self.apply_spec_with_report(parent, spec).map(|r| r.root_node_id).map_err(|e| e.to_string())
    }

    /// Materializes `spec` under `parent`, resolving `var:`, `style:`, `mode`, `$easeui.*`,
    /// and `"instance"` references against local document tokens/styles/components and returning a [`SpecApplyReport`].
    pub fn apply_spec_with_report(&mut self, parent: NodeId, spec: &NodeSpec) -> Result<SpecApplyReport, SpecBindError> {
        self.apply_spec_with_libraries(parent, spec, &[])
    }

    /// Materializes `spec` under `parent`, resolving `var:`, `style:`, `mode`, `$easeui.*`,
    /// and `"instance"` references against both local tokens/styles/components and enabled `libraries`.
    /// On any error, `self` is rolled back to its exact pre-call state.
    pub fn apply_spec_with_libraries(&mut self, parent: NodeId, spec: &NodeSpec, libraries: &[LibrarySnapshot]) -> Result<SpecApplyReport, SpecBindError> {
        let pnode = self.get(parent).ok_or_else(|| SpecBindError {
            code: "NODE_NOT_FOUND".to_string(),
            message: format!("Parent node {} not found", parent.0),
            path: "/parent".to_string(),
            received: Some(parent.0.to_string()),
            expected: Some("an existing container node id".to_string()),
            valid_options: Vec::new(),
            corrected_example: None,
            safe_to_retry: true,
        })?;
        if !pnode.kind.is_container() {
            return Err(SpecBindError {
                code: "NOT_A_CONTAINER".to_string(),
                message: format!("Parent node {} is not a container", parent.0),
                path: "/parent".to_string(),
                received: Some(parent.0.to_string()),
                expected: Some("a container node id (page, frame, section, component, group)".to_string()),
                valid_options: Vec::new(),
                corrected_example: None,
                safe_to_retry: true,
            });
        }

        let saved_next_id = self.next_id;
        let prev_gap_cursor = self.gap_cursor;
        let prev_dirty = self.dirty.clone();
        let prev_pending_relayout = self.pending_relayout.clone();
        let prev_in_progress = self.in_progress.clone();
        let parent_has_al = pnode.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some();
        let mut saved_parent_geom = Vec::new();
        if parent_has_al {
            self.walk(parent, |n, _| saved_parent_geom.push((n.id, n.transform, n.size)));
        }
        let data_before = crate::spec_data::SpecDataSnapshot::of(self);
        let mut inserted_ids: Vec<NodeId> = Vec::new();
        let mut ctx = crate::spec_instance::SpecBuildCtx::new(self, libraries);
        let mut index = SpecBindIndex::build(self);
        let mut report = SpecApplyReport { parent_id: parent, ..Default::default() };

        match self.build_spec_node_bound(parent, spec, "spec", &mut index, libraries, &mut ctx, &mut report, &mut inserted_ids) {
            Ok(root_id) => {
                report.root_node_id = root_id;
                crate::spec_instance::populate_name_to_id(self, root_id, "", &mut report.name_to_id);
                report.instances = ctx.instances;
                report.imported_assets = ctx.imported_assets;
                report.stale_library_assets = ctx.stale_library_assets;
                report.issues = ctx.issues;
                report.in_progress =
                    self.in_progress.iter().filter(|(id, ip)| prev_in_progress.get(*id) != Some(*ip)).map(|(id, ip)| (*id, ip.clone())).collect();
                report.bindings_created += ctx.bindings_created;
                report.styles_linked += ctx.styles_linked;
                report.warnings.extend(ctx.warnings);
                for fid in ctx.measured_with_fallback {
                    if !report.measured_with_fallback.contains(&fid) {
                        report.measured_with_fallback.push(fid);
                    }
                }
                if !report.measured_with_fallback.is_empty() && !report.warnings.iter().any(|w| w.code == "TEXT_MEASURED_WITH_FALLBACK_FONT") {
                    report.warnings.push(SpecWarning {
                        code: "TEXT_MEASURED_WITH_FALLBACK_FONT".to_string(),
                        path: "spec".to_string(),
                        token: None,
                        replacement: None,
                        message: Some("One or more text nodes were measured with a fallback font because the requested font_family is not loaded.".to_string()),
                    });
                }
                let relayout_root =
                    if self.get(parent).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()).is_some() { parent } else { root_id };
                self.relayout_subtree(relayout_root);
                self.flush_pending_relayout();
                Ok(report)
            }
            Err(err) => {
                for id in inserted_ids.into_iter().rev() {
                    let _ = self.remove(id);
                }
                for (nid, tf, sz) in saved_parent_geom {
                    if let Some(n) = self.get_mut(nid) {
                        n.transform = tf;
                        n.size = sz;
                    }
                }
                for inv in ctx.import_invs.into_iter().rev() {
                    let _ = inv.apply(self);
                }
                if data_before.library_page.is_none() {
                    if let Some(lp_id) = self.library_page.take() {
                        self.nodes.remove(&lp_id);
                    }
                }
                self.next_id = saved_next_id;
                self.gap_cursor = prev_gap_cursor;
                self.dirty = prev_dirty;
                self.pending_relayout = prev_pending_relayout;
                self.in_progress = prev_in_progress;
                data_before.restore(self);
                Err(err)
            }
        }
    }

    pub(crate) fn build_spec_node_with_ctx(
        &mut self,
        parent: NodeId,
        spec: &NodeSpec,
        path: &str,
        ctx: &mut crate::spec_instance::SpecBuildCtx<'_>,
    ) -> Result<NodeId, crate::spec_instance::SpecError> {
        let mut index = SpecBindIndex::build(self);
        let mut sub_report = SpecApplyReport::default();
        let mut inserted_ids = Vec::new();
        let snapshots = ctx.snapshots;
        match self.build_spec_node_bound(parent, spec, path, &mut index, snapshots, ctx, &mut sub_report, &mut inserted_ids) {
            Ok(id) => {
                ctx.bindings_created += sub_report.bindings_created;
                ctx.styles_linked += sub_report.styles_linked;
                ctx.warnings.extend(sub_report.warnings);
                for fid in sub_report.measured_with_fallback {
                    if !ctx.measured_with_fallback.contains(&fid) {
                        ctx.measured_with_fallback.push(fid);
                    }
                }
                Ok(id)
            }
            Err(e) => {
                for id in inserted_ids.into_iter().rev() {
                    let _ = self.remove(id);
                }
                Err(crate::spec_instance::SpecError::from(e))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_spec_node_bound(
        &mut self,
        parent: NodeId,
        spec: &NodeSpec,
        path: &str,
        index: &mut SpecBindIndex,
        libraries: &[LibrarySnapshot],
        ctx: &mut crate::spec_instance::SpecBuildCtx<'_>,
        report: &mut SpecApplyReport,
        inserted_ids: &mut Vec<NodeId>,
    ) -> Result<NodeId, SpecBindError> {
        let kind_str = if spec.kind.is_empty() {
            if spec.component.is_some() {
                "instance"
            } else if spec.preset.is_some() || spec.layout.is_some() || !spec.children.is_empty() {
                "frame"
            } else if spec.text.is_some() || spec.text_style.is_some() || spec.font_size.is_some() || spec.line_height.is_some() {
                "text"
            } else {
                "frame"
            }
        } else {
            spec.kind.as_str()
        };
        if kind_str.eq_ignore_ascii_case("instance") {
            let inst_id = self.build_instance_spec_node(parent, spec, ctx).map_err(|mut e| {
                if !path.is_empty() && path != "spec" && !e.path.starts_with("spec.") && !e.path.starts_with(path) {
                    e.path = if e.path.is_empty() { path.to_string() } else { format!("{path}.{}", e.path) };
                }
                e
            })?;
            inserted_ids.push(inst_id);
            return Ok(inst_id);
        }
        if !spec.slots.is_empty() {
            return Err(crate::spec_instance::SpecError::named(
                "OVERRIDE_FIELD_NOT_APPLICABLE",
                "slots can only be set on instance nodes",
                Some(format!("{path}.slots")),
                Vec::new(),
                None,
            )
            .into());
        }

        let id = self.try_alloc_id().ok_or_else(|| SpecBindError {
            code: "ID_EXHAUSTED".to_string(),
            message: "Document has exhausted node IDs".to_string(),
            path: path.to_string(),
            received: None,
            expected: None,
            valid_options: Vec::new(),
            corrected_example: None,
            safe_to_retry: true,
        })?;
        let is_text_kind = kind_str.eq_ignore_ascii_case("text");
        // `text: "var:..."` binds `characters` to a String variable: the node starts empty and takes the value after insert.
        let text_var: Option<&str> = spec.text.as_deref().map(str::trim).filter(|t| t.starts_with("var:"));
        let w = spec.width.as_ref().and_then(NumberOrVar::as_num).unwrap_or(if is_text_kind { 120.0 } else { 200.0 });
        let h = spec.height.as_ref().and_then(NumberOrVar::as_num).unwrap_or(if is_text_kind { 24.0 } else { 100.0 });
        let rect = Rect::new(spec.x, spec.y, w.max(0.0), h.max(0.0));

        let mut node = match kind_str.to_ascii_lowercase().as_str() {
            "frame" => Node::frame(id, rect),
            "component" => {
                let mut n = Node::frame(id, rect);
                n.kind = NodeKind::Component(FrameProps::default());
                n.fills = vec![Paint::solid(Color::rgb(0x18, 0x18, 0x1B))];
                n
            }
            "section" => {
                let mut n = Node::frame(id, rect);
                n.kind = NodeKind::Section;
                n.fills = vec![Paint::solid(Color::rgb(0x18, 0x18, 0x1B))];
                n
            }
            "rectangle" | "rect" => Node::rectangle(id, rect),
            "ellipse" | "circle" => Node::ellipse(id, rect),
            "text" => {
                let mut style = TextStyle::default();
                style.font_size = spec.font_size.as_ref().and_then(NumberOrVar::as_num).unwrap_or(14.0);
                style.font_weight = spec.font_weight.unwrap_or(400);
                style.fills = vec![Paint::solid(crate::default_text_colour(self, parent))];
                let content = if text_var.is_some() { String::new() } else { spec.text.clone().unwrap_or_default() };
                Node::text(id, rect, content, style)
            }
            other => {
                return Err(SpecBindError {
                    code: "UNSUPPORTED_NODE_KIND".to_string(),
                    message: format!("Unsupported spec node kind: {other}"),
                    path: format!("{path}.kind"),
                    received: Some(other.to_string()),
                    expected: Some("frame, rectangle, ellipse, text, section, component, instance".to_string()),
                    valid_options: vec![
                        "frame".into(),
                        "rectangle".into(),
                        "ellipse".into(),
                        "text".into(),
                        "section".into(),
                        "component".into(),
                        "instance".into(),
                    ],
                    corrected_example: None,
                    safe_to_retry: true,
                })
            }
        };

        if let Some(preset) = &spec.preset {
            apply_easeui_preset(&mut node, preset);
        }
        if !spec.name.is_empty() {
            node.name = spec.name.clone();
        } else if let Some(preset) = &spec.preset {
            node.name = preset.clone();
        } else if let (Some(t), None) = (&spec.text, text_var) {
            node.name = t.chars().take(32).collect();
        }

        // LayoutSpec on frame (applied BEFORE width/height sizing & before variable bindings so auto_layout exists)
        if let Some(ls) = &spec.layout {
            if let Some(f) = node.kind.frame_props_mut() {
                let mut al = f.auto_layout.take().unwrap_or_else(AutoLayout::v1);
                if !ls.direction.is_empty() {
                    al.direction = parse_direction(&ls.direction);
                }
                al.wrap = ls.wrap;
                if let Some(g) = ls.gap.as_num() {
                    al.item_spacing = g;
                }
                if let Some(cg) = ls.counter_gap.as_num() {
                    al.counter_spacing = cg;
                }
                let pad_nums = [ls.padding[0].unwrap_or(0.0), ls.padding[1].unwrap_or(0.0), ls.padding[2].unwrap_or(0.0), ls.padding[3].unwrap_or(0.0)];
                let any_pad_set = ls.padding.iter().any(|p| !p.is_zero());
                if any_pad_set {
                    al.padding = pad_nums;
                }
                if !ls.primary_align.is_empty() {
                    al.primary_align = parse_align(&ls.primary_align);
                }
                if !ls.counter_align.is_empty() {
                    al.counter_align = parse_align(&ls.counter_align);
                }
                if let Some(cols) = ls.grid_columns {
                    al.grid_columns = cols;
                }
                if let Some(rows) = ls.grid_rows {
                    al.grid_rows = rows;
                }
                f.auto_layout = Some(al);
            }
        }

        // An explicit width or height (a literal, or a variable bound after insert) pins that axis of an auto-layout frame.
        if let Some(width) = &spec.width {
            if let Some(w_explicit) = width.as_num() {
                node.size.x = w_explicit.max(0.0);
            }
            if spec.width_sizing.is_none() {
                if let Some(al) = node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                    if al.primary_is_horizontal() {
                        al.primary_sizing = Sizing::Fixed;
                    } else {
                        al.counter_sizing = Sizing::Fixed;
                    }
                }
            }
        }
        if let Some(height) = &spec.height {
            if let Some(h_explicit) = height.as_num() {
                node.size.y = h_explicit.max(0.0);
            }
            if spec.height_sizing.is_none() {
                if let Some(al) = node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                    if !al.primary_is_horizontal() {
                        al.primary_sizing = Sizing::Fixed;
                    } else {
                        al.counter_sizing = Sizing::Fixed;
                    }
                }
            }
        }
        if let Some(BoolOrVar::Bool(visible)) = &spec.visible {
            node.visible = *visible;
        }
        if let Some(op) = spec.opacity.as_num() {
            node.opacity = op.clamp(0.0, 1.0);
        }
        if let Some(clips) = spec.clips_content {
            if let Some(f) = node.kind.frame_props_mut() {
                f.clips_content = clips;
            }
        }

        // Sizing (both as a child of an auto-layout parent and on its own AutoLayout axes)
        if let Some(ws) = &spec.width_sizing {
            let s = parse_sizing(ws);
            node.layout_child.horizontal = s;
            if let Some(al) = node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                if al.primary_is_horizontal() {
                    al.primary_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                } else {
                    al.counter_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                }
            }
        }
        if let Some(hs) = &spec.height_sizing {
            let s = parse_sizing(hs);
            node.layout_child.vertical = s;
            if let Some(al) = node.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
                if !al.primary_is_horizontal() {
                    al.primary_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                } else {
                    al.counter_sizing = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
                }
            }
        }
        node.layout_child.min_w = spec.min_width;
        node.layout_child.max_w = spec.max_width;
        node.layout_child.min_h = spec.min_height;
        node.layout_child.max_h = spec.max_height;
        if let Some(cs) = spec.col_span {
            node.layout_child.col_span = cs;
        }
        if let Some(rs) = spec.row_span {
            node.layout_child.row_span = rs;
        }

        // Dev annotations & status
        if let Some(ann) = &spec.annotation {
            if !ann.trim().is_empty() {
                node.dev.annotations.push(Annotation { text: ann.clone(), properties: Vec::new(), category: None });
            }
        }
        for ann in &spec.annotations {
            node.dev.annotations.push(Annotation { text: ann.clone(), properties: Vec::new(), category: None });
        }
        if let Some(status) = &spec.dev_status {
            match status.to_ascii_lowercase().as_str() {
                "ready" | "ready_for_dev" | "readyfordev" => {
                    node.dev.status = Some(DevStatus::ReadyForDev);
                }
                "completed" | "done" => {
                    node.dev.status = Some(DevStatus::Completed);
                }
                _ => {}
            }
        }

        // Resolve explicit variable modes on this node BEFORE inserting and resolving bindings
        if !spec.mode.is_empty() {
            node.variables.modes = crate::resolve_mode_map(self, &spec.mode, path)?;
        }

        let node_kind = node.kind.clone();
        self.insert(parent, node).map_err(|e| SpecBindError {
            code: "INSERT_FAILED".to_string(),
            message: e.to_string(),
            path: path.to_string(),
            received: None,
            expected: None,
            valid_options: Vec::new(),
            corrected_example: None,
            safe_to_retry: true,
        })?;
        inserted_ids.push(id);
        if let Some(ip_spec) = &spec.in_progress {
            let state = ip_spec.clone().into_state();
            if state.is_some() {
                if let Err(root_opt) = self.is_in_progress_root(id) {
                    let message = match root_opt {
                        Some(r) => format!("node {} is not a root frame (use root frame {})", id.0, r.0),
                        None => format!("node {} is not a root frame", id.0),
                    };
                    return Err(SpecBindError {
                        code: "IN_PROGRESS_TARGET_NOT_ROOT".to_string(),
                        message,
                        path: path.to_string(),
                        received: None,
                        expected: None,
                        valid_options: root_opt.map(|r| r.0.to_string()).into_iter().collect(),
                        corrected_example: None,
                        safe_to_retry: true,
                    });
                }
            }
            self.set_in_progress(id, state);
        }

        // 1. Bind preset EaseUI colours (before explicit fill/stroke overrides)
        if let Some(preset) = &spec.preset {
            let (fill_tok, stroke_tok) = crate::preset_easeui_tokens(preset);
            if fill_tok.is_some() || stroke_tok.is_some() {
                let coll_name = crate::ensure_easeui_deprecated_collection(self);
                index.ensure_easeui_indexed(self);
                if let Some((canonical, token_str)) = fill_tok {
                    let var_id = format!("{EASEUI_DEPRECATED_COLLECTION_ID}/{canonical}");
                    report.bindings_created += crate::apply_variable_binding(self, id, &["fills/0"], &var_id, &format!("{path}.preset"), &mut report.warnings);
                    report.warnings.push(SpecWarning {
                        code: "DEPRECATED_EASEUI_TOKEN".to_string(),
                        path: format!("{path}.preset"),
                        token: Some(token_str.to_string()),
                        replacement: Some(format!("var:{coll_name}/{canonical}")),
                        message: None,
                    });
                }
                if let Some((canonical, token_str)) = stroke_tok {
                    let var_id = format!("{EASEUI_DEPRECATED_COLLECTION_ID}/{canonical}");
                    report.bindings_created +=
                        crate::apply_variable_binding(self, id, &["strokes/0"], &var_id, &format!("{path}.preset"), &mut report.warnings);
                    report.warnings.push(SpecWarning {
                        code: "DEPRECATED_EASEUI_TOKEN".to_string(),
                        path: format!("{path}.preset"),
                        token: Some(token_str.to_string()),
                        replacement: Some(format!("var:{coll_name}/{canonical}")),
                        message: None,
                    });
                }
            }
        }

        // 2. Effect style link
        if let Some(eff_ref) = &spec.effect_style {
            let style_key = crate::resolve_style_ref(self, index, libraries, eff_ref, StyleProperty::Effect, "effect_style", &format!("{path}.effect_style"))?;
            if let Some(style) = self.styles.get(&style_key).cloned() {
                if let Some(n) = self.get_mut(id) {
                    n.style_refs.insert("effect".to_string(), style_key.clone());
                    crate::apply_style_values(n, &style, &style_key);
                    report.styles_linked += 1;
                }
            }
        }

        // 2b. Layout grid style link
        if let Some(grid_ref) = &spec.grid_style {
            let style_key = crate::resolve_style_ref(self, index, libraries, grid_ref, StyleProperty::Grid, "grid_style", &format!("{path}.grid_style"))?;
            if let Some(style) = self.styles.get(&style_key).cloned() {
                if let Some(n) = self.get_mut(id) {
                    n.style_refs.insert("grid".to_string(), style_key.clone());
                    crate::apply_style_values(n, &style, &style_key);
                    report.styles_linked += 1;
                }
            }
        }

        // 3. Text style link (on text nodes)
        if is_text_kind {
            if let Some(ts_ref) = &spec.text_style {
                let style_key = crate::resolve_style_ref(self, index, libraries, ts_ref, StyleProperty::Text, "text_style", &format!("{path}.text_style"))?;
                if let Some(style) = self.styles.get(&style_key).cloned() {
                    if let Some(n) = self.get_mut(id) {
                        n.style_refs.insert("text".to_string(), style_key.clone());
                        crate::apply_style_values(n, &style, &style_key);
                        report.styles_linked += 1;
                    }
                }
            }
        }

        // 4. Fill / text_color
        if is_text_kind {
            let col_input = spec.text_color.as_deref().map(|s| ("text_color", s)).or_else(|| spec.fill.as_deref().map(|s| ("fill", s)));
            if let Some((field_name, raw_col)) = col_input {
                let fpath = format!("{path}.{field_name}");
                let resolved = crate::resolve_colour_ref(self, index, libraries, id, &node_kind, "fills/0", field_name, raw_col, &fpath, &mut report.warnings)?;
                match resolved {
                    ResolvedColourRef::NoneOrTransparent => {
                        if let Some(n) = self.get_mut(id) {
                            crate::detach_fill_paints(n);
                            if let NodeKind::Text(tp) = &mut n.kind {
                                tp.style.fills.clear();
                            }
                        }
                    }
                    ResolvedColourRef::Literal(c) => {
                        if let Some(n) = self.get_mut(id) {
                            crate::detach_fill_paints(n);
                            if let NodeKind::Text(tp) = &mut n.kind {
                                tp.style.fills = vec![Paint::solid(c)];
                            }
                        }
                    }
                    ResolvedColourRef::Variable { var_id, .. } => {
                        if let Some(n) = self.get_mut(id) {
                            n.style_refs.remove("fill");
                        }
                        report.bindings_created += crate::apply_variable_binding(self, id, &["fills/0"], &var_id, &fpath, &mut report.warnings);
                    }
                    ResolvedColourRef::Style { style_key, .. } => {
                        if let Some(style) = self.styles.get(&style_key).cloned() {
                            if let Some(n) = self.get_mut(id) {
                                n.variables.bindings.remove("fills/0");
                                n.style_refs.insert("fill".to_string(), style_key.clone());
                                crate::apply_style_values(n, &style, &style_key);
                                report.styles_linked += 1;
                            }
                        }
                    }
                }
            }
        } else if let Some(fill_str) = &spec.fill {
            let fpath = format!("{path}.fill");
            let resolved = crate::resolve_colour_ref(self, index, libraries, id, &node_kind, "fills/0", "fill", fill_str, &fpath, &mut report.warnings)?;
            match resolved {
                ResolvedColourRef::NoneOrTransparent => {
                    if let Some(n) = self.get_mut(id) {
                        crate::detach_fill_paints(n);
                        n.fills.clear();
                    }
                }
                ResolvedColourRef::Literal(c) => {
                    if let Some(n) = self.get_mut(id) {
                        crate::detach_fill_paints(n);
                        n.fills = vec![Paint::solid(c)];
                    }
                }
                ResolvedColourRef::Variable { var_id, .. } => {
                    if let Some(n) = self.get_mut(id) {
                        n.style_refs.remove("fill");
                    }
                    report.bindings_created += crate::apply_variable_binding(self, id, &["fills/0"], &var_id, &fpath, &mut report.warnings);
                }
                ResolvedColourRef::Style { style_key, .. } => {
                    if let Some(style) = self.styles.get(&style_key).cloned() {
                        if let Some(n) = self.get_mut(id) {
                            n.variables.bindings.remove("fills/0");
                            n.style_refs.insert("fill".to_string(), style_key.clone());
                            crate::apply_style_values(n, &style, &style_key);
                            report.styles_linked += 1;
                        }
                    }
                }
            }
        }

        // 5. Stroke colour
        if let Some(stroke_str) = &spec.stroke {
            let fpath = format!("{path}.stroke");
            let resolved = crate::resolve_colour_ref(self, index, libraries, id, &node_kind, "strokes/0", "stroke", stroke_str, &fpath, &mut report.warnings)?;
            match resolved {
                ResolvedColourRef::NoneOrTransparent => {
                    if let Some(n) = self.get_mut(id) {
                        n.variables.bindings.remove("strokes/0");
                        n.style_refs.remove("stroke");
                        n.stroke.paints.clear();
                    }
                }
                ResolvedColourRef::Literal(c) => {
                    if let Some(n) = self.get_mut(id) {
                        n.variables.bindings.remove("strokes/0");
                        n.style_refs.remove("stroke");
                        n.stroke.paints = vec![Paint::solid(c)];
                    }
                }
                ResolvedColourRef::Variable { var_id, .. } => {
                    if let Some(n) = self.get_mut(id) {
                        n.style_refs.remove("stroke");
                    }
                    report.bindings_created += crate::apply_variable_binding(self, id, &["strokes/0"], &var_id, &fpath, &mut report.warnings);
                }
                ResolvedColourRef::Style { style_key, .. } => {
                    if let Some(style) = self.styles.get(&style_key).cloned() {
                        if let Some(n) = self.get_mut(id) {
                            n.variables.bindings.remove("strokes/0");
                            n.style_refs.insert("stroke".to_string(), style_key.clone());
                            crate::apply_style_values(n, &style, &style_key);
                            report.styles_linked += 1;
                        }
                    }
                }
            }
        }

        // 6. Bindable numeric properties
        if let Some(NumberOrVar::Ref(raw_ref)) = &spec.width {
            self.bind_variable_ref(id, &node_kind, raw_ref, &["width"], "width", &format!("{path}.width"), index, libraries, report)?;
        }
        if let Some(NumberOrVar::Ref(raw_ref)) = &spec.height {
            self.bind_variable_ref(id, &node_kind, raw_ref, &["height"], "height", &format!("{path}.height"), index, libraries, report)?;
        }
        if let Some(BoolOrVar::Ref(raw_ref)) = &spec.visible {
            self.bind_variable_ref(id, &node_kind, raw_ref, &["visible"], "visible", &format!("{path}.visible"), index, libraries, report)?;
        }
        if let Some(sw) = &spec.stroke_weight {
            if let Some(v) =
                self.bind_or_eval_number(id, &node_kind, sw, &["strokeWeight"], "stroke_weight", format!("{path}.stroke_weight"), index, libraries, report)?
            {
                if let Some(n) = self.get_mut(id) {
                    n.stroke.weight = v.max(0.0);
                }
            }
        }
        if let Some(cr) = &spec.corner_radius {
            if let Some(v) = self.bind_or_eval_number(
                id,
                &node_kind,
                cr,
                &["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"],
                "corner_radius",
                format!("{path}.corner_radius"),
                index,
                libraries,
                report,
            )? {
                if let Some(n) = self.get_mut(id) {
                    n.corner_radii = [v.max(0.0); 4];
                }
            }
        }
        if let Some(radii) = &spec.corner_radii {
            let corner_keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
            for (i, r_item) in radii.iter().enumerate() {
                if let Some(v) = self.bind_or_eval_number(
                    id,
                    &node_kind,
                    r_item,
                    &[corner_keys[i]],
                    "corner_radii",
                    format!("{path}.corner_radii[{i}]"),
                    index,
                    libraries,
                    report,
                )? {
                    if let Some(n) = self.get_mut(id) {
                        n.corner_radii[i] = v.max(0.0);
                    }
                }
            }
        }
        if let NumberOrVar::Ref(_) = &spec.opacity {
            let _ = self.bind_or_eval_number(id, &node_kind, &spec.opacity, &["opacity"], "opacity", format!("{path}.opacity"), index, libraries, report)?;
        }
        if let Some(ls) = &spec.layout {
            if let NumberOrVar::Ref(_) = &ls.gap {
                let _ = self.bind_or_eval_number(id, &node_kind, &ls.gap, &["itemSpacing"], "gap", format!("{path}.layout.gap"), index, libraries, report)?;
            }
            if let NumberOrVar::Ref(_) = &ls.counter_gap {
                let _ = self.bind_or_eval_number(
                    id,
                    &node_kind,
                    &ls.counter_gap,
                    &["counterAxisSpacing"],
                    "counter_gap",
                    format!("{path}.layout.counter_gap"),
                    index,
                    libraries,
                    report,
                )?;
            }
            let pad_keys = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
            for (i, p_item) in ls.padding.iter().enumerate() {
                if let NumberOrVar::Ref(_) = p_item {
                    let _ = self.bind_or_eval_number(
                        id,
                        &node_kind,
                        p_item,
                        &[pad_keys[i]],
                        "padding",
                        format!("{path}.layout.padding[{i}]"),
                        index,
                        libraries,
                        report,
                    )?;
                }
            }
        }

        // 7. Font size, line height and weight on Text nodes (or when spec.text is None)
        if is_text_kind || spec.text.is_none() {
            self.apply_text_overrides(id, &node_kind, spec, path, index, libraries, report)?;
        }

        // `text: "var:..."` on a text node binds `characters` before the text is measured.
        if let (true, Some(raw_ref)) = (is_text_kind, text_var) {
            self.bind_characters(id, &node_kind, raw_ref, &spec.name, spec.preset.is_some(), &format!("{path}.text"), index, libraries, report)?;
        }

        // Measure (or estimate when no measurer is installed) intrinsic text bounds after text_style / font_size / line_height / font_family application,
        // and align the glyph box with any centring on the parent auto-layout frame when the text hugs that axis.
        let (parent_centre_h, parent_centre_v) = self.parent_centres_children(parent);
        let hugs_h = spec.width.is_none() && spec.width_sizing.as_deref().is_none_or(|s| parse_sizing(s) == Sizing::Hug);
        let hugs_v = spec.height.is_none() && spec.height_sizing.as_deref().is_none_or(|s| parse_sizing(s) == Sizing::Hug);
        if let Some(n) = self.get_mut(id) {
            if let NodeKind::Text(tp) = &mut n.kind {
                if parent_centre_h && spec.text_style.is_none() && tp.style.align_h == TextAlignH::Left && hugs_h {
                    tp.style.align_h = TextAlignH::Center;
                }
                if parent_centre_v && spec.text_style.is_none() && tp.style.align_v == TextAlignV::Top && hugs_v {
                    tp.style.align_v = TextAlignV::Center;
                }
                let needs_measure = spec.width.is_none() || spec.height.is_none();
                if spec.width.is_some() || spec.width_sizing.as_deref() == Some("fill") {
                    tp.auto_resize = TextAutoResize::Height;
                    if spec.height.is_none() {
                        n.size.y = crate::text_measure::intrinsic_text_size(tp, Some(n.size.x.max(20.0))).y;
                    }
                } else {
                    let sz = crate::text_measure::intrinsic_text_size(tp, None);
                    if spec.width.is_none() {
                        n.size.x = sz.x;
                    }
                    if spec.height.is_none() {
                        n.size.y = sz.y;
                    }
                }
                if needs_measure && crate::text_measure::uses_fallback_font(tp) && !report.measured_with_fallback.contains(&id.0) {
                    report.measured_with_fallback.push(id.0);
                }
            }
        }

        // Convenience: if a frame/preset (like easeui-button-primary or easeui-badge) specifies `text`,
        // automatically insert a child Text node inside it!
        if !is_text_kind {
            if let Some(label) = &spec.text {
                let (self_centre_h, self_centre_v) = self.parent_centres_children(id);
                let tid = self.try_alloc_id().ok_or_else(|| SpecBindError {
                    code: "ID_EXHAUSTED".to_string(),
                    message: "Document has exhausted node IDs".to_string(),
                    path: format!("{path}.text"),
                    received: None,
                    expected: None,
                    valid_options: Vec::new(),
                    corrected_example: None,
                    safe_to_retry: true,
                })?;
                let mut style = TextStyle::default();
                style.font_size = spec.font_size.as_ref().and_then(NumberOrVar::as_num).unwrap_or(13.0);
                style.font_weight = spec.font_weight.unwrap_or(500);
                style.fills = vec![Paint::solid(crate::default_text_colour(self, id))];
                let initial = if text_var.is_some() { String::new() } else { label.clone() };
                let mut tnode = Node::text(tid, Rect::new(0.0, 0.0, 60.0, 18.0), initial, style);
                if text_var.is_none() {
                    tnode.name = label.clone();
                }
                let tkind = tnode.kind.clone();
                self.insert(id, tnode).map_err(|e| SpecBindError {
                    code: "INSERT_FAILED".to_string(),
                    message: e.to_string(),
                    path: format!("{path}.text"),
                    received: None,
                    expected: None,
                    valid_options: Vec::new(),
                    corrected_example: None,
                    safe_to_retry: true,
                })?;
                inserted_ids.push(tid);

                if let Some(ts_ref) = &spec.text_style {
                    let style_key = crate::resolve_style_ref(self, index, libraries, ts_ref, StyleProperty::Text, "text_style", &format!("{path}.text_style"))?;
                    if let Some(s) = self.styles.get(&style_key).cloned() {
                        if let Some(tn) = self.get_mut(tid) {
                            tn.style_refs.insert("text".to_string(), style_key.clone());
                            crate::apply_style_values(tn, &s, &style_key);
                            report.styles_linked += 1;
                        }
                    }
                }
                if let Some(raw_tc) = &spec.text_color {
                    let fpath = format!("{path}.text_color");
                    let resolved =
                        crate::resolve_colour_ref(self, index, libraries, tid, &tkind, "fills/0", "text_color", raw_tc, &fpath, &mut report.warnings)?;
                    match resolved {
                        ResolvedColourRef::NoneOrTransparent => {
                            if let Some(tn) = self.get_mut(tid) {
                                if let NodeKind::Text(tp) = &mut tn.kind {
                                    tp.style.fills.clear();
                                }
                            }
                        }
                        ResolvedColourRef::Literal(c) => {
                            if let Some(tn) = self.get_mut(tid) {
                                if let NodeKind::Text(tp) = &mut tn.kind {
                                    tp.style.fills = vec![Paint::solid(c)];
                                }
                            }
                        }
                        ResolvedColourRef::Variable { var_id, .. } => {
                            report.bindings_created += crate::apply_variable_binding(self, tid, &["fills/0"], &var_id, &fpath, &mut report.warnings);
                        }
                        ResolvedColourRef::Style { style_key, .. } => {
                            if let Some(s) = self.styles.get(&style_key).cloned() {
                                if let Some(tn) = self.get_mut(tid) {
                                    tn.style_refs.insert("fill".to_string(), style_key.clone());
                                    crate::apply_style_values(tn, &s, &style_key);
                                    report.styles_linked += 1;
                                }
                            }
                        }
                    }
                }
                // The generated label takes the same explicit overrides a text node does, after its style.
                self.apply_text_overrides(tid, &tkind, spec, path, index, libraries, report)?;
                if let Some(raw_ref) = text_var {
                    self.bind_characters(tid, &tkind, raw_ref, "", false, &format!("{path}.text"), index, libraries, report)?;
                }
                if let Some(tn) = self.get_mut(tid) {
                    if let NodeKind::Text(tp) = &mut tn.kind {
                        if self_centre_h && spec.text_style.is_none() && tp.style.align_h == TextAlignH::Left {
                            tp.style.align_h = TextAlignH::Center;
                        }
                        if self_centre_v && spec.text_style.is_none() && tp.style.align_v == TextAlignV::Top {
                            tp.style.align_v = TextAlignV::Center;
                        }
                        let sz = crate::text_measure::intrinsic_text_size(tp, None);
                        let fallback = crate::text_measure::uses_fallback_font(tp);
                        tn.size = sz;
                        if fallback && !report.measured_with_fallback.contains(&tid.0) {
                            report.measured_with_fallback.push(tid.0);
                        }
                    }
                }
            }
        }

        for (i, child_spec) in spec.children.iter().enumerate() {
            let child_path = format!("{path}.children[{i}]");
            self.build_spec_node_bound(id, child_spec, &child_path, index, libraries, ctx, report, inserted_ids)?;
        }

        Ok(id)
    }

    /// Applies the explicit `font_size`, `line_height` and `font_weight` of `spec` to the text node
    /// `id` after its text style, so they override the style. Binding `font_size` to a variable
    /// together with `text_style` detaches the style link (`STYLE_DETACHED_BY_SPEC`).
    #[allow(clippy::too_many_arguments)]
    fn apply_text_overrides(
        &mut self,
        id: NodeId,
        node_kind: &NodeKind,
        spec: &NodeSpec,
        path: &str,
        index: &mut SpecBindIndex,
        libraries: &[LibrarySnapshot],
        report: &mut SpecApplyReport,
    ) -> Result<(), SpecBindError> {
        if let Some(fs) = &spec.font_size {
            if let Some(v) = self.bind_or_eval_number(id, node_kind, fs, &["fontSize"], "font_size", format!("{path}.font_size"), index, libraries, report)? {
                if let Some(n) = self.get_mut(id) {
                    if let NodeKind::Text(tp) = &mut n.kind {
                        tp.style.font_size = v.max(1.0);
                    }
                }
            } else if spec.text_style.is_some() {
                if let Some(n) = self.get_mut(id) {
                    if n.style_refs.remove("text").is_some() {
                        report.warnings.push(SpecWarning {
                            code: "STYLE_DETACHED_BY_SPEC".to_string(),
                            path: format!("{path}.font_size"),
                            token: spec.text_style.clone(),
                            replacement: None,
                            message: Some("A variable-bound font_size overrides and detaches text_style.".to_string()),
                        });
                    }
                }
            }
        }
        if let Some(lh) = &spec.line_height {
            if let Some(v) =
                self.bind_or_eval_number(id, node_kind, lh, &["lineHeight"], "line_height", format!("{path}.line_height"), index, libraries, report)?
            {
                if let Some(n) = self.get_mut(id) {
                    if let NodeKind::Text(tp) = &mut n.kind {
                        tp.style.line_height = LineHeight::Pixels(v.max(0.0));
                    }
                }
            }
        }
        if let Some(fw) = spec.font_weight {
            if let Some(n) = self.get_mut(id) {
                if let NodeKind::Text(tp) = &mut n.kind {
                    tp.style.font_weight = fw;
                }
            }
        }
        if let Some(ff) = &spec.font_family {
            if !ff.trim().is_empty() {
                if let Some(n) = self.get_mut(id) {
                    if let NodeKind::Text(tp) = &mut n.kind {
                        tp.style.font_family = ff.trim().to_string();
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_or_eval_number(
        &mut self,
        id: NodeId,
        node_kind: &NodeKind,
        nv: &NumberOrVar,
        keys: &[&str],
        fname: &str,
        fpath: String,
        index: &mut SpecBindIndex,
        libraries: &[LibrarySnapshot],
        report: &mut SpecApplyReport,
    ) -> Result<Option<f32>, SpecBindError> {
        match nv {
            NumberOrVar::Num(v) => Ok(Some(*v)),
            NumberOrVar::Ref(raw_ref) => {
                self.bind_variable_ref(id, node_kind, raw_ref, keys, fname, &fpath, index, libraries, report)?;
                Ok(None)
            }
        }
    }

    /// Binds `characters` of the text node `id` to a String variable and, when the spec names neither the node nor a
    /// preset, names the node after the resolved content.
    #[allow(clippy::too_many_arguments)]
    fn bind_characters(
        &mut self,
        id: NodeId,
        node_kind: &NodeKind,
        raw_ref: &str,
        name: &str,
        has_preset: bool,
        fpath: &str,
        index: &mut SpecBindIndex,
        libraries: &[LibrarySnapshot],
        report: &mut SpecApplyReport,
    ) -> Result<(), SpecBindError> {
        self.bind_variable_ref(id, node_kind, raw_ref, &["characters"], "text", fpath, index, libraries, report)?;
        if name.is_empty() && !has_preset {
            if let Some(n) = self.get_mut(id) {
                if let NodeKind::Text(tp) = &n.kind {
                    if !tp.content.is_empty() {
                        n.name = tp.content.chars().take(32).collect();
                    }
                }
            }
        }
        Ok(())
    }

    /// Resolves `raw_ref` (`"var:..."`), checks its kind, applicability and scope for `keys` on the node, binds it
    /// and writes its resolved value into the node. Returns the bound variable id.
    #[allow(clippy::too_many_arguments)]
    fn bind_variable_ref(
        &mut self,
        id: NodeId,
        node_kind: &NodeKind,
        raw_ref: &str,
        keys: &[&str],
        fname: &str,
        fpath: &str,
        index: &mut SpecBindIndex,
        libraries: &[LibrarySnapshot],
        report: &mut SpecApplyReport,
    ) -> Result<String, SpecBindError> {
        let var_id = crate::resolve_variable_ref_of_kind(self, index, libraries, raw_ref, fname, fpath, keys.first().and_then(|k| crate::property_kind(k)))?;
        crate::validate_variable_binding_for_keys(self, index, &var_id, keys, node_kind, raw_ref, fname, fpath)?;
        report.bindings_created += crate::apply_variable_binding(self, id, keys, &var_id, fpath, &mut report.warnings);
        Ok(var_id)
    }

    fn export_var_ref(&self, index: &SpecBindIndex, var_id: &str) -> Option<String> {
        index.format_variable_ref(self, var_id)
    }

    fn export_style_ref(&self, style_key: &str, with_prefix: bool) -> Option<String> {
        let name = self
            .style_meta
            .get(style_key)
            .map(|m| m.name.as_str())
            .filter(|n| !n.is_empty())
            .or_else(|| self.styles.contains_key(style_key).then_some(style_key))?;
        Some(if with_prefix { format!("style:{name}") } else { name.to_string() })
    }

    /// Exports `root` and its descendants into a declarative [`NodeSpec`] tree,
    /// including variable bindings (`var:`), style links (`style:` / `text_style` / `effect_style`),
    /// explicit variable modes (`mode`), Dev Annotations, and Canvas Comment Threads pinned within `root`.
    pub fn export_spec(&self, root: NodeId) -> Result<NodeSpec, String> {
        let all_comments = self.resolve_comment_targets();
        let index = SpecBindIndex::build(self);
        self.export_spec_inner(root, &all_comments, &index)
    }

    pub(crate) fn export_spec_inner(&self, id: NodeId, all_comments: &[ResolvedCommentThread], index: &SpecBindIndex) -> Result<NodeSpec, String> {
        let n = self.get(id).ok_or_else(|| format!("Node {} not found", id.0))?;
        if matches!(n.kind, NodeKind::Instance { .. }) {
            return self.export_instance_spec_node(id, all_comments);
        }
        let is_text = matches!(n.kind, NodeKind::Text(_));

        let fill = if is_text {
            None
        } else if let Some(skey) = n.style_refs.get("fill") {
            self.export_style_ref(skey, true)
        } else if let Some(vid) = n.variables.bindings.get("fills/0") {
            self.export_var_ref(index, vid)
        } else {
            n.fills.first().and_then(|p| match &p.kind {
                PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
                _ => None,
            })
        };

        let stroke = if let Some(skey) = n.style_refs.get("stroke") {
            self.export_style_ref(skey, true)
        } else if let Some(vid) = n.variables.bindings.get("strokes/0") {
            self.export_var_ref(index, vid)
        } else {
            n.stroke.paints.first().and_then(|p| match &p.kind {
                PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
                _ => None,
            })
        };

        let stroke_weight = if let Some(vid) = n.variables.bindings.get("strokeWeight") {
            self.export_var_ref(index, vid).map(NumberOrVar::Ref)
        } else {
            (!n.stroke.paints.is_empty()).then_some(NumberOrVar::Num(n.stroke.weight))
        };

        let corner_keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
        let corner_vars: [Option<String>; 4] =
            std::array::from_fn(|i| n.variables.bindings.get(corner_keys[i]).and_then(|vid| self.export_var_ref(index, vid)));
        let (corner_radius, corner_radii) = if corner_vars.iter().all(|v| v.is_some() && *v == corner_vars[0]) {
            (corner_vars[0].clone().map(NumberOrVar::Ref), None)
        } else if corner_vars.iter().any(|v| v.is_some()) {
            let arr: [NumberOrVar; 4] =
                std::array::from_fn(|i| corner_vars[i].clone().map(NumberOrVar::Ref).unwrap_or_else(|| NumberOrVar::Num(n.corner_radii[i])));
            (None, Some(arr))
        } else {
            let uniform = (n.corner_radii != [0.0; 4] && n.corner_radii.iter().all(|r| *r == n.corner_radii[0])).then_some(NumberOrVar::Num(n.corner_radii[0]));
            let per_corner = (n.corner_radii != [0.0; 4] && uniform.is_none()).then(|| n.corner_radii.map(NumberOrVar::Num));
            (uniform, per_corner)
        };

        let opacity = n
            .variables
            .bindings
            .get("opacity")
            .and_then(|vid| self.export_var_ref(index, vid))
            .map(NumberOrVar::Ref)
            .unwrap_or_else(|| NumberOrVar::Num(n.opacity));

        let text_style = n.style_refs.get("text").and_then(|k| self.export_style_ref(k, false));
        let effect_style = n.style_refs.get("effect").and_then(|k| self.export_style_ref(k, false));
        let grid_style = n.style_refs.get("grid").and_then(|k| self.export_style_ref(k, false));
        let bound_or = |key: &str, literal: f32| -> NumberOrVar {
            n.variables.bindings.get(key).and_then(|vid| self.export_var_ref(index, vid)).map(NumberOrVar::Ref).unwrap_or(NumberOrVar::Num(literal))
        };
        let width = bound_or("width", n.size.x);
        let height = bound_or("height", n.size.y);
        let visible = match n.variables.bindings.get("visible").and_then(|vid| self.export_var_ref(index, vid)) {
            Some(r) => Some(BoolOrVar::Ref(r)),
            None => (!n.visible).then_some(BoolOrVar::Bool(false)),
        };

        let (text, font_size, line_height, font_weight, text_color) = match &n.kind {
            NodeKind::Text(tp) => {
                let linked_ts = text_style.as_ref().and_then(|_| n.style_refs.get("text")).and_then(|k| match self.styles.get(k) {
                    Some(Style::Text(ts)) => Some(ts),
                    _ => None,
                });
                let fs = if let Some(vid) = n.variables.bindings.get("fontSize") {
                    self.export_var_ref(index, vid).map(NumberOrVar::Ref)
                } else if text_style.is_none() || linked_ts.is_some_and(|ts| ts.font_size != tp.style.font_size) {
                    Some(NumberOrVar::Num(tp.style.font_size))
                } else {
                    None
                };
                let lh = if let Some(vid) = n.variables.bindings.get("lineHeight") {
                    self.export_var_ref(index, vid).map(NumberOrVar::Ref)
                } else if text_style.is_none() || linked_ts.is_some_and(|ts| ts.line_height != tp.style.line_height) {
                    match tp.style.line_height {
                        LineHeight::Pixels(px) => Some(NumberOrVar::Num(px)),
                        _ => None,
                    }
                } else {
                    None
                };
                let fw = (text_style.is_none() || linked_ts.is_some_and(|ts| ts.font_weight != tp.style.font_weight)).then_some(tp.style.font_weight);
                let tc = if let Some(skey) = n.style_refs.get("fill") {
                    self.export_style_ref(skey, true)
                } else if let Some(vid) = n.variables.bindings.get("fills/0") {
                    self.export_var_ref(index, vid)
                } else {
                    tp.style.fills.first().and_then(|p| match &p.kind {
                        PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
                        _ => None,
                    })
                };
                let content = n.variables.bindings.get("characters").and_then(|vid| self.export_var_ref(index, vid)).unwrap_or_else(|| tp.content.clone());
                (Some(content), fs, lh, fw, tc)
            }
            _ => (None, None, None, None, None),
        };

        let layout = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| {
            let gap = n
                .variables
                .bindings
                .get("itemSpacing")
                .and_then(|vid| self.export_var_ref(index, vid))
                .map(NumberOrVar::Ref)
                .unwrap_or_else(|| NumberOrVar::Num(al.item_spacing));
            let counter_gap = n
                .variables
                .bindings
                .get("counterAxisSpacing")
                .and_then(|vid| self.export_var_ref(index, vid))
                .map(NumberOrVar::Ref)
                .unwrap_or_else(|| NumberOrVar::Num(al.counter_spacing));
            let pad_keys = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
            let padding: [NumberOrVar; 4] = std::array::from_fn(|i| {
                n.variables
                    .bindings
                    .get(pad_keys[i])
                    .and_then(|vid| self.export_var_ref(index, vid))
                    .map(NumberOrVar::Ref)
                    .unwrap_or_else(|| NumberOrVar::Num(al.padding[i]))
            });
            LayoutSpec {
                direction: match al.direction {
                    LayoutDirection::Horizontal => "row".into(),
                    LayoutDirection::Vertical => "column".into(),
                    LayoutDirection::Grid => "grid".into(),
                },
                wrap: al.wrap,
                gap,
                counter_gap,
                padding,
                primary_align: format!("{:?}", al.primary_align).to_ascii_lowercase(),
                counter_align: format!("{:?}", al.counter_align).to_ascii_lowercase(),
                grid_columns: (al.grid_columns > 0).then_some(al.grid_columns),
                grid_rows: (al.grid_rows > 0).then_some(al.grid_rows),
            }
        });

        let mut mode = std::collections::BTreeMap::new();
        for (cid, mid) in &n.variables.modes {
            if let Some(coll) = self.variable_collections.get(cid) {
                if let Some(m) = coll.modes.iter().find(|m| m.id == *mid) {
                    mode.insert(coll.name.clone(), m.name.clone());
                }
            }
        }

        let dev_status = n.dev.status.map(|s| s.label().to_string());
        let annotations: Vec<String> = n.dev.annotations.iter().map(|a| a.text.clone()).collect();
        let in_progress = self.get_in_progress(id).cloned().map(InProgressSpec::Detailed);
        let comments: Vec<ResolvedCommentThread> = all_comments
            .iter()
            .filter(|c| c.target_node_id == Some(id.0) || (c.target_node_id.is_none() && c.target_frame_id == Some(id.0)))
            .cloned()
            .collect();
        let mut children = Vec::with_capacity(n.children.len());
        for c in &n.children {
            if let Ok(cs) = self.export_spec_inner(*c, all_comments, index) {
                children.push(cs);
            }
        }
        Ok(NodeSpec {
            id: Some(id.0),
            kind: n.kind.default_name().to_ascii_lowercase(),
            name: n.name.clone(),
            component: None,
            variant: BTreeMap::new(),
            props: BTreeMap::new(),
            overrides: Vec::new(),
            slots: BTreeMap::new(),
            preset: None,
            x: n.transform.tx,
            y: n.transform.ty,
            width: Some(width),
            height: Some(height),
            width_sizing: (n.layout_child.horizontal != Sizing::Fixed).then(|| format!("{:?}", n.layout_child.horizontal).to_ascii_lowercase()),
            height_sizing: (n.layout_child.vertical != Sizing::Fixed).then(|| format!("{:?}", n.layout_child.vertical).to_ascii_lowercase()),
            min_width: n.layout_child.min_w,
            max_width: n.layout_child.max_w,
            min_height: n.layout_child.min_h,
            max_height: n.layout_child.max_h,
            col_span: (n.layout_child.col_span > 1).then_some(n.layout_child.col_span),
            row_span: (n.layout_child.row_span > 1).then_some(n.layout_child.row_span),
            fill,
            stroke,
            stroke_weight,
            corner_radius,
            corner_radii,
            opacity,
            visible,
            clips_content: n.kind.frame_props().and_then(|f| f.clips_content.then_some(true)),
            text,
            font_family: None,
            font_size,
            line_height,
            font_weight,
            text_color,
            text_style,
            effect_style,
            grid_style,
            mode,
            layout,
            dev_status,
            annotation: None,
            annotations,
            in_progress,
            comments,
            children,
        })
    }

    /// Resolves every canvas [`CommentThread`] in the document to its world coordinates,
    /// the deepest visible target node under or anchored to the pin, and the containing top-level screen/frame.
    pub fn resolve_comment_targets(&self) -> Vec<ResolvedCommentThread> {
        let mut out = Vec::with_capacity(self.comments.len());
        for t in &self.comments {
            let page_id = t.anchor.node.and_then(|nid| self.page_of(nid)).unwrap_or(t.anchor.page);
            let page_name = self.get(page_id).map(|p| p.name.clone()).unwrap_or_default();
            let world_pt = match t.anchor.node {
                Some(nid) if self.get(nid).is_some() => self.world_transform(nid).apply(t.anchor.offset),
                _ => t.anchor.offset,
            };
            let target_node_id = t.anchor.node.filter(|nid| self.get(*nid).is_some()).or_else(|| self.deepest_node_at_point(page_id, world_pt));
            let (target_node_name, target_node_kind) = match target_node_id.and_then(|nid| self.get(nid)) {
                Some(n) => (Some(n.name.clone()), Some(n.kind.default_name().to_string())),
                None => (None, None),
            };
            let target_frame_id = target_node_id.and_then(|nid| self.top_level_frame_of(nid));
            let target_frame_name = target_frame_id.and_then(|fid| self.get(fid)).map(|f| f.name.clone());
            out.push(ResolvedCommentThread {
                id: t.id.clone(),
                resolved: t.resolved,
                page_id: page_id.0,
                page_name,
                world_x: world_pt.x,
                world_y: world_pt.y,
                target_node_id: target_node_id.map(|n| n.0),
                target_node_name,
                target_node_kind,
                target_frame_id: target_frame_id.map(|f| f.0),
                target_frame_name,
                messages: t.messages.clone(),
            });
        }
        out
    }

    fn top_level_frame_of(&self, mut id: NodeId) -> Option<NodeId> {
        while let Some(n) = self.get(id) {
            let Some(p) = n.parent else { return None };
            if self.pages.contains(&p) {
                return Some(id);
            }
            id = p;
        }
        None
    }

    fn deepest_node_at_point(&self, page: NodeId, pt: Vec2) -> Option<NodeId> {
        let mut hit = None;
        self.walk(page, |n, depth| {
            if depth == 0 || !n.visible {
                return;
            }
            if let Some(inv) = self.world_transform(n.id).invert() {
                let local = inv.apply(pt);
                if local.x >= 0.0 && local.y >= 0.0 && local.x <= n.size.x && local.y <= n.size.y {
                    hit = Some(n.id);
                }
            }
        });
        hit
    }

    /// Returns `(centre_h, centre_v)` indicating whether `parent` is an auto-layout
    /// container that centres its children horizontally and/or vertically.
    pub fn parent_centres_children(&self, parent: NodeId) -> (bool, bool) {
        let Some(al) = self.get(parent).and_then(|p| p.kind.frame_props()).and_then(|f| f.auto_layout.as_ref()) else {
            return (false, false);
        };
        if al.primary_is_horizontal() {
            (al.primary_align == LayoutAlign::Center, al.counter_align == LayoutAlign::Center)
        } else {
            (al.counter_align == LayoutAlign::Center, al.primary_align == LayoutAlign::Center)
        }
    }
}

pub fn estimate_intrinsic_text_size(tp: &TextProps) -> Vec2 {
    let lines: Vec<&str> = tp.content.split('\n').collect();
    let fs = tp.style.font_size.max(8.0);
    let weight_boost = if tp.style.font_weight >= 600 { 1.05 } else { 1.0 };
    let max_w = lines
        .iter()
        .map(|l| {
            let ems: f32 = l
                .chars()
                .map(|c| match c {
                    ' ' => 0.32,
                    'i' | 'l' | 'I' | '1' | '.' | ',' | ':' | ';' | '|' | '!' => 0.34,
                    'W' | 'M' | '—' | '@' | '%' => 0.82,
                    c if c.is_ascii_uppercase() => 0.68,
                    _ => 0.56,
                })
                .sum();
            ems * fs * weight_boost + 4.0
        })
        .fold(16.0_f32, f32::max)
        .ceil();
    let line_count = lines.len().max(1) as f32;
    let h = (line_count * fs * 1.38).ceil().max(fs);
    Vec2::new(max_w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_export_easeui_spec_with_autolayout_and_comments() {
        let mut doc = Document::new("Spec Test");
        let page = doc.add_page("Screens");
        let spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "name": "IAM · Login Screen",
            "preset": "easeui-screen",
            "dev_status": "ready",
            "annotations": ["Use BrainWave 2.0 dark tokens"],
            "children": [
                {
                    "name": "Login Card",
                    "preset": "easeui-card",
                    "width": 400.0,
                    "height_sizing": "hug",
                    "children": [
                        {
                            "kind": "text",
                            "text": "Sign in to DESIGN:OS",
                            "font_size": 20.0,
                            "font_weight": 600,
                            "text_color": "$easeui.text.primary"
                        },
                        {
                            "name": "Email Input",
                            "preset": "easeui-input",
                            "width_sizing": "fill",
                            "text": "jang@designos.dev",
                            "text_color": "$easeui.text.secondary"
                        },
                        {
                            "name": "Submit Button",
                            "preset": "easeui-button-primary",
                            "width_sizing": "fill",
                            "text": "Continue"
                        }
                    ]
                }
            ]
        }))
        .unwrap();

        let screen_id = doc.apply_spec(page, &spec).unwrap();
        let screen = doc.get(screen_id).unwrap();
        assert_eq!(screen.name, "IAM · Login Screen");
        assert_eq!(screen.size, Vec2::new(1440.0, 900.0));
        assert_eq!(screen.dev.status, Some(DevStatus::ReadyForDev));

        let card_id = screen.children[0];
        let card = doc.get(card_id).unwrap();
        assert_eq!(card.size.x, 400.0);
        assert!(card.size.y > 100.0, "Auto-Layout Hug computed card height");

        let btn_id = card.children[2];
        let btn = doc.get(btn_id).unwrap();
        assert_eq!(btn.size.x, 360.0, "Fill button stretched to 400 - 2*20px card padding");

        // Pin a comment onto the Submit Button
        doc.comments.push(CommentThread {
            id: "t1".into(),
            anchor: CommentAnchor { page, node: Some(btn_id), offset: Vec2::new(10.0, 10.0) },
            messages: vec![Comment {
                id: "m1".into(),
                author: "Jang".into(),
                author_id: None,
                author_color: None,
                text: "Make button height 44px".into(),
                created: "2026-10-14T23:30:00Z".into(),
                edited: None,
            }],
            resolved: false,
        });

        let resolved = doc.resolve_comment_targets();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].target_node_id, Some(btn_id.0));
        assert_eq!(resolved[0].target_node_name.as_deref(), Some("Submit Button"));
        assert_eq!(resolved[0].target_frame_id, Some(screen_id.0));
        assert_eq!(resolved[0].target_frame_name.as_deref(), Some("IAM · Login Screen"));

        let exported = doc.export_spec(screen_id).unwrap();
        assert_eq!(exported.name, "IAM · Login Screen");
        assert_eq!(exported.children[0].children[2].comments.len(), 1);
    }

    #[test]
    fn grid_frame_width_and_height_sizing_map_to_primary_and_counter() {
        let mut doc = Document::new("Grid Spec");
        let page = doc.add_page("P");
        let hug_w_spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "name": "HugWidthGrid",
            "kind": "frame",
            "width_sizing": "hug",
            "height": 300.0,
            "layout": { "direction": "grid", "grid_columns": 3, "gap": 10.0 },
            "children": [
                { "kind": "rectangle", "width": 50.0, "height": 20.0 },
                { "kind": "rectangle", "width": 50.0, "height": 20.0 },
                { "kind": "rectangle", "width": 50.0, "height": 20.0 }
            ]
        }))
        .unwrap();
        let hug_w_id = doc.apply_spec(page, &hug_w_spec).unwrap();
        let al_w = doc.get(hug_w_id).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(
            (al_w.primary_sizing, al_w.counter_sizing),
            (Sizing::Hug, Sizing::Fixed),
            "width_sizing=hug and explicit height on a grid frame must set primary_sizing=Hug, counter_sizing=Fixed"
        );
        assert_eq!(doc.get(hug_w_id).unwrap().size, Vec2::new(170.0, 300.0));

        let hug_h_spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "name": "HugHeightGrid",
            "kind": "frame",
            "width": 400.0,
            "height_sizing": "hug",
            "layout": { "direction": "grid", "grid_columns": 2, "gap": 10.0 },
            "children": [
                { "kind": "rectangle", "width": 50.0, "height": 30.0 },
                { "kind": "rectangle", "width": 50.0, "height": 30.0 }
            ]
        }))
        .unwrap();
        let hug_h_id = doc.apply_spec(page, &hug_h_spec).unwrap();
        let al_h = doc.get(hug_h_id).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(
            (al_h.primary_sizing, al_h.counter_sizing),
            (Sizing::Fixed, Sizing::Hug),
            "explicit width and height_sizing=hug on a grid frame must set primary_sizing=Fixed, counter_sizing=Hug"
        );
        assert_eq!(doc.get(hug_h_id).unwrap().size, Vec2::new(400.0, 30.0));
    }

    #[test]
    fn grid_height_hug_keeps_an_explicit_width_pinned() {
        let mut doc = Document::new("Grid Spec");
        let page = doc.add_page("P");
        // A preset leaves the grid hugging both ways. The explicit width pins the primary axis and
        // height_sizing=hug keeps the counter axis hugging, so the width stays 400.
        let preset_spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "name": "PresetGrid",
            "kind": "frame",
            "preset": "easeui-panel",
            "width": 400.0,
            "height_sizing": "hug",
            "layout": { "direction": "grid", "grid_columns": 2, "gap": 10.0 },
            "children": [
                { "kind": "rectangle", "width": 50.0, "height": 30.0 },
                { "kind": "rectangle", "width": 50.0, "height": 30.0 }
            ]
        }))
        .unwrap();
        let preset_id = doc.apply_spec(page, &preset_spec).unwrap();
        let al_p = doc.get(preset_id).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(
            (al_p.primary_sizing, al_p.counter_sizing),
            (Sizing::Fixed, Sizing::Hug),
            "an explicit width on a hugging grid frame must pin primary_sizing, and height_sizing=hug must leave counter_sizing hugging"
        );
        assert_eq!(doc.get(preset_id).unwrap().size, Vec2::new(400.0, 62.0), "400 wide, one row of 30 plus the preset's 16px padding above and below");
    }

    #[test]
    fn grid_width_and_height_hug_together_hug_both_axes() {
        let mut doc = Document::new("Grid Spec");
        let page = doc.add_page("P");
        let hug_both_spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "name": "HugBothGrid",
            "kind": "frame",
            "width_sizing": "hug",
            "height_sizing": "hug",
            "layout": { "direction": "grid", "grid_columns": 2, "gap": 10.0 },
            "children": [
                { "kind": "rectangle", "width": 50.0, "height": 30.0 },
                { "kind": "rectangle", "width": 50.0, "height": 30.0 }
            ]
        }))
        .unwrap();
        let hug_both_id = doc.apply_spec(page, &hug_both_spec).unwrap();
        let al_b = doc.get(hug_both_id).unwrap().kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(
            (al_b.primary_sizing, al_b.counter_sizing),
            (Sizing::Hug, Sizing::Hug),
            "width_sizing=hug and height_sizing=hug on a grid frame must hug the columns and the rows"
        );
        assert_eq!(doc.get(hug_both_id).unwrap().size, Vec2::new(110.0, 30.0));
    }

    #[test]
    fn layout_dir_is_an_alias_of_direction_and_a_typo_is_still_refused() {
        let mut doc = Document::new("Alias");
        let page = doc.add_page("P");
        let spec: NodeSpec = serde_json::from_value(serde_json::json!({"kind": "frame", "layout": {"dir": "column"}})).unwrap();
        let id = doc.apply_spec(page, &spec).unwrap();
        let al = doc.get(id).unwrap().kind.frame_props().and_then(|f| f.auto_layout.clone()).expect("auto layout");
        assert_eq!(al.direction, LayoutDirection::Vertical, "`dir` must set the layout direction");

        let err = serde_json::from_value::<NodeSpec>(serde_json::json!({"kind": "frame", "layout": {"dirr": "row"}})).unwrap_err().to_string();
        let (bad, options) = extract_unknown_field_options(&err).expect("unknown field error");
        assert_eq!(bad, "dirr");
        assert!(options.contains(&"direction".to_string()), "{options:?}");
    }

    fn in_progress_spec_json(run_id: &str, status: &str) -> serde_json::Value {
        serde_json::json!({ "run_id": run_id, "agent": "Claude Code", "since_ms": 9_000, "stale_after_s": 300, "status": status })
    }

    #[test]
    fn spec_in_progress_flag_applies_and_exports_on_a_root_frame() {
        set_thread_mock_now_ms(Some(10_000));
        let mut doc = Document::new("Spec in_progress");
        let page = doc.add_page("P");
        let spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "kind": "frame",
            "name": "Hero",
            "in_progress": in_progress_spec_json("run-1", "Building hero..."),
            "children": [{ "kind": "rectangle", "width": 20.0, "height": 20.0 }]
        }))
        .unwrap();

        let report = doc.apply_spec_with_report(page, &spec).unwrap();
        let flag = doc.get_in_progress(report.root_node_id).expect("the root frame is flagged");
        assert_eq!((flag.run_id.as_deref(), flag.agent.as_deref(), flag.status.as_deref()), (Some("run-1"), Some("Claude Code"), Some("Building hero...")));
        assert_eq!(report.in_progress.len(), 1, "the report lists the flag this call set");

        let exported = doc.export_spec(report.root_node_id).unwrap();
        match exported.in_progress {
            Some(InProgressSpec::Detailed(ip)) => assert_eq!((ip.run_id.as_deref(), ip.status.as_deref()), (Some("run-1"), Some("Building hero..."))),
            other => panic!("export must carry the flag, got {other:?}"),
        }

        let cleared: NodeSpec = serde_json::from_value(serde_json::json!({ "kind": "frame", "name": "Done", "in_progress": false })).unwrap();
        let cleared_id = doc.apply_spec(page, &cleared).unwrap();
        assert!(doc.get_in_progress(cleared_id).is_none(), "`in_progress: false` sets no flag");
        set_thread_mock_now_ms(None);
    }

    #[test]
    fn spec_in_progress_flag_on_a_non_root_node_is_refused_with_the_root_and_rolls_back() {
        let mut doc = Document::new("Spec in_progress not root");
        let page = doc.add_page("P");
        let children_before = doc.get(page).unwrap().children.len();
        let spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "kind": "frame",
            "name": "Hero",
            "in_progress": in_progress_spec_json("run-1", "Building..."),
            "children": [{ "kind": "frame", "name": "Inner", "in_progress": in_progress_spec_json("run-1", "Inner...") }]
        }))
        .unwrap();

        let err = doc.apply_spec_with_report(page, &spec).unwrap_err();
        assert_eq!(err.code, "IN_PROGRESS_TARGET_NOT_ROOT");
        assert!(!err.message.contains("IN_PROGRESS_TARGET_NOT_ROOT"), "the code lives in `code` only: {}", err.message);
        assert!(err.message.contains("is not a root frame (use root frame "), "{}", err.message);
        assert_eq!(err.path, "spec.children[0]");
        let root_id = err.message.rsplit(' ').next().unwrap().trim_end_matches(')').to_string();
        assert_eq!(err.valid_options, vec![root_id], "valid_options names the root frame to flag instead");

        assert!(doc.in_progress.is_empty(), "the root's flag set before the failure is rolled back");
        assert_eq!(doc.get(page).unwrap().children.len(), children_before, "no node survives the refused spec");
    }

    #[test]
    fn no_measurer_keeps_the_estimate() {
        let mut doc = Document::new("Estimate fallback");
        let page = doc.add_page("P");
        let spec: NodeSpec = serde_json::from_value(serde_json::json!({
            "kind": "text",
            "name": "DeleteLabel",
            "text": "Delete",
            "font_size": 14.0,
            "font_weight": 600
        }))
        .unwrap();
        let tid = doc.apply_spec(page, &spec).unwrap();
        let tn = doc.get(tid).unwrap();
        assert_eq!(tn.size.x, 52.0, "without an installed measurer, 'Delete' at 14px Semibold keeps the 52.0px estimate");
    }
}
