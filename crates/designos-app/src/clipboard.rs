//! In-app clipboard: nodes (with subtrees) serialized as JSON, plus subtree cloning,
//! SVG markup pasted as text, and Figma clipboard payloads pasted as HTML.

use crate::app::App;
use crate::edit::Tx;
use crate::layout;
use designos_doc::{Affine, Command, Document, Node, NodeId, NodeKind, PropKind, PropValue, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Places a dropped `.svg` file's markup centered on screen position `at`
/// (the drop point; the viewport center when it is outside the canvas), into
/// the frame under that point. `nth` staggers several files dropped together.
pub fn drop_svg(app: &mut App, svg: &str, name: &str, at: Option<egui::Pos2>, nth: usize) {
    let view = app.canvas_rect;
    let screen = at.filter(|p| view.contains(*p)).unwrap_or(view.center());
    let mut p = app.view.to_page(view.min, screen);
    p.x += 24.0 * nth as f32;
    p.y += 24.0 * nth as f32;
    let parent = crate::canvas::create::container_at(&app.doc, app.page, p, &[]);
    if let Err(e) = place_svg(app, svg, name, p, parent) {
        app.show_toast(e, true);
    }
}

/// Imports SVG markup as a Frame named `name` centered on page point `center`
/// (rounded to whole pixels), appended to `parent`'s children as one undoable
/// step, and selects it.
pub fn place_svg(app: &mut App, svg: &str, name: &str, center: Vec2, parent: NodeId) -> Result<NodeId, String> {
    let mut out = designos_svg::svg_to_nodes(svg, name, &mut || app.doc.alloc_id()).map_err(|e| format!("Cannot import SVG: {e}"))?;
    if let Some(err) = crate::ops::slot_edit_error(&app.doc, &[], Some((parent, &[&out.nodes[0]]))) {
        return Err(err.to_string());
    }
    let inv = app.doc.world_transform(parent).invert().ok_or("Cannot place into a collapsed layer")?;
    let c = inv.apply(center);
    let size = out.nodes[0].size;
    out.nodes[0].transform = Affine::translate((c.x - size.x / 2.0).round(), (c.y - size.y / 2.0).round());
    app.doc.images.append(&mut out.images);
    let root = out.nodes[0].id;
    let index = app.doc.get(parent).map(|p| p.children.len()).unwrap_or(0);
    let mut tx = Tx::new();
    tx.cmd(&mut app.doc, Command::Insert { parent, index, nodes: out.nodes }).map_err(|e| e.to_string())?;
    App::mark_slot_content_override(&mut app.doc, &mut tx.ed, parent);
    layout::relayout_around(&mut app.doc, &mut tx.ed, parent);
    app.run_tx(tx);
    app.selection = vec![root];
    app.fonts.load_for(&app.doc);
    Ok(root)
}

const MAGIC: u32 = 0xDE51_6A01;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Clip {
    magic: u32,
    pub roots: Vec<NodeId>,
    pub nodes: Vec<Node>,
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
    Some(Clip { magic: MAGIC, roots: ids.to_vec(), nodes })
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
