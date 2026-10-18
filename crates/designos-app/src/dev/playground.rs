//! Read-only Component Playground in Dev Mode (#159 / E3-14).
//!
//! Lets developers toggle Boolean, Text, Variant, and InstanceSwap component
//! properties on an in-memory scratch instance in the Dev Mode inspector without
//! ever mutating `app.doc` (`doc_hash` and `mutation_epoch` remain unchanged),
//! while live preview and code generation update immediately.

use crate::app::App;
use crate::panels::inspect::{section_title, small_button, Cur, PAD};
use crate::theme;
use crate::widgets::field;
use crate::widgets::menu::dropdown;
use crate::widgets::{self, elide};
use designos_doc::*;
use egui::{Align2, Color32, Context, Pos2, Rect, Sense, Stroke, TextureHandle, Ui, Vec2};
use std::collections::{BTreeMap, BTreeSet};

type PreviewCacheKey = (u64, u64, NodeId, u64);

#[derive(Default)]
pub struct PlaygroundState {
    /// Inspected node (`id`) this scratch session belongs to.
    pub node: Option<NodeId>,
    /// Scratch property overrides keyed by property name (`ComponentPropDef::name`).
    pub values: BTreeMap<String, PropValue>,
    /// Monotonic revision bumped on every scratch override edit or reset.
    pub rev: u64,
    /// Cached raster preview of the scratch instance for the inspector stage.
    preview_cache: Option<(PreviewCacheKey, Option<TextureHandle>)>,
    /// File-wide instance-swap choices for `(document, revision, component)`, built while a menu is open.
    swap_cache: Option<((u64, u64, NodeId), Vec<NodeId>)>,
}

impl PlaygroundState {
    pub fn ensure_node(&mut self, id: NodeId) {
        if self.node != Some(id) {
            self.node = Some(id);
            self.values.clear();
            self.rev = self.rev.wrapping_add(1);
            self.preview_cache = None;
        }
    }

    pub fn set_prop(&mut self, id: NodeId, name: String, value: PropValue) {
        self.ensure_node(id);
        self.values.insert(name, value);
        self.rev = self.rev.wrapping_add(1);
        self.preview_cache = None;
    }

    pub fn reset(&mut self, id: NodeId) {
        self.ensure_node(id);
        if !self.values.is_empty() {
            self.values.clear();
            self.rev = self.rev.wrapping_add(1);
            self.preview_cache = None;
        }
    }

    pub fn has_overrides_for(&self, id: NodeId) -> bool {
        self.node == Some(id) && !self.values.is_empty()
    }
}

/// Resolves `(target_node, main_component)` for `id`: the nearest `Instance` at or above it
/// (nested instances included), else the `Component` / `ComponentSet` it is or lies inside.
pub fn playground_target(doc: &Document, id: NodeId) -> Option<(NodeId, NodeId)> {
    let n = doc.get(id)?;
    let mut cur = Some(id);
    while let Some(c) = cur {
        let node = doc.get(c)?;
        if let Some(comp) = instance_component(node) {
            return Some((c, comp));
        }
        cur = node.parent;
    }
    match &n.kind {
        NodeKind::Component(_) => Some((id, id)),
        NodeKind::ComponentSet(_) => components_of(doc, id).into_iter().next().map(|c| (id, c)),
        _ => enclosing_component(doc, id).map(|c| (c, c)),
    }
}

fn comp_label(doc: &Document, comp: NodeId) -> String {
    let Some(c) = doc.get(comp) else { return "Missing component".into() };
    match c.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))) {
        Some(set) => format!("{} / {}", set.name, c.name),
        None => c.name.clone(),
    }
}

fn current_prop_value(doc: &Document, target_id: NodeId, comp_id: NodeId, def: &ComponentPropDef, pg: &PlaygroundState, id: NodeId) -> PropValue {
    if pg.node == Some(id) {
        if let Some(v) = pg.values.get(&def.name) {
            return v.clone();
        }
    }
    if doc.get(target_id).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. })) {
        return prop_value(doc, target_id, def);
    }
    if matches!(def.kind, PropKind::Variant { .. }) {
        if let Some(c) = doc.get(comp_id) {
            if let Some((_, v)) = parse_variant_name(&c.name).into_iter().find(|(k, _)| *k == def.name) {
                return PropValue::Variant(v);
            }
        }
    }
    def.default.clone()
}

