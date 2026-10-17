use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn default_true() -> bool {
    true
}
fn is_true(v: &bool) -> bool {
    *v
}
fn is_false(v: &bool) -> bool {
    !*v
}
fn default_one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_zero_radii(r: &[f32; 4]) -> bool {
    *r == [0.0; 4]
}
fn default_node_blend() -> BlendMode {
    BlendMode::PassThrough
}
fn is_default_node_blend(b: &BlendMode) -> bool {
    *b == BlendMode::PassThrough
}
fn default_node_stroke() -> Stroke {
    Stroke { weight: 1.0, miter_limit: 4.0, ..Default::default() }
}
fn is_default_node_stroke(s: &Stroke) -> bool {
    s.paints.is_empty()
        && s.weight == 1.0
        && s.side_weights.is_none()
        && s.align == StrokeAlign::Inside
        && s.cap == StrokeCap::None
        && s.join == StrokeJoin::Miter
        && s.miter_limit == 4.0
        && s.dashes.is_empty()
}
fn is_default_constraints(c: &Constraints) -> bool {
    *c == Constraints::default()
}
fn is_default_layout_child(l: &LayoutChild) -> bool {
    *l == LayoutChild::default()
}
fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}
fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}
fn is_zero_or_one_u16(v: &u16) -> bool {
    *v <= 1
}
fn is_default_grid_tracks(t: &[GridTrack; 8]) -> bool {
    *t == [GridTrack::default(); 8]
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// Original id from an imported file (Figma "12:34"), for round-tripping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub name: String,
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<NodeId>,
    /// Back-to-front order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeId>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    #[serde(default = "default_one", skip_serializing_if = "is_one")]
    pub opacity: f32,
    #[serde(default = "default_node_blend", skip_serializing_if = "is_default_node_blend")]
    pub blend: BlendMode,
    /// Transform relative to the parent (Figma `relativeTransform`).
    pub transform: Affine,
    /// Untransformed size.
    pub size: Vec2,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fills: Vec<Paint>,
    #[serde(default = "default_node_stroke", skip_serializing_if = "is_default_node_stroke")]
    pub stroke: Stroke,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    /// Corner radii (top-left, top-right, bottom-right, bottom-left).
    #[serde(default, skip_serializing_if = "is_zero_radii")]
    pub corner_radii: [f32; 4],
    /// iOS-style corner smoothing 0..1.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub corner_smoothing: f32,
    #[serde(default, skip_serializing_if = "is_default_constraints")]
    pub constraints: Constraints,
    /// Child behaviour inside an auto-layout parent.
    #[serde(default, skip_serializing_if = "is_default_layout_child")]
    pub layout_child: LayoutChild,
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_mask: bool,
    /// Style id per property ("fill", "stroke", "text", "effect", "grid").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub style_refs: BTreeMap<String, String>,
    /// Precomputed fill outline in local space (Figma `fillGeometry`). When
    /// present, renderers may use it instead of deriving geometry, e.g. for
    /// boolean operations whose result was computed by Figma.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_geometry: Option<Vec<Path>>,
    /// Precomputed stroke outline in local space (Figma `strokeGeometry`),
    /// already expanded by weight/align; filled with the stroke paints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_geometry: Option<Vec<Path>>,
    /// Component/instance bookkeeping; empty for ordinary nodes.
    #[serde(default, skip_serializing_if = "ComponentLink::is_empty")]
    pub component: ComponentLink,
    /// Prototype interactions and scroll behaviour; empty for most nodes.
    #[serde(default, skip_serializing_if = "PrototypeProps::is_empty")]
    pub prototype: PrototypeProps,
    /// Variable bindings and explicit modes; empty for most nodes.
    #[serde(default, skip_serializing_if = "NodeVariables::is_empty")]
    pub variables: NodeVariables,
    /// Export presets shown in the inspector's Export section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub export_settings: Vec<ExportPreset>,
    /// Dev mode status, links and annotations; empty for most nodes.
    #[serde(default, skip_serializing_if = "DevInfo::is_empty")]
    pub dev: DevInfo,
    /// Presentation metadata; ordinary design nodes omit this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide: Option<SlideProps>,
    /// Whether width/height aspect ratio is locked (`proportionsConstrained` / `targetAspectRatio`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_aspect: bool,
    /// Optional external URL or internal node hyperlink (`"guid:s:l"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
    /// Plugin metadata entries (`"{pluginID}:{key}" -> value` or `key -> value`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugin_data: BTreeMap<String, String>,
    /// Detached symbol provenance (`detachedSymbolId`, `"guid:s:l"` or `"assetRef:key:version"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detached_symbol_id: Option<String>,
    /// Lossless `.fig` v106 provenance & metadata bag (`editInfo`, `userFacingVersion`,
    /// `sharedSymbolVersion`, `isSymbolPublishable`, `publishedVersion`, `isPublishable`,
    /// `version`, `publishID`, `widgetCachedAncestor`, `pluginRelaunchData`,
    /// `nodeGenerationData`, `brushType`, `shapeWithTextType`, etc.).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fig_metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideProps {
    #[serde(default)]
    pub notes: String,
}

