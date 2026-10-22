//! Spec-Driven Declarative UI Builder, EaseUI BrainWave 2.0 Token/Preset Engine,
//! Auto-Layout 5.0 Subtree Solver, and Canvas Comment Pin Target Resolver.

use crate::*;
use serde::{Deserialize, Serialize};

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}
fn is_one_f32(v: &f32) -> bool {
    (*v - 1.0).abs() < 1e-6
}
fn default_one_f32() -> f32 {
    1.0
}

///Declarative Auto-Layout 5.0 specification for a container frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutSpec {
    /// `"row"` / `"horizontal"` or `"column"` / `"vertical"`.
    #[serde(default, alias = "dir")]
    pub direction: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrap: bool,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub gap: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub counter_gap: f32,
    /// `[top, right, bottom, left]` padding in px.
    #[serde(default)]
    pub padding: [f32; 4],
    /// `"start"`, `"center"`, `"end"`, `"space-between"`, `"space-around"`, `"space-evenly"`, or `"baseline"`.
    #[serde(default, alias = "justify", skip_serializing_if = "String::is_empty")]
    pub primary_align: String,
    /// `"start"`, `"center"`, `"end"`, or `"baseline"`.
    #[serde(default, alias = "align", skip_serializing_if = "String::is_empty")]
    pub counter_align: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_columns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_rows: Option<u32>,
}

/// Declarative node specification used by Agents and CLI/MCP tools (`apply_spec` / `export_spec`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// `"frame"`, `"rectangle"`, `"ellipse"`, `"text"`, `"section"`, `"component"`, `"instance"`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    /// Component reference for `"instance"` nodes (`"node:<id>"`, component key, or component/library name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Variant property selection `{property: value}` for `"instance"` nodes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variant: BTreeMap<String, String>,
    /// Component property overrides `{prop_name: value}` on an `"instance"` root.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, serde_json::Value>,
    /// Ordered descendant overrides applied inside an `"instance"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<OverrideSpec>,
    /// Slot fills or disable flags `{slot_name: [NodeSpec...] | false}` on an `"instance"` root (E15-2).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, SlotFillSpec>,
    /// Optional EaseUI BrainWave 2.0 preset:
    /// `"easeui-screen"`, `"easeui-panel"`, `"easeui-card"`, `"easeui-button-primary"`,
    /// `"easeui-button-secondary"`, `"easeui-input"`, `"easeui-badge"`, `"easeui-row"`, `"easeui-column"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub x: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub y: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    /// `"fixed"`, `"hug"`, or `"fill"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_sizing: Option<String>,
    /// `"fixed"`, `"hug"`, or `"fill"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_sizing: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col_span: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_span: Option<u16>,
    /// Hex color (`"#18181B"`) or EaseUI token (`"$easeui.bg.panel"`, `"$easeui.accent.orange"`, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// Hex color or EaseUI token for stroke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(default, alias = "stroke_width", skip_serializing_if = "Option::is_none")]
    pub stroke_weight: Option<f32>,
    #[serde(default, alias = "radius", skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radii: Option<[f32; 4]>,
    #[serde(default = "default_one_f32", skip_serializing_if = "is_one_f32")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clips_content: Option<bool>,
    /// Text content (for `"text"` nodes, or automatically creates a child label if set on a button/badge frame!).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(default, alias = "color", skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutSpec>,
    /// `"ready"` (`ReadyForDev`) or `"completed"` (`Completed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<String>,
    /// Attached canvas comments when exporting a subtree spec.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<ResolvedCommentThread>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeSpec>,
}

/// Literal `false` used by [`SlotFillSpec::Disabled`] (`slots: {"Actions": false}`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FalseLiteral;

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

/// Slot fill or disable specification in `NodeSpec::slots` / `OverrideSpec::slots` (E15-2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SlotFillSpec {
    Disabled(FalseLiteral),
    Fill(Vec<NodeSpec>),
}

/// Descendant override specification inside a `NodeSpec` of `kind: "instance"`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverrideSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variant: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, SlotFillSpec>,
}