fn refit_text_in_subtree(doc: &mut Document, root: NodeId) {
    let mut stack = vec![root];
    while let Some(cur) = stack.pop() {
        let kids = doc.get(cur).map(|n| n.children.clone()).unwrap_or_default();
        if let Some(n) = doc.get_mut(cur) {
            if let NodeKind::Text(t) = &n.kind {
                if matches!(t.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height) {
                    n.size = crate::layout::estimate_text_size(t);
                }
            }
        }
        stack.extend(kids);
    }
}

/// Components `seeds` need to behave like in the real document: each one with its whole set (the
/// sibling variants a variant prop switches to), plus every component its layers instantiate or
/// offer as an instance-swap choice. Returns every node id of those trees.
fn component_closure(base: &Document, seeds: Vec<NodeId>) -> BTreeSet<NodeId> {
    let mut keep = BTreeSet::new();
    let mut owners = BTreeSet::new();
    let mut pending = seeds;
    while let Some(c) = pending.pop() {
        let Some(cn) = base.get(c) else { continue };
        let set = cn.parent.filter(|p| base.get(*p).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet(_))));
        let owner = set.unwrap_or(c);
        if !owners.insert(owner) {
            continue;
        }
        base.walk(owner, |n, _| {
            keep.insert(n.id);
            referenced_components(n, &mut pending);
        });
    }
    keep
}

/// Components `n` points at: its main, instance-swap choices and defaults, and chosen values.
fn referenced_components(n: &Node, out: &mut Vec<NodeId>) {
    out.extend(instance_component(n));
    for def in &n.component.props {
        if let PropKind::InstanceSwap { preferred } = &def.kind {
            out.extend(preferred.iter().copied());
        }
        if let PropValue::Instance(c) = &def.default {
            out.push(*c);
        }
    }
    out.extend(n.component.values.values().filter_map(|v| if let PropValue::Instance(c) = v { Some(*c) } else { None }));
}

/// Image keys painted by `n` (fills, stroke, text fills).
fn image_refs(n: &Node, out: &mut BTreeSet<String>) {
    let mut lists: Vec<&Vec<Paint>> = vec![&n.fills, &n.stroke.paints];
    if let NodeKind::Text(t) = &n.kind {
        lists.push(&t.style.fills);
        lists.extend(t.runs.iter().map(|r| &r.style.fills));
    }
    for p in lists.into_iter().flatten() {
        if let PaintKind::Image { image_ref, .. } = &p.kind {
            out.insert(image_ref.clone());
        }
    }
}

/// Re-points a copied instance node at the main component tree (`keep`) instead of the outer
/// instance or component it mirrored in the real document, so the copy syncs like a top-level
/// instance. Overrides and values of every skipped link move onto the node, so what the nested
/// instance showed stays as it was.
fn relink_to_scratch(base: &Document, keep: &BTreeSet<NodeId>, n: &mut Node) {
    for _ in 0..8 {
        let Some(s) = n.component.source.filter(|s| !keep.contains(s)) else { return };
        let Some(sn) = base.get(s) else { break };
        n.component.overrides.extend(sn.component.overrides.iter().copied());
        for (k, v) in &sn.component.values {
            n.component.values.entry(k.clone()).or_insert_with(|| v.clone());
        }
        n.component.source = sn.component.source;
    }
    n.component.source = n.component.source.filter(|s| keep.contains(s));
}

