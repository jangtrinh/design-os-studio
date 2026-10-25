//! In-app clipboard: nodes (with subtrees) serialized as JSON, plus subtree cloning,
//! SVG markup pasted as text, and Figma clipboard payloads pasted as HTML.

use crate::app::App;
use crate::edit::Tx;
use crate::layout;
use designos_doc::{create_instance, Affine, Command, Document, Node, NodeId, NodeKind, PropKind, PropValue, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Pastes `text` if it is SVG markup (`<svg` or `<?xml` prolog): the layers
/// appear centered in the viewport, as one undo step, selected. Returns whether
/// the text was SVG (even if it failed to import, then a toast says why), so
/// the caller can fall back to the in-app clipboard otherwise.
pub fn paste_svg(app: &mut App, text: &str) -> bool {
    if !designos_svg::looks_like_svg(text) {
        return false;
    }
    let view = app.canvas_rect;
    let center = app.view.to_page(view.min, view.center());
    let parent = app.container_for_insert();
    if let Err(e) = place_svg(app, text, "svg", center, parent) {
        app.show_toast(e, true);
    }
    true
}

/// The system clipboard's `text/html`, only when it carries a Figma payload. `egui` hands the app text
/// only, so the paste handlers ask the pasteboard for the HTML flavour themselves.
pub fn system_figma_html() -> Option<String> {
    let html = arboard::Clipboard::new().ok()?.get().html().ok()?;
    designos_fig::looks_like_figma_clipboard(&html).then_some(html)
}

/// Tells a Figma-HTML-only paste apart from the key events egui-winit really produces.
///
/// egui-winit (0.35, `src/lib.rs:1000-1015`) turns a V press with the command modifier into `Event::Paste(text)`
/// when the clipboard has plain text and into nothing at all otherwise, and never forwards that press as an
/// `Event::Key`. A Figma copy can carry HTML alone, so the only trace of such a paste is the V *release*
/// (`lib.rs:1018`; winit still delivers a keyUp while Cmd is held, `platform_impl/macos/app.rs:25-34`).
/// Shift or Alt change nothing: egui-winit ignores them for paste, so Shift+Cmd+V is the normal paste too.
///
/// Feed every key event of a frame through [`PasteChord::on_event`] and call [`PasteChord::begin_frame`] once
/// per frame; `on_event` returns true when the clipboard HTML must be read now.
#[derive(Default, Debug)]
pub struct PasteChord {
    /// A plain V press arrived (a tool shortcut), so its release is not the end of a paste chord.
    plain_v_down: bool,
    /// An `Event::Paste` already handled the chord, so its release must not paste again.
    text_paste_seen: bool,
    /// Whether the command modifier was down in the previous frame; Cmd may be let go before V.
    command_last_frame: bool,
    command_now: bool,
}

impl PasteChord {
    pub fn begin_frame(&mut self, command_down: bool) {
        self.command_last_frame = self.command_now;
        self.command_now = command_down;
    }

    pub fn on_event(&mut self, ev: &egui::Event) -> bool {
        match ev {
            egui::Event::Paste(_) => {
                self.text_paste_seen = true;
                false
            }
            egui::Event::WindowFocused(false) => {
                // a release that happens while unfocused never arrives
                self.plain_v_down = false;
                self.text_paste_seen = false;
                false
            }
            egui::Event::Key { key: egui::Key::V, pressed: true, .. } => {
                self.plain_v_down = true;
                false
            }
            egui::Event::Key { key: egui::Key::V, pressed: false, modifiers, .. } => {
                let plain = std::mem::take(&mut self.plain_v_down);
                let handled = std::mem::take(&mut self.text_paste_seen);
                !plain && !handled && (modifiers.command || self.command_last_frame)
            }
            _ => false,
        }
    }
}

/// Pastes `html` if it carries a Figma clipboard payload: the copied layers appear centered in the
/// viewport, as one undo step, selected. Returns whether the HTML was a Figma payload (even when it
/// failed to import, then a toast says why), so the caller can fall back to text otherwise.
pub fn paste_figma(app: &mut App, html: &str) -> bool {
    if !designos_fig::looks_like_figma_clipboard(html) {
        return false;
    }
    if let Err(e) = place_figma(app, html) {
        app.show_toast(e, true);
    }
    true
}

/// Decodes a Figma payload and appends its top-level layers to the insert container, centered on the
/// viewport (rounded to whole pixels), as one undoable step. Returns the new top-level layers.
///
/// es-debt: styles, variables, prototype links and library components of the payload are not merged
/// into the document, so those references are dropped (resolved paint, text and effect values stay on
/// the layers); upgrade when a paste must keep shared styles or flows.
fn place_figma(app: &mut App, html: &str) -> Result<Vec<NodeId>, String> {
    let payload = designos_fig::parse_figma_clipboard_html(html).map_err(|e| format!("Cannot paste from Figma: {e}"))?;
    let frag = designos_fig::import_figma_clipboard(&payload).map_err(|e| format!("Cannot paste from Figma: {e}"))?;
    let roots: Vec<NodeId> = frag.pages.iter().filter_map(|p| frag.get(*p)).flat_map(|p| p.children.iter().copied()).collect();
    let mut clip = capture(&frag, &roots).ok_or("The Figma clipboard holds no layers")?;
    // Ids of the payload and of this document are unrelated: a connector end outside the payload must not
    // fall back to a same-numbered node here.
    let inside: std::collections::HashSet<NodeId> = clip.nodes.iter().map(|n| n.id).collect();
    for n in &mut clip.nodes {
        if let NodeKind::Connector(c) = &mut n.kind {
            for e in [&mut c.start, &mut c.end] {
                e.node = e.node.filter(|t| inside.contains(t));
            }
        }
    }

    let parent = app.container_for_insert();
    let view = app.canvas_rect;
    let target = app.view.to_page(view.min, view.center());

    let (mut lists, map) = instantiate_mapped(&mut app.doc, &clip);
    let mut new_roots = Vec::new();
    let mut dropped_refs = 0usize;
    for nodes in &mut lists {
        for n in nodes.iter_mut() {
            dropped_refs += detach_foreign_refs(n, &map);
        }
        if let Some(root) = nodes.first() {
            new_roots.push(root.id);
        }
    }
    let slot_roots: Vec<&Node> = lists.iter().filter_map(|l| l.first()).collect();
    if let Some(err) = crate::ops::slot_edit_error(&app.doc, &[], Some((parent, &slot_roots))) {
        return Err(err.to_string());
    }
    let mut tx = Tx::new();
    // Image bytes go through the same undo step as the layers that reference them.
    if !frag.images.is_empty() {
        if let Err(e) = tx.cmd(&mut app.doc, Command::SetImages { add: frag.images, remove: Vec::new() }) {
            tx.cancel(&mut app.doc);
            return Err(format!("Cannot paste from Figma: {e}"));
        }
    }
    for nodes in lists {
        let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
        if let Err(e) = tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes }) {
            tx.cancel(&mut app.doc);
            return Err(format!("Cannot paste from Figma: {e}"));
        }
    }
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    // Figma already laid the payload out, so its layers keep their sizes; only the ancestors (an auto-layout
    // or group parent) re-flow around them.
    for r in &new_roots {
        layout::relayout_up(&mut app.doc, &mut tx.ed, *r);
    }
    // An auto-layout parent owns the position of its children, so there the layers stay where it puts them.
    let parent_laid_out = app.doc.get(parent).and_then(|p| p.kind.frame_props()).is_some_and(|f| f.auto_layout.is_some());
    if let (false, Some(placed)) = (parent_laid_out, crate::util::union_all(new_roots.iter().map(|r| crate::util::world_aabb(&app.doc, *r)))) {
        let shift = Vec2::new((target.x - (placed.x + placed.w / 2.0)).round(), (target.y - (placed.y + placed.h / 2.0)).round());
        let shift = crate::util::delta_to_parent(&app.doc, new_roots[0], shift);
        for r in &new_roots {
            if let Some(n) = tx.ed.node(&mut app.doc, *r) {
                n.transform.tx += shift.x;
                n.transform.ty += shift.y;
            }
        }
    }
    app.run_tx(tx);
    app.selection = new_roots.clone();
    app.fonts.load_for(&app.doc);
    if dropped_refs > 0 {
        app.show_toast(format!("Pasted from Figma ({dropped_refs} unknown component or slot reference(s) dropped)"), false);
    }
    Ok(new_roots)
}

