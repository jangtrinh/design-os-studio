//! The canvas: renders the page into a texture, handles viewport gestures,
//! tools and interactions, and paints the selection overlays.

pub mod agent_changes_chip;
pub mod agent_overlay;
pub mod builder;
pub mod create;
pub mod draw;
pub mod gpu;
pub mod grids;
pub mod handles;
pub mod hit;
pub mod interact;
pub mod polyfill;
pub mod proto;
pub mod reorder;
pub mod rulers;
pub mod snap;
pub mod textsession;
pub mod tiles;
pub mod tools;
pub mod vector;

use crate::app::App;
use crate::edit::Tx;
use designos_doc::{Affine, NodeId, Rect, Vec2 as DV};
use egui::{Context, Rect as ERect, Sense, Ui};
use handles::Handle;

pub struct Pending {
    pub node: NodeId,
    pub start_screen: egui::Pos2,
    pub start_page: DV,
    pub shift: bool,
    pub was_selected: bool,
}

pub struct MoveState {
    pub tx: Tx,
    pub ids: Vec<NodeId>,
    pub start: DV,
    pub orig_bbox: Rect,
    pub targets: Vec<Rect>,
    pub moved: bool,
    /// Auto-layout reorder target while dragging a child inside its frame.
    pub reorder: Option<reorder::Insertion>,
    /// `Some` while dragging Alt-copies (`ids` are the copies). Holds the slots whose limit a copy
    /// breaks if it stays in them; the verdict is taken at release, once the drop is known.
    pub dup_blocked: Option<Vec<(NodeId, designos_doc::SlotConstraintError)>>,
}

pub enum ResizeKind {
    Single { id: NodeId, world0: Affine, t0: Affine, size0: DV },
    Multi { bbox0: Rect, nodes: Vec<(NodeId, Rect, Affine, DV)> },
}

pub struct ResizeState {
    pub tx: Tx,
    pub handle: Handle,
    pub kind: ResizeKind,
    pub targets: Vec<Rect>,
}

pub struct RotateState {
    pub tx: Tx,
    pub id: NodeId,
    pub center: DV,
    pub start_angle: f32,
    pub t0: Affine,
    pub size: DV,
    pub rot0: f32,
    /// Multi-selection: every rotated node with its world transform at drag start.
    pub group: Vec<(NodeId, Affine)>,
}

pub struct DrawState {
    pub tx: Tx,
    pub tool: tools::Tool,
    pub id: NodeId,
    pub parent: NodeId,
    pub start: DV,
    pub last_page: DV,
    pub targets: Vec<Rect>,
    pub dragged: bool,
}

pub struct CornerRadiusDragState {
    pub tx: Tx,
    pub id: NodeId,
    pub corner: usize,
    pub start_radii: [f32; 4],
}

pub struct AutoLayoutDragState {
    pub tx: Tx,
    pub id: NodeId,
    pub kind: handles::AutoLayoutHandleKind,
    pub start_page: DV,
    pub start_value: f32,
    pub moved: bool,
}

pub struct SmartSelectionDragState {
    pub tx: Tx,
    pub kind: handles::SmartSelectionHandleKind,
    pub start_page: DV,
    /// Sorted `(id, world_aabb, start_tx, start_ty)` along the primary axis at drag start.
    pub sorted: Vec<(NodeId, Rect, f32, f32)>,
    pub start_gap: f32,
}

#[derive(Clone, Debug)]
pub struct AutoLayoutInlineEdit {
    pub id: NodeId,
    pub kind: handles::AutoLayoutHandleKind,
    pub anchor_page: DV,
    pub text: String,
    pub request_focus: bool,
}

pub struct ShapeControlDragState {
    pub tx: Tx,
    pub id: NodeId,
    pub kind: handles::ShapeControlKind,
    pub start_page: DV,
    pub start_count: u32,
    pub start_ratio: f32,
    pub start_end_angle: f32,
}

#[derive(Default)]
pub enum Interaction {
    #[default]
    None,
    Pan,
    Pending(Pending),
    Marquee {
        start: DV,
        cur: DV,
        base: Vec<NodeId>,
    },
    ZoomMarquee {
        start: DV,
        cur: DV,
        alt: bool,
    },
    Move(MoveState),
    Resize(ResizeState),
    Rotate(RotateState),
    CornerRadius(CornerRadiusDragState),
    AutoLayoutDrag(AutoLayoutDragState),
    SmartSelectionDrag(SmartSelectionDragState),
    ShapeControl(ShapeControlDragState),
    Draw(DrawState),
    Pencil(Vec<DV>),
    PenDrag,
    /// Dragging a vertex/handle in vector edit mode (state lives in `CanvasState::vedit`).
    VectorDrag,
    /// Brushing over regions with the shape builder (state lives in `CanvasState::builder`).
    Builder,
    /// Creating or moving a ruler guide.
    Guide(rulers::GuideDrag),
    /// Dragging a connector out of a board node (or from empty canvas).
    Connect(crate::board::connect::ConnectDrag),
}

