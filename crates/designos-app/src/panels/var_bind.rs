//! Inspector side of variables: the "bind variable" hover button, the bound
//! variable pill that replaces a field, the picker, and the per-collection
//! mode selector of the Appearance section.

use super::properties::{row_end, Pane};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use crate::variables::widgets::{hexagon, pick_item, picker, pill, to_c32, PickResult, PillResponse};
use crate::variables::BindPicker;
use crate::widgets::menu::dropdown;
use crate::widgets::{self};
use designos_doc::*;
use egui::{Align2, Context, Id, Pos2, Rect, Sense, Ui, Vec2};

/// Picker ids of library variables: `LIB_PREFIX <library id> LIB_SEP <asset key>`.
const LIB_PREFIX: &str = "lib\u{1f}";
const LIB_SEP: char = '\u{1e}';

#[derive(Clone)]
struct BoundStopPill {
    label: String,
    swatch: Option<egui::Color32>,
    style_name: Option<String>,
}

#[derive(Clone, Default)]
struct PickerStopBindState {
    prefix: Option<(&'static str, usize)>,
    has_vars: bool,
    shot_hover: bool,
    open_key: Option<String>,
    bound_stops: std::collections::BTreeMap<String, BoundStopPill>,
    cached_paint: Option<Paint>,
    cached_ids: Vec<NodeId>,
    cached_target: Option<super::properties::PickTarget>,
    cached_runtime_id: u64,
    cached_epoch: u64,
    cached_library_stamp: u64,
}

#[derive(Clone, Default)]
enum PickerStopBindAction {
    #[default]
    None,
    Open {
        key: String,
        anchor: Rect,
    },
    Detach {
        key: String,
    },
}

fn picker_state_id() -> Id {
    Id::new("picker-stop-bind-state")
}

fn picker_action_id() -> Id {
    Id::new("picker-stop-bind-action")
}

/// The paint a picker slot currently holds in the document, or `None` when the node no longer
/// has that slot (fill/stroke index, visible text fill, grid, effect of the right kind, noise
/// slot). `SelColor` has no document source and is not handled here.
fn picker_slot_paint(n: &Node, slot: super::properties::PickTarget) -> Option<Paint> {
    use super::properties::PickTarget;
    match slot {
        PickTarget::Fill(pi) => n.fills.get(pi).cloned(),
        PickTarget::Stroke(pi) => n.stroke.paints.get(pi).cloned(),
        PickTarget::TextFill => match &n.kind {
            NodeKind::Text(t) => crate::richtext::summary(t, 0..t.content.len()).fills.and_then(|f| f.into_iter().find(|p| p.visible)),
            _ => None,
        },
        PickTarget::PageBg => match n.kind {
            NodeKind::Page { background } => Some(Paint::solid(background)),
            _ => None,
        },
        PickTarget::GridColor(i) => n.kind.frame_props().and_then(|fp| fp.grids.get(i)).map(|g| Paint::solid(super::layoutguide::color_of(g))),
        PickTarget::Shadow(i) => n.effects.get(i).and_then(|e| match e {
            Effect::DropShadow { color, .. } | Effect::InnerShadow { color, .. } => Some(Paint::solid(*color)),
            _ => None,
        }),
        PickTarget::NoiseColor(i, second) => n.effects.get(i).and_then(|e| match e {
            Effect::Noise(noise) => match (&noise.mode, second) {
                (NoiseMode::Mono { color }, false) | (NoiseMode::Duo { color, .. }, false) => Some(Paint::solid(*color)),
                (NoiseMode::Duo { secondary, .. }, true) => Some(Paint::solid(*secondary)),
                _ => None,
            },
            _ => None,
        }),
        PickTarget::SelColor(_) => None,
    }
}

/// After undo or redo: the picker shows what the document now holds, whatever paint type that is.
/// A picker whose node or slot no longer exists is closed, so the next edit cannot hit a
/// missing index. `SelColor` has no document source and keeps its paint.
pub(crate) fn adopt_picker_paint_after_history(app: &mut App) {
    let Some(picker) = app.props.picker.as_ref() else { return };
    if !picker.tx.ed.touched().is_empty() {
        return;
    }
    let doc_paint = if matches!(picker.target, super::properties::PickTarget::SelColor(_)) {
        Some(picker.paint.clone())
    } else {
        picker.ids.first().and_then(|id| app.doc.get(*id)).and_then(|n| picker_slot_paint(n, picker.target))
    };
    let Some(doc_paint) = doc_paint else {
        app.props.picker = None;
        app.props.eyedropper = None;
        return;
    };
    let Some(pk) = app.props.picker.as_mut() else { return };
    pk.paint = doc_paint;
    let n_stops = crate::widgets::picker::stops_of(&pk.paint.kind).map(|s| s.len()).unwrap_or(1);
    pk.state.sel = pk.state.sel.min(n_stops.saturating_sub(1));
    pk.state.reset_stop_origins(&pk.paint);
    let rgba = crate::widgets::picker::active_rgba(&pk.paint, pk.state.sel);
    pk.state.sync(rgba);
}

fn sync_picker_paint_from_doc(app: &mut App) {
    let Some(pk) = app.props.picker.as_mut() else { return };
    if !pk.tx.ed.touched().is_empty() {
        return;
    }
    let Some(&first) = pk.ids.first() else { return };
    let Some(n) = app.doc.get(first) else { return };
    let doc_paint = match pk.target {
        super::properties::PickTarget::Fill(pi) => n.fills.get(pi).cloned(),
        super::properties::PickTarget::Stroke(pi) => n.stroke.paints.get(pi).cloned(),
        _ => None,
    };
    if let Some(p) = doc_paint {
        if crate::widgets::picker::mode_of(&p.kind) == crate::widgets::picker::mode_of(&pk.paint.kind) {
            pk.paint = p;
            let n_stops = crate::widgets::picker::stops_of(&pk.paint.kind).map(|s| s.len()).unwrap_or(1);
            pk.state.sel = pk.state.sel.min(n_stops.saturating_sub(1));
            pk.state.reset_stop_origins(&pk.paint);
            let rgba = crate::widgets::picker::active_rgba(&pk.paint, pk.state.sel);
            pk.state.sync(rgba);
        }
    }
}

#[derive(Clone, Default)]
struct CachedLibVars {
    stamp: u64,
    vars: std::sync::Arc<[(VariableKind, Vec<String>)]>,
}

/// Figma scope names `scope_allows` knows; any other scope string never matches a key.
const SCOPE_NAMES: [&str; 23] = [
    "ALL_FILLS",
    "FRAME_FILL",
    "SHAPE_FILL",
    "TEXT_FILL",
    "STROKE",
    "STROKE_COLOR",
    "EFFECT_COLOR",
    "EFFECT_FLOAT",
    "WIDTH_HEIGHT",
    "CORNER_RADIUS",
    "GAP",
    "OPACITY",
    "COLOR_OPACITY",
    "STROKE_FLOAT",
    "FONT_SIZE",
    "FONT_WEIGHT",
    "LINE_HEIGHT",
    "LETTER_SPACING",
    "PARAGRAPH_SPACING",
    "PARAGRAPH_INDENT",
    "FONT_FAMILY",
    "FONT_STYLE",
    "TEXT_CONTENT",
];

fn scope_name_strings() -> &'static [String] {
    static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| SCOPE_NAMES.iter().map(|s| s.to_string()).chain(std::iter::once("__unknown__".to_string())).collect())
}