/// Rewrites references that pointed into the payload's document: component links follow `map` to the
/// pasted copies, anything else (styles, variable bindings, prototype links) is dropped.
/// Returns the number of unknown component, slot, or instance-swap target ids that were dropped.
fn detach_foreign_refs(n: &mut Node, map: &HashMap<NodeId, NodeId>) -> usize {
    let mut dropped = 0usize;
    if let NodeKind::Instance { component, .. } = &mut n.kind {
        if let Some(c) = *component {
            if let Some(&mapped) = map.get(&c) {
                *component = Some(mapped);
            } else {
                *component = None;
                dropped += usize::from(c.0 != 0);
            }
        }
    }
    n.component.source = n.component.source.and_then(|s| map.get(&s).copied());
    for prop in &mut n.component.props {
        if let PropKind::InstanceSwap { preferred } = &mut prop.kind {
            preferred.retain_mut(|id| {
                if let Some(&mapped) = map.get(id) {
                    *id = mapped;
                    true
                } else {
                    dropped += usize::from(id.0 != 0);
                    false
                }
            });
        }
        remap_foreign_prop_value(&mut prop.default, map, &mut dropped);
    }
    for val in n.component.values.values_mut() {
        remap_foreign_prop_value(val, map, &mut dropped);
    }
    for cfg in n.component.slot_configs.values_mut() {
        cfg.preferred.retain_mut(|id| {
            if let Some(&mapped) = map.get(id) {
                *id = mapped;
                true
            } else {
                dropped += usize::from(id.0 != 0);
                false
            }
        });
    }
    n.style_refs.clear();
    n.variables = Default::default();
    n.prototype.reactions.clear();
    n.prototype.fixed_children = n.prototype.fixed_children.iter().filter_map(|c| map.get(c).copied()).collect();
    dropped
}

fn remap_foreign_prop_value(val: &mut PropValue, map: &HashMap<NodeId, NodeId>, dropped: &mut usize) {
    match val {
        PropValue::Instance(id) => {
            if id.0 != 0 {
                if let Some(&mapped) = map.get(id) {
                    *id = mapped;
                } else {
                    *id = NodeId(0);
                    *dropped += 1;
                }
            }
        }
        PropValue::Slot(ids) => {
            ids.retain_mut(|id| {
                if let Some(&mapped) = map.get(id) {
                    *id = mapped;
                    true
                } else {
                    *dropped += usize::from(id.0 != 0);
                    false
                }
            });
        }
        PropValue::Bool(_) | PropValue::Text(_) | PropValue::Variant(_) => {}
    }
}

/// Page point a dropped file lands on: screen position `at` (the drop point; the viewport center when it is outside
/// the canvas), moved 24 px down and right for each file `nth` dropped before it.
fn drop_point(app: &App, at: Option<egui::Pos2>, nth: usize) -> Vec2 {
    let view = app.canvas_rect;
    let screen = at.filter(|p| view.contains(*p)).unwrap_or(view.center());
    let mut p = app.view.to_page(view.min, screen);
    p.x += 24.0 * nth as f32;
    p.y += 24.0 * nth as f32;
    p
}

/// Adds a dropped `.svg` file's markup to `tx`, centered on the drop point, into the frame under that point. `nth`
/// staggers several files dropped together. Shows a toast and returns `None` when the markup cannot be placed.
pub fn drop_svg(app: &mut App, tx: &mut Tx, svg: &str, name: &str, at: Option<egui::Pos2>, nth: usize) -> Option<NodeId> {
    let p = drop_point(app, at, nth);
    let parent = crate::canvas::create::container_at(&app.doc, app.page, p, &[]);
    match place_svg_in(app, tx, svg, name, p, parent) {
        Ok(id) => Some(id),
        Err(e) => {
            app.show_toast(e, true);
            None
        }
    }
}

/// Adds a dropped PNG, JPEG or WebP file to `tx` as an image layer, through the Image tool's placement: into the frame
/// under the drop point, centered on it (moved inside a frame that clips its content), at most 400 px on its longest
/// side. `nth` staggers several files dropped together, as for SVG. Shows a toast and returns `None` when the file is
/// not a picture.
pub fn drop_image(app: &mut App, tx: &mut Tx, bytes: Vec<u8>, name: &str, at: Option<egui::Pos2>, nth: usize) -> Option<NodeId> {
    let landing = crate::paste_placement::drop_landing(app, drop_point(app, at, nth));
    crate::paste_placement::insert_image_at(app, tx, &landing, name, bytes)
}