pub use textsession::TextSession;

pub struct PenState {
    pub pts: Vec<create::PenPoint>,
}

#[derive(Default)]
pub struct CanvasState {
    /// The document changed since the last paint (bumps the tile revision).
    pub dirty: bool,
    /// Tile cache and per-tile textures.
    pub tiles: tiles::TileView,
    /// GPU canvas renderer state (preference, availability, caches).
    pub gpu: gpu::GpuView,
    pub inter: Interaction,
    pub guides: Vec<snap::Guide>,
    pub measures: Vec<snap::Measure>,
    pub pen: Option<PenState>,
    pub text: Option<TextSession>,
    /// Point editing of a Vector node.
    pub vedit: Option<vector::VectorEdit>,
    /// The shape builder session.
    pub builder: Option<builder::Builder>,
    /// On-canvas inline input pill for Auto-Layout padding/gap single-click (`S1-08`).
    pub al_inline: Option<AutoLayoutInlineEdit>,
    pub pointer_page: Option<DV>,
    pub cursor: Option<egui::CursorIcon>,
    pub last_render_ms: f32,
    /// Cached `(pointer_screen_pos, tile_rev, deep_mod, zoom)` for hover hit-test deduplication.
    pub last_hover_query: Option<(egui::Pos2, u64, bool, f32)>,
}

