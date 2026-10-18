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
fn is_default_grid_tracks(t: &GridTracks) -> bool {
    t.is_default()
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

/// Settings of a [`PropKind::Slot`] property (`SlotPropConfig` plus the slot's
/// `preferredValues`). Every scalar is optional: `None` means the file did not
/// write the field, which is not the same as writing its default, and is kept so.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotPropConfig {
    /// `stretchChildOnInsert`: inserted content stretches to the slot's width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretch_child_on_insert: Option<bool>,
    /// `displayByDefault`: the slot shows its content before anything is inserted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_by_default: Option<bool>,
    /// `minChildren`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_children: Option<u32>,
    /// `maxChildren`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_children: Option<u32>,
    /// `allowPreferredValuesOnly`: only [`SlotPropConfig::preferred`] may be inserted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_preferred_values_only: Option<bool>,
    /// Components offered for this slot (`preferredValues`), as local node ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preferred: Vec<NodeId>,
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
    /// Slot settings by property name, for the [`PropKind::Slot`] definitions in `props`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slot_configs: BTreeMap<String, SlotPropConfig>,
    /// The frame is a slot (`isSlot`, tag=463).
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_slot: bool,
    /// The node is content placed in a slot (`isSlotContent`, tag=495).
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_slot_content: bool,
    /// Guid (`session:local`) a node carries as `slot-content-id`. Kept opaque: it names
    /// slot content that need not be a node of this document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_content_id: Option<String>,
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
            && self.slot_configs.is_empty()
            && !self.is_slot
            && !self.is_slot_content
            && self.slot_content_id.is_none()
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
    /// Anything the importer could not map, kept so nothing is silently lost. It keeps its children
    /// (see [`NodeKind::can_hold_children`]) but is not a [`NodeKind::is_container`] and is not drawn.
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

    /// Whether the tree edits (insert, reparent, and the undo of a delete) accept this kind as a parent:
    /// every [`Self::is_container`] kind, plus `Unknown`. The importer keeps the children of a layer it
    /// cannot map (a table, a slide, a keyframe track, ...) under that `Unknown` node, and the undo of
    /// deleting one of them has to put it back. This is only about tree membership. An `Unknown` layer
    /// stays a non-container for clipping, auto layout, hit testing and painting, like a group without
    /// frame properties.
    pub fn can_hold_children(&self) -> bool {
        self.is_container() || matches!(self, NodeKind::Unknown { .. })
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

/// Most columns and spanned rows one grid auto layout solves; also the minimum length of a [`GridTracks`] list.
pub const MAX_GRID_TRACKS: usize = 64;

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

/// Growable list of CSS Grid column or row tracks (at least [`MAX_GRID_TRACKS`] = 64 tracks
/// available by default, and growable beyond that) with backward-compatible array serde.
#[derive(Clone, Debug)]
pub struct GridTracks(pub Vec<GridTrack>);

impl GridTracks {
    pub fn is_default(&self) -> bool {
        let def = GridTrack::default();
        self.0.iter().all(|&t| t == def)
    }

    pub fn as_slice(&self) -> &[GridTrack] {
        &self.0
    }

    pub fn as_mut_slice(&mut self) -> &mut [GridTrack] {
        &mut self.0
    }

    pub fn push(&mut self, track: GridTrack) {
        self.0.push(track);
    }

    pub fn resize_with_default(&mut self, new_len: usize) {
        if self.0.len() < new_len {
            self.0.resize(new_len, GridTrack::default());
        }
    }
}

impl PartialEq for GridTracks {
    fn eq(&self, other: &Self) -> bool {
        let max_len = self.0.len().max(other.0.len());
        let def = GridTrack::default();
        (0..max_len).all(|i| self.0.get(i).copied().unwrap_or(def) == other.0.get(i).copied().unwrap_or(def))
    }
}

impl Default for GridTracks {
    fn default() -> Self {
        Self(vec![GridTrack::default(); MAX_GRID_TRACKS])
    }
}

impl std::ops::Deref for GridTracks {
    type Target = [GridTrack];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for GridTracks {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<const N: usize> From<[GridTrack; N]> for GridTracks {
    fn from(arr: [GridTrack; N]) -> Self {
        Self::from(arr.as_slice())
    }
}

impl From<&[GridTrack]> for GridTracks {
    fn from(slice: &[GridTrack]) -> Self {
        let len = slice.len().max(MAX_GRID_TRACKS);
        let mut vec = vec![GridTrack::default(); len];
        vec[..slice.len()].copy_from_slice(slice);
        Self(vec)
    }
}

impl From<Vec<GridTrack>> for GridTracks {
    fn from(mut vec: Vec<GridTrack>) -> Self {
        if vec.len() < MAX_GRID_TRACKS {
            vec.resize(MAX_GRID_TRACKS, GridTrack::default());
        }
        Self(vec)
    }
}

impl<const N: usize> PartialEq<[GridTrack; N]> for GridTracks {
    fn eq(&self, other: &[GridTrack; N]) -> bool {
        let def = GridTrack::default();
        other.iter().enumerate().all(|(i, &t)| self.0.get(i).copied().unwrap_or(def) == t)
    }
}

impl PartialEq<[GridTrack]> for GridTracks {
    fn eq(&self, other: &[GridTrack]) -> bool {
        let def = GridTrack::default();
        other.iter().enumerate().all(|(i, &t)| self.0.get(i).copied().unwrap_or(def) == t)
    }
}

impl PartialEq<&[GridTrack]> for GridTracks {
    fn eq(&self, other: &&[GridTrack]) -> bool {
        self == *other
    }
}

impl Serialize for GridTracks {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let def = GridTrack::default();
        let last_non_default = self.0.iter().rposition(|&t| t != def).map(|i| i + 1).unwrap_or(0);
        let len = last_non_default.max(8).min(self.0.len());
        self.0[..len].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GridTracks {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let vec = Vec::<GridTrack>::deserialize(deserializer)?;
        Ok(Self::from(vec))
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    pub grid_col_tracks: GridTracks,
    #[serde(default, skip_serializing_if = "is_default_grid_tracks")]
    pub grid_row_tracks: GridTracks,
}

impl AutoLayout {
    /// Default auto-layout for new frames created in Studio (version 1: CSS-aligned, borders inside the box).
    pub fn v1() -> Self {
        Self { layout_version: 1, ..Self::default() }
    }

    /// Effective `[top, right, bottom, left]` inset used by the layout solver.
    ///
    /// In version 0 (`layout_version == 0`), when `stroke_included` (`bordersTakeSpace`) is true,
    /// the frame's inward stroke widths add to padding. In version 1 and up (`layout_version >= 1`),
    /// strokes never add inset space (borders sit inside the box without pushing children).
    pub fn effective_padding(&self, stroke: &Stroke) -> [f32; 4] {
        if self.layout_version == 0 && self.stroke_included {
            let s = stroke.layout_inset();
            [self.padding[0] + s[0], self.padding[1] + s[1], self.padding[2] + s[2], self.padding[3] + s[3]]
        } else {
            self.padding
        }
    }

    /// Returns a copy of `self` with `padding` replaced by [`Self::effective_padding`].
    pub fn with_stroke(&self, stroke: &Stroke) -> Self {
        Self { padding: self.effective_padding(stroke), ..self.clone() }
    }

    /// Whether the primary axis (and `primary_sizing`) runs along the width. Figma's convention: a
    /// `Horizontal` frame and a `Grid` frame (columns) do, a `Vertical` frame runs along the height.
    pub fn primary_is_horizontal(&self) -> bool {
        self.direction != LayoutDirection::Vertical
    }

    /// The sizing mode this frame has along the width (`horizontal`) or the height.
    pub fn sizing_along(&self, horizontal: bool) -> Sizing {
        if self.primary_is_horizontal() == horizontal {
            self.primary_sizing
        } else {
            self.counter_sizing
        }
    }
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
    /// Per-child margins `[top, right, bottom, left]` in an auto-layout parent (`stackChildMarginTop/Right/Bottom/Left`, Kiwi 606..609).
    #[serde(default, skip_serializing_if = "is_zero_radii")]
    pub margin: [f32; 4],
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

#[cfg(test)]
mod auto_layout_axis_tests {
    use super::*;

    #[test]
    fn primary_runs_along_the_width_for_horizontal_and_grid_frames_and_along_the_height_for_vertical() {
        let al = |direction| AutoLayout { direction, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Fixed, ..Default::default() };
        for direction in [LayoutDirection::Horizontal, LayoutDirection::Grid] {
            let a = al(direction);
            assert!(a.primary_is_horizontal(), "{direction:?}");
            assert_eq!((a.sizing_along(true), a.sizing_along(false)), (Sizing::Hug, Sizing::Fixed), "{direction:?}");
        }
        let v = al(LayoutDirection::Vertical);
        assert!(!v.primary_is_horizontal());
        assert_eq!((v.sizing_along(true), v.sizing_along(false)), (Sizing::Fixed, Sizing::Hug));
    }
}