/// Per variable kind: is there any variable with no scope limit, any scoped variable at all, and which known scopes occur.
#[derive(Clone, Copy, Default)]
struct ScopeCensus {
    pass_nr: u64,
    local_vars_len: usize,
    enabled_len: usize,
    lib_stamp: u64,
    unscoped: [bool; 4],
    scoped: [bool; 4],
    bits: [u32; 4],
}

fn kind_index(kind: VariableKind) -> usize {
    match kind {
        VariableKind::Color => 0,
        VariableKind::Number => 1,
        VariableKind::String => 2,
        VariableKind::Boolean => 3,
    }
}

fn lib_meta_stamp(app: &App) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    app.libs.root.hash(&mut h);
    app.doc.library.enabled.len().hash(&mut h);
    for e in &app.doc.library.enabled {
        e.id.hash(&mut h);
        e.version.hash(&mut h);
    }
    app.libs.hash_versions(&mut h);
    h.finish()
}

fn shared_lib_vars(app: &mut App, ctx: &Context, stamp: u64) -> std::sync::Arc<[(VariableKind, Vec<String>)]> {
    if app.doc.library.enabled.is_empty() {
        return std::sync::Arc::from([]);
    }
    let cache_id = Id::new("var-bind-lib-vars-cache");
    if let Some(c) = ctx.data(|d| d.get_temp::<CachedLibVars>(cache_id)).filter(|c| c.stamp == stamp) {
        return c.vars;
    }
    let v: std::sync::Arc<[(VariableKind, Vec<String>)]> = app.library_variables().into_iter().map(|lv| (lv.kind, lv.scopes)).collect::<Vec<_>>().into();
    let stamp = lib_meta_stamp(app);
    ctx.data_mut(|d| {
        d.insert_temp(cache_id, CachedLibVars { stamp, vars: std::sync::Arc::clone(&v) });
    });
    v
}

fn census_add(c: &mut ScopeCensus, kind: VariableKind, scopes: &[String]) {
    let ki = kind_index(kind);
    if c.unscoped[ki] {
        return;
    }
    if scopes.is_empty() || scopes.iter().any(|s| s == "ALL_SCOPES") {
        c.unscoped[ki] = true;
        return;
    }
    c.scoped[ki] = true;
    for s in scopes {
        if let Some(i) = SCOPE_NAMES.iter().position(|n| n == s) {
            c.bits[ki] |= 1 << i;
        }
    }
}

fn pass_scope_census(app: &mut App, ctx: &Context) -> ScopeCensus {
    let pass_nr = ctx.cumulative_pass_nr();
    let local_vars_len = app.doc.variables.len();
    let enabled_len = app.doc.library.enabled.len();
    let lib_stamp = if enabled_len > 0 { lib_meta_stamp(app) } else { 0 };
    let id = Id::new("var-bind-scope-census");
    if let Some(t) = ctx
        .data(|d| d.get_temp::<ScopeCensus>(id))
        .filter(|t| t.pass_nr == pass_nr && t.local_vars_len == local_vars_len && t.enabled_len == enabled_len && t.lib_stamp == lib_stamp)
    {
        return t;
    }
    let mut c = ScopeCensus { pass_nr, local_vars_len, enabled_len, lib_stamp, ..Default::default() };
    #[cfg(test)]
    let mut visits = 0usize;
    for coll in app.doc.variable_collections.values().filter(|c| !c.is_soft_deleted()) {
        for v in coll.variables.iter().filter_map(|id| app.doc.variables.get(id)).filter(|v| !v.is_soft_deleted()) {
            #[cfg(test)]
            {
                visits += 1;
            }
            census_add(&mut c, v.kind, &v.scopes);
        }
    }
    if enabled_len > 0 {
        let vars = shared_lib_vars(app, ctx, lib_stamp);
        c.lib_stamp = lib_meta_stamp(app);
        for (k, scopes) in vars.iter() {
            #[cfg(test)]
            {
                visits += 1;
            }
            census_add(&mut c, *k, scopes);
        }
    }
    ctx.data_mut(|d| {
        d.insert_temp(id, c);
        #[cfg(test)]
        {
            let visit_id = Id::new("var-bind-pass-visits");
            let prev = d.get_temp::<(u64, usize)>(visit_id).filter(|(p, _)| *p == pass_nr).map(|(_, n)| n).unwrap_or(0);
            d.insert_temp(visit_id, (pass_nr, prev + visits));
        }
    });
    c
}

/// Whether at least one variable in `app.doc.variable_collections` or an enabled library
/// matches `kind` and `scope_allows(&v.scopes, key0, target_kind)`.
pub(super) fn has_matching_var(app: &mut App, ctx: &Context, key0: &str, kind: VariableKind, target_kind: &NodeKind) -> bool {
    let c = pass_scope_census(app, ctx);
    let ki = kind_index(kind);
    if c.unscoped[ki] {
        return true;
    }
    if !c.scoped[ki] {
        return false;
    }
    let names = scope_name_strings();
    // a key that `scope_allows` accepts whatever the scopes are (e.g. `visible`)
    if scope_allows(std::slice::from_ref(&names[SCOPE_NAMES.len()]), key0, target_kind) {
        return true;
    }
    (0..SCOPE_NAMES.len()).any(|i| c.bits[ki] & (1 << i) != 0 && scope_allows(std::slice::from_ref(&names[i]), key0, target_kind))
}

fn text_target_kind() -> &'static NodeKind {
    static KIND: std::sync::OnceLock<NodeKind> = std::sync::OnceLock::new();
    KIND.get_or_init(|| NodeKind::Text(TextProps::default()))
}

fn node_target_kind(app: &App, sel: &[NodeId]) -> &'static NodeKind {
    match sel.first().and_then(|id| app.doc.get(*id)).map(|n| &n.kind) {
        Some(NodeKind::Text(_)) => text_target_kind(),
        Some(NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Section) => &NodeKind::Section,
        _ => &NodeKind::Rectangle,
    }
}

fn style_target_kind(app: &App, style_key: &str) -> &'static NodeKind {
    match app.doc.styles.get(style_key) {
        Some(Style::Text(_)) => text_target_kind(),
        _ => &NodeKind::Rectangle,
    }
}