/// Fast axis-aligned bounding box of `(0, 0, size.x, size.y)` transformed by `t` with zero heap allocations.
#[inline]
pub fn affine_rect_aabb(t: &Affine, size: DV) -> Rect {
    if t.b == 0.0 && t.c == 0.0 && t.a >= 0.0 && t.d >= 0.0 {
        Rect::new(t.tx, t.ty, t.a * size.x, t.d * size.y)
    } else {
        let p0 = t.apply(DV::ZERO);
        let p1 = t.apply(DV::new(size.x, 0.0));
        let p2 = t.apply(DV::new(size.x, size.y));
        let p3 = t.apply(DV::new(0.0, size.y));
        let x0 = p0.x.min(p1.x).min(p2.x).min(p3.x);
        let y0 = p0.y.min(p1.y).min(p2.y).min(p3.y);
        let x1 = p0.x.max(p1.x).max(p2.x).max(p3.x);
        let y1 = p0.y.max(p1.y).max(p2.y).max(p3.y);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// Computes `doc.world_transform(id)` on the stack (no heap `Vec` allocation).
#[inline]
pub fn fast_world_transform(doc: &designos_doc::Document, id: NodeId) -> Affine {
    let Some(first) = doc.get(id) else {
        return Affine::IDENTITY;
    };
    let Some(mut cur_parent) = first.parent else {
        return first.transform;
    };
    let mut stack = [Affine::IDENTITY; 32];
    stack[0] = first.transform;
    let mut len = 1usize;
    while let Some(n) = doc.get(cur_parent) {
        if matches!(n.kind, designos_doc::NodeKind::Page { .. }) {
            break;
        }
        if len >= stack.len() {
            return doc.world_transform(id);
        }
        stack[len] = n.transform;
        len += 1;
        let Some(p) = n.parent else { break };
        cur_parent = p;
    }
    let mut acc = Affine::IDENTITY;
    for i in (0..len).rev() {
        acc = acc * stack[i];
    }
    acc
}

/// Computes page-space corners of `id` without heap allocation.
#[inline]
pub fn fast_world_corners(doc: &designos_doc::Document, id: NodeId) -> [DV; 4] {
    let Some(n) = doc.get(id) else {
        return [DV::ZERO; 4];
    };
    let w = fast_world_transform(doc, id);
    [w.apply(DV::ZERO), w.apply(DV::new(n.size.x, 0.0)), w.apply(DV::new(n.size.x, n.size.y)), w.apply(DV::new(0.0, n.size.y))]
}

/// Fast page-space AABB for `id`: uses `index.bounds_of(id)` when exact or stack-based `fast_world_transform`
/// without any `O(subtree)` recursion or heap allocation.
#[inline]
pub fn fast_world_aabb(doc: &designos_doc::Document, index: Option<&designos_render::SceneIndex>, id: NodeId) -> Rect {
    let Some(n) = doc.get(id) else {
        if let Some((x0, y0, x1, y1)) = index.and_then(|idx| idx.bounds_of(id)) {
            return Rect::new(x0, y0, x1 - x0, y1 - y0);
        }
        return Rect::default();
    };
    if let Some(idx) = index {
        let exact_in_index = n.effects.is_empty()
            && n.stroke.paints.is_empty()
            && !matches!(n.kind, designos_doc::NodeKind::Text(_) | designos_doc::NodeKind::Sticky(_))
            && n.children.is_empty();
        if exact_in_index {
            if let Some((x0, y0, x1, y1)) = idx.bounds_of(id) {
                return Rect::new(x0, y0, x1 - x0, y1 - y0);
            }
        }
    }
    affine_rect_aabb(&fast_world_transform(doc, id), n.size)
}

impl CanvasState {
    /// Drops all in-flight state without touching any document (used when the
    /// document itself is being replaced).
    pub fn reset_interaction(&mut self) {
        self.inter = Interaction::None;
        self.guides.clear();
        self.measures.clear();
        self.pen = None;
        self.text = None;
        self.vedit = None;
        self.builder = None;
        self.al_inline = None;
        self.last_hover_query = None;
        // the page or the whole document may be different now
        self.tiles.invalidate_all();
    }
}

impl App {
    /// Cancels the running interaction, restoring the document.
    pub fn cancel_interaction(&mut self) {
        let inter = std::mem::take(&mut self.canvas.inter);
        match inter {
            Interaction::Move(m) => m.tx.cancel(&mut self.doc),
            Interaction::Resize(r) => r.tx.cancel(&mut self.doc),
            Interaction::Rotate(r) => r.tx.cancel(&mut self.doc),
            Interaction::CornerRadius(c) => c.tx.cancel(&mut self.doc),
            Interaction::AutoLayoutDrag(a) => a.tx.cancel(&mut self.doc),
            Interaction::SmartSelectionDrag(s) => s.tx.cancel(&mut self.doc),
            Interaction::ShapeControl(s) => s.tx.cancel(&mut self.doc),
            Interaction::Draw(d) => d.tx.cancel(&mut self.doc),
            Interaction::Connect(c) => {
                c.tx.cancel(&mut self.doc);
                self.board.target = None;
            }
            Interaction::VectorDrag => {
                if let Some(v) = self.canvas.vedit.as_mut() {
                    v.marquee = None;
                    if let Some(d) = v.drag.take() {
                        d.tx.cancel(&mut self.doc);
                    }
                }
                vector::reload(self);
            }
            Interaction::Builder => {
                if let Some(b) = self.canvas.builder.as_mut() {
                    b.brush = None;
                }
            }
            _ => {}
        }
        self.canvas.guides.clear();
        self.canvas.measures.clear();
        self.canvas.pen = None;
        self.canvas.al_inline = None;
        self.canvas.last_hover_query = None;
        if let Some(t) = self.canvas.text.take() {
            t.tx.cancel(&mut self.doc);
        }
        self.changed();
        self.sanitize();
    }

    /// Switches tool, finishing any pen path or text edit in progress.
    pub fn set_tool(&mut self, tool: tools::Tool) {
        interact::finish_pen(self);
        interact::finish_text(self, true);
        self.cancel_interaction();
        builder::finish(self);
        self.tool = tool;
        if tool == tools::Tool::ShapeBuilder && !builder::start(self) {
            self.tool = tools::Tool::Move;
        }
    }
}

pub fn show(app: &mut App, ui: &mut Ui, rect: ERect) {
    app.canvas_rect = rect;
    if !app.first_fit_done {
        app.first_fit_done = true;
        match app.startup_zoom.take() {
            Some(z) => app.zoom_on_content(z),
            None if app.is_deck() => {
                if let Some(id) = app.active_slide() {
                    app.select_slide(id);
                } else {
                    app.fit_all();
                }
            }
            None => app.fit_all(),
        }
    }
    if app.is_board() {
        // the dotted board under the (transparent) page tiles
        crate::board::canvas::background(app, ui, rect);
    }
    tiles::paint(app, ui, rect);
    let resp = ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag());
    // While the colour picker waits for an eyedropper click the canvas must not react.
    let eyedropping = app.props.picker.as_ref().is_some_and(|p| p.eyedrop);
    // An old version shown from the version history is read-only.
    let read_only = app.history.viewing.is_some();
    if !eyedropping && !read_only && !proto::process(app, ui, rect, &resp) {
        interact::process(app, ui, rect, &resp);
    }
    crate::comments::pins::process(app, rect, &resp);
    grids::paint(app, ui, rect);
    draw::overlays(app, ui, rect);
    agent_overlay::paint(app, ui, rect);
    agent_changes_chip::paint(app, ui, rect);
    draw::empty_slot_picker_overlay(app, ui, rect);
    interact::auto_layout_inline_overlay(app, ui, rect);
    vector::toolbar(app, ui, rect);
    rulers::overlay(app, ui, rect);
    proto::overlays(app, ui, rect);
    crate::panels::inspect::measure_overlay(app, ui, rect);
    crate::dev::annot::overlay(app, ui, rect);
    if resp.hovered() || !matches!(app.canvas.inter, Interaction::None) {
        if let Some(c) = app.canvas.cursor {
            ui.ctx().set_cursor_icon(c);
        }
    }
    if proto::active(app) && (app.proto.hot.is_some() || app.proto.connect.is_some()) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // Comment pins and popovers sit above everything painted on the canvas.
    crate::comments::pins::overlay(app, ui, rect);
}