/// Ends a drop of several files: everything `tx` gathered is one undo step, and the placed layers are selected.
pub fn commit_dropped(app: &mut App, tx: Tx, placed: Vec<NodeId>, any_image: bool) {
    if placed.is_empty() {
        tx.cancel(&mut app.doc);
        return;
    }
    if !app.run_tx(tx) {
        return;
    }
    app.selection = placed;
    if any_image {
        app.tool = crate::canvas::tools::Tool::Move;
    }
    app.fonts.load_for(&app.doc);
}

/// Imports SVG markup as a Frame named `name` centered on page point `center`
/// (rounded to whole pixels), appended to `parent`'s children as one undoable
/// step, and selects it.
pub fn place_svg(app: &mut App, svg: &str, name: &str, center: Vec2, parent: NodeId) -> Result<NodeId, String> {
    let mut tx = Tx::new();
    let root = place_svg_in(app, &mut tx, svg, name, center, parent)?;
    app.run_tx(tx);
    app.selection = vec![root];
    app.fonts.load_for(&app.doc);
    Ok(root)
}

/// [`place_svg`] through `tx`: the caller commits it and selects the layer. Nothing is changed on `Err`.
fn place_svg_in(app: &mut App, tx: &mut Tx, svg: &str, name: &str, center: Vec2, parent: NodeId) -> Result<NodeId, String> {
    let mut out = designos_svg::svg_to_nodes(svg, name, &mut || app.doc.alloc_id()).map_err(|e| format!("Cannot import SVG: {e}"))?;
    if let Some(err) = crate::ops::slot_edit_error(&app.doc, &[], Some((parent, &[&out.nodes[0]]))) {
        return Err(err.to_string());
    }
    let inv = app.doc.world_transform(parent).invert().ok_or("Cannot place into a collapsed layer")?;
    let c = inv.apply(center);
    let size = out.nodes[0].size;
    out.nodes[0].transform = Affine::translate((c.x - size.x / 2.0).round(), (c.y - size.y / 2.0).round());
    let root = out.nodes[0].id;
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: out.nodes }).map_err(|e| e.to_string())?;
    app.doc.images.append(&mut out.images);
    App::stretch_into_slot_if_configured(&mut app.doc, &mut tx.ed, parent, root);
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    layout::relayout_around(&mut app.doc, &mut tx.ed, parent);
    Ok(root)
}

const MAGIC: u32 = 0xDE51_6A01;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clip {
    magic: u32,
    pub roots: Vec<NodeId>,
    pub nodes: Vec<Node>,
    /// The document the layers were copied from. A clip from an older build has none, so it never links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<Origin>,
}

/// An open document: which Studio process it is open in, and which document there (its runtime id, which a loaded
/// or new document gets afresh and a clone keeps). The clip travels through the system clipboard, so another
/// Studio window's document that happens to have the same runtime id must not pass for this one.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
struct Origin {
    process: u64,
    document: u64,
}

impl Origin {
    fn of(doc: &Document) -> Self {
        static PROCESS: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        let process = *PROCESS.get_or_init(|| {
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
            (nanos ^ u64::from(std::process::id()).rotate_left(32)).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        });
        Self { process, document: doc.runtime_id() }
    }
}

/// Serializes `ids` (and their subtrees) from `doc`.
pub fn capture(doc: &Document, ids: &[NodeId]) -> Option<Clip> {
    let mut nodes = Vec::new();
    for id in ids {
        doc.walk(*id, |n, _| nodes.push(n.clone()));
    }
    if nodes.is_empty() {
        return None;
    }
    Some(Clip { magic: MAGIC, roots: ids.to_vec(), nodes, origin: Some(Origin::of(doc)) })
}

pub fn to_json(clip: &Clip) -> Option<String> {
    designos_doc::finite_json::to_string_finite(clip).ok().map(|(s, _)| s)
}

pub fn from_json(s: &str) -> Option<Clip> {
    let (c, _): (Clip, _) = designos_doc::finite_json::from_str_repairing(s).ok()?;
    (c.magic == MAGIC && !c.roots.is_empty()).then_some(c)
}

/// Creates fresh-id copies of the clip: one root-first node list per root.
/// Roots have `parent = None`; the Insert command assigns the new parent.
pub fn instantiate(doc: &mut Document, clip: &Clip) -> Vec<Vec<Node>> {
    instantiate_mapped(doc, clip).0
}

/// [`instantiate`] plus the old-id -> new-id map of every copied node.
fn instantiate_mapped(doc: &mut Document, clip: &Clip) -> (Vec<Vec<Node>>, HashMap<NodeId, NodeId>) {
    let mut map: HashMap<NodeId, NodeId> = HashMap::new();
    for n in &clip.nodes {
        map.insert(n.id, doc.alloc_id());
    }
    let by_id: HashMap<NodeId, &Node> = clip.nodes.iter().map(|n| (n.id, n)).collect();
    let mut out = Vec::new();
    for root in &clip.roots {
        let mut list = Vec::new();
        let mut stack = vec![*root];
        while let Some(id) = stack.pop() {
            let Some(src) = by_id.get(&id) else { continue };
            let mut n = (*src).clone();
            n.id = map[&id];
            n.source_id = None;
            // a slot content guid names one node; copies must not share it
            n.component.slot_content_id = None;
            // connectors copied together with their nodes join the copies; a connector copied alone
            // keeps its nodes while they exist, and is left loose (at its last point) otherwise
            if let NodeKind::Connector(c) = &mut n.kind {
                for e in [&mut c.start, &mut c.end] {
                    e.node = e.node.and_then(|t| map.get(&t).copied().or_else(|| doc.get(t).map(|_| t)));
                }
            }
            n.children = src.children.iter().filter_map(|c| map.get(c).copied()).collect();
            n.parent = if id == *root { None } else { src.parent.and_then(|p| map.get(&p).copied()) };
            stack.extend(src.children.iter().rev().copied());
            list.push(n);
        }
        out.push(list);
    }
    (out, map)
}

/// The main component a copied root stands for in `doc`, when `doc` is the file it was copied from.
struct MainLink {
    /// The main an instance of the root is linked to: the root itself, or a set's default variant (its first one,
    /// as Assets lists it).
    comp: NodeId,
    /// The layer that holds `comp` and its set, if any: an instance cannot go inside it. A variant's is its set.
    scope: NodeId,
}