/// The key `scope_allows` is asked about for `prop` of style `style_key`. A stroke style keeps its
/// paints and bindings under `fills/…`, but its variables are scoped like the strokes of a layer.
fn style_scope_key<'a>(app: &App, style_key: &str, prop: &'a str) -> std::borrow::Cow<'a, str> {
    let is_stroke = app.doc.style_meta.get(style_key).is_some_and(|m| m.property == StyleProperty::Stroke);
    match prop.strip_prefix("fills/") {
        Some(rest) if is_stroke => std::borrow::Cow::Owned(format!("strokes/{rest}")),
        _ => std::borrow::Cow::Borrowed(prop),
    }
}

fn narrow_for_bind(rect: Rect, reserve: bool) -> Rect {
    if reserve {
        let max_x = (rect.right() - theme::TARGET_MIN - theme::BORDER_WIDTH).max(rect.left());
        Rect::from_min_max(rect.min, Pos2::new(max_x, rect.max.y))
    } else {
        rect
    }
}

fn tracks_gradient_stops(kind: &PaintKind) -> bool {
    crate::widgets::picker::mode_of(kind).is_gradient()
}

/// Returns the sub-rect available for a node field's value widget when `keys[0]` is not bound,
/// reserving the right-hand `TARGET_MIN + BORDER_WIDTH` strip whenever a matching variable exists.
pub fn value_rect(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str], kind: VariableKind) -> Rect {
    let target_kind = node_target_kind(app, sel);
    let has_vars = has_matching_var(app, ui.ctx(), keys[0], kind, target_kind);
    narrow_for_bind(rect, has_vars)
}

/// Returns the sub-rect available for a style editor field's value widget when `prop` is not bound,
/// reserving the right-hand `TARGET_MIN + BORDER_WIDTH` strip whenever a matching variable exists.
pub fn style_value_rect(app: &mut App, ui: &mut Ui, rect: Rect, style_key: &str, prop: &str, kind: VariableKind) -> Rect {
    let target_kind = style_target_kind(app, style_key);
    let scope_key = style_scope_key(app, style_key, prop);
    let has_vars = has_matching_var(app, ui.ctx(), &scope_key, kind, target_kind);
    narrow_for_bind(rect, has_vars)
}