/// A node property that an instance can override independently of its main
/// component. Syncing a component into its instances copies every field that
/// is not in the instance node's [`ComponentLink::overrides`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OverrideField {
    Name,
    Visible,
    Locked,
    Opacity,
    Blend,
    Size,
    Transform,
    Fills,
    Stroke,
    Effects,
    CornerRadii,
    /// Text characters.
    Characters,
    /// Text style and runs.
    TextStyle,
    /// Frame props: clipping, auto layout, grids.
    Layout,
    /// Which component an instance points at (instance swap).
    MainComponent,
    /// Custom children inside a component slot container.
    SlotContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PropValue {
    Bool(bool),
    Text(String),
    /// Selected option of a variant property.
    Variant(String),
    /// Component swapped in, by node id.
    Instance(NodeId),
    /// Custom nodes inserted into a component slot.
    Slot(Vec<NodeId>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PropKind {
    Boolean,
    Text,
    Variant { options: Vec<String> },
    InstanceSwap { preferred: Vec<NodeId> },
    Slot,
}

/// Component property definition, on a Component or ComponentSet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentPropDef {
    pub name: String,
    pub kind: PropKind,
    pub default: PropValue,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ComponentLink {
    /// For a node inside an instance: the node of the main component it mirrors.
    /// Instance roots point at their component via `NodeKind::Instance::component`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<NodeId>,
    /// Properties changed on this instance node; preserved when syncing.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub overrides: BTreeSet<OverrideField>,
    /// Property definitions (Component / ComponentSet only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<ComponentPropDef>,
    /// Property values chosen on an instance root, by property name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, PropValue>,
    /// Binds a node property to a component property, e.g.
    /// `Visible -> "Show icon"`, `Characters -> "Label"`, `MainComponent -> "Icon"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<OverrideField, String>,
    /// Component or component-set description (`symbolDescription`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Whether nested instance properties are bubbled/exposed (`propsAreBubbled`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub expose_nested_instances: bool,
    /// Variant property specifications (`variantPropSpecs`, tag=483) on a variant component.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variant_props: BTreeMap<String, String>,
    /// Override hierarchy depth (`overrideLevel`, tag=321).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub override_level: Option<i32>,
    /// Detached symbol provenance (`detachedSymbolId`, tag=342).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detached_symbol_id: Option<String>,
}

impl ComponentLink {
    pub fn is_empty(&self) -> bool {
        self.source.is_none()
            && self.overrides.is_empty()
            && self.props.is_empty()
            && self.values.is_empty()
            && self.bindings.is_empty()
            && self.description.is_empty()
            && !self.expose_nested_instances
            && self.variant_props.is_empty()
            && self.override_level.is_none()
            && self.detached_symbol_id.is_none()
    }
}

impl Node {
    pub fn new(id: NodeId, kind: NodeKind) -> Self {
        Self {
            id,
            source_id: None,
            name: kind.default_name().into(),
            kind,
            parent: None,
            children: Vec::new(),
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: BlendMode::PassThrough,
            transform: Affine::IDENTITY,
            size: Vec2::ZERO,
            fills: Vec::new(),
            stroke: Stroke { weight: 1.0, miter_limit: 4.0, ..Default::default() },
            effects: Vec::new(),
            corner_radii: [0.0; 4],
            corner_smoothing: 0.0,
            constraints: Constraints::default(),
            layout_child: LayoutChild::default(),
            is_mask: false,
            style_refs: BTreeMap::new(),
            fill_geometry: None,
            stroke_geometry: None,
            component: ComponentLink::default(),
            prototype: PrototypeProps::default(),
            variables: NodeVariables::default(),
            export_settings: Vec::new(),
            dev: DevInfo::default(),
            slide: None,
            lock_aspect: false,
            hyperlink: None,
            plugin_data: BTreeMap::new(),
            detached_symbol_id: None,
            fig_metadata: BTreeMap::new(),
        }
    }

    fn placed(id: NodeId, kind: NodeKind, r: Rect) -> Self {
        let mut n = Self::new(id, kind);
        n.transform = Affine::translate(r.x, r.y);
        n.size = Vec2::new(r.w, r.h);
        n
    }

    pub fn frame(id: NodeId, r: Rect) -> Self {
        let mut n = Self::placed(id, NodeKind::Frame(FrameProps::default()), r);
        n.fills = vec![Paint::solid(Color::WHITE)];
        n
    }
    pub fn rectangle(id: NodeId, r: Rect) -> Self {
        let mut n = Self::placed(id, NodeKind::Rectangle, r);
        n.fills = vec![Paint::solid(Color::rgb(0xD9, 0xD9, 0xD9))];
        n
    }
    pub fn ellipse(id: NodeId, r: Rect) -> Self {
        let mut n = Self::placed(id, NodeKind::Ellipse(ArcData::default()), r);
        n.fills = vec![Paint::solid(Color::rgb(0xD9, 0xD9, 0xD9))];
        n
    }
    pub fn text(id: NodeId, r: Rect, content: impl Into<String>, style: TextStyle) -> Self {
        Self::placed(
            id,
            NodeKind::Text(TextProps {
                content: content.into(),
                style,
                runs: Vec::new(),
                auto_resize: TextAutoResize::WidthAndHeight,
                max_lines: None,
                ..Default::default()
            }),
            r,
        )
    }

    /// Axis-aligned local bounds (0, 0, w, h).
    pub fn local_bounds(&self) -> Rect {
        Rect::new(0.0, 0.0, self.size.x, self.size.y)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    Page {
        background: Color,
    },
    Frame(FrameProps),
    Group,
    Section,
    Rectangle,
    Ellipse(ArcData),
    Line,
    /// Regular polygon with `count` sides.
    Polygon {
        count: u32,
    },
    Star {
        count: u32,
        inner_ratio: f32,
    },
    /// Arbitrary vector geometry in local space.
    ///
    /// `paths` is the rendered geometry. When `network` is present it is the
    /// source of truth (vertices, segments, filled regions, per-vertex corner
    /// radius) and `paths` is derived from it with `VectorNetwork::to_paths`.
    Vector {
        paths: Vec<Path>,
        #[serde(default)]
        network: Option<VectorNetwork>,
    },
    Text(TextProps),
    BooleanOp(BooleanOp),
    /// Main component; frame-like.
    Component(FrameProps),
    ComponentSet(FrameProps),
    /// Instance of a component; children are the resolved (overridden) copy.
    Instance {
        frame: FrameProps,
        component: Option<NodeId>,
        component_key: Option<String>,
    },
    /// Board sticky note.
    Sticky(StickyProps),
    /// Board connector between two nodes (or free points); see [`ConnectorProps`].
    Connector(ConnectorProps),
    /// Board shape that carries centered text.
    ShapeWithText(ShapeWithTextProps),
    /// Board stamp: a small badge drawn from vector artwork (`kind` names the design).
    Stamp {
        kind: String,
    },
    /// Anything the importer could not map, kept so nothing is silently lost.
    Unknown {
        type_name: String,
    },
}

impl NodeKind {
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            NodeKind::Page { .. }
                | NodeKind::Frame(_)
                | NodeKind::Group
                | NodeKind::Section
                | NodeKind::BooleanOp(_)
                | NodeKind::Component(_)
                | NodeKind::ComponentSet(_)
                | NodeKind::Instance { .. }
        )
    }

    /// Board items that are drawn by lowering to ordinary shapes and text.
    pub fn is_board_item(&self) -> bool {
        matches!(self, NodeKind::Sticky(_) | NodeKind::Connector(_) | NodeKind::ShapeWithText(_) | NodeKind::Stamp { .. })
    }

    /// The text of a board item that has its own text (sticky, shape with text), if any.
    pub fn board_text(&self) -> Option<&TextProps> {
        match self {
            NodeKind::Sticky(s) => Some(&s.text),
            NodeKind::ShapeWithText(s) => Some(&s.text),
            NodeKind::Connector(c) => c.label.as_ref(),
            _ => None,
        }
    }

    /// The editable text of a text layer or of a board item that carries text.
    pub fn any_text(&self) -> Option<&TextProps> {
        match self {
            NodeKind::Text(t) => Some(t),
            k => k.board_text(),
        }
    }

    pub fn any_text_mut(&mut self) -> Option<&mut TextProps> {
        match self {
            NodeKind::Text(t) => Some(t),
            k => k.board_text_mut(),
        }
    }

    pub fn board_text_mut(&mut self) -> Option<&mut TextProps> {
        match self {
            NodeKind::Sticky(s) => Some(&mut s.text),
            NodeKind::ShapeWithText(s) => Some(&mut s.text),
            NodeKind::Connector(c) => c.label.as_mut(),
            _ => None,
        }
    }

    pub fn frame_props(&self) -> Option<&FrameProps> {
        match self {
            NodeKind::Frame(f) | NodeKind::Component(f) | NodeKind::ComponentSet(f) => Some(f),
            NodeKind::Instance { frame, .. } => Some(frame),
            _ => None,
        }
    }

    pub fn frame_props_mut(&mut self) -> Option<&mut FrameProps> {
        match self {
            NodeKind::Frame(f) | NodeKind::Component(f) | NodeKind::ComponentSet(f) => Some(f),
            NodeKind::Instance { frame, .. } => Some(frame),
            _ => None,
        }
    }

    pub fn default_name(&self) -> &'static str {
        match self {
            NodeKind::Page { .. } => "Page",
            NodeKind::Frame(_) => "Frame",
            NodeKind::Group => "Group",
            NodeKind::Section => "Section",
            NodeKind::Rectangle => "Rectangle",
            NodeKind::Ellipse(_) => "Ellipse",
            NodeKind::Line => "Line",
            NodeKind::Polygon { .. } => "Polygon",
            NodeKind::Star { .. } => "Star",
            NodeKind::Vector { .. } => "Vector",
            NodeKind::Text(_) => "Text",
            NodeKind::BooleanOp(_) => "Union",
            NodeKind::Component(_) => "Component",
            NodeKind::ComponentSet(_) => "Component set",
            NodeKind::Instance { .. } => "Instance",
            NodeKind::Sticky(_) => "Sticky note",
            NodeKind::Connector(_) => "Connector",
            NodeKind::ShapeWithText(_) => "Shape",
            NodeKind::Stamp { .. } => "Stamp",
            NodeKind::Unknown { .. } => "Layer",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanOp {
    Union,
    Subtract,
    Intersect,
    Exclude,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcData {
    pub start: f32,
    pub end: f32,
    pub inner_radius: f32,
}

impl Default for ArcData {
    fn default() -> Self {
        Self { start: 0.0, end: std::f32::consts::TAU, inner_radius: 0.0 }
    }
}

fn is_default_paragraph(p: &ParagraphProps) -> bool {
    *p == ParagraphProps::default()
}
fn is_default_direction(d: &TextDirection) -> bool {
    *d == TextDirection::default()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextProps {
    pub content: String,
    pub style: TextStyle,
    /// Style overrides over byte ranges of `content`; empty means uniform.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<TextRun>,
    pub auto_resize: TextAutoResize,
    /// Maximum visible lines when `auto_resize` is `Truncate`; overflow ends in "…".
    /// `None` truncates at the box height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u32>,
    /// Paragraph settings (indent, list spacing, hanging punctuation / lists, leading trim).
    #[serde(default, skip_serializing_if = "is_default_paragraph")]
    pub paragraph: ParagraphProps,
    /// Bulleted and numbered lists as byte ranges covering whole paragraphs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lists: Vec<ListRange>,
    /// Paragraph writing direction.
    #[serde(default, skip_serializing_if = "is_default_direction")]
    pub direction: TextDirection,
    /// Whether the layer name automatically tracks text content (`autoRename`, tag=14).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub auto_rename: bool,
    /// Whether underlines skip glyph descenders (`textDecorationSkipInk`, tag=412).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub decoration_skip_ink: bool,
    /// Line-breaking strategy (`textWrapStyle`, tag=594).
    #[serde(default, skip_serializing_if = "is_default_text_wrap")]
    pub text_wrap: TextWrap,
}

/// CSS `text-wrap` for a text node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextWrap {
    /// Greedy: fill each line, break at the last word that fits.
    #[default]
    Auto,
    /// Even line widths for the same line count (`text-wrap: balance`).
    Balance,
    /// Avoid a single-word last line (`text-wrap: pretty`).
    Pretty,
}

fn is_default_text_wrap(w: &TextWrap) -> bool {
    *w == TextWrap::default()
}

impl Default for TextProps {
    fn default() -> Self {
        Self {
            content: String::new(),
            style: TextStyle::default(),
            runs: Vec::new(),
            auto_resize: TextAutoResize::default(),
            max_lines: None,
            paragraph: ParagraphProps::default(),
            lists: Vec::new(),
            direction: TextDirection::default(),
            auto_rename: true,
            decoration_skip_ink: true,
            text_wrap: TextWrap::Auto,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameProps {
    #[serde(default, skip_serializing_if = "is_false")]
    pub clips_content: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_layout: Option<AutoLayout>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grids: Vec<LayoutGrid>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutDirection {
    #[default]
    Horizontal,
    Vertical,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GridTrack {
    Flex(f32),
    Fixed(f32),
    Hug,
}

impl Default for GridTrack {
    fn default() -> Self {
        GridTrack::Flex(1.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutAlign {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    Baseline,
    /// CSS `justify-content: space-around`: equal space on both sides of every item, so the edge gap is half the inner gap.
    SpaceAround,
    /// CSS `justify-content: space-evenly`: every gap, edges included, is equal.
    SpaceEvenly,
}

/// How wrapped lines distribute along the counter axis (`stackCounterAlignContent`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterAlignContent {
    /// Lines sit at the start, `counter_spacing` apart.
    #[default]
    Auto,
    /// Lines spread over the frame's counter extent, edge to edge.
    SpaceBetween,
}

fn is_default_counter_align_content(c: &CounterAlignContent) -> bool {
    *c == CounterAlignContent::default()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sizing {
    #[default]
    Fixed,
    Hug,
    Fill,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoLayout {
    pub direction: LayoutDirection,
    pub wrap: bool,
    pub item_spacing: f32,
    pub counter_spacing: f32,
    /// top, right, bottom, left
    pub padding: [f32; 4],
    pub primary_align: LayoutAlign,
    pub counter_align: LayoutAlign,
    /// Wrapped lines along the counter axis (`stackCounterAlignContent`); only used when `wrap` is on.
    #[serde(default, skip_serializing_if = "is_default_counter_align_content")]
    pub counter_align_content: CounterAlignContent,
    /// Layout engine revision (`stackLayoutVersion`); 1 and up is the CSS-aligned layout.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub layout_version: u8,
    pub primary_sizing: Sizing,
    pub counter_sizing: Sizing,
    pub stroke_included: bool,
    pub reverse_z: bool,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub grid_columns: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub grid_rows: u32,
    #[serde(default, skip_serializing_if = "is_default_grid_tracks")]
    pub grid_col_tracks: [GridTrack; 8],
    #[serde(default, skip_serializing_if = "is_default_grid_tracks")]
    pub grid_row_tracks: [GridTrack; 8],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutChild {
    pub absolute: bool,
    pub horizontal: Sizing,
    pub vertical: Sizing,
    pub grow: f32,
    pub min_w: Option<f32>,
    pub max_w: Option<f32>,
    pub min_h: Option<f32>,
    pub max_h: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_column: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_row: Option<u16>,
    #[serde(default, skip_serializing_if = "is_zero_or_one_u16")]
    pub col_span: u16,
    #[serde(default, skip_serializing_if = "is_zero_or_one_u16")]
    pub row_span: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constraint {
    #[default]
    Min,
    Max,
    Center,
    Stretch,
    Scale,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraints {
    pub horizontal: Constraint,
    pub vertical: Constraint,
}