/// The main `root` is in `doc` right now: a standalone `Component`, a variant, or a `ComponentSet` (through its
/// default variant). `None` for anything else.
fn live_main(doc: &Document, root: NodeId) -> Option<MainLink> {
    let live = doc.get(root)?;
    match &live.kind {
        NodeKind::Component(_) => {
            let in_set = live.parent.filter(|p| matches!(doc.get(*p).map(|n| &n.kind), Some(NodeKind::ComponentSet(_))));
            Some(MainLink { comp: root, scope: in_set.unwrap_or(root) })
        }
        NodeKind::ComponentSet(_) => {
            let comp = live.children.iter().copied().find(|c| matches!(doc.get(*c).map(|n| &n.kind), Some(NodeKind::Component(_))))?;
            Some(MainLink { comp, scope: root })
        }
        _ => None,
    }
}

/// Whether `id` is a main component or a component set in `doc`.
pub fn is_main(doc: &Document, id: NodeId) -> bool {
    live_main(doc, id).is_some()
}

/// For each root of `clip`, `Some` when it was a main component (or a component set) and still is, in the document
/// it was copied from. A clip pasted into another file, a main since cut or deleted, and a layer made a main after it
/// was copied all give `None`: those copy for real.
fn main_links(doc: &Document, clip: &Clip) -> Vec<Option<MainLink>> {
    let live: Vec<Option<MainLink>> = if clip.origin == Some(Origin::of(doc)) { clip.roots.iter().map(|r| live_main(doc, *r)).collect() } else { Vec::new() };
    if live.iter().all(Option::is_none) {
        return clip.roots.iter().map(|_| None).collect();
    }
    // One pass over the clip, and only when some root is a live main: what the roots were when they were copied.
    let roots: HashSet<NodeId> = clip.roots.iter().copied().collect();
    let was: HashMap<NodeId, &Node> = clip.nodes.iter().filter(|n| roots.contains(&n.id)).map(|n| (n.id, n)).collect();
    let kind_of = |id: NodeId| was.get(&id).map(|n| std::mem::discriminant(&n.kind));
    clip.roots.iter().zip(live).map(|(root, link)| link.filter(|_| kind_of(*root) == doc.get(*root).map(|n| std::mem::discriminant(&n.kind)))).collect()
}

/// Whether an instance of the main `link` can sit in `parent` without the file coming apart: not in a component
/// set (a copy there is a variant), not in the main or set itself, not in a layer of one of its own instances (the
/// next edit of the main removes it), and not in a main whose content holds this one, which would make that main
/// hold itself.
fn instance_fits(doc: &Document, link: &MainLink, parent: NodeId) -> bool {
    let inside = |outer: NodeId| parent == outer || doc.is_ancestor(outer, parent);
    if matches!(doc.get(parent).map(|n| &n.kind), Some(NodeKind::ComponentSet(_))) || inside(link.scope) {
        return false;
    }
    let mut at = Some(parent);
    while let Some(n) = at.and_then(|id| doc.get(id)) {
        if designos_doc::instance_component(n) == Some(link.comp) {
            return false;
        }
        at = n.parent;
    }
    !designos_doc::mains_held_by(doc, &[link.comp]).into_iter().any(inside)
}

/// Where a paste of `clip` goes when the insert container is `parent`: the same, unless that container would hold
/// itself. An instance cannot sit inside the main it links to, nor can a layer that holds an instance of a main sit
/// in that main, directly or through the mains in between. Such a paste goes beside the main, in its own parent.
pub fn paste_parent(doc: &Document, clip: &Clip, mut parent: NodeId) -> NodeId {
    // the mains a pasted layer holds an instance of (the ids a clip names, which may be another file's)
    let mut shut = designos_doc::mains_held_through(doc, clip.nodes.iter().filter_map(designos_doc::instance_component));
    for (root, link) in clip.roots.iter().zip(main_links(doc, clip)) {
        let Some(link) = link else { continue };
        if link.scope == *root {
            shut.push(*root);
        }
        shut.extend(designos_doc::mains_held_by(doc, &[link.comp]));
    }
    while let Some(main) = shut.iter().find(|m| parent == **m || doc.is_ancestor(**m, parent)) {
        match doc.get(*main).and_then(|n| n.parent) {
            Some(above) => parent = above,
            None => break,
        }
    }
    parent
}

/// [`instantiate`], except that in the file a main component (or a component set) was copied from, its copy is an
/// instance of it (of the set's default variant): the file keeps one main, as Figma does. `parent_of` gives the
/// parent each root will be inserted under, by the root's id in `clip`. A copy that would sit where an instance
/// cannot go (see [`instance_fits`]) stays a copy; in a component set that is how a variant is added.
/// The instance takes the place and visibility of the layer it replaces, and connectors copied along with it attach
/// to it instead of to the copy it replaces.
pub fn instantiate_linking_mains(doc: &mut Document, clip: &Clip, parent_of: impl Fn(&Document, NodeId) -> Option<NodeId>) -> Vec<Vec<Node>> {
    let (mut lists, copy_of) = instantiate_mapped(doc, clip);
    let links = main_links(doc, clip);
    let mut original_of: Option<HashMap<NodeId, NodeId>> = None;
    // layers of the copies that were replaced -> the layer that stands in for them (`None`: a free end)
    let mut stand_in: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    for ((root, link), list) in clip.roots.iter().zip(links).zip(lists.iter_mut()) {
        let Some(link) = link else { continue };
        let Some(parent) = parent_of(doc, *root) else { continue };
        if !instance_fits(doc, &link, parent) {
            continue;
        }
        let Command::Insert { nodes: mut instance, .. } = create_instance(doc, link.comp, parent, Vec2::ZERO) else { continue };
        if let (Some(copy), Some(root_instance)) = (list.first(), instance.first_mut()) {
            root_instance.transform = copy.transform;
            root_instance.visible = copy.visible;
        }
        let original_of = original_of.get_or_insert_with(|| copy_of.iter().map(|(original, copy)| (*copy, *original)).collect());
        let mirror_of: HashMap<NodeId, NodeId> = instance.iter().filter_map(|n| Some((n.component.source?, n.id))).collect();
        for gone in list.iter() {
            let original = original_of[&gone.id];
            stand_in.insert(gone.id, stand_in_for(doc, Some(original), original == *root, instance[0].id, &mirror_of));
        }
        *list = instance;
    }
    if !stand_in.is_empty() {
        for node in lists.iter_mut().flatten() {
            restand_connector(node, &stand_in);
        }
    }
    lists
}