pub(super) fn sync_picker_context(app: &mut App, ctx: &Context) {
    sync_picker_paint_from_doc(app);
    let shot_hover = app.canvas.shot_pointer.is_some();
    // Stop bindings only exist for gradient paints. Solid and image pickers
    // used to walk every selected node's bindings on every frame even though
    // they can never render a stop field. Clear the cached state when the
    // picker changes mode so a previous gradient cannot leak into the next
    // picker, then skip the per-frame node binding walk on cache hits.
    let Some(pk) = app.props.picker.as_ref() else {
        ctx.data_mut(|d| d.insert_temp(picker_state_id(), PickerStopBindState { shot_hover, ..Default::default() }));
        return;
    };
    if !matches!(pk.target, super::properties::PickTarget::Fill(_) | super::properties::PickTarget::Stroke(_)) || !tracks_gradient_stops(&pk.paint.kind) {
        ctx.data_mut(|d| d.insert_temp(picker_state_id(), PickerStopBindState { shot_hover, ..Default::default() }));
        return;
    }
    let target = pk.target;
    let open_key = app.vars.bind_picker.as_ref().and_then(|b| b.keys.first()).map(String::as_str);
    let runtime_id = app.doc.runtime_id();
    let epoch = app.doc.mutation_epoch();
    let library_stamp = lib_meta_stamp(app);
    let cache_valid = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<PickerStopBindState>(picker_state_id());
        let valid = s.cached_target == Some(target)
            && s.cached_ids.as_slice() == pk.ids.as_slice()
            && s.cached_paint.as_ref() == Some(&pk.paint)
            && s.cached_runtime_id == runtime_id
            && s.cached_epoch == epoch
            && s.cached_library_stamp == library_stamp;
        if valid {
            s.shot_hover = shot_hover;
            if s.open_key.as_deref() != open_key {
                s.open_key = open_key.map(str::to_owned);
            }
        }
        valid
    });
    if cache_valid {
        return;
    }
    let ids = pk.ids.clone();
    let paint = pk.paint.clone();
    let mut state = PickerStopBindState {
        shot_hover,
        cached_ids: ids.clone(),
        cached_target: Some(target),
        cached_paint: Some(paint),
        cached_runtime_id: runtime_id,
        cached_epoch: epoch,
        cached_library_stamp: library_stamp,
        open_key: open_key.map(str::to_owned),
        ..Default::default()
    };
    let pk_info = app.props.picker.as_ref().and_then(|pk| {
        let prefix = match pk.target {
            super::properties::PickTarget::Fill(pi) => Some(("fills", pi)),
            super::properties::PickTarget::Stroke(pi) => Some(("strokes", pi)),
            _ => None,
        };
        prefix.filter(|_| !pk.ids.is_empty()).map(|(pfx, pi)| (pfx, pi, pk.ids.clone()))
    });
    if let Some((pfx, pi, pk_ids)) = pk_info {
        state.prefix = Some((pfx, pi));
        let nodes: Vec<NodeId> = pk_ids.iter().copied().filter(|&id| app.doc.get(id).is_some()).collect();
        let target_kind = node_target_kind(app, &nodes);
        let sample_key = format!("{pfx}/{pi}/stops/0");
        state.has_vars = has_matching_var(app, ctx, &sample_key, VariableKind::Color, target_kind);
        state.cached_library_stamp = lib_meta_stamp(app);
        let mut keys = std::collections::BTreeSet::new();
        for &id in &nodes {
            collect_bound_stop_keys(&app.doc, id, pfx, pi, &mut keys);
        }
        for k in keys {
            let mut uniform: Option<(&str, &str, Option<egui::Color32>)> = None;
            let mut mixed = false;
            for &id in &nodes {
                match bound_var(app, id, &k) {
                    Some(var) => {
                        let swatch = match app.doc.bound_value(id, &k) {
                            Some(VariableValue::Color(c)) => Some(to_c32(c)),
                            _ => None,
                        };
                        match uniform {
                            None => uniform = Some((var.id.as_str(), var.name.as_str(), swatch)),
                            Some((fid, fname, fsw)) => {
                                if fid != var.id.as_str() {
                                    mixed = true;
                                    break;
                                }
                                if fsw != swatch {
                                    uniform = Some((fid, fname, None));
                                }
                            }
                        }
                    }
                    None => {
                        mixed = true;
                        break;
                    }
                }
            }
            let style_name = nodes.iter().find_map(|&id| style_bound_stop_style_name(&app.doc, id, &k).map(str::to_string));
            if mixed {
                state.bound_stops.insert(k, BoundStopPill { label: "Mixed".to_string(), swatch: None, style_name });
            } else if let Some((_, name, swatch)) = uniform {
                state.bound_stops.insert(k, BoundStopPill { label: name.to_string(), swatch, style_name });
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(picker_state_id(), state));
}

#[cfg(test)]
mod tests {
    use super::{picker_state_id, sync_picker_context, PickerStopBindState};
    use crate::app::App;
    use crate::edit::Tx;
    use crate::panels::properties::{PickTarget, Picker};
    use crate::variables::widgets::to_c32;
    use designos_doc::{Color, Document, GradientStop, Node, Paint, PaintKind, VariableKind, VariableStore, VariableValue, Vec2};
    use egui::{Context, Pos2, Rect};

    fn tmp_test_lib_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("designos-var-bind-{tag}-{}", designos_library::new_id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn solid_picker_clears_gradient_stop_context() {
        let mut doc = Document::new("PickerContext");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        let paint = Paint::solid(Color::BLACK);
        app.props.picker = Some(Picker {
            target: PickTarget::Fill(0),
            tx: Tx::new(),
            anchor: Rect::from_min_max(Pos2::ZERO, Pos2::ZERO),
            ids: vec![page],
            paint: paint.clone(),
            state: crate::widgets::picker::State::new(&paint),
            sources: Vec::new(),
            eyedrop: false,
        });
        let ctx = Context::default();
        ctx.data_mut(|data| {
            let stale = PickerStopBindState { prefix: Some(("fills", 0)), ..Default::default() };
            data.insert_temp(picker_state_id(), stale);
        });

        sync_picker_context(&mut app, &ctx);

        let state = ctx.data(|data| data.get_temp::<PickerStopBindState>(picker_state_id()));
        assert!(state.is_some_and(|state| state.prefix.is_none()), "solid picker must not retain gradient stop state");
    }

    #[test]
    fn unchanged_gradient_picker_reuses_binding_context() {
        let root = tmp_test_lib_dir("picker-cache-reuse");

        // Publish library v1 with only a Number variable (no colour variables in v1).
        let mut lib_doc = Document::new("SharedLib");
        lib_doc.add_page("Page 1");
        let mut lib_app = App::new(lib_doc);
        lib_app.libs.set_root(root.clone());
        let mut lib_store = VariableStore::default();
        let lib_coll = lib_store.add_collection("Tokens");
        let lib_mode = lib_store.collections[&lib_coll].default_mode.clone();
        let num_var = lib_store.add_variable(&lib_coll, "Space/Gap", VariableKind::Number).unwrap();
        lib_store.set_value(&num_var, &lib_mode, VariableValue::Number(8.0));
        lib_app.commit_store(lib_store.clone(), &[], &[]);
        assert!(lib_app.publish_file(), "publish library v1 without colour variables");
        let lib_id = lib_app.doc.library.published.as_ref().unwrap().library_id.clone();

        let mut doc = Document::new("PickerContext");
        let page = doc.add_page("Page 1");
        let rect_id = doc.alloc_id();
        let start_color = Color::rgb(0x12, 0x34, 0x56);
        let updated_color = Color::rgb(0xAB, 0xCD, 0xEF);
        let paint = Paint {
            kind: PaintKind::LinearGradient {
                handles: [Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: start_color }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: designos_doc::BlendMode::Normal,
        };
        let mut rect = Node::rectangle(rect_id, designos_doc::Rect::new(0.0, 0.0, 100.0, 100.0));
        rect.fills = vec![paint.clone()];
        doc.insert(page, rect).unwrap();

        let mut app = App::new(doc);
        app.libs.set_root(root.clone());
        app.set_library_enabled(&lib_id, true);

        let mut store = VariableStore::default();
        let coll = store.add_collection("Local");
        let mode = store.collections[&coll].default_mode.clone();
        let stop_var = store.add_variable(&coll, "Grad/StopStart", VariableKind::Color).unwrap();
        store.set_value(&stop_var, &mode, VariableValue::Color(start_color));
        store.variables.get_mut(&stop_var).unwrap().scopes = vec!["ALL_FILLS".into()];
        app.commit_store(store.clone(), &[], &[]);
        app.bind_variable(&[rect_id], &["fills/0/stops/0"], &stop_var);

        let active_paint = app.doc.get(rect_id).unwrap().fills[0].clone();
        app.props.picker = Some(Picker {
            target: PickTarget::Fill(0),
            tx: Tx::new(),
            anchor: Rect::from_min_max(Pos2::ZERO, Pos2::ZERO),
            ids: vec![rect_id],
            paint: active_paint.clone(),
            state: crate::widgets::picker::State::new(&active_paint),
            sources: Vec::new(),
            eyedrop: false,
        });
        let ctx = Context::default();
        sync_picker_context(&mut app, &ctx);

        let pill =
            ctx.data(|data| data.get_temp::<PickerStopBindState>(picker_state_id()).and_then(|s| s.bound_stops.get("fills/0/stops/0").cloned())).unwrap();
        assert_eq!(pill.label, "Grad/StopStart");
        assert_eq!(pill.swatch, Some(to_c32(start_color)));
        assert_eq!(pill.style_name, None);

        // Overwrite the cached label in place; an unchanged second sync must reuse the cached entry.
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<PickerStopBindState>(picker_state_id()).bound_stops.get_mut("fills/0/stops/0").unwrap().label = "cached-hit".into();
        });
        sync_picker_context(&mut app, &ctx);
        let pill_cached =
            ctx.data(|data| data.get_temp::<PickerStopBindState>(picker_state_id()).and_then(|s| s.bound_stops.get("fills/0/stops/0").cloned())).unwrap();
        assert_eq!(pill_cached.label, "cached-hit", "unchanged picker must reuse cached binding context");

        // A real variable edit invalidates the cache and rebuilds the stop pill from the document.
        store.variables.get_mut(&stop_var).unwrap().name = "Grad/StopRenamed".into();
        store.set_value(&stop_var, &mode, VariableValue::Color(updated_color));
        app.commit_store(store, &[], &[]);
        sync_picker_context(&mut app, &ctx);
        let pill_rebuilt =
            ctx.data(|data| data.get_temp::<PickerStopBindState>(picker_state_id()).and_then(|s| s.bound_stops.get("fills/0/stops/0").cloned())).unwrap();
        assert_eq!(pill_rebuilt.label, "Grad/StopRenamed", "real edit must invalidate and rebuild bound_stops");
        assert_eq!(pill_rebuilt.swatch, Some(to_c32(updated_color)));

        // Remove local variables so neither local nor library v1 has a colour variable; `has_vars` is false.
        app.detach_variable(&[rect_id], &["fills/0/stops/0"]);
        app.commit_store(VariableStore::default(), &[], &[]);
        sync_picker_context(&mut app, &ctx);
        let has_vars_v1 = ctx.data(|data| data.get_temp::<PickerStopBindState>(picker_state_id()).map(|s| s.has_vars));
        assert_eq!(has_vars_v1, Some(false), "with no local or library v1 colour variables, has_vars must be false");

        // Publish library v2 with an ALL_FILLS colour variable and advance `libs_tick` while leaving `doc.library.enabled` pinned at v1.
        let lib_color_var = lib_store.add_variable(&lib_coll, "Color/LibStop", VariableKind::Color).unwrap();
        lib_store.set_value(&lib_color_var, &lib_mode, VariableValue::Color(Color::rgb(0x33, 0x88, 0xFF)));
        lib_store.variables.get_mut(&lib_color_var).unwrap().scopes = vec!["ALL_FILLS".into()];
        lib_app.commit_store(lib_store, &[], &[]);
        assert!(lib_app.publish_file(), "publish library v2 with ALL_FILLS colour variable");

        app.now += 10.0;
        app.libs_tick();
        assert_eq!(app.doc.library.enabled[0].version, 1, "enabled library pin must stay at v1 before applying update");

        sync_picker_context(&mut app, &ctx);
        let has_vars_v2 = ctx.data(|data| data.get_temp::<PickerStopBindState>(picker_state_id()).map(|s| s.has_vars));
        assert_eq!(has_vars_v2, Some(true), "advancing library latest/pending via libs_tick must invalidate cache and set has_vars to true");

        let _ = std::fs::remove_dir_all(root);
    }
}

/// Remaps index-keyed variable bindings under `prefix` (e.g. `"fills/0/stops/"` or `"effects/"`)
/// using `origins[new_idx] = Some(old_idx)`. Keys whose `old_idx` does not appear in `origins`
/// are removed; keys whose `old_idx` moves to `new_idx` keep their trailing suffix (empty for
/// stops, `"/{field}"` for effects) at `"{prefix}{new_idx}{suffix}"`.
pub(super) fn remap_indexed_bindings(bindings: &mut std::collections::BTreeMap<String, String>, prefix: &str, origins: &[Option<usize>]) {
    let old: Vec<(usize, String, String)> = bindings
        .keys()
        .filter_map(|k| {
            let rest = k.strip_prefix(prefix)?;
            let (idx_str, suffix) = match rest.find('/') {
                Some(pos) => (&rest[..pos], &rest[pos..]),
                None => (rest, ""),
            };
            let idx = idx_str.parse::<usize>().ok()?;
            Some((idx, suffix.to_string(), k.clone()))
        })
        .collect();
    if old.is_empty() {
        return;
    }
    let mut removed = Vec::with_capacity(old.len());
    for (idx, suffix, k) in old {
        if let Some(vid) = bindings.remove(&k) {
            removed.push((idx, suffix, vid));
        }
    }
    for (new_idx, orig) in origins.iter().enumerate() {
        if let Some(orig_idx) = orig {
            for (old_idx, suffix, vid) in &removed {
                if old_idx == orig_idx {
                    bindings.insert(format!("{prefix}{new_idx}{suffix}"), vid.clone());
                }
            }
        }
    }
}

fn commit_picker_tx_before_bind(app: &mut App) {
    app.commit_pending_picker_tx();
}

fn draw_pill(
    ui: &mut Ui,
    rect: Rect,
    record_name: impl FnOnce() -> String,
    id: impl crate::widgets::Key,
    label: &str,
    swatch: Option<egui::Color32>,
) -> PillResponse {
    widgets::record_with(ui, record_name, rect);
    pill(ui, rect, id, label, swatch, true)
}

fn draw_hover_button(
    ui: &mut Ui,
    rect: Rect,
    record_name: impl FnOnce() -> String,
    id: impl crate::widgets::Key,
    has_vars: bool,
    is_open: bool,
    always_visible: bool,
) -> bool {
    if !has_vars || (!always_visible && !ui.rect_contains_pointer(rect) && !is_open) {
        return false;
    }
    let b = Rect::from_center_size(Pos2::new(rect.right() - theme::TARGET_MIN / 2.0, rect.center().y), Vec2::splat(theme::TARGET_MIN));
    let r = ui.interact(b, ui.id().with(id), Sense::click());
    let p = ui.painter();
    if always_visible {
        if r.hovered() || is_open {
            p.rect_filled(b, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
            hexagon(p, b.center(), 5.0, theme::ICON, false);
        } else {
            hexagon(p, b.center(), 5.0, theme::ICON_QUATERNARY, false);
        }
    } else {
        p.rect_filled(b, theme::cr(theme::RADIUS_CONTROL), if r.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        hexagon(p, b.center(), 5.0, if r.hovered() { theme::ICON } else { theme::ICON_SECONDARY }, false);
    }
    widgets::record_with(ui, record_name, b);
    let clicked = r.clicked();
    r.on_hover_text("Apply variable");
    clicked
}

/// Keeps the colour picker open while the variable bind picker is active on a gradient stop.
pub(crate) fn picker_keep_open(ctx: &Context, st: &mut crate::widgets::picker::State) {
    let is_open = ctx.data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())).is_some_and(|s| s.open_key.is_some());
    if is_open {
        st.menu_open = true;
    }
}

/// Whether gradient stop `stop_idx` on the currently edited fill/stroke paint is bound to a variable.
pub(crate) fn picker_stop_is_bound(ctx: &Context, stop_idx: usize) -> bool {
    let Some(st_ctx) = ctx.data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())) else { return false };
    let Some((pfx, pi)) = st_ctx.prefix else { return false };
    let key = format!("{pfx}/{pi}/stops/{stop_idx}");
    st_ctx.bound_stops.contains_key(&key)
}

