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
    /// `"frame"`, `"rectangle"`, `"ellipse"`, `"text"`, `"section"`, `"component"`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
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

fn parse_sizing(s: &str) -> Sizing {
    match s.trim().to_ascii_lowercase().as_str() {
        "hug" | "auto" => Sizing::Hug,
        "fill" | "stretch" | "100%" => Sizing::Fill,
        _ => Sizing::Fixed,
    }
}

fn parse_align(s: &str) -> LayoutAlign {
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

fn parse_direction(s: &str) -> LayoutDirection {
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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
        let pnode = self.get(parent).ok_or_else(|| format!("Parent node {} not found", parent.0))?;
        if !pnode.kind.is_container() {
            return Err(format!("Parent node {} is not a container", parent.0));
        }
        let root_id = self.build_spec_node(parent, spec)?;
        // Run Auto-Layout 5.0 reflow from parent (if parent has auto-layout) or root_id
        let relayout_root = if self.get(parent).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout).is_some() { parent } else { root_id };
        self.relayout_subtree(relayout_root);
        Ok(root_id)
    }

    fn build_spec_node(&mut self, parent: NodeId, spec: &NodeSpec) -> Result<NodeId, String> {
        let id = self.try_alloc_id().ok_or("Document has exhausted node IDs")?;
        let w = spec.width.unwrap_or(if spec.kind.eq_ignore_ascii_case("text") { 120.0 } else { 200.0 });
        let h = spec.height.unwrap_or(if spec.kind.eq_ignore_ascii_case("text") { 24.0 } else { 100.0 });
        let rect = Rect::new(spec.x, spec.y, w.max(0.0), h.max(0.0));
        let kind_str = if spec.kind.is_empty() {
            if spec.preset.is_some() || spec.layout.is_some() || !spec.children.is_empty() {
                "frame"
            } else if spec.text.is_some() {
                "text"
            } else {
                "frame"
            }
        } else {
            spec.kind.as_str()
        };

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
            other => return Err(format!("Unsupported spec node kind: {other}")),
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
                let mut al = f.auto_layout.unwrap_or_default();
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
                    if al.direction == LayoutDirection::Horizontal {
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
                    if al.direction == LayoutDirection::Vertical {
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
                if al.direction == LayoutDirection::Horizontal {
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
                if al.direction == LayoutDirection::Vertical {
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

        self.insert(parent, node).map_err(|e| e.to_string())?;

        // Convenience: if a frame/preset (like easeui-button-primary or easeui-badge) specifies `text`,
        // automatically insert a child Text node inside it!
        if !matches!(kind_str.to_ascii_lowercase().as_str(), "text") {
            if let Some(label) = &spec.text {
                let tid = self.try_alloc_id().ok_or("Document has exhausted node IDs")?;
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
                self.insert(id, tnode).map_err(|e| e.to_string())?;
            }
        }

        for child_spec in &spec.children {
            self.build_spec_node(id, child_spec)?;
        }

        Ok(id)
    }

    /// Bottom-up Auto-Layout 5.0 solver supporting Horizontal/Vertical flow, `wrap`,
    /// `Hug`/`Fill`/`Fixed` sizing, `SpaceBetween`/`Center`/`End`, and `min_w`/`max_w`/`min_h`/`max_h` clamping.
    pub fn relayout_subtree(&mut self, root: NodeId) {
        let Some(n) = self.get(root) else { return };
        let children = n.children.clone();
        for c in &children {
            self.relayout_subtree(*c);
        }
        let Some(al) = self.get(root).and_then(|n| n.kind.frame_props()).and_then(|f| f.auto_layout) else {
            return;
        };
        let flow_ids: Vec<NodeId> = children.iter().copied().filter(|cid| self.get(*cid).is_some_and(|k| k.visible && !k.layout_child.absolute)).collect();

        if al.direction == LayoutDirection::Grid {
            let [pt, pr, pb, pl] = al.padding;
            let cols = (if al.grid_columns == 0 { 3 } else { al.grid_columns }).max(1) as usize;
            let col_gap = al.item_spacing;
            let row_gap = if al.counter_spacing > 0.0 { al.counter_spacing } else { al.item_spacing };
            let hug_h = al.counter_sizing == Sizing::Hug || al.primary_sizing == Sizing::Hug;
            let cur_frame = self.get(root).unwrap().size;

            if flow_ids.is_empty() {
                if hug_h {
                    if let Some(fnode) = self.get_mut(root) {
                        fnode.size.y = pt + pb;
                    }
                }
                return;
            }

            let mut occupied = std::collections::BTreeSet::<(usize, usize)>::new();
            let mut placements: Vec<(NodeId, usize, usize, usize, usize)> = Vec::with_capacity(flow_ids.len());
            for cid in &flow_ids {
                let lc = self.get(*cid).unwrap().layout_child;
                let cs = (lc.col_span.max(1) as usize).min(cols);
                let rs = lc.row_span.max(1) as usize;
                let (placed_r, placed_c) = if let (Some(gr), Some(gc)) = (lc.grid_row, lc.grid_column) {
                    (gr as usize, (gc as usize).min(cols - cs))
                } else {
                    let start_r = lc.grid_row.unwrap_or(0) as usize;
                    let mut r = start_r;
                    let mut found = None;
                    while found.is_none() {
                        let c_range = if let Some(gc) = lc.grid_column {
                            let c0 = (gc as usize).min(cols - cs);
                            c0..=c0
                        } else {
                            0..=(cols - cs)
                        };
                        for c in c_range {
                            let fits = (r..r + rs).all(|rr| (c..c + cs).all(|cc| !occupied.contains(&(rr, cc))));
                            if fits {
                                found = Some((r, c));
                                break;
                            }
                        }
                        r += 1;
                    }
                    found.unwrap()
                };
                for rr in placed_r..placed_r + rs {
                    for cc in placed_c..placed_c + cs {
                        occupied.insert((rr, cc));
                    }
                }
                placements.push((*cid, placed_r, placed_c, rs, cs));
            }

            let num_rows = placements.iter().map(|&(_, r, _, rs, _)| r + rs).max().unwrap_or(0).max(al.grid_rows as usize).max(1);

            let avail_w = (cur_frame.x - pl - pr - col_gap * (cols.saturating_sub(1) as f32)).max(0.0);
            let mut col_widths = vec![0.0_f32; cols];
            let mut used_w = 0.0_f32;
            let mut total_fr = 0.0_f32;
            for c in 0..cols {
                let track = al.grid_col_tracks.get(c).copied().unwrap_or(GridTrack::Flex(1.0));
                match track {
                    GridTrack::Fixed(w) => {
                        col_widths[c] = w.max(0.0);
                        used_w += col_widths[c];
                    }
                    GridTrack::Hug => {
                        let max_w = placements
                            .iter()
                            .filter(|&&(_, _, pc, _, cs)| pc == c && cs == 1)
                            .filter_map(|&(cid, _, _, _, _)| self.get(cid).map(|n| n.size.x))
                            .fold(0.0_f32, f32::max);
                        col_widths[c] = max_w;
                        used_w += max_w;
                    }
                    GridTrack::Flex(fr) => {
                        total_fr += fr.max(0.0);
                    }
                }
            }
            let free_w = (avail_w - used_w).max(0.0);
            if total_fr > 0.0 {
                for c in 0..cols {
                    if let GridTrack::Flex(fr) = al.grid_col_tracks.get(c).copied().unwrap_or(GridTrack::Flex(1.0)) {
                        col_widths[c] = free_w * (fr.max(0.0) / total_fr);
                    }
                }
            }

            let mut row_heights = vec![0.0_f32; num_rows];
            for r in 0..num_rows {
                if let Some(GridTrack::Fixed(h)) = al.grid_row_tracks.get(r).copied() {
                    row_heights[r] = h.max(0.0);
                } else {
                    let max_h = placements
                        .iter()
                        .filter(|&&(_, pr_idx, _, rs, _)| pr_idx == r && rs == 1)
                        .filter_map(|&(cid, _, _, _, _)| self.get(cid).map(|n| n.size.y))
                        .fold(0.0_f32, f32::max);
                    row_heights[r] = max_h;
                }
            }
            for &(cid, r, _, rs, _) in &placements {
                if rs > 1 {
                    let ch = self.get(cid).map(|n| n.size.y).unwrap_or(0.0);
                    let cur_span_h: f32 = (r..r + rs).map(|rr| row_heights[rr]).sum::<f32>() + row_gap * ((rs - 1) as f32);
                    if ch > cur_span_h {
                        let extra = (ch - cur_span_h) / (rs as f32);
                        for rr in r..r + rs {
                            row_heights[rr] += extra;
                        }
                    }
                }
            }

            let mut col_x = vec![0.0_f32; cols];
            let mut cx = pl;
            for c in 0..cols {
                col_x[c] = cx;
                cx += col_widths[c] + col_gap;
            }
            let mut row_y = vec![0.0_f32; num_rows];
            let mut ry = pt;
            for r in 0..num_rows {
                row_y[r] = ry;
                ry += row_heights[r] + row_gap;
            }

            if hug_h {
                let total_h = pt + pb + row_heights.iter().sum::<f32>() + row_gap * (num_rows.saturating_sub(1) as f32);
                if let Some(fnode) = self.get_mut(root) {
                    fnode.size.y = total_h;
                }
            }

            for (cid, r, c, rs, cs) in placements {
                let cell_w: f32 = (c..c + cs).map(|cc| col_widths[cc]).sum::<f32>() + col_gap * ((cs - 1) as f32);
                let cell_h: f32 = (r..r + rs).map(|rr| row_heights[rr]).sum::<f32>() + row_gap * ((rs - 1) as f32);
                let lc = self.get(cid).unwrap().layout_child;
                let cur_sz = self.get(cid).unwrap().size;
                let mut w = if lc.horizontal == Sizing::Fill || lc.grow > 0.0 || cs > 1 { cell_w } else { cur_sz.x };
                let mut h = if lc.vertical == Sizing::Fill || rs > 1 { cell_h } else { cur_sz.y };
                if let Some(mw) = lc.min_w {
                    w = w.max(mw);
                }
                if let Some(xw) = lc.max_w {
                    w = w.min(xw);
                }
                if let Some(mh) = lc.min_h {
                    h = h.max(mh);
                }
                if let Some(xh) = lc.max_h {
                    h = h.min(xh);
                }
                let size_changed = (cur_sz.x - w).abs() > 1e-3 || (cur_sz.y - h).abs() > 1e-3;
                if let Some(cnode) = self.get_mut(cid) {
                    cnode.transform.tx = col_x[c];
                    cnode.transform.ty = row_y[r];
                    cnode.size = Vec2::new(w, h);
                }
                if size_changed {
                    self.relayout_subtree(cid);
                }
            }
            return;
        }

        let horiz = al.direction == LayoutDirection::Horizontal;

        let [pt, pr, pb, pl] = al.padding;
        let (pm_start, pm_end, pc_start, pc_end) = if horiz { (pl, pr, pt, pb) } else { (pt, pb, pl, pr) };
        let pm = pm_start + pm_end;
        let pc = pc_start + pc_end;
        let hug_main = al.primary_sizing == Sizing::Hug && !al.wrap;
        let hug_cross = al.counter_sizing == Sizing::Hug;

        let cur_frame = self.get(root).unwrap().size;
        let mut frame_main = if horiz { cur_frame.x } else { cur_frame.y };
        let mut frame_cross = if horiz { cur_frame.y } else { cur_frame.x };

        if flow_ids.is_empty() {
            if hug_main {
                frame_main = pm;
            }
            if hug_cross {
                frame_cross = pc;
            }
            if let Some(fnode) = self.get_mut(root) {
                fnode.size = if horiz { Vec2::new(frame_main, frame_cross) } else { Vec2::new(frame_cross, frame_main) };
            }
            return;
        }

        struct KidInfo {
            id: NodeId,
            main: f32,
            cross: f32,
            main_fill: bool,
            cross_fill: bool,
            min_w: Option<f32>,
            max_w: Option<f32>,
            min_h: Option<f32>,
            max_h: Option<f32>,
        }

        let kids: Vec<KidInfo> = flow_ids
            .iter()
            .map(|cid| {
                let k = self.get(*cid).unwrap();
                let lc = k.layout_child;
                let (ms, cs) = if horiz { (lc.horizontal, lc.vertical) } else { (lc.vertical, lc.horizontal) };
                let (m, c) = if horiz { (k.size.x, k.size.y) } else { (k.size.y, k.size.x) };
                KidInfo {
                    id: *cid,
                    main: m,
                    cross: c,
                    main_fill: ms == Sizing::Fill || lc.grow > 0.0,
                    cross_fill: cs == Sizing::Fill,
                    min_w: lc.min_w,
                    max_w: lc.max_w,
                    min_h: lc.min_h,
                    max_h: lc.max_h,
                }
            })
            .collect();

        let mut lines: Vec<Vec<usize>> = Vec::new();
        if al.wrap {
            let avail = (frame_main - pm).max(0.0);
            let mut cur = Vec::new();
            let mut used = 0.0;
            for (i, k) in kids.iter().enumerate() {
                let add = if cur.is_empty() { k.main } else { al.item_spacing + k.main };
                if !cur.is_empty() && used + add > avail + 1e-3 {
                    lines.push(std::mem::take(&mut cur));
                    cur.push(i);
                    used = k.main;
                } else {
                    cur.push(i);
                    used += add;
                }
            }
            if !cur.is_empty() {
                lines.push(cur);
            }
        } else {
            lines.push((0..kids.len()).collect());
        }

        if hug_main {
            let total: f32 = kids.iter().map(|k| k.main).sum::<f32>() + al.item_spacing * (kids.len() as f32 - 1.0);
            frame_main = total + pm;
        }
        let main_avail = (frame_main - pm).max(0.0);

        let mut line_cross: Vec<f32> =
            lines.iter().map(|l| l.iter().filter(|i| !kids[**i].cross_fill || al.wrap).map(|i| kids[*i].cross).fold(0.0, f32::max)).collect();
        if !al.wrap {
            if hug_cross {
                frame_cross = line_cross[0] + pc;
            } else {
                line_cross[0] = (frame_cross - pc).max(0.0);
            }
        } else {
            let total: f32 = line_cross.iter().sum::<f32>() + al.counter_spacing * (lines.len() as f32 - 1.0);
            if hug_cross {
                frame_cross = total + pc;
            }
        }

        if let Some(fnode) = self.get_mut(root) {
            let mut new_sz = if horiz { Vec2::new(frame_main, frame_cross) } else { Vec2::new(frame_cross, frame_main) };
            if let Some(mw) = fnode.layout_child.min_w {
                new_sz.x = new_sz.x.max(mw);
            }
            if let Some(xw) = fnode.layout_child.max_w {
                new_sz.x = new_sz.x.min(xw);
            }
            if let Some(mh) = fnode.layout_child.min_h {
                new_sz.y = new_sz.y.max(mh);
            }
            if let Some(xh) = fnode.layout_child.max_h {
                new_sz.y = new_sz.y.min(xh);
            }
            fnode.size = new_sz;
        }

        // Wrapped lines spread edge to edge (CSS align-content: space-between); the counter spacing stays the minimum gap.
        let counter_gap = if al.wrap && al.counter_align_content == CounterAlignContent::SpaceBetween && lines.len() > 1 && !hug_cross {
            ((frame_cross - pc - line_cross.iter().sum::<f32>()) / (lines.len() as f32 - 1.0)).max(al.counter_spacing)
        } else {
            al.counter_spacing
        };
        let mut cross_cursor = pc_start;
        for (li, line) in lines.iter().enumerate() {
            let k_count = line.len() as f32;
            let fixed: f32 = line.iter().filter(|i| !kids[**i].main_fill || al.wrap).map(|i| kids[*i].main).sum();
            let fill_cnt = line.iter().filter(|i| kids[**i].main_fill && !al.wrap).count() as f32;
            let free_for_fill = (main_avail - al.item_spacing * (k_count - 1.0) - fixed).max(0.0);
            let mains: Vec<f32> = line
                .iter()
                .map(|i| {
                    let kid = &kids[*i];
                    if kid.main_fill && !al.wrap && !hug_main && fill_cnt > 0.0 {
                        free_for_fill / fill_cnt
                    } else {
                        kid.main
                    }
                })
                .collect();
            let sum_main: f32 = mains.iter().sum();
            // CSS justify-content: the free space ignores the fixed gap; with no free space the items pack from the start.
            let free = main_avail - sum_main;
            let (start, gap) = match al.primary_align {
                LayoutAlign::SpaceBetween if k_count > 1.0 => (0.0, (free / (k_count - 1.0)).max(al.item_spacing.min(0.0))),
                LayoutAlign::SpaceAround if free > 0.0 => (free / k_count / 2.0, free / k_count),
                LayoutAlign::SpaceEvenly if free > 0.0 => (free / (k_count + 1.0), free / (k_count + 1.0)),
                LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly => (0.0, 0.0),
                LayoutAlign::Center => (((main_avail - sum_main - al.item_spacing * (k_count - 1.0)) / 2.0).max(0.0), al.item_spacing),
                LayoutAlign::End => ((main_avail - sum_main - al.item_spacing * (k_count - 1.0)).max(0.0), al.item_spacing),
                _ => (0.0, al.item_spacing),
            };
            let mut cursor = pm_start + start;
            for (j, i) in line.iter().enumerate() {
                let kid = &kids[*i];
                let m = mains[j];
                let c = if kid.cross_fill { line_cross[li] } else { kid.cross };
                let off = match al.counter_align {
                    LayoutAlign::Center => (line_cross[li] - c) / 2.0,
                    LayoutAlign::End => line_cross[li] - c,
                    _ => 0.0,
                };
                let (mut x, mut y, mut w, mut h) = if horiz { (cursor, cross_cursor + off, m, c) } else { (cross_cursor + off, cursor, c, m) };
                if let Some(mw) = kid.min_w {
                    w = w.max(mw);
                }
                if let Some(xw) = kid.max_w {
                    w = w.min(xw);
                }
                if let Some(mh) = kid.min_h {
                    h = h.max(mh);
                }
                if let Some(xh) = kid.max_h {
                    h = h.min(xh);
                }
                let _ = (&mut x, &mut y);
                let size_changed = self.get(kid.id).is_some_and(|n| (n.size.x - w).abs() > 1e-3 || (n.size.y - h).abs() > 1e-3);
                if let Some(cnode) = self.get_mut(kid.id) {
                    cnode.transform.tx = x;
                    cnode.transform.ty = y;
                    cnode.size = Vec2::new(w, h);
                }
                if size_changed {
                    self.relayout_subtree(kid.id);
                }
                cursor += m + gap;
            }
            cross_cursor += line_cross[li] + counter_gap;
        }
    }

    /// Exports `root` and its descendants into a declarative [`NodeSpec`] tree,
    /// including any Dev Annotations and Canvas Comment Threads pinned within `root`.
    pub fn export_spec(&self, root: NodeId) -> Result<NodeSpec, String> {
        let all_comments = self.resolve_comment_targets();
        self.export_spec_inner(root, &all_comments)
    }

    fn export_spec_inner(&self, id: NodeId, all_comments: &[ResolvedCommentThread]) -> Result<NodeSpec, String> {
        let n = self.get(id).ok_or_else(|| format!("Node {} not found", id.0))?;
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
        let layout = n.kind.frame_props().and_then(|f| f.auto_layout).map(|al| LayoutSpec {
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

fn estimate_intrinsic_text_size(tp: &TextProps) -> Vec2 {
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
}