/// The layer that stands in for a layer of a copy that is replaced: the new root for the copy's root, else the new
/// layer made from the same `original` (`new_of`). A layer with no counterpart is stood in by its original when that
/// still exists; `None` is a free end.
fn stand_in_for(doc: &Document, original: Option<NodeId>, is_root: bool, new_root: NodeId, new_of: &HashMap<NodeId, NodeId>) -> Option<NodeId> {
    if is_root {
        return Some(new_root);
    }
    let original = original?;
    new_of.get(&original).copied().or_else(|| doc.get(original).map(|_| original))
}

/// Points the connector `node` (any other node is left alone) at the layers that replaced the ones it ended on.
pub fn restand_connector(node: &mut Node, stand_in: &HashMap<NodeId, Option<NodeId>>) {
    let NodeKind::Connector(c) = &mut node.kind else { return };
    for end in [&mut c.start, &mut c.end] {
        if let Some(to) = end.node.and_then(|n| stand_in.get(&n)) {
            end.node = *to;
        }
    }
}

/// What replaces an Alt-copy at the drop (see [`recast_copy`]).
pub struct Recast {
    /// The nodes that stand in its place, root first.
    pub nodes: Vec<Node>,
    /// Each layer of the replaced copy, and the layer that stands in for it (`None`: a free end). For the connectors
    /// that were copied along and end on it.
    pub stand_in: HashMap<NodeId, Option<NodeId>>,
}

/// An Alt-drag makes its copy before it knows where the copy lands. At the drop, `copy` (made from the main `source`
/// and now under its final parent) is checked against the rules of [`instantiate_linking_mains`] again. Returns the
/// nodes that should stand in its place, with its transform, when they are not what it is now: an instance that
/// landed where it cannot go becomes a real copy, and a real copy that left its set becomes an instance. The layers
/// of `copy` go away, so [`Recast::stand_in`] says where the connectors that ended on them go.
pub fn recast_copy(doc: &mut Document, source: NodeId, copy: NodeId) -> Option<Recast> {
    let link = live_main(doc, source)?;
    let placed = doc.get(copy)?;
    let parent = placed.parent?;
    let is_instance = matches!(placed.kind, NodeKind::Instance { .. });
    if instance_fits(doc, &link, parent) == is_instance {
        return None;
    }
    let (transform, visible) = (placed.transform, placed.visible);
    // each layer of the copy, with the layer of the main it came from, and the new layers by the layer they came from
    let mut gone: Vec<(NodeId, Option<NodeId>)> = Vec::new();
    let (mut nodes, new_of) = if is_instance {
        // the layers of an instance mirror those of the main, which `component.source` names
        doc.walk(copy, |n, _| gone.push((n.id, n.component.source)));
        let clip = capture(doc, &[source])?;
        let (mut lists, made_from) = instantiate_mapped(doc, &clip);
        (lists.pop()?, made_from)
    } else {
        // a real copy was cloned from the main layer by layer, so both walk in the same order
        let (mut copy_layers, mut main_layers) = (Vec::new(), Vec::new());
        doc.walk(copy, |n, _| copy_layers.push(n.id));
        doc.walk(source, |n, _| main_layers.push(n.id));
        let same_shape = copy_layers.len() == main_layers.len();
        gone.extend(copy_layers.iter().enumerate().map(|(k, id)| (*id, same_shape.then(|| main_layers[k]))));
        let Command::Insert { nodes, .. } = create_instance(doc, link.comp, parent, Vec2::ZERO) else { return None };
        let mirrors = nodes.iter().filter_map(|n| Some((n.component.source?, n.id))).collect();
        (nodes, mirrors)
    };
    let new_root = nodes.first()?.id;
    let stand_in = gone.into_iter().map(|(layer, original)| (layer, stand_in_for(doc, original, layer == copy, new_root, &new_of))).collect();
    let root = nodes.first_mut()?;
    root.transform = transform;
    root.visible = visible;
    Some(Recast { nodes, stand_in })
}