/// Draws the bound-variable pill in place of the colour picker's value row when gradient stop `stop_idx` is bound.
pub(crate) fn picker_stop_pill_field(ui: &mut Ui, st: &mut crate::widgets::picker::State, rect: Rect, stop_idx: usize) -> bool {
    let Some(st_ctx) = ui.ctx().data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())) else { return false };
    let Some((pfx, pi)) = st_ctx.prefix else { return false };
    let key = format!("{pfx}/{pi}/stops/{stop_idx}");
    let Some(info) = st_ctx.bound_stops.get(&key).cloned() else { return false };
    widgets::record_with(ui, || format!("bound:{key}"), rect);
    let r = pill(ui, rect, ("bound", key.as_str()), &info.label, info.swatch, info.style_name.is_none());
    if let Some(style_name) = info.style_name {
        let tip = format!("Bound in style {style_name}");
        if st_ctx.shot_hover && r.body.hovered() {
            egui::Area::new(Id::new(("picker-stop-style-tip", key)))
                .order(egui::Order::Tooltip)
                .fixed_pos(Pos2::new(rect.left() + theme::INSET_CONTROL, rect.bottom() + theme::SPACE_FACT))
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    crate::widgets::kit::tooltip_frame().show(ui, |ui| {
                        ui.label(&tip);
                    });
                });
        }
        r.body.on_hover_text(tip);
    } else if r.detach {
        st.menu_open = true;
        ui.ctx().data_mut(|d| d.insert_temp(picker_action_id(), PickerStopBindAction::Detach { key }));
    } else if r.body.clicked() {
        st.menu_open = true;
        ui.ctx().data_mut(|d| d.insert_temp(picker_action_id(), PickerStopBindAction::Open { key, anchor: rect }));
    }
    true
}