/// A canvas comment thread enriched with its resolved world position and the
/// exact target node and top-level screen/frame under the pin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    match s {
        "$easeui.bg.canvas" | "$easeui.bg" => Some(Color::rgb(0x12, 0x12, 0x15)),
        "$easeui.bg.panel" | "$easeui.panel" => Some(Color::rgb(0x18, 0x18, 0x1B)),
        "$easeui.bg.surface" | "$easeui.surface" => Some(Color::rgb(0x27, 0x27, 0x2A)),
        "$easeui.bg.elevated" | "$easeui.elevated" => Some(Color::rgb(0x30, 0x30, 0x36)),
        "$easeui.border" => Some(Color::rgb(0x27, 0x27, 0x2A)),
        "$easeui.border.subtle" => Some(Color::rgb(0x3F, 0x3F, 0x46)),
        "$easeui.text.primary" | "$easeui.text" => Some(Color::rgb(0xF4, 0xF4, 0xF5)),
        "$easeui.text.secondary" => Some(Color::rgb(0xA1, 0xA1, 0xAA)),
        "$easeui.text.muted" => Some(Color::rgb(0x71, 0x71, 0x7A)),
        "$easeui.accent.orange" | "$easeui.brand" | "$easeui.primary" => Some(Color::rgb(0xE3, 0x63, 0x23)),
        "$easeui.accent.blue" | "$easeui.selection" => Some(Color::rgb(0x35, 0x82, 0xFF)),
        "$easeui.accent.violet" | "$easeui.component" => Some(Color::rgb(0x87, 0x55, 0xE9)),
        "$easeui.accent.emerald" | "$easeui.status.success" | "$easeui.success" => Some(Color::rgb(0x18, 0xB3, 0x68)),
        "$easeui.status.warning" | "$easeui.warning" => Some(Color::rgb(0xF5, 0x9E, 0x0B)),
        "$easeui.status.danger" | "$easeui.danger" => Some(Color::rgb(0xEF, 0x44, 0x44)),
        "$easeui.light.bg" => Some(Color::rgb(0xFA, 0xFA, 0xFA)),
        "$easeui.light.surface" => Some(Color::rgb(0xFF, 0xFF, 0xFF)),
        "$easeui.light.border" => Some(Color::rgb(0xE4, 0xE4, 0xE7)),
        "$easeui.light.text" => Some(Color::rgb(0x18, 0x18, 0x1B)),
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
        self.apply_spec_with_libraries(parent, spec, &[]).map(|r| r.root_node_id).map_err(|e| e.to_string())
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn build_spec_node_with_ctx(
        &mut self,
        parent: NodeId,
        spec: &NodeSpec,
        ctx: &mut crate::spec_instance::SpecBuildCtx<'_>,
    ) -> Result<NodeId, crate::spec_instance::SpecError> {
        let kind_str = if spec.kind.is_empty() {
            if spec.component.is_some() {
                "instance"
            } else if spec.preset.is_some() || spec.layout.is_some() || !spec.children.is_empty() {
                "frame"
            } else if spec.text.is_some() {
                "text"
            } else {
                "frame"
            }
        } else {
            spec.kind.as_str()
        };

        if kind_str.eq_ignore_ascii_case("instance") {
            return self.build_instance_spec_node(parent, spec, ctx);
        }
        if !spec.slots.is_empty() {
            return Err(crate::spec_instance::SpecError::plain("slots can only be set on instance nodes"));
        }

        let id = self.try_alloc_id().ok_or_else(|| crate::spec_instance::SpecError::plain("Document has exhausted node IDs"))?;
        let w = spec.width.unwrap_or(if spec.kind.eq_ignore_ascii_case("text") { 120.0 } else { 200.0 });
        let h = spec.height.unwrap_or(if spec.kind.eq_ignore_ascii_case("text") { 24.0 } else { 100.0 });
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
                style.font_size = spec.font_size.unwrap_or(14.0);
                style.font_weight = spec.font_weight.unwrap_or(400);
                let col = spec.text_color.as_deref().or(spec.fill.as_deref()).and_then(resolve_easeui_color).unwrap_or_else(|| Color::rgb(0xF4, 0xF4, 0xF5));
                style.fills = vec![Paint::solid(col)];
                Node::text(id, rect, spec.text.clone().unwrap_or_default(), style)
            }
            other => return Err(crate::spec_instance::SpecError::plain(format!("Unsupported spec node kind: {other}"))),
        };

        if let Some(preset) = &spec.preset {
            apply_easeui_preset(&mut node, preset);
        }
        if !spec.name.is_empty() {
            node.name = spec.name.clone();
        } else if let Some(preset) = &spec.preset {
            node.name = preset.clone();
        } else if let Some(t) = &spec.text {
            node.name = t.chars().take(32).collect();
        }
        // LayoutSpec on frame (applied BEFORE width/height sizing so al.direction is final)
        if let Some(ls) = &spec.layout {
            if let Some(f) = node.kind.frame_props_mut() {
                let mut al = f.auto_layout.take().unwrap_or_else(AutoLayout::v1);
                if !ls.direction.is_empty() {
                    al.direction = parse_direction(&ls.direction);
                }
                al.wrap = ls.wrap;
                al.item_spacing = ls.gap;
                al.counter_spacing = ls.counter_gap;
                if ls.padding != [0.0; 4] {
                    al.padding = ls.padding;
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

        if let Some(w_explicit) = spec.width {
            node.size.x = w_explicit.max(0.0);
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
        if let Some(h_explicit) = spec.height {
            node.size.y = h_explicit.max(0.0);
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
        node.opacity = spec.opacity.clamp(0.0, 1.0);

        if let Some(fill_str) = &spec.fill {
            if !matches!(node.kind, NodeKind::Text(_)) {
                if fill_str.eq_ignore_ascii_case("none") || fill_str.eq_ignore_ascii_case("transparent") {
                    node.fills.clear();
                } else if let Some(c) = resolve_easeui_color(fill_str) {
                    node.fills = vec![Paint::solid(c)];
                }
            }
        }
        if let Some(stroke_str) = &spec.stroke {
            if let Some(c) = resolve_easeui_color(stroke_str) {
                node.stroke.paints = vec![Paint::solid(c)];
            }
        }
        if let Some(sw) = spec.stroke_weight {
            node.stroke.weight = sw.max(0.0);
        }
        if let Some(r) = spec.corner_radius {
            node.corner_radii = [r.max(0.0); 4];
        }
        if let Some(radii) = spec.corner_radii {
            node.corner_radii = radii;
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

        // Estimate intrinsic text bounds for text nodes
        if let NodeKind::Text(tp) = &mut node.kind {
            let est = estimate_intrinsic_text_size(tp);
            if spec.width.is_some() || spec.width_sizing.as_deref() == Some("fill") {
                tp.auto_resize = TextAutoResize::Height;
                if spec.height.is_none() {
                    let lines = (est.x / node.size.x.max(20.0)).ceil().max(1.0);
                    node.size.y = (lines * tp.style.font_size.max(8.0) * 1.45).ceil();
                }
            } else {
                if spec.width.is_none() {
                    node.size.x = est.x;
                }
                if spec.height.is_none() {
                    node.size.y = est.y;
                }
            }
        }

        self.insert(parent, node).map_err(|e| crate::spec_instance::SpecError::plain(e.to_string()))?;

        // Convenience: if a frame/preset (like easeui-button-primary or easeui-badge) specifies `text`,
        // automatically insert a child Text node inside it!
        if !matches!(kind_str.to_ascii_lowercase().as_str(), "text") {
            if let Some(label) = &spec.text {
                let tid = self.try_alloc_id().ok_or_else(|| crate::spec_instance::SpecError::plain("Document has exhausted node IDs"))?;
                let mut style = TextStyle::default();
                style.font_size = spec.font_size.unwrap_or(13.0);
                style.font_weight = spec.font_weight.unwrap_or(500);
                let col = spec.text_color.as_deref().and_then(resolve_easeui_color).unwrap_or_else(|| Color::rgb(0xF4, 0xF4, 0xF5));
                style.fills = vec![Paint::solid(col)];
                let mut tnode = Node::text(tid, Rect::new(0.0, 0.0, 60.0, 18.0), label.clone(), style);
                tnode.name = label.clone();
                if let NodeKind::Text(tp) = &tnode.kind {
                    tnode.size = estimate_intrinsic_text_size(tp);
                }
                self.insert(id, tnode).map_err(|e| crate::spec_instance::SpecError::plain(e.to_string()))?;
            }
        }

        for child_spec in &spec.children {
            self.build_spec_node_with_ctx(id, child_spec, ctx)?;
        }

        Ok(id)
    }

    /// Exports `root` and its descendants into a declarative [`NodeSpec`] tree,
    /// including any Dev Annotations and Canvas Comment Threads pinned within `root`.
    pub fn export_spec(&self, root: NodeId) -> Result<NodeSpec, String> {
        let all_comments = self.resolve_comment_targets();
        self.export_spec_inner(root, &all_comments)
    }

    pub(crate) fn export_spec_inner(&self, id: NodeId, all_comments: &[ResolvedCommentThread]) -> Result<NodeSpec, String> {
        let n = self.get(id).ok_or_else(|| format!("Node {} not found", id.0))?;
        if matches!(n.kind, NodeKind::Instance { .. }) {
            return self.export_instance_spec_node(id, all_comments);
        }
        let fill = n.fills.first().and_then(|p| match &p.kind {
            PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
            _ => None,
        });
        let stroke = n.stroke.paints.first().and_then(|p| match &p.kind {
            PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
            _ => None,
        });
        let stroke_weight = (!n.stroke.paints.is_empty()).then_some(n.stroke.weight);
        let corner_radius = (n.corner_radii != [0.0; 4] && n.corner_radii.iter().all(|r| *r == n.corner_radii[0])).then_some(n.corner_radii[0]);
        let corner_radii = (n.corner_radii != [0.0; 4] && corner_radius.is_none()).then_some(n.corner_radii);
        let (text, font_size, font_weight, text_color) = match &n.kind {
            NodeKind::Text(tp) => (
                Some(tp.content.clone()),
                Some(tp.style.font_size),
                Some(tp.style.font_weight),
                tp.style.fills.first().and_then(|p| match &p.kind {
                    PaintKind::Solid(c) => Some(format!("#{}", c.to_hex())),
                    _ => None,
                }),
            ),
            _ => (None, None, None, None),
        };
        let layout = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| LayoutSpec {
            direction: match al.direction {
                LayoutDirection::Horizontal => "row".into(),
                LayoutDirection::Vertical => "column".into(),
                LayoutDirection::Grid => "grid".into(),
            },
            wrap: al.wrap,
            gap: al.item_spacing,
            counter_gap: al.counter_spacing,
            padding: al.padding,
            primary_align: format!("{:?}", al.primary_align).to_ascii_lowercase(),
            counter_align: format!("{:?}", al.counter_align).to_ascii_lowercase(),
            grid_columns: (al.grid_columns > 0).then_some(al.grid_columns),
            grid_rows: (al.grid_rows > 0).then_some(al.grid_rows),
        });
        let dev_status = n.dev.status.map(|s| s.label().to_string());
        let annotations: Vec<String> = n.dev.annotations.iter().map(|a| a.text.clone()).collect();
        let comments: Vec<ResolvedCommentThread> = all_comments
            .iter()
            .filter(|c| c.target_node_id == Some(id.0) || (c.target_node_id.is_none() && c.target_frame_id == Some(id.0)))
            .cloned()
            .collect();
        let mut children = Vec::with_capacity(n.children.len());
        for c in &n.children {
            if let Ok(cs) = self.export_spec_inner(*c, all_comments) {
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
            width: Some(n.size.x),
            height: Some(n.size.y),
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
            opacity: n.opacity,
            clips_content: n.kind.frame_props().and_then(|f| f.clips_content.then_some(true)),
            text,
            font_size,
            font_weight,
            text_color,
            layout,
            dev_status,
            annotation: None,
            annotations,
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
}

pub(crate) fn estimate_intrinsic_text_size(tp: &TextProps) -> Vec2 {
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
}