/// Fresh-id deep copy of the subtree rooted at `id` (root first).
pub fn clone_subtree(doc: &mut Document, id: NodeId) -> Vec<Node> {
    let Some(clip) = capture(doc, &[id]) else { return vec![] };
    instantiate(doc, &clip).into_iter().next().unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use designos_doc::Rect;

    fn doc() -> (Document, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        d.insert(f, Node::rectangle(rid, Rect::new(5.0, 5.0, 10.0, 10.0))).unwrap();
        (d, page, f)
    }

    #[test]
    fn json_roundtrip_and_rejects_foreign_text() {
        let (d, _, f) = doc();
        let clip = capture(&d, &[f]).unwrap();
        let s = to_json(&clip).unwrap();
        let back = from_json(&s).unwrap();
        assert_eq!(back.nodes.len(), 2);
        assert!(from_json("hello").is_none());
        assert!(from_json("{\"a\":1}").is_none());
    }

    #[test]
    fn instantiate_assigns_fresh_ids_and_links() {
        let (mut d, _, f) = doc();
        let copy = clone_subtree(&mut d, f);
        assert_eq!(copy.len(), 2);
        assert_ne!(copy[0].id, f);
        assert!(copy[0].parent.is_none());
        assert_eq!(copy[0].children, vec![copy[1].id]);
        assert_eq!(copy[1].parent, Some(copy[0].id));
        assert!(d.get(copy[0].id).is_none(), "clone is not inserted yet");
    }

    #[test]
    fn pasting_into_a_document_with_an_exhausted_id_counter_keeps_every_copy_distinct() {
        let (d, page, f) = doc();
        let mut d = d;
        d.insert(page, Node::rectangle(NodeId(u64::MAX), Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
        // Loaders that do not renumber (library store, history) still hand such a document over.
        let (mut d, report) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
        assert!(report.id_space_exhausted);
        let clip = capture(&d, &[f]).unwrap();

        let copies = instantiate(&mut d, &clip);
        let mut ids: Vec<NodeId> = copies.iter().flatten().map(|n| n.id).collect();
        let more = instantiate(&mut d, &clip);
        ids.extend(more.iter().flatten().map(|n| n.id));
        assert_eq!(ids.len(), 4);
        assert_eq!(ids.iter().collect::<std::collections::BTreeSet<_>>().len(), 4, "{ids:?}");
        assert!(ids.iter().all(|id| d.get(*id).is_none()), "none collides with an existing node");
    }

    // ---- copies of a main component

    /// A page holding a main component `main` (with one layer in it) and a plain frame `plain`.
    fn doc_with_a_main() -> (Document, NodeId, NodeId, NodeId) {
        let (mut d, page, f) = doc();
        let plain_id = d.alloc_id();
        let plain = d.insert(page, Node::frame(plain_id, Rect::new(300.0, 0.0, 50.0, 50.0))).unwrap();
        designos_doc::create_component(&mut d, f).apply(&mut d).unwrap();
        d.get_mut(f).unwrap().transform = Affine::translate(40.0, 60.0);
        (d, page, f, plain)
    }

    fn linked(d: &mut Document, clip: &Clip, parent: NodeId) -> Vec<Vec<Node>> {
        instantiate_linking_mains(d, clip, |_, _| Some(parent))
    }

    #[test]
    fn a_clip_names_the_document_it_came_from_in_its_json() {
        let (d, _, main, _) = doc_with_a_main();
        let clip = from_json(&to_json(&capture(&d, &[main]).unwrap()).unwrap()).unwrap();
        assert!(main_links(&d, &clip)[0].is_some(), "the file it was copied from");
        let mut other = d.clone();
        other.renew_runtime_id();
        assert!(main_links(&other, &clip)[0].is_none(), "another file with the same node ids");

        // The same document number in another Studio process (the clip came through the system clipboard).
        let mut value: serde_json::Value = serde_json::from_str(&to_json(&clip).unwrap()).unwrap();
        let process = value["origin"]["process"].as_u64().unwrap();
        value["origin"]["process"] = serde_json::json!(process.wrapping_add(1));
        let foreign = from_json(&value.to_string()).unwrap();
        assert!(main_links(&d, &foreign)[0].is_none(), "another process with the same document number");
    }

    #[test]
    fn a_clip_from_an_older_build_has_no_origin_and_copies_for_real() {
        let (mut d, page, main, _) = doc_with_a_main();
        let mut value: serde_json::Value = serde_json::from_str(&to_json(&capture(&d, &[main]).unwrap()).unwrap()).unwrap();
        assert!(value.as_object_mut().unwrap().remove("origin").is_some(), "the newer build writes the origin");
        let clip = from_json(&value.to_string()).expect("an old clip still reads");
        let lists = linked(&mut d, &clip, page);
        assert!(matches!(lists[0][0].kind, NodeKind::Component(_)));
    }

    #[test]
    fn a_main_copied_in_its_own_file_becomes_an_instance_where_the_original_stood() {
        let (mut d, page, main, plain) = doc_with_a_main();
        let clip = capture(&d, &[main, plain]).unwrap();
        let lists = linked(&mut d, &clip, page);
        assert_eq!(lists.len(), 2);
        let inst = &lists[0][0];
        assert!(matches!(inst.kind, NodeKind::Instance { component: Some(c), .. } if c == main), "{:?}", inst.kind);
        assert_eq!(inst.component.source, Some(main));
        assert_eq!(inst.transform, d.get(main).unwrap().transform, "same place as the main");
        assert_eq!(lists[0].len(), 2, "the layer inside the main is mirrored");
        assert!(inst.parent.is_none() && lists[0][1].parent == Some(inst.id) && lists[0][1].component.source.is_some());
        assert!(matches!(lists[1][0].kind, NodeKind::Frame(_)), "a plain frame is still a copy");
        assert!(d.get(inst.id).is_none(), "nothing is inserted yet");
    }

    #[test]
    fn a_main_that_was_cut_or_changed_since_the_copy_is_copied_for_real() {
        let (mut d, page, main, _) = doc_with_a_main();
        let clip = capture(&d, &[main]).unwrap();
        let mut detached = d.clone();
        detached.get_mut(main).unwrap().kind = NodeKind::Frame(Default::default());
        assert!(matches!(linked(&mut detached, &clip, page)[0][0].kind, NodeKind::Component(_)), "no longer a main: copied as it was");
        d.remove(main).unwrap();
        assert!(matches!(linked(&mut d, &clip, page)[0][0].kind, NodeKind::Component(_)), "no main left to link to");

        let (mut d, page, frame) = doc();
        let clip = capture(&d, &[frame]).unwrap();
        designos_doc::create_component(&mut d, frame).apply(&mut d).unwrap();
        assert!(matches!(linked(&mut d, &clip, page)[0][0].kind, NodeKind::Frame(_)), "what was copied was a frame");
    }

    #[test]
    fn a_main_pasted_into_itself_or_a_set_stays_a_copy_and_a_paste_moves_out_of_the_main() {
        let (mut d, page, main, _) = doc_with_a_main();
        let clip = capture(&d, &[main]).unwrap();
        let inner = d.get(main).unwrap().children[0];
        assert_eq!(paste_parent(&d, &clip, main), page, "beside the main");
        assert_eq!(paste_parent(&d, &clip, inner), page, "from a layer inside it too");
        assert_eq!(paste_parent(&d, &clip, page), page);
        assert!(matches!(linked(&mut d, &clip, main)[0][0].kind, NodeKind::Component(_)), "an instance cannot sit in its own main");

        designos_doc::combine_as_variants(&mut d, &[main]).apply(&mut d).unwrap();
        let set = d.get(main).unwrap().parent.unwrap();
        assert!(matches!(d.get(set).unwrap().kind, NodeKind::ComponentSet(_)));
        let clip = capture(&d, &[main]).unwrap();
        assert!(matches!(linked(&mut d, &clip, set)[0][0].kind, NodeKind::Component(_)), "a variant copied in its set is a new variant");
        let lists = linked(&mut d, &clip, page);
        assert!(matches!(lists[0][0].kind, NodeKind::Instance { component: Some(c), .. } if c == main), "a variant copied elsewhere is linked");
    }

    // ---- paste from Figma

    use base64::Engine;
    use designos_fig::container::Compression;

    pub(crate) fn figma_html(buffer: &[u8]) -> String {
        let b64 = base64::engine::general_purpose::STANDARD;
        format!(
            "<meta charset=\"utf-8\"><div><span data-metadata=\"<!--(figmeta){}(/figmeta)-->\"></span><span data-buffer=\"<!--(figma){}(/figma)-->\"></span></div>",
            b64.encode(r#"{"dataType":"scene"}"#),
            b64.encode(buffer)
        )
    }

    fn app_with_one_rect() -> App {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        App::new(d)
    }

    fn page_children(app: &App) -> usize {
        app.doc.get(app.page).unwrap().children.len()
    }

    fn by_name(app: &App, name: &str) -> Node {
        app.doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).clone()
    }

    #[test]
    fn figma_paste_adds_the_layers_centered_selected_with_one_undo_step() {
        let mut app = app_with_one_rect();
        let nodes_before = app.doc.nodes.len();
        let html = figma_html(&designos_fig::testkit::sample_fig(Compression::Zstd, true));

        assert!(paste_figma(&mut app, &html));

        // the payload has four top-level layers: Card, Pie, Star, Note
        assert_eq!(page_children(&app), 1 + 4);
        assert!(app.toast.is_none(), "{:?}", app.toast.as_ref().map(|t| &t.text));
        assert_eq!(app.selection.len(), 4);
        let center = crate::util::union_all(app.selection.iter().map(|id| crate::util::world_aabb(&app.doc, *id))).unwrap();
        let (cx, cy) = (center.x + center.w / 2.0, center.y + center.h / 2.0);
        assert!((cx - 400.0).abs() <= 0.5 && (cy - 300.0).abs() <= 0.5, "centered on the 800x600 viewport, got ({cx}, {cy})");
        assert_eq!(by_name(&app, "Card").size, Vec2::new(200.0, 300.0));
        assert!(app.doc.images.contains_key("deadbeef"), "embedded image hash");

        app.undo();
        assert_eq!(page_children(&app), 1, "one undo removes every pasted layer");
        assert_eq!(app.doc.nodes.len(), nodes_before);
    }

    #[test]
    fn undoing_a_figma_paste_takes_its_images_along_and_redo_brings_them_back() {
        let mut app = app_with_one_rect();
        let images_before = app.doc.images.len();
        let html = figma_html(&designos_fig::testkit::sample_fig(Compression::Zstd, true));

        assert!(paste_figma(&mut app, &html));
        assert!(app.doc.images.contains_key("deadbeef"));
        assert_eq!(app.doc.images.len(), images_before + 1);

        app.undo();
        assert_eq!(app.doc.images.len(), images_before, "no orphaned image bytes after undo");
        app.redo();
        assert_eq!(app.doc.images.get("deadbeef").map(Vec::as_slice), Some(&[0x89, b'P', b'N', b'G'][..]), "redo restores the bytes the layers reference");
        app.undo();
        assert_eq!(app.doc.images.len(), images_before);
    }

    #[test]
    fn undoing_a_figma_paste_keeps_an_image_the_document_already_held() {
        let mut app = app_with_one_rect();
        app.doc.images.insert("deadbeef".into(), vec![1, 2, 3]);
        let html = figma_html(&designos_fig::testkit::sample_fig(Compression::Zstd, true));
        assert!(paste_figma(&mut app, &html));
        assert_eq!(app.doc.images["deadbeef"], vec![1, 2, 3], "the document's own bytes win");
        app.undo();
        assert_eq!(app.doc.images["deadbeef"], vec![1, 2, 3], "undo only removes what the paste added");
    }

    #[test]
    fn a_figma_payload_over_the_clipboard_budget_toasts_instead_of_aborting() {
        let mut app = app_with_one_rect();
        let nodes_before = app.doc.nodes.len();
        let schema = designos_fig::testkit::figma_like_schema();
        // 300 MiB of zeros deflate to ~300 KB, yet exceed the 256 MiB clipboard inflate budget
        let bomb = designos_fig::container::write_container(&schema.encode(), &vec![0u8; 300 << 20], Compression::Deflate, 101);
        assert!(paste_figma(&mut app, &figma_html(&bomb)), "a Figma marker is claimed even when the payload is over budget");
        let toast = app.toast.as_ref().expect("a toast explains the failure");
        assert!(toast.error && toast.text.starts_with("Cannot paste from Figma") && toast.text.contains("too large"), "{}", toast.text);
        assert_eq!(app.doc.nodes.len(), nodes_before);
    }

    #[test]
    fn figma_paste_points_instances_at_the_pasted_component_and_drops_foreign_refs() {
        let mut app = app_with_one_rect();
        assert!(paste_figma(&mut app, &figma_html(&designos_fig::testkit::sample_fig(Compression::Zstd, true))));
        let button = by_name(&app, "Button");
        let copy = by_name(&app, "Button copy");
        let NodeKind::Instance { component, .. } = copy.kind else { panic!("Button copy is an instance") };
        assert_eq!(component, Some(button.id));
        assert!(app.doc.get(button.id).is_some());
        assert!(app.doc.nodes.values().all(|n| n.style_refs.is_empty()));
    }

    #[test]
    fn malformed_figma_payload_toasts_and_leaves_the_document_alone() {
        let mut app = app_with_one_rect();
        let nodes_before = app.doc.nodes.len();
        for html in [figma_html(b"not a fig archive"), "<!--(figma)@@@(/figma)-->".to_string(), "<!--(figma)aGVsbG8=".to_string()] {
            app.toast = None;
            assert!(paste_figma(&mut app, &html), "a Figma marker is claimed even when the payload is bad");
            let toast = app.toast.as_ref().expect("a toast explains the failure");
            assert!(toast.error && toast.text.starts_with("Cannot paste from Figma"), "{}", toast.text);
            assert_eq!(app.doc.nodes.len(), nodes_before);
        }
    }

    #[test]
    fn non_figma_html_falls_through_to_svg_and_text_paste() {
        let mut app = app_with_one_rect();
        assert!(!paste_figma(&mut app, "<b>just html with (figma) in prose</b>"));
        assert_eq!(page_children(&app), 1);

        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>"#;
        app.paste_with_html(Some(svg), Some("<b>plain</b>"));
        assert_eq!(page_children(&app), 2, "svg text still pastes when the HTML is not Figma's");
    }

    #[test]
    fn clipboard_json_roundtrips_non_finite_numbers_and_repairs_legacy_nulls() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut rect = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 20.0));
        rect.size.x = f32::NAN;
        rect.opacity = f32::NAN;
        rect.transform.tx = f32::INFINITY;
        d.insert(page, rect).unwrap();

        let clip = capture(&d, &[id]).expect("clip");
        let json = to_json(&clip).expect("to_json");
        assert!(!json.contains("null"), "clipboard JSON must not write null for non-finite numbers: {json}");
        let roundtripped = from_json(&json).expect("from_json on finite-sanitized clip");
        assert_eq!(
            (roundtripped.nodes[0].size.x, roundtripped.nodes[0].size.y, roundtripped.nodes[0].opacity, roundtripped.nodes[0].transform.tx),
            (0.0, 20.0, 1.0, 0.0)
        );

        let legacy_json = serde_json::to_string(&clip).unwrap();
        assert!(legacy_json.contains("null"));
        let repaired = from_json(&legacy_json).expect("from_json repairs legacy null floats");
        assert_eq!((repaired.nodes[0].size.x, repaired.nodes[0].opacity), (0.0, 1.0));
    }

    #[test]
    fn figma_paste_remaps_slot_and_swap_ids_and_drops_and_reports_unknown_ids() {
        use designos_doc::{ComponentPropDef, FrameProps, PropKind, PropValue, SlotPropConfig};

        let mut app = app_with_one_rect();
        for _ in 0..15 {
            let id = app.doc.alloc_id();
            app.doc.insert(app.page, Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
        }
        let pre_paste_ids: std::collections::HashSet<NodeId> = app.doc.nodes.keys().copied().collect();

        // Build a Figma clipboard payload document where `ForeignIcon` lives on the internal library page
        // (so `import_figma_clipboard` resolves its GUID to a valid `frag` NodeId, which is not in the
        // pasted page roots and would alias an existing `app.doc` node if not dropped).
        let mut frag_doc = Document::new("frag");
        let fpage = frag_doc.add_page("Page");
        let lib_page = frag_doc.alloc_id();
        frag_doc.nodes.insert(lib_page, Node::new(lib_page, NodeKind::Page { background: designos_doc::Color::WHITE }));
        frag_doc.library_page = Some(lib_page);

        let foreign_icon_id = frag_doc.alloc_id();
        let mut foreign_icon = Node::new(foreign_icon_id, NodeKind::Component(FrameProps::default()));
        foreign_icon.name = "ForeignIcon".into();
        frag_doc.insert(lib_page, foreign_icon).unwrap();

        let local_icon_id = frag_doc.alloc_id();
        let mut local_icon = Node::new(local_icon_id, NodeKind::Component(FrameProps::default()));
        local_icon.name = "LocalIcon".into();
        frag_doc.insert(fpage, local_icon).unwrap();

        let panel_id = frag_doc.alloc_id();
        let mut panel = Node::new(panel_id, NodeKind::Component(FrameProps::default()));
        panel.name = "Panel".into();
        panel.component.props = vec![
            ComponentPropDef {
                name: "SwapLocal#1:1".into(),
                kind: PropKind::InstanceSwap { preferred: vec![local_icon_id, foreign_icon_id] },
                default: PropValue::Instance(local_icon_id),
            },
            ComponentPropDef {
                name: "SwapForeign#1:2".into(),
                kind: PropKind::InstanceSwap { preferred: vec![foreign_icon_id] },
                default: PropValue::Instance(foreign_icon_id),
            },
            ComponentPropDef { name: "Body#1:3".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) },
        ];
        panel.component.slot_configs.insert("Body#1:3".into(), SlotPropConfig { preferred: vec![local_icon_id, foreign_icon_id], ..Default::default() });
        frag_doc.insert(fpage, panel).unwrap();

        let inst_id = frag_doc.alloc_id();
        let mut panel_inst = Node::new(inst_id, NodeKind::Instance { component: Some(panel_id), component_key: None, frame: FrameProps::default() });
        panel_inst.name = "PanelInst".into();
        frag_doc.insert(fpage, panel_inst).unwrap();

        let inst_slot_child_id = frag_doc.alloc_id();
        let mut inst_slot_child = Node::frame(inst_slot_child_id, Rect::new(0.0, 0.0, 20.0, 20.0));
        inst_slot_child.name = "InstSlotChild".into();
        frag_doc.insert(inst_id, inst_slot_child).unwrap();

        {
            let inst_mut = frag_doc.get_mut(inst_id).unwrap();
            inst_mut.component.values.insert("SwapLocal#1:1".into(), PropValue::Instance(local_icon_id));
            inst_mut.component.values.insert("SwapForeign#1:2".into(), PropValue::Instance(foreign_icon_id));
            inst_mut.component.values.insert("Body#1:3".into(), PropValue::Slot(vec![inst_slot_child_id, foreign_icon_id]));
        }

        let fig_bytes = designos_fig::export_fig(&frag_doc).unwrap();
        assert!(paste_figma(&mut app, &figma_html(&fig_bytes)));

        let local_icon = by_name(&app, "LocalIcon");
        let panel = by_name(&app, "Panel");
        let panel_inst = by_name(&app, "PanelInst");
        let inst_slot_child = by_name(&app, "InstSlotChild");

        assert!(!pre_paste_ids.contains(&local_icon.id));
        assert!(!pre_paste_ids.contains(&inst_slot_child.id));

        // 1. SlotPropConfig::preferred on Panel points only at pasted LocalIcon, dropping ForeignIcon
        let slot_cfg = panel.component.slot_configs.get("Body").expect("Body slot config exists");
        assert_eq!(slot_cfg.preferred, vec![local_icon.id], "slot_configs[*].preferred must remap pasted ids and drop unknown ids");

        // 2. InstanceSwap::preferred and PropValue::Instance defaults on Panel
        let swap_local_def = panel.component.props.iter().find(|p| p.name == "SwapLocal").unwrap();
        assert_eq!(swap_local_def.kind, PropKind::InstanceSwap { preferred: vec![local_icon.id] });
        assert_eq!(swap_local_def.default, PropValue::Instance(local_icon.id));

        let swap_foreign_def = panel.component.props.iter().find(|p| p.name == "SwapForeign").unwrap();
        assert_eq!(swap_foreign_def.kind, PropKind::InstanceSwap { preferred: vec![] });
        assert_eq!(swap_foreign_def.default, PropValue::Instance(NodeId(0)), "unknown InstanceSwap default must not alias pre-existing node");

        // 3. Instance prop values on PanelInst
        assert_eq!(panel_inst.component.values.get("SwapLocal"), Some(&PropValue::Instance(local_icon.id)));
        assert_eq!(
            panel_inst.component.values.get("SwapForeign"),
            Some(&PropValue::Instance(NodeId(0))),
            "unknown Instance prop value must not alias pre-existing node"
        );
        assert_eq!(
            panel_inst.component.values.get("Body"),
            Some(&PropValue::Slot(vec![inst_slot_child.id])),
            "instance PropValue::Slot must remap pasted ids and drop unknown ids"
        );

        // 4. Dropped unknown ids are reported via a non-error toast
        let toast = app.toast.as_ref().expect("dropped unknown ids are reported");
        assert!(!toast.error, "dropped foreign ref notice is informational, not a fatal error: {}", toast.text);
        assert!(toast.text.contains("dropped"), "toast reports dropped references: {}", toast.text);
    }
}