/// Small hexagon button shown on hover in the colour picker's value row that opens the variable picker for gradient stop `stop_idx`.
pub(crate) fn picker_stop_hover_button(ui: &mut Ui, st: &mut crate::widgets::picker::State, rect: Rect, stop_idx: usize) {
    let Some(st_ctx) = ui.ctx().data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())) else { return };
    let Some((pfx, pi)) = st_ctx.prefix else { return };
    let key = format!("{pfx}/{pi}/stops/{stop_idx}");
    let is_open = st_ctx.open_key.as_deref() == Some(key.as_str());
    if draw_hover_button(ui, rect, || format!("bind:{key}"), ("bind-btn", key.as_str()), st_ctx.has_vars, is_open, false) {
        st.menu_open = true;
        ui.ctx().data_mut(|d| d.insert_temp(picker_action_id(), PickerStopBindAction::Open { key, anchor: rect }));
    }
}

/// The variable bound to `key` on `node` in `doc` (directly or through a linked style), if any.
pub(super) fn doc_bound_var<'a>(doc: &'a Document, node: NodeId, key: &str) -> Option<&'a Variable> {
    let id = doc.effective_binding(node, key)?;
    doc.variables.get(id)
}

/// When `key` on `node` resolves to a live variable through a linked fill/stroke style (and not
/// a direct node binding), returns that style's display name.
pub(super) fn style_bound_stop_style_name<'a>(doc: &'a Document, node: NodeId, key: &str) -> Option<&'a str> {
    let n = doc.get(node)?;
    if n.variables.bindings.contains_key(key) {
        return None;
    }
    doc_bound_var(doc, node, key)?;
    let slot = if key.starts_with("strokes/") { "stroke" } else { "fill" };
    let sk = n.style_refs.get(slot)?;
    Some(doc.style_name(sk))
}

/// Collects all stop binding keys (`"{pfx}/{pi}/stops/{si}"`) that resolve to a live variable on
/// `node` (directly or through a linked fill/stroke style).
pub(super) fn collect_bound_stop_keys(doc: &Document, node: NodeId, pfx: &str, pi: usize, out: &mut std::collections::BTreeSet<String>) {
    let Some(n) = doc.get(node) else { return };
    let stop_prefix = format!("{pfx}/{pi}/stops/");
    for k in n.variables.bindings.keys() {
        if k.starts_with(&stop_prefix) && doc_bound_var(doc, node, k).is_some() {
            out.insert(k.clone());
        }
    }
    let slot = if pfx == "strokes" { "stroke" } else { "fill" };
    let style_stop_prefix = format!("fills/{pi}/stops/");
    if let Some(meta) = n.style_refs.get(slot).and_then(|sk| doc.style_meta.get(sk)) {
        for bkey in meta.bindings.keys() {
            if let Some(rest) = bkey.strip_prefix(&style_stop_prefix) {
                let k = format!("{stop_prefix}{rest}");
                if doc_bound_var(doc, node, &k).is_some() {
                    out.insert(k);
                }
            }
        }
    }
}

/// Whether `node` has any live gradient stop binding under `"{pfx}/{pi}/stops/"` (directly or through a linked style).
pub(super) fn has_bound_stops(doc: &Document, node: NodeId, pfx: &str, pi: usize) -> bool {
    let mut keys = std::collections::BTreeSet::new();
    collect_bound_stop_keys(doc, node, pfx, pi, &mut keys);
    !keys.is_empty()
}

/// The variable bound to `key` on `node`, if any.
pub fn bound_var<'a>(app: &'a App, node: NodeId, key: &str) -> Option<&'a Variable> {
    doc_bound_var(&app.doc, node, key)
}

pub(super) fn open_picker(app: &mut App, sel: &[NodeId], keys: &[&str], kind: VariableKind, anchor: Rect) {
    app.vars.bind_picker =
        Some(BindPicker { keys: keys.iter().map(|k| k.to_string()).collect(), kind, nodes: sel.to_vec(), style_key: None, anchor, query: String::new() });
}

fn open_style_picker(app: &mut App, style_key: &str, prop: &str, kind: VariableKind, anchor: Rect) {
    app.vars.bind_picker =
        Some(BindPicker { keys: vec![prop.to_string()], kind, nodes: Vec::new(), style_key: Some(style_key.to_string()), anchor, query: String::new() });
}

/// Draws the bound-variable pill in place of a field when `keys[0]` is bound
/// on the first selected node. Returns whether it did.
pub fn pill_field(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str]) -> bool {
    let Some(first) = sel.first() else { return false };
    let k0 = keys[0];
    // Only the node's own binding draws a detachable pill here; a binding that comes from a linked style is shown elsewhere.
    let node_var = app.doc.get(*first).and_then(|n| n.variables.bindings.get(k0)).and_then(|id| app.doc.variables.get(id)).cloned();
    let (label, kind) = match node_var {
        Some(var) => (var.name.clone(), var.kind),
        None => {
            let Some(prop) = k0.strip_prefix(PROP_BINDING_PREFIX) else { return false };
            let Some(vid) = app.doc.get(*first).and_then(|n| n.variables.bindings.get(k0)).cloned() else { return false };
            let kind = app
                .doc
                .get(*first)
                .and_then(instance_component)
                .and_then(|c| prop_defs(&app.doc, c).into_iter().find(|d| d.name == prop))
                .and_then(|d| bindable_prop_kind(&d.kind))
                .unwrap_or(VariableKind::String);
            (format!("{vid} (missing)"), kind)
        }
    };
    let swatch = match app.doc.bound_value(*first, k0) {
        Some(VariableValue::Color(c)) => Some(to_c32(c)),
        _ => None,
    };
    let r = draw_pill(ui, rect, || format!("bound:{k0}"), ("bound", k0), &label, swatch);
    if let Some(prop) = k0.strip_prefix(PROP_BINDING_PREFIX) {
        if let Some(problem) = prop_binding_problem(&app.doc, *first, prop) {
            r.body.clone().on_hover_text(problem.to_string());
        }
    }
    if r.detach {
        app.detach_variable(sel, keys);
    } else if r.body.clicked() {
        open_picker(app, sel, keys, kind, rect);
    }
    true
}