/// Builds an in-memory scratch `(Document, codegen_node_id, preview_node_id)` with `pg.values`
/// applied, without mutating `base`. The scratch document holds only the instance (a copy of the
/// target instance, or a fresh one of the selected component) and the components it needs, so its
/// size follows the component, never the file.
pub fn build_scratch_doc(base: &Document, id: NodeId, pg: &PlaygroundState) -> Option<(Document, NodeId, NodeId)> {
    let (target_id, comp_id) = playground_target(base, id)?;
    let empty = BTreeMap::new();
    let values = if pg.node == Some(id) { &pg.values } else { &empty };
    let target_is_instance = base.get(target_id).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. }));

    let mut inst_ids = Vec::new();
    if target_is_instance {
        base.walk(target_id, |n, _| inst_ids.push(n.id));
    }
    let mut seeds = vec![comp_id];
    seeds.extend(values.values().filter_map(|v| if let PropValue::Instance(c) = v { Some(*c) } else { None }));
    for nid in &inst_ids {
        if let Some(n) = base.get(*nid) {
            referenced_components(n, &mut seeds);
        }
    }
    let keep = component_closure(base, seeds);

    let mut scratch = Document::new("component playground");
    scratch.styles = base.styles.clone();
    scratch.style_meta = base.style_meta.clone();
    for nid in &keep {
        let Some(n) = base.get(*nid) else { continue };
        let mut c = n.clone();
        c.parent = c.parent.filter(|p| keep.contains(p));
        scratch.nodes.insert(*nid, c);
    }
    let page_id = NodeId(base.nodes.keys().next_back().map_or(0, |k| k.0) + 1);
    let mut page = Node::new(page_id, NodeKind::Page { background: Color::TRANSPARENT });
    if target_is_instance {
        for nid in &inst_ids {
            let Some(n) = base.get(*nid) else { continue };
            let mut c = n.clone();
            relink_to_scratch(base, &keep, &mut c);
            if *nid == target_id {
                c.parent = Some(page_id);
                if c.component.source.is_some() {
                    c.component.source = instance_component(&c);
                }
            }
            scratch.nodes.insert(*nid, c);
        }
        page.children = vec![target_id];
    }
    scratch.nodes.insert(page_id, page);
    scratch.pages.push(page_id);
    scratch.reserve_ids(page_id);

    let mut refs = BTreeSet::new();
    let mut bound = false;
    for n in scratch.nodes.values() {
        image_refs(n, &mut refs);
        bound |= !n.variables.bindings.is_empty();
    }
    for r in refs {
        if let Some(bytes) = base.images.get(&r) {
            scratch.images.insert(r, bytes.clone());
        }
    }
    if bound {
        scratch.variables = base.variables.clone();
        scratch.variable_collections = base.variable_collections.clone();
    }

    let inst_id = if target_is_instance {
        target_id
    } else {
        let cmd = create_instance(&mut scratch, comp_id, page_id, designos_doc::Vec2::ZERO);
        let inst_id = match &cmd {
            Command::Insert { nodes, .. } => nodes.first().map(|n| n.id)?,
            _ => return None,
        };
        cmd.apply(&mut scratch).ok()?;
        inst_id
    };
    for (k, v) in values {
        let _ = set_instance_prop(&mut scratch, inst_id, k, Some(v.clone())).apply(&mut scratch);
    }
    refit_text_in_subtree(&mut scratch, inst_id);
    scratch.relayout_subtree(inst_id);

    // The code is for the layer that was selected, as it appears in the scratch instance.
    let in_instance = id == inst_id || scratch.is_ancestor(inst_id, id);
    let code_node = if in_instance && scratch.get(id).is_some() {
        id
    } else {
        let mut found = None;
        scratch.walk(inst_id, |n, _| {
            if found.is_none() && n.component.source == Some(id) {
                found = Some(n.id);
            }
        });
        found.unwrap_or(inst_id)
    };
    let scratch = if bound { apply_bound_variables(&scratch) } else { scratch };
    Some((scratch, code_node, inst_id))
}

fn preview_texture(ctx: &Context, base_doc: &Document, id: NodeId, pg: &mut PlaygroundState) -> Option<TextureHandle> {
    let key: PreviewCacheKey = (base_doc.runtime_id(), base_doc.mutation_epoch(), id, pg.rev);
    if let Some((k, cached)) = &pg.preview_cache {
        if *k == key {
            return cached.clone();
        }
    }
    let (scratch, _, preview_id) = build_scratch_doc(base_doc, id, pg)?;
    let dim = scratch.get(preview_id).map(|n| n.size.x.max(n.size.y)).unwrap_or(1.0).max(1.0);
    let png = designos_export::export_png(&scratch, preview_id, 168.0 / dim);
    let tex = image::load_from_memory(&png).ok().map(|img| {
        let rgba = img.to_rgba8();
        let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
        ctx.load_texture("dev-playground-preview-tex", ci, egui::TextureOptions::LINEAR)
    });
    pg.preview_cache = Some((key, tex.clone()));
    tex
}

fn toggle_switch(ui: &mut Ui, rect: Rect, key: &str, on: bool) -> Option<bool> {
    widgets::record(ui, key, rect);
    let r = ui.interact(rect, ui.id().with(("pg-tog", key)), Sense::click());
    let sw = Rect::from_min_size(Pos2::new(rect.right() - theme::CONTROL_STANDARD, rect.center().y - theme::SPACE_INLINE), Vec2::new(28.0, 16.0));
    let p = ui.painter();
    p.rect_filled(
        sw,
        theme::cr_pill(sw.height()),
        if on {
            theme::BG_BRAND
        } else if r.hovered() {
            theme::BG_TERTIARY
        } else {
            theme::BG_SECONDARY
        },
    );
    let kx = if on { sw.right() - theme::SPACE_INLINE } else { sw.left() + theme::SPACE_INLINE };
    p.circle_filled(Pos2::new(kx, sw.center().y), 6.0, Color32::WHITE);
    r.clicked().then_some(!on)
}