/// Small hexagon button that opens the variable picker when a matching variable exists.
pub fn hover_button(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str], kind: VariableKind) {
    let target_kind = node_target_kind(app, sel);
    let k0 = keys[0];
    let has_vars = has_matching_var(app, ui.ctx(), k0, kind, target_kind);
    let is_open = app.vars.bind_picker.as_ref().is_some_and(|b| b.style_key.is_none() && b.keys.first().map(String::as_str) == Some(k0));
    if draw_hover_button(ui, rect, || format!("bind:{k0}"), ("bind-btn", k0), has_vars, is_open, true) {
        open_picker(app, sel, keys, kind, rect);
    }
}

/// A numeric field that turns into a variable pill when bound, with an
/// always-visible bind button when a matching number variable exists.
pub fn numeric(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str], field: impl FnOnce(&mut Ui, Rect) -> Option<f32>) -> Option<f32> {
    if pill_field(app, ui, rect, sel, keys) {
        return None;
    }
    let val_r = value_rect(app, ui, rect, sel, keys, VariableKind::Number);
    let v = field(ui, val_r);
    hover_button(app, ui, rect, sel, keys, VariableKind::Number);
    v
}

/// Draws the bound-variable pill in place of a style editor field when `prop` is bound on `style_key`.
pub fn style_pill_field(app: &mut App, ui: &mut Ui, rect: Rect, style_key: &str, prop: &str) -> bool {
    let Some(vid) = app.doc.style_meta.get(style_key).and_then(|m| m.bindings.get(prop)).cloned() else {
        return false;
    };
    let (label, kind) = match app.doc.variables.get(&vid) {
        Some(var) => (var.name.clone(), var.kind),
        None => (format!("{vid} (missing)"), property_kind(prop).unwrap_or(VariableKind::Color)),
    };
    let swatch = match app.doc.resolve_variable_default(&vid) {
        Some(VariableValue::Color(c)) => Some(to_c32(c)),
        _ => None,
    };
    let r = draw_pill(ui, rect, || format!("style-bound:{style_key}:{prop}"), ("style-bound", style_key, prop), &label, swatch);
    if r.detach {
        app.unbind_style_variable(style_key, prop);
    } else if r.body.clicked() {
        open_style_picker(app, style_key, prop, kind, rect);
    }
    true
}

/// Small hexagon button in the style editor that opens the variable picker for `prop`.
pub fn style_hover_button(app: &mut App, ui: &mut Ui, rect: Rect, style_key: &str, prop: &str, kind: VariableKind) {
    let target_kind = style_target_kind(app, style_key);
    let scope_key = style_scope_key(app, style_key, prop);
    let has_vars = has_matching_var(app, ui.ctx(), &scope_key, kind, target_kind);
    let is_open = app.vars.bind_picker.as_ref().is_some_and(|b| b.style_key.as_deref() == Some(style_key) && b.keys.first().map(String::as_str) == Some(prop));
    if draw_hover_button(ui, rect, || format!("style-bind:{style_key}:{prop}"), ("style-bind-btn", style_key, prop), has_vars, is_open, true) {
        open_style_picker(app, style_key, prop, kind, rect);
    }
}

/// A numeric field in the style editor that turns into a variable pill when bound.
pub fn style_numeric(app: &mut App, ui: &mut Ui, rect: Rect, style_key: &str, prop: &str, field: impl FnOnce(&mut Ui, Rect) -> Option<f32>) -> Option<f32> {
    if style_pill_field(app, ui, rect, style_key, prop) {
        return None;
    }
    let val_r = style_value_rect(app, ui, rect, style_key, prop, VariableKind::Number);
    let v = field(ui, val_r);
    style_hover_button(app, ui, rect, style_key, prop, VariableKind::Number);
    v
}

/// Whether binding `key` detaches a linked text style: a property the style owns that Figma does not let
/// a layer override while linked. A weight or font style variable keeps the link and is recorded as an
/// override instead (`bind_variable_in`).
fn is_text_style_key(key: &str) -> bool {
    matches!(key, "fontFamily" | "fontStyle" | "fontWeight" | "fontSize" | "lineHeight" | "letterSpacing" | "paragraphSpacing" | "paragraphIndent")
        && !designos_doc::binding_key_is_overridable(key)
}

fn detach_text_style_in(doc: &mut Document, tx: &mut crate::edit::Tx, nodes: &[NodeId]) {
    let rk = crate::styles::StyleKind::Text.ref_key();
    for &nid in nodes {
        if doc.get(nid).is_some_and(|n| matches!(n.kind, NodeKind::Text(_)) && n.style_refs.contains_key(rk)) {
            if let Some(n) = tx.ed.node(doc, nid) {
                n.style_refs.remove(rk);
                n.text_style_overrides = designos_doc::OwnedFieldSet::EMPTY;
            }
        }
    }
}