fn live_text_input(ui: &mut Ui, rect: Rect, key: &str, value: &str) -> Option<String> {
    let id = ui.id().with(("pg-txt", key));
    field::field_bg(ui, rect, id);
    let mut buf = value.to_string();
    let te = egui::TextEdit::singleline(&mut buf)
        .id(id)
        .font(theme::font_body())
        .text_color(theme::TEXT)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::symmetric(theme::FIELD_PLAIN_X as i8, 0))
        .vertical_align(egui::Align::Center)
        .min_size(rect.size())
        .desired_width(rect.width());
    let out = ui.put(rect, te);
    (out.changed() && buf != value).then_some(buf)
}

thread_local! {
    /// Builds of the file-wide swap list (tests assert it is not rebuilt per frame).
    #[cfg(test)]
    static SWAP_LIST_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Number of times the file-wide swap list was built on this thread (tests).
#[cfg(test)]
pub(crate) fn swap_list_builds() -> usize {
    SWAP_LIST_BUILDS.with(|c| c.get())
}

/// The component `comp_id` belongs to: its whole set, or itself.
fn component_family(doc: &Document, comp_id: NodeId) -> Vec<NodeId> {
    match doc.get(comp_id).and_then(|c| c.parent).filter(|p| doc.get(*p).is_some_and(|n| matches!(n.kind, NodeKind::ComponentSet(_)))) {
        Some(set) => components_of(doc, set),
        None => vec![comp_id],
    }
}

/// Main components the playground may swap an instance of `comp_id` to, sorted by label like the
/// inspector's swap menu: every `Component` in the file except `comp_id`'s own family and the
/// components that (directly or through other components) contain an instance of it, because
/// swapping one in would make the component hold itself.
fn valid_swap_targets(doc: &Document, comp_id: NodeId) -> Vec<NodeId> {
    let mut holds: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for n in doc.nodes.values() {
        if let (Some(inner), Some(outer)) = (instance_component(n), enclosing_component(doc, n.id)) {
            holds.entry(outer).or_default().push(inner);
        }
    }
    let mut cyclic: BTreeSet<NodeId> = component_family(doc, comp_id).into_iter().collect();
    loop {
        let before = cyclic.len();
        for (outer, inner) in &holds {
            if inner.iter().any(|c| cyclic.contains(c)) {
                cyclic.insert(*outer);
            }
        }
        if cyclic.len() == before {
            break;
        }
    }
    let mut out: Vec<NodeId> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_)) && !cyclic.contains(&n.id)).map(|n| n.id).collect();
    out.sort_by_key(|id| comp_label(doc, *id));
    out
}

/// Choices of an instance-swap prop: its `preferred` components, or, when it names none, the
/// file's valid swap targets. That list is built only while the menu is `open` and is kept per
/// document revision, so an open menu does not rescan the file every frame.
pub(crate) fn swap_choices(app: &mut App, open: bool, preferred: &[NodeId], comp_id: NodeId) -> Vec<NodeId> {
    if !preferred.is_empty() {
        return preferred.iter().copied().filter(|cid| app.doc.get(*cid).is_some()).collect();
    }
    if !open {
        return Vec::new();
    }
    let key = (app.doc.runtime_id(), app.doc.mutation_epoch(), comp_id);
    if let Some((k, list)) = &app.dev.playground.swap_cache {
        if *k == key {
            return list.clone();
        }
    }
    let list = valid_swap_targets(&app.doc, comp_id);
    #[cfg(test)]
    SWAP_LIST_BUILDS.with(|c| c.set(c.get() + 1));
    app.dev.playground.swap_cache = Some((key, list.clone()));
    list
}

/// Whether the file has a component the prop could swap to. Cheap while the menu is closed: it
/// stops at the first component outside `comp_id`'s family, and reuses the list when one is cached.
pub(crate) fn swap_target_exists(app: &App, comp_id: NodeId) -> bool {
    let key = (app.doc.runtime_id(), app.doc.mutation_epoch(), comp_id);
    if let Some((k, list)) = &app.dev.playground.swap_cache {
        if *k == key {
            return !list.is_empty();
        }
    }
    let family = component_family(&app.doc, comp_id);
    app.doc.nodes.values().any(|n| matches!(n.kind, NodeKind::Component(_)) && !family.contains(&n.id))
}

fn dropdown_open_id(ui: &Ui, key: &str) -> egui::Id {
    ui.id().with(("dd", key))
}

/// A select that cannot be opened, with the reason it shows instead of a menu.
fn disabled_select(ui: &mut Ui, rect: Rect, reason: &str) {
    let p = ui.painter();
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    let font = theme::font_menu();
    let shown = elide(p, reason, &font, rect.width() - theme::SPACE_BLOCK);
    p.text(Pos2::new(rect.left() + theme::SPACE_INLINE, rect.center().y), Align2::LEFT_CENTER, shown, font, theme::TEXT_DISABLED);
}

/// Renders the read-only Component Playground section in the Dev Mode Inspect panel.
pub fn playground_section(app: &mut App, ui: &mut Ui, c: &mut Cur, id: NodeId) {
    let Some((target_id, comp_id)) = playground_target(&app.doc, id) else { return };
    let defs = prop_defs(&app.doc, comp_id);
    if defs.is_empty() {
        return;
    }

    section_title(ui, c, "Component playground");
    let reset_r = c.rect(c.w - PAD - 56.0, -theme::CONTROL_DENSE, 56.0, theme::CONTROL_DENSE);
    widgets::record(ui, "dev-playground-reset", reset_r);
    if small_button(ui, reset_r, "dev-playground-reset", "Reset") {
        app.dev.playground.reset(id);
    }

    // Read-only badge + live preview stage.
    let w = c.w - 2.0 * PAD;
    let stage_h = 96.0;
    let stage_r = c.rect(PAD, 0.0, w, stage_h);
    widgets::record(ui, "dev-playground-preview", stage_r);
    ui.painter().rect_filled(stage_r, theme::cr(theme::RADIUS_CONTROL), Color32::from_rgb(0x1E, 0x1E, 0x1E));
    ui.painter().rect_stroke(stage_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);

    // Subtle 12px dot grid inside the stage so both light and dark components read cleanly.
    let mut gx = stage_r.left() + theme::SPACE_INLINE;
    while gx < stage_r.right() - theme::SPACE_INLINE {
        let mut gy = stage_r.top() + theme::SPACE_SECTION;
        while gy < stage_r.bottom() - theme::SPACE_INLINE {
            ui.painter().circle_filled(Pos2::new(gx, gy), 0.75, Color32::from_white_alpha(18));
            gy += 12.0;
        }
        gx += 12.0;
    }

    let badge_text = if app.dev.playground.has_overrides_for(id) { "Scratch modified" } else { "Read-only" };
    let badge_w = if app.dev.playground.has_overrides_for(id) { 88.0 } else { 58.0 };
    let badge_r = Rect::from_min_size(Pos2::new(stage_r.left() + theme::SPACE_INLINE, stage_r.top() + theme::SPACE_ROW), Vec2::new(badge_w, 16.0));
    ui.painter().rect_filled(badge_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    ui.painter().text(badge_r.center(), Align2::CENTER_CENTER, badge_text, theme::font_caption(), theme::TEXT_SECONDARY);

    let canvas_area = Rect::from_min_max(
        Pos2::new(stage_r.left() + theme::SPACE_GROUP, stage_r.top() + theme::SPACE_SECTION),
        Pos2::new(stage_r.right() - theme::SPACE_GROUP, stage_r.bottom() - theme::SPACE_INLINE),
    );
    if let Some(tex) = preview_texture(ui.ctx(), &app.doc, id, &mut app.dev.playground) {
        let sz = tex.size_vec2();
        if sz.x > 0.0 && sz.y > 0.0 {
            let k = (canvas_area.width() / sz.x).min(canvas_area.height() / sz.y).min(1.0);
            let img_r = Rect::from_center_size(canvas_area.center(), sz * k);
            ui.painter().image(tex.id(), img_r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
    }
    c.y += stage_h + 8.0;

    for def in defs {
        if matches!(def.kind, PropKind::Slot) {
            continue;
        }
        let val = current_prop_value(&app.doc, target_id, comp_id, &def, &app.dev.playground, id);
        let is_overridden = app.dev.playground.node == Some(id) && app.dev.playground.values.contains_key(&def.name);
        let row_r = c.rect(PAD, 0.0, w, 28.0);
        let label_w = 92.0;
        let clean_name = def.name.split('#').next().unwrap_or(&def.name);
        let shown = elide(ui.painter(), clean_name, &theme::font_body(), label_w - 4.0);
        ui.painter().text(
            Pos2::new(row_r.left(), row_r.center().y),
            Align2::LEFT_CENTER,
            shown,
            if is_overridden { theme::font_body_strong() } else { theme::font_body() },
            if is_overridden { theme::TEXT } else { theme::TEXT_SECONDARY },
        );
        let ctl = Rect::from_min_size(
            Pos2::new(row_r.left() + label_w, row_r.top() + (row_r.height() - theme::CONTROL_DENSE) / 2.0),
            Vec2::new(w - label_w, theme::CONTROL_DENSE),
        );
        let key = format!("dev-playground-prop-{clean_name}");
        widgets::record(ui, &key, ctl);
        match (&def.kind, &val) {
            (PropKind::Boolean, PropValue::Bool(b)) => {
                if let Some(next) = toggle_switch(ui, ctl, &key, *b) {
                    app.dev.playground.set_prop(id, def.name.clone(), PropValue::Bool(next));
                }
            }
            (PropKind::Text, PropValue::Text(s)) => {
                if let Some(next) = live_text_input(ui, ctl, &key, s) {
                    app.dev.playground.set_prop(id, def.name.clone(), PropValue::Text(next));
                }
            }
            (PropKind::Variant { options }, v) => {
                let cur = match v {
                    PropValue::Variant(s) => s.clone(),
                    _ => options.first().cloned().unwrap_or_default(),
                };
                if let Some(i) = dropdown(ui, ctl, &key, &cur, options) {
                    if let Some(chosen) = options.get(i) {
                        app.dev.playground.set_prop(id, def.name.clone(), PropValue::Variant(chosen.clone()));
                    }
                }
            }
            (PropKind::InstanceSwap { preferred }, v) => {
                let cur = match v {
                    PropValue::Instance(cid) => comp_label(&app.doc, *cid),
                    _ => String::new(),
                };
                let dd_id = dropdown_open_id(ui, &key);
                let open = ui.data(|d| d.get_temp::<bool>(dd_id)).unwrap_or(false);
                let candidates = swap_choices(app, open, preferred, comp_id);
                if open && candidates.is_empty() {
                    ui.data_mut(|d| d.insert_temp(dd_id, false));
                }
                if candidates.is_empty() && !(preferred.is_empty() && !open && swap_target_exists(app, comp_id)) {
                    disabled_select(ui, ctl, "No components to swap to");
                } else {
                    let names: Vec<String> = candidates.iter().map(|cid| comp_label(&app.doc, *cid)).collect();
                    if let Some(i) = dropdown(ui, ctl, &key, &cur, &names) {
                        if let Some(&chosen) = candidates.get(i) {
                            app.dev.playground.set_prop(id, def.name.clone(), PropValue::Instance(chosen));
                        }
                    }
                }
            }
            _ => {}
        }
        c.y += theme::CONTROL_STANDARD;
    }
    c.y += theme::SPACE_INLINE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev::Language;

    struct Fx {
        d: Document,
        page: NodeId,
        button: NodeId,
        star: NodeId,
        card: NodeId,
        card_inst: NodeId,
        nested_button: NodeId,
    }

    fn run(d: &mut Document, f: impl FnOnce(&mut Document) -> Command) {
        let c = f(d);
        c.apply(d).unwrap();
    }

    fn named_child(d: &Document, parent: NodeId, name: &str) -> NodeId {
        *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
    }

    /// A Button component (`Label` text and `Show icon` boolean props), a `Star` component, a Card
    /// component holding a Button instance whose main is swappable through its `Icon` prop, and an
    /// instance of that Card on the page.
    fn fixture(unrelated: usize) -> Fx {
        let mut d = Document::new("playground");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut b = Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 100.0, 40.0));
        b.name = "Button".into();
        let button = d.insert(page, b).unwrap();
        let id = d.alloc_id();
        let mut t = Node::text(id, designos_doc::Rect::new(10.0, 10.0, 60.0, 20.0), "Label", TextStyle::default());
        t.name = "label".into();
        let label = d.insert(button, t).unwrap();
        let id = d.alloc_id();
        let mut ic = Node::ellipse(id, designos_doc::Rect::new(75.0, 10.0, 20.0, 20.0));
        ic.name = "icon".into();
        let icon = d.insert(button, ic).unwrap();
        run(&mut d, |d| create_component(d, button));
        let defs = [
            ("Label", PropKind::Text, PropValue::Text("Label".into()), label, OverrideField::Characters),
            ("Show icon", PropKind::Boolean, PropValue::Bool(true), icon, OverrideField::Visible),
        ];
        for (name, kind, default, layer, field) in defs {
            run(&mut d, |d| add_prop(d, button, ComponentPropDef { name: name.into(), kind, default }));
            run(&mut d, |d| bind_prop(d, layer, field, Some(name)));
        }

        let id = d.alloc_id();
        let mut star = Node::frame(id, designos_doc::Rect::new(0.0, 100.0, 24.0, 24.0));
        star.name = "Star".into();
        let star = d.insert(page, star).unwrap();
        let id = d.alloc_id();
        d.insert(star, Node::ellipse(id, designos_doc::Rect::new(0.0, 0.0, 8.0, 8.0))).unwrap();
        run(&mut d, |d| create_component(d, star));

        let id = d.alloc_id();
        let mut card = Node::frame(id, designos_doc::Rect::new(0.0, 200.0, 300.0, 100.0));
        card.name = "Card".into();
        let card = d.insert(page, card).unwrap();
        run(&mut d, |d| create_component(d, card));
        run(&mut d, |d| create_instance(d, button, card, designos_doc::Vec2::ZERO));
        let slot = *d.get(card).unwrap().children.last().unwrap();
        let swap =
            ComponentPropDef { name: "Icon".into(), kind: PropKind::InstanceSwap { preferred: vec![star, button] }, default: PropValue::Instance(button) };
        run(&mut d, |d| add_prop(d, card, swap));
        run(&mut d, |d| bind_prop(d, slot, OverrideField::MainComponent, Some("Icon")));
        run(&mut d, |d| create_instance(d, card, page, designos_doc::Vec2::new(0.0, 400.0)));
        let card_inst = *d.get(page).unwrap().children.last().unwrap();
        let nested_button = d.get(card_inst).unwrap().children[0];
        assert!(matches!(d.get(nested_button).unwrap().kind, NodeKind::Instance { .. }), "the card holds a button instance");

        for i in 0..unrelated {
            let id = d.alloc_id();
            let mut f = Node::frame(id, designos_doc::Rect::new(i as f32, 900.0, 10.0, 10.0));
            f.fills = vec![Paint {
                kind: PaintKind::Image { image_ref: "unrelated".into(), scale_mode: ImageScaleMode::Fill },
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
            }];
            d.insert(page, f).unwrap();
        }
        d.images.insert("unrelated".into(), vec![7u8; 4096]);
        Fx { d, page, button, star, card, card_inst, nested_button }
    }

    fn pg_with(id: NodeId, values: &[(&str, PropValue)]) -> PlaygroundState {
        let mut pg = PlaygroundState::default();
        for (k, v) in values {
            pg.set_prop(id, (*k).to_string(), v.clone());
        }
        pg
    }

    #[test]
    fn scratch_document_follows_the_component_not_the_file() {
        let small = fixture(0);
        let big = fixture(600);
        let pg_small = pg_with(small.card_inst, &[("Icon", PropValue::Instance(small.star))]);
        let pg_big = pg_with(big.card_inst, &[("Icon", PropValue::Instance(big.star))]);
        let (s_small, _, _) = build_scratch_doc(&small.d, small.card_inst, &pg_small).unwrap();
        let (s_big, _, _) = build_scratch_doc(&big.d, big.card_inst, &pg_big).unwrap();
        assert!(big.d.nodes.len() > 600, "the unrelated frames are in the file");
        assert_eq!(s_big.nodes.len(), s_small.nodes.len(), "unrelated layers must not reach the scratch document");
        assert!(s_big.nodes.len() < 40, "scratch holds {} nodes", s_big.nodes.len());
        assert!(s_big.images.is_empty(), "images nothing in the scratch document paints must stay behind");
        // The component selected directly is bounded the same way.
        let (s_main, _, _) = build_scratch_doc(&big.d, big.card, &pg_with(big.card, &[("Icon", PropValue::Instance(big.star))])).unwrap();
        assert!(s_main.nodes.len() < 40, "scratch holds {} nodes", s_main.nodes.len());
    }

    #[test]
    fn a_nested_instance_is_the_playground_target() {
        let f = fixture(0);
        assert_eq!(playground_target(&f.d, f.nested_button), Some((f.nested_button, f.button)));
        let label = named_child(&f.d, f.nested_button, "label");
        assert_eq!(playground_target(&f.d, label), Some((f.nested_button, f.button)), "a layer inside the nested button belongs to the button");
        assert_eq!(playground_target(&f.d, f.card_inst), Some((f.card_inst, f.card)));

        let pg = pg_with(f.nested_button, &[("Show icon", PropValue::Bool(false)), ("Label", PropValue::Text("Pay now".into()))]);
        let (scratch, code_id, preview_id) = build_scratch_doc(&f.d, f.nested_button, &pg).unwrap();
        assert_eq!(code_id, f.nested_button);
        assert_eq!(preview_id, f.nested_button);
        let icon = named_child(&scratch, preview_id, "icon");
        assert!(!scratch.get(icon).unwrap().visible, "the preview shows the button without its icon");
        let label = named_child(&scratch, preview_id, "label");
        assert!(matches!(&scratch.get(label).unwrap().kind, NodeKind::Text(t) if t.content == "Pay now"), "the preview shows the new label");
        let code = Language::React.generate(&scratch, code_id, false);
        assert!(code.contains("showIcon={false}") && code.contains("label=\"Pay now\""), "code follows the nested button:\n{code}");
        assert!(!code.contains("<Card"), "code is not the card's:\n{code}");
        // The file is untouched.
        let base_icon = named_child(&f.d, f.nested_button, "icon");
        assert!(f.d.get(base_icon).unwrap().visible);
    }

    #[test]
    fn a_nested_instance_keeps_what_its_card_customised_while_a_prop_is_edited() {
        let mut f = fixture(0);
        // The card's own copy of the button says "Pay"; the card instance mirrors that.
        let in_card = named_child(&f.d, f.card, "Button");
        let in_card_label = named_child(&f.d, in_card, "label");
        let mut n = f.d.get(in_card_label).unwrap().clone();
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = "Pay".into();
        }
        n.component.overrides.insert(OverrideField::Characters);
        Command::Update { node: Box::new(n) }.apply(&mut f.d).unwrap();
        run(&mut f.d, |d| sync_instances(d, f.card));
        let label = named_child(&f.d, f.nested_button, "label");
        assert!(matches!(&f.d.get(label).unwrap().kind, NodeKind::Text(t) if t.content == "Pay"), "the card instance shows the card's label");
        let kids_before = f.d.get(f.nested_button).unwrap().children.len();

        let pg = pg_with(f.nested_button, &[("Show icon", PropValue::Bool(false))]);
        let (scratch, _, preview_id) = build_scratch_doc(&f.d, f.nested_button, &pg).unwrap();
        assert_eq!(scratch.get(preview_id).unwrap().children.len(), kids_before, "layers are kept, not rebuilt");
        let label = named_child(&scratch, preview_id, "label");
        assert!(matches!(&scratch.get(label).unwrap().kind, NodeKind::Text(t) if t.content == "Pay"), "the card's label survives the edit");
        assert!(!scratch.get(named_child(&scratch, preview_id, "icon")).unwrap().visible);
    }

    #[test]
    fn an_icon_swap_on_a_main_component_reaches_the_code_and_the_preview() {
        let f = fixture(0);
        let pg = pg_with(f.card, &[("Icon", PropValue::Instance(f.star))]);
        let (scratch, code_id, preview_id) = build_scratch_doc(&f.d, f.card, &pg).unwrap();
        assert_eq!(code_id, preview_id, "the code is generated from the node the preview renders");
        assert!(matches!(scratch.get(code_id).unwrap().kind, NodeKind::Instance { .. }));
        let swapped = scratch.get(preview_id).unwrap().children[0];
        assert_eq!(instance_component(scratch.get(swapped).unwrap()), Some(f.star), "the preview shows the star");
        let code = Language::React.generate(&scratch, code_id, false);
        assert!(code.contains("icon={<Star />}"), "swap missing from the code:\n{code}");
        // A layer inside the main component maps to its copy in the scratch instance.
        let (_, layer_code_id, _) = build_scratch_doc(&f.d, f.button, &pg_with(f.button, &[("Show icon", PropValue::Bool(false))])).unwrap();
        assert_ne!(layer_code_id, f.button, "the code is for the scratch instance, not the main component");
    }

    #[test]
    fn scratch_code_matches_the_file_when_nothing_is_edited() {
        let f = fixture(0);
        let base = Language::React.generate(&f.d, f.card_inst, false);
        let pg = pg_with(f.card_inst, &[("Icon", PropValue::Instance(f.button))]);
        let (scratch, code_id, _) = build_scratch_doc(&f.d, f.card_inst, &pg).unwrap();
        assert_eq!(Language::React.generate(&scratch, code_id, false), base);
        assert!(f.d.get(f.page).is_some());
    }

    #[test]
    fn swap_targets_exclude_the_component_family_and_components_that_would_hold_themselves() {
        let f = fixture(0);
        // The card holds a button instance: swapping the card into the button would make the card hold itself.
        assert_eq!(valid_swap_targets(&f.d, f.button), vec![f.star]);
        let mut for_card = valid_swap_targets(&f.d, f.card);
        for_card.sort();
        let mut expected = vec![f.button, f.star];
        expected.sort();
        assert_eq!(for_card, expected);
    }
}