/// Renders the picker opened by [`hover_button`] / [`style_hover_button`] / a pill.
pub fn bind_picker(app: &mut App, ctx: &Context) {
    if let Some(action) = ctx.data_mut(|d| d.remove_temp::<PickerStopBindAction>(picker_action_id())) {
        let ids = app.props.picker.as_ref().map(|pk| pk.ids.clone()).unwrap_or_default();
        if !ids.is_empty() {
            match action {
                PickerStopBindAction::None => {}
                PickerStopBindAction::Open { key, anchor } => {
                    if ids.iter().all(|&id| style_bound_stop_style_name(&app.doc, id, &key).is_none()) {
                        open_picker(app, &ids, &[key.as_str()], VariableKind::Color, anchor);
                    }
                }
                PickerStopBindAction::Detach { key } => {
                    if ids.iter().all(|&id| style_bound_stop_style_name(&app.doc, id, &key).is_none()) {
                        commit_picker_tx_before_bind(app);
                        app.detach_variable(&ids, &[key.as_str()]);
                    }
                }
            }
        }
    }
    let Some(mut bp) = app.vars.bind_picker.take() else {
        return;
    };
    let key0 = bp.keys.first().cloned().unwrap_or_default();
    let (current, kind_node, scope_key) = if let Some(skey) = &bp.style_key {
        let cur = app.doc.style_meta.get(skey).and_then(|m| m.bindings.get(&key0)).cloned();
        let kn = match app.doc.styles.get(skey) {
            Some(Style::Text(_)) => NodeKind::Text(TextProps::default()),
            _ => NodeKind::Rectangle,
        };
        (cur, kn, style_scope_key(app, skey, &key0).into_owned())
    } else {
        let Some(first) = bp.nodes.first().copied() else {
            return;
        };
        let Some(node) = app.doc.get(first) else {
            return;
        };
        (node.variables.bindings.get(&key0).cloned(), node.kind.clone(), key0.clone())
    };
    let mut items = Vec::new();
    for c in app.doc.variable_collections.values().filter(|c| !c.is_soft_deleted()) {
        for vid in &c.variables {
            let Some(v) = app.doc.variables.get(vid).filter(|v| !v.is_soft_deleted()) else { continue };
            if v.kind == bp.kind && scope_allows(&v.scopes, &scope_key, &kind_node) {
                items.push(pick_item(&app.doc, v));
            }
        }
    }
    // Variables of enabled libraries, one section per library; copied into the file when picked.
    for lv in app.library_variables().into_iter().filter(|v| v.kind == bp.kind && v.local_id.is_none() && scope_allows(&v.scopes, &scope_key, &kind_node)) {
        let Some(render) = app.library_render_doc(&lv.library_id) else { continue };
        let Some(v) = render.variables.get(lv.key.strip_prefix("var:").unwrap_or(&lv.key)) else { continue };
        let mut item = pick_item(&render, v);
        item.id = format!("{LIB_PREFIX}{}{LIB_SEP}{}", lv.library_id, lv.key);
        item.group = lv.library_name.clone();
        item.name = lv.name.clone();
        items.push(item);
    }
    let title = match bp.kind {
        VariableKind::Color => "Colors",
        VariableKind::Number => "Numbers",
        VariableKind::String => "Strings",
        VariableKind::Boolean => "Booleans",
    };
    let screen = ctx.content_rect();
    let pos = Pos2::new((bp.anchor.left() - 256.0).max(screen.left() + 8.0), bp.anchor.top() - 8.0);
    let keys: Vec<String> = bp.keys.clone();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    match picker(ctx, Id::new("bind-picker"), pos, Some(bp.anchor), title, &mut bp.query, &items, current.as_deref(), current.is_some()) {
        PickResult::Open => app.vars.bind_picker = Some(bp),
        PickResult::Closed => {}
        PickResult::Picked(id) => {
            if let Some(skey) = &bp.style_key {
                match id.strip_prefix(LIB_PREFIX).and_then(|r| r.split_once(LIB_SEP)) {
                    Some((lib, key)) => {
                        app.bind_style_library_variable(skey, &key0, lib, key);
                    }
                    None => {
                        app.bind_style_variable(skey, &key0, &id);
                    }
                }
            } else {
                commit_picker_tx_before_bind(app);
                crate::canvas::textsession::requesting_new_faces(app, &bp.nodes, |app| {
                    let style_keys = is_text_style_key(&key0);
                    match id.strip_prefix(LIB_PREFIX).and_then(|r| r.split_once(LIB_SEP)) {
                        Some((lib, key)) if style_keys => {
                            let mut tx = crate::edit::Tx::new();
                            match app.import_asset_in(&mut tx, lib, key) {
                                Some(local) => {
                                    app.bind_variable_in(&mut tx, &bp.nodes, &key_refs, &local);
                                    detach_text_style_in(&mut app.doc, &mut tx, &bp.nodes);
                                    app.run_tx(tx);
                                }
                                None => {
                                    tx.cancel(&mut app.doc);
                                    app.show_toast("Library variable could not be loaded", true);
                                }
                            }
                            app.libs.invalidate();
                        }
                        Some((lib, key)) => {
                            app.bind_library_variable(&bp.nodes, &key_refs, lib, key);
                        }
                        None if style_keys => {
                            let mut tx = crate::edit::Tx::new();
                            app.bind_variable_in(&mut tx, &bp.nodes, &key_refs, &id);
                            detach_text_style_in(&mut app.doc, &mut tx, &bp.nodes);
                            app.run_tx(tx);
                        }
                        None => {
                            app.bind_variable(&bp.nodes, &key_refs, &id);
                        }
                    }
                });
            }
        }
        PickResult::Detach => {
            if let Some(skey) = &bp.style_key {
                app.unbind_style_variable(skey, &key0);
            } else {
                commit_picker_tx_before_bind(app);
                app.detach_variable(&bp.nodes, &key_refs);
            }
        }
    }
}

/// "Variable mode" rows for a container: one dropdown per collection, where
/// "Auto" inherits the mode from the parent.
pub(super) fn mode_rows(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId]) {
    if app.doc.variable_collections.is_empty() {
        return;
    }
    let Some(first) = sel.first().copied() else { return };
    let colls: Vec<VariableCollection> = app.doc.variable_collections.values().filter(|c| !c.is_soft_deleted()).cloned().collect();
    for c in colls {
        let explicit = app.doc.get(first).and_then(|n| n.variables.modes.get(&c.id)).cloned();
        let effective = app.doc.variable_mode(first, &c.id).unwrap_or_default();
        let mode_name = |id: &str| c.modes.iter().find(|m| m.id == id).map(|m| m.name.clone()).unwrap_or_else(|| id.to_string());
        let mut options: Vec<String> = vec!["Auto".into()];
        options.extend(c.modes.iter().map(|m| m.name.clone()));
        let label = match &explicit {
            Some(m) => mode_name(m),
            None => format!("Auto ({})", mode_name(&effective)),
        };
        let lr = pane.col(0, 0.0);
        icons::paint(ui.painter(), Icon::Variables, Pos2::new(lr.left() + 8.0, lr.center().y), 14.0, theme::ICON_SECONDARY);
        let nm = widgets::elide(ui.painter(), &c.name, &theme::font_body(), lr.width() - 28.0);
        ui.painter().text(Pos2::new(lr.left() + 24.0, lr.center().y), Align2::LEFT_CENTER, nm, theme::font_body(), theme::TEXT_SECONDARY);
        let dd = pane.col(1, 0.0);
        widgets::record_with(ui, || format!("mode:{}", c.name), dd);
        if let Some(i) = dropdown(ui, dd, ("var-mode", &c.id), &label, &options) {
            let pick = if i == 0 { None } else { Some(c.modes[i - 1].id.clone()) };
            app.set_node_mode(sel, &c.id, pick.as_deref());
        }
        row_end(pane);
    }
}

#[cfg(test)]
mod detach_tests {
    use super::*;

    #[test]
    fn binding_a_size_detaches_the_style_and_leaves_no_stale_override_set() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let id = doc.alloc_id();
        let mut text = Node::text(id, designos_doc::Rect::new(0.0, 0.0, 100.0, 20.0), "Hi", designos_doc::TextStyle::default());
        text.style_refs.insert("text".into(), "Body".into());
        text.text_style_overrides.insert(designos_doc::OwnedField::Weight);
        doc.insert(page, text).unwrap();
        let mut tx = crate::edit::Tx::new();
        detach_text_style_in(&mut doc, &mut tx, &[id]);
        let mut hist = designos_doc::History::default();
        tx.commit(&mut doc, &mut hist).unwrap();
        let n = doc.get(id).unwrap();
        assert!(!n.style_refs.contains_key("text"));
        assert!(n.text_style_overrides.is_empty(), "an unlinked layer has no overrides of a style");
    }
}
