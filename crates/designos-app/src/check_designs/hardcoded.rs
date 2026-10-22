//! Hard-coded values rule: a raw fill, stroke, number property, text style or effect list that
//! equals an existing variable or shared style (in this file or an enabled library) but is not
//! linked to it.
//!
//! A variable wins over a style for paints and text (one finding per property). Styles are matched
//! on the whole list: a layer whose paints, text properties or effects equal a style's values
//! exactly.

use super::{Finding, Fix, LibrarySources, Rule};
use crate::styles::rgb_key;
use designos_doc::{
    local_key, scope_allows, style_property, Color, Document, LayoutAlign, LibraryAssetKind, LibrarySnapshot, LineHeight, Node, NodeId, NodeKind, Paint,
    PaintKind, Style, StyleProperty, TextStyle, Variable, VariableKind, VariableStore, VariableValue, REMOTE_LIBRARY_PREFIX,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Alpha tolerance when comparing colours: half an 8-bit step.
const ALPHA_EPS: f32 = 0.002;
/// Tolerance when comparing exact numbers (0.0005).
const NUM_EPS: f32 = 0.0005;

const PADDING_KEYS: [&str; 4] = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
const RADIUS_KEYS: [&str; 4] = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];

/// Where an entry's value resolves from. Owned, so an [`Index`] can outlive the borrow of the
/// document it was built from and be held by a scan that spans several frames.
#[derive(Clone)]
enum VarSource {
    Local,
    Library { snap: Arc<LibrarySnapshot>, store_idx: usize },
}

/// A variable and whether it resolves to one and the same value in every mode.
pub(super) struct Entry {
    pub var: Arc<Variable>,
    /// Variable id to write on fix (`v.id` when local/imported, `<lib_id>/<v.id>` when in a snapshot).
    pub fix_id: String,
    /// Library display name when the variable comes from an enabled library (not yet imported).
    pub library_name: Option<String>,
    constant: bool,
    source: VarSource,
}

struct StyleEntry {
    fix_key: String,
    name: String,
    property: StyleProperty,
    style: Style,
    library_name: Option<String>,
}

/// What [`Index::find`] or [`Index::find_number`] concluded for one property.
pub(super) enum Match<'b> {
    None,
    /// Exactly one variable is the right one.
    One(&'b Entry),
    /// Several variables fit equally well, so none may be picked for the user.
    Ambiguous(Vec<&'b Entry>),
}

/// Colour and number variables (plus styles) from the document and enabled libraries.
pub(super) struct Index {
    lib_stores: Vec<VariableStore>,
    by_rgb: HashMap<u32, Vec<Entry>>,
    by_num: HashMap<i64, Vec<Entry>>,
    styles: Vec<StyleEntry>,
}

/// The Studio library a variable came from, when it is a library asset. An imported variable is
/// named by its `library.external` entry; its collection's `source_library_key` counts only when it
/// equals a Studio library id (enabled now or recorded by an earlier import). Figma's own
/// `sourceLibraryKey`, which `.fig` and REST import store in the same field, names no Studio
/// library, so such a variable stays a local candidate.
pub(super) fn variable_library_id<'a>(doc: &'a Document, v: &Variable) -> Option<&'a str> {
    let imported = doc
        .library
        .external
        .values()
        .find(|e| e.kind == LibraryAssetKind::Variable && e.local_key.as_deref() == Some(v.id.as_str()))
        .map(|e| e.library_id.as_str());
    imported.or_else(|| {
        let key = doc.variable_collections.get(&v.collection)?.source_library_key.as_deref()?;
        let known = doc.library.enabled.iter().any(|r| r.id == key) || doc.library.external.values().any(|e| e.library_id == key);
        known.then_some(key)
    })
}

fn style_library_id<'a>(doc: &'a Document, key: &str) -> Option<&'a str> {
    doc.library.external.values().find(|e| e.kind == LibraryAssetKind::Style && e.local_key.as_deref() == Some(key)).map(|e| e.library_id.as_str())
}

fn enabled_library_name<'a>(doc: &'a Document, lib_id: &str) -> Option<&'a str> {
    doc.library.enabled.iter().find(|r| r.id == lib_id && !r.id.starts_with(REMOTE_LIBRARY_PREFIX)).map(|r| r.name.as_str())
}

fn default_style_property(style: &Style) -> StyleProperty {
    match style {
        Style::Fill(_) => StyleProperty::Fill,
        Style::Text(_) => StyleProperty::Text,
        Style::Effect(_) => StyleProperty::Effect,
        Style::Grid(_) => StyleProperty::Grid,
    }
}

impl Index {
    pub(super) fn with_libraries(doc: &Document, libraries: &LibrarySources) -> Self {
        let store = VariableStore::from_doc(doc);
        let mut lib_stores = Vec::new();
        let mut by_rgb: HashMap<u32, Vec<Entry>> = HashMap::new();
        let mut by_num: HashMap<i64, Vec<Entry>> = HashMap::new();
        let mut styles: Vec<StyleEntry> = Vec::new();

        for v in doc.variables.values() {
            if let Some(lib_id) = variable_library_id(doc, v) {
                if enabled_library_name(doc, lib_id).is_none() {
                    continue; // Disabled library: never suggest even if previously imported.
                }
            }
            let Some(coll) = doc.variable_collections.get(&v.collection) else { continue };
            // Per §3.2: a library asset that already has a local copy counts as a local candidate.
            Self::insert_variable(&store, v, coll, v.id.clone(), None, VarSource::Local, &mut by_rgb, &mut by_num);
        }

        for (k, s) in &doc.styles {
            if let Some(lib_id) = style_library_id(doc, k) {
                if enabled_library_name(doc, lib_id).is_none() {
                    continue; // Disabled library: never suggest.
                }
            }
            styles.push(StyleEntry {
                fix_key: k.clone(),
                name: doc.style_name(k).to_string(),
                property: style_property(doc, k),
                style: s.clone(),
                library_name: None,
            });
        }

        for (lib_ref, snap) in &libraries.libs {
            let lib_name = lib_ref.name.as_str();
            let store_idx = lib_stores.len();
            lib_stores.push(VariableStore { variables: snap.variables.clone(), collections: snap.variable_collections.clone() });
            let snap_store = &lib_stores[store_idx];

            for asset in &snap.assets {
                let ext_key = local_key(&lib_ref.id, &asset.key);
                if doc.library.external.contains_key(&ext_key) {
                    continue;
                }
                match asset.kind {
                    LibraryAssetKind::Variable => {
                        let Some(raw) = asset.key.strip_prefix("var:") else { continue };
                        let fix_id = local_key(&lib_ref.id, raw);
                        if doc.variables.contains_key(&fix_id) {
                            continue;
                        }
                        let Some(v) = snap.variables.get(raw) else { continue };
                        let Some(coll) = snap.variable_collections.get(&v.collection) else { continue };
                        Self::insert_variable(
                            snap_store,
                            v,
                            coll,
                            fix_id,
                            Some(lib_name),
                            VarSource::Library { snap: Arc::clone(snap), store_idx },
                            &mut by_rgb,
                            &mut by_num,
                        );
                    }
                    LibraryAssetKind::Style => {
                        let Some(raw) = asset.key.strip_prefix("style:") else { continue };
                        let fix_key = local_key(&lib_ref.id, raw);
                        if doc.styles.contains_key(&fix_key) {
                            continue;
                        }
                        let Some(s) = snap.styles.get(raw) else { continue };
                        let meta = snap.style_meta.get(raw);
                        let name = meta.map(|m| m.name.as_str()).filter(|n| !n.is_empty()).unwrap_or(asset.name.as_str());
                        let property = meta.map(|m| m.property).unwrap_or_else(|| default_style_property(s));
                        styles.push(StyleEntry { fix_key, name: name.to_string(), property, style: s.clone(), library_name: Some(lib_name.to_string()) });
                    }
                    _ => {}
                }
            }
        }

        for list in by_rgb.values_mut() {
            list.sort_by(|a, b| (&a.var.name, a.library_name.as_deref(), &a.fix_id).cmp(&(&b.var.name, b.library_name.as_deref(), &b.fix_id)));
        }
        for list in by_num.values_mut() {
            list.sort_by(|a, b| (&a.var.name, a.library_name.as_deref(), &a.fix_id).cmp(&(&b.var.name, b.library_name.as_deref(), &b.fix_id)));
        }

        Index { lib_stores, by_rgb, by_num, styles }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_variable(
        store: &VariableStore,
        v: &Variable,
        coll: &designos_doc::VariableCollection,
        fix_id: String,
        library_name: Option<&str>,
        source: VarSource,
        by_rgb: &mut HashMap<u32, Vec<Entry>>,
        by_num: &mut HashMap<i64, Vec<Entry>>,
    ) {
        let var = Arc::new(v.clone());
        match v.kind {
            VariableKind::Color => {
                let values: Vec<Option<Color>> = coll
                    .modes
                    .iter()
                    .map(|mode| match store.resolve(&v.id, &mode.id) {
                        Some(VariableValue::Color(c)) => Some(c),
                        _ => None,
                    })
                    .collect();
                let constant = !values.is_empty() && values.iter().all(|c| matches!((c, &values[0]), (Some(a), Some(b)) if same_colour(a, b)));
                let mut seen = Vec::new();
                for c in values.iter().flatten() {
                    let key = rgb_key(c);
                    if !seen.contains(&key) {
                        seen.push(key);
                        by_rgb.entry(key).or_default().push(Entry {
                            var: Arc::clone(&var),
                            fix_id: fix_id.clone(),
                            library_name: library_name.map(str::to_string),
                            constant,
                            source: source.clone(),
                        });
                    }
                }
            }
            VariableKind::Number => {
                let values: Vec<Option<f32>> = coll
                    .modes
                    .iter()
                    .map(|mode| match store.resolve(&v.id, &mode.id) {
                        Some(VariableValue::Number(x)) if x.is_finite() => Some(x),
                        _ => None,
                    })
                    .collect();
                let constant = !values.is_empty() && values.iter().all(|x| matches!((x, &values[0]), (Some(a), Some(b)) if same_number(*a, *b)));
                let mut seen = Vec::new();
                for x in values.iter().flatten() {
                    let key = num_key(*x);
                    if !seen.contains(&key) {
                        seen.push(key);
                        by_num.entry(key).or_default().push(Entry {
                            var: Arc::clone(&var),
                            fix_id: fix_id.clone(),
                            library_name: library_name.map(str::to_string),
                            constant,
                            source: source.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn resolve_entry(&self, doc: &Document, node: NodeId, e: &Entry) -> Option<VariableValue> {
        match &e.source {
            VarSource::Local => doc.resolve_variable(node, &e.var.id),
            VarSource::Library { snap, store_idx } => resolve_in_snapshot(doc, snap, &self.lib_stores[*store_idx], node, &e.var),
        }
    }

    /// The variable that holds `c` at `node` for property `key`. Local candidates take priority
    /// over library candidates (§3.4). A variable with the same value in every mode is preferred
    /// over one that only matches in the mode in effect here.
    fn find<'b>(&'b self, doc: &Document, node: &Node, key: &str, c: &Color) -> Match<'b> {
        let Some(list) = self.by_rgb.get(&rgb_key(c)) else { return Match::None };
        let holding: Vec<&'b Entry> = list
            .iter()
            .filter(|e| {
                scope_allows(&e.var.scopes, key, &node.kind)
                    && matches!(self.resolve_entry(doc, node.id, e), Some(VariableValue::Color(vc)) if same_colour(&vc, c))
            })
            .collect();
        let locals: Vec<&'b Entry> = holding.iter().copied().filter(|e| e.library_name.is_none()).collect();
        let candidates = if locals.is_empty() { holding } else { locals };
        let steady: Vec<&'b Entry> = candidates.iter().copied().filter(|e| e.constant).collect();
        let pool = if steady.is_empty() { candidates } else { steady };
        match pool.as_slice() {
            [] => Match::None,
            [only] => Match::One(only),
            many => Match::Ambiguous(many.to_vec()),
        }
    }

    /// The number variable that holds `x` at `node` for property `key`. Local candidates take
    /// priority over library candidates (§3.4).
    fn find_number<'b>(&'b self, doc: &Document, node: &Node, key: &str, x: f32) -> Match<'b> {
        let base = num_key(x);
        let scope_key = scope_key_for_number(key);
        let mut seen_ids: Vec<&str> = Vec::new();
        let mut holding: Vec<&'b Entry> = Vec::new();
        for bucket in [base - 1, base, base + 1] {
            if let Some(list) = self.by_num.get(&bucket) {
                for e in list {
                    if !seen_ids.contains(&e.fix_id.as_str())
                        && scope_allows(&e.var.scopes, scope_key, &node.kind)
                        && matches!(self.resolve_entry(doc, node.id, e), Some(VariableValue::Number(vx)) if same_number(vx, x))
                    {
                        seen_ids.push(e.fix_id.as_str());
                        holding.push(e);
                    }
                }
            }
        }
        holding.sort_by(|a, b| (&a.var.name, a.library_name.as_deref(), &a.fix_id).cmp(&(&b.var.name, b.library_name.as_deref(), &b.fix_id)));
        let locals: Vec<&'b Entry> = holding.iter().copied().filter(|e| e.library_name.is_none()).collect();
        let candidates = if locals.is_empty() { holding } else { locals };
        let steady: Vec<&'b Entry> = candidates.iter().copied().filter(|e| e.constant).collect();
        let pool = if steady.is_empty() { candidates } else { steady };
        match pool.as_slice() {
            [] => Match::None,
            [only] => Match::One(only),
            many => Match::Ambiguous(many.to_vec()),
        }
    }
}

fn scope_key_for_number(key: &str) -> &str {
    match key {
        "padding" => "paddingTop",
        "cornerRadius" => "topLeftRadius",
        other => other,
    }
}

fn resolve_in_snapshot(doc: &Document, snap: &LibrarySnapshot, store: &VariableStore, node: NodeId, v: &Variable) -> Option<VariableValue> {
    let coll = snap.variable_collections.get(&v.collection)?;
    let imported_coll = local_key(&snap.library_id, &v.collection);
    // The collection's own default mode, as `Document::resolve_variable` uses once it is imported.
    let default_mode = if coll.default_mode.is_empty() { coll.modes.first().map(|m| m.id.as_str()).unwrap_or_default() } else { coll.default_mode.as_str() };
    let mode = doc.variable_mode(node, &imported_coll).unwrap_or_else(|| default_mode.to_string());
    store.resolve(&v.id, &mode).or_else(|| store.resolve(&v.id, default_mode))
}

/// Same colour to the 8-bit step, and the same alpha within [`ALPHA_EPS`].
fn same_colour(a: &Color, b: &Color) -> bool {
    rgb_key(a) == rgb_key(b) && (a.a - b.a).abs() <= ALPHA_EPS
}

fn num_key(x: f32) -> i64 {
    (x as f64 * 1000.0).round() as i64
}

fn same_number(a: f32, b: f32) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= NUM_EPS
}

fn fmt_num(x: f32) -> String {
    if (x - x.round()).abs() < NUM_EPS {
        format!("{}", x.round() as i64)
    } else {
        let r = (x * 1000.0).round() / 1000.0;
        format!("{r}")
    }
}

fn variable_holds(doc: &Document, node: &Node, key: &str, v: &Variable, c: &Color) -> bool {
    scope_allows(&v.scopes, key, &node.kind) && matches!(doc.resolve_variable(node.id, &v.id), Some(VariableValue::Color(vc)) if same_colour(&vc, c))
}

fn number_variable_holds(doc: &Document, node: &Node, key: &str, v: &Variable, x: f32) -> bool {
    scope_allows(&v.scopes, scope_key_for_number(key), &node.kind)
        && matches!(doc.resolve_variable(node.id, &v.id), Some(VariableValue::Number(vx)) if same_number(vx, x))
}

/// The paint list a slot reads from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Fill,
    Stroke,
}

impl Slot {
    fn prefix(self) -> &'static str {
        match self {
            Slot::Fill => "fills",
            Slot::Stroke => "strokes",
        }
    }

    fn ref_key(self) -> &'static str {
        match self {
            Slot::Fill => "fill",
            Slot::Stroke => "stroke",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Slot::Fill => "Fill",
            Slot::Stroke => "Stroke",
        }
    }

    fn property(self) -> StyleProperty {
        match self {
            Slot::Fill => StyleProperty::Fill,
            Slot::Stroke => StyleProperty::Stroke,
        }
    }

    fn paints(self, n: &Node) -> &[Paint] {
        match (self, &n.kind) {
            (Slot::Fill, NodeKind::Text(t)) => &t.style.fills,
            (Slot::Fill, _) => &n.fills,
            (Slot::Stroke, _) => &n.stroke.paints,
        }
    }
}

fn finding(n: &Node, rule: Rule, property: impl Into<String>, message: String, fix: Fix, fix_label: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule, property: property.into(), message, fix: Some(fix), fix_label }
}

/// A finding that is reported but cannot be fixed without a choice only the user can make.
fn report_only(n: &Node, rule: Rule, property: impl Into<String>, message: String) -> Finding {
    Finding { node: n.id, node_name: n.name.clone(), rule, property: property.into(), message, fix: None, fix_label: String::new() }
}

fn fmt_var_quoted(e: &Entry) -> String {
    match e.library_name.as_deref() {
        Some(lib) => format!("\"{}\" from library \"{lib}\"", e.var.name),
        None => format!("\"{}\"", e.var.name),
    }
}

fn fmt_var_bind_label(e: &Entry) -> String {
    match e.library_name.as_deref() {
        Some(lib) => format!("Bind to variable {} from library {}", e.var.name, lib),
        None => format!("Bind to variable {}", e.var.name),
    }
}

fn fmt_style_quoted(s: &StyleEntry) -> String {
    match s.library_name.as_deref() {
        Some(lib) => format!("\"{}\" from library \"{lib}\"", s.name),
        None => format!("\"{}\"", s.name),
    }
}

fn fmt_style_apply_label(s: &StyleEntry) -> String {
    match s.library_name.as_deref() {
        Some(lib) => format!("Apply style {} from library {}", s.name, lib),
        None => format!("Apply style {}", s.name),
    }
}

/// `"A", "B"` for up to three variable names, then an ellipsis.
fn name_list(vars: &[&Entry]) -> String {
    let mut names: Vec<String> = vars.iter().take(3).map(|e| fmt_var_quoted(e)).collect();
    if vars.len() > 3 {
        names.push("…".to_string());
    }
    names.join(", ")
}

fn rule_on(doc: &Document, rule: Rule) -> bool {
    !doc.check_designs.disabled_rules.contains(rule.figma_id())
}

pub(super) fn check(doc: &Document, index: &Index, n: &Node, out: &mut Vec<Finding>) {
    if matches!(n.kind, NodeKind::Unknown { .. }) {
        return;
    }
    let resolved = designos_doc::resolved_node(doc, n.id);
    let rn = resolved.as_deref().unwrap_or(n);
    let src_resolved = n.component.source.and_then(|s| designos_doc::resolved_node(doc, s));
    if n.component.source.is_some() && src_resolved.is_none() {
        return;
    }
    let src = src_resolved.as_deref();
    if rule_on(doc, Rule::HardCodedValue) {
        paints_rule(doc, index, rn, src, Slot::Fill, out);
        if rn.stroke.weight > 0.0 {
            paints_rule(doc, index, rn, src, Slot::Stroke, out);
        }
    }
    numbers_rule(doc, index, rn, src, out);
    if rule_on(doc, Rule::TypographyAndStroke) {
        check_typography_numbers(doc, index, rn, src, out);
    }
    if rule_on(doc, Rule::TextStyles) {
        super::text_styles::check(index, rn, src, out);
    }
    if rule_on(doc, Rule::HardCodedValue) {
        effects_rule(index, rn, src, out);
    }
}

#[allow(clippy::too_many_arguments)]
fn check_number_prop(
    doc: &Document,
    index: &Index,
    n: &Node,
    rule: Rule,
    prop_key: &str,
    keys: &[&str],
    label: &str,
    val: f32,
    emit: bool,
    out: &mut Vec<Finding>,
) -> bool {
    let Some(&first_key) = keys.first() else { return false };
    match index.find_number(doc, n, first_key, val) {
        Match::None => false,
        Match::One(e) => {
            if emit {
                let message = format!("{label} {} matches variable {}", fmt_num(val), fmt_var_quoted(e));
                let fix = if keys.len() == 1 {
                    Fix::BindVariable { key: first_key.into(), variable: e.fix_id.clone() }
                } else {
                    Fix::BindVariables { keys: keys.iter().map(|k| (*k).to_string()).collect(), variable: e.fix_id.clone() }
                };
                out.push(finding(n, rule, prop_key, message, fix, fmt_var_bind_label(e)));
            }
            true
        }
        Match::Ambiguous(vars) => {
            if emit {
                let message = format!("{label} {} matches {} variables: {}", fmt_num(val), vars.len(), name_list(&vars));
                out.push(report_only(n, rule, prop_key, message));
            }
            true
        }
    }
}

/// Raw positive numeric value on `n` for binding `key`, when that property applies to `n`.
fn number_value_at(n: &Node, key: &str) -> Option<f32> {
    let v = match key {
        "itemSpacing" => {
            let al = n.kind.frame_props()?.auto_layout.as_ref()?;
            if matches!(al.primary_align, LayoutAlign::SpaceBetween | LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly) {
                return None;
            }
            al.item_spacing
        }
        "counterAxisSpacing" => {
            let al = n.kind.frame_props()?.auto_layout.as_ref()?;
            al.counter_spacing
        }
        "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft" => {
            let al = n.kind.frame_props()?.auto_layout.as_ref()?;
            let i = match key {
                "paddingTop" => 0,
                "paddingRight" => 1,
                "paddingBottom" => 2,
                _ => 3,
            };
            al.padding[i]
        }
        "topLeftRadius" | "topRightRadius" | "bottomRightRadius" | "bottomLeftRadius" => {
            if matches!(n.kind, NodeKind::Text(_) | NodeKind::Line | NodeKind::Group | NodeKind::Vector { .. } | NodeKind::Connector { .. }) {
                return None;
            }
            let i = match key {
                "topLeftRadius" => 0,
                "topRightRadius" => 1,
                "bottomRightRadius" => 2,
                _ => 3,
            };
            n.corner_radii[i]
        }
        "strokeWeight" => {
            if n.stroke.side_weights.is_some() || !n.stroke.paints.iter().any(|p| p.visible) {
                return None;
            }
            n.stroke.weight
        }
        "strokeTopWeight" | "strokeRightWeight" | "strokeBottomWeight" | "strokeLeftWeight" => {
            if !n.stroke.paints.iter().any(|p| p.visible) {
                return None;
            }
            let sides = n.stroke.side_weights?;
            let i = match key {
                "strokeTopWeight" => 0,
                "strokeRightWeight" => 1,
                "strokeBottomWeight" => 2,
                _ => 3,
            };
            sides[i]
        }
        "fontSize" => {
            let NodeKind::Text(t) = &n.kind else { return None };
            if n.style_refs.contains_key("text") || !t.runs.iter().all(|r| same_number(r.style.font_size, t.style.font_size)) {
                return None;
            }
            t.style.font_size
        }
        "lineHeight" => {
            let NodeKind::Text(t) = &n.kind else { return None };
            let LineHeight::Pixels(lh) = t.style.line_height else { return None };
            if n.style_refs.contains_key("text") || !t.runs.iter().all(|r| matches!(r.style.line_height, LineHeight::Pixels(rlh) if same_number(rlh, lh))) {
                return None;
            }
            lh
        }
        _ => return None,
    };
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Whether `key` on an instance's layer is exactly what its main component's layer already has, so
/// the finding belongs to the main component and not to the instance.
fn number_inherited(n: &Node, src: Option<&Node>, key: &str) -> bool {
    src.is_some_and(|s| n.variables.bindings.get(key) == s.variables.bindings.get(key) && number_value_at(n, key) == number_value_at(s, key))
}

#[allow(clippy::too_many_arguments)]
fn check_single_number(
    doc: &Document,
    index: &Index,
    n: &Node,
    src: Option<&Node>,
    rule: Rule,
    key: &str,
    label: &str,
    emit: bool,
    out: &mut Vec<Finding>,
) -> bool {
    if n.variables.bindings.contains_key(key) || number_inherited(n, src, key) {
        return false;
    }
    let Some(val) = number_value_at(n, key) else { return false };
    check_number_prop(doc, index, n, rule, key, &[key], label, val, emit, out)
}

#[allow(clippy::too_many_arguments)]
fn check_quad(
    doc: &Document,
    index: &Index,
    n: &Node,
    src: Option<&Node>,
    rule: Rule,
    uniform_key: &str,
    uniform_label: &str,
    sides: [(&str, &str); 4],
    out: &mut Vec<Finding>,
) {
    if !rule_on(doc, rule) || sides.iter().all(|(k, _)| number_inherited(n, src, k)) {
        return;
    }
    let all_unbound = sides.iter().all(|(k, _)| !n.variables.bindings.contains_key(*k));
    if all_unbound {
        if let Some(v0) = number_value_at(n, sides[0].0) {
            let all_equal = sides[1..].iter().all(|(k, _)| number_value_at(n, k).is_some_and(|v| same_number(v, v0)));
            if all_equal {
                let keys = [sides[0].0, sides[1].0, sides[2].0, sides[3].0];
                check_number_prop(doc, index, n, rule, uniform_key, &keys, uniform_label, v0, true, out);
                return;
            }
        }
    }
    for (k, label) in sides {
        check_single_number(doc, index, n, src, rule, k, label, true, out);
    }
}

/// Spec 158 section 3.1: the gap of a horizontal stack is a horizontal spacing field, the gap of a
/// vertical stack and every `counterAxisSpacing` (the row gap) are vertical ones.
fn spacing_rule_for(n: &Node, key: &str) -> Rule {
    let dir = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).map(|al| al.direction).unwrap_or(designos_doc::LayoutDirection::Horizontal);
    match (dir, key) {
        (_, "counterAxisSpacing") | (designos_doc::LayoutDirection::Vertical, "itemSpacing") => Rule::VerticalSpacing,
        _ => Rule::HorizontalSpacing,
    }
}

fn numbers_rule(doc: &Document, index: &Index, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    if index.by_num.is_empty() {
        return;
    }
    let gap_rule = spacing_rule_for(n, "itemSpacing");
    if rule_on(doc, gap_rule) {
        check_single_number(doc, index, n, src, gap_rule, "itemSpacing", "Gap", true, out);
    }
    let rgap_rule = spacing_rule_for(n, "counterAxisSpacing");
    if rule_on(doc, rgap_rule) {
        check_single_number(doc, index, n, src, rgap_rule, "counterAxisSpacing", "Row gap", true, out);
    }
    check_quad(
        doc,
        index,
        n,
        src,
        Rule::Padding,
        "padding",
        "Padding",
        [("paddingTop", "Top padding"), ("paddingRight", "Right padding"), ("paddingBottom", "Bottom padding"), ("paddingLeft", "Left padding")],
        out,
    );
    check_quad(
        doc,
        index,
        n,
        src,
        Rule::CornerRadius,
        "cornerRadius",
        "Corner radius",
        [
            ("topLeftRadius", "Top-left radius"),
            ("topRightRadius", "Top-right radius"),
            ("bottomRightRadius", "Bottom-right radius"),
            ("bottomLeftRadius", "Bottom-left radius"),
        ],
        out,
    );
    if rule_on(doc, Rule::TypographyAndStroke) {
        check_single_number(doc, index, n, src, Rule::TypographyAndStroke, "strokeWeight", "Stroke weight", true, out);
        for (k, label) in [
            ("strokeTopWeight", "Stroke top weight"),
            ("strokeRightWeight", "Stroke right weight"),
            ("strokeBottomWeight", "Stroke bottom weight"),
            ("strokeLeftWeight", "Stroke left weight"),
        ] {
            check_single_number(doc, index, n, src, Rule::TypographyAndStroke, k, label, true, out);
        }
    }
}

fn paints_rule(doc: &Document, index: &Index, n: &Node, src: Option<&Node>, slot: Slot, out: &mut Vec<Finding>) {
    let paints = slot.paints(n);
    if paints.is_empty() || n.style_refs.contains_key(slot.ref_key()) {
        return;
    }
    if let Some(s) = src {
        let same_paints = slot.paints(s) == paints && s.style_refs.get(slot.ref_key()) == n.style_refs.get(slot.ref_key());
        let same_bindings = (0..paints.len()).all(|i| {
            let key = format!("{}/{i}", slot.prefix());
            s.variables.bindings.get(&key) == n.variables.bindings.get(&key)
        });
        if same_paints && same_bindings {
            return;
        }
    }
    let mut matched_variable = false;
    for (i, p) in paints.iter().enumerate() {
        let PaintKind::Solid(c) = &p.kind else { continue };
        let key = format!("{}/{i}", slot.prefix());
        if !p.visible || n.variables.bindings.contains_key(&key) {
            continue;
        }
        if src.is_some_and(|s| slot.paints(s).get(i) == Some(p) && s.variables.bindings.get(&key) == n.variables.bindings.get(&key)) {
            continue;
        }
        match index.find(doc, n, &key, c) {
            Match::None => {}
            Match::One(e) => {
                matched_variable = true;
                let message = format!("{} #{} matches variable {}", slot.label(), c.to_hex(), fmt_var_quoted(e));
                out.push(finding(n, Rule::HardCodedValue, key.clone(), message, Fix::BindVariable { key, variable: e.fix_id.clone() }, fmt_var_bind_label(e)));
            }
            Match::Ambiguous(vars) => {
                matched_variable = true;
                let message = format!("{} #{} matches {} variables: {}", slot.label(), c.to_hex(), vars.len(), name_list(&vars));
                out.push(report_only(n, Rule::HardCodedValue, key, message));
            }
        }
    }
    // A style replaces the whole list, so it is only suggested when no paint of the layer is bound.
    let any_bound = n.variables.bindings.keys().any(|k| k.starts_with(slot.prefix()));
    if matched_variable || any_bound {
        return;
    }
    let style = index.styles.iter().find(|s| matches!(&s.style, Style::Fill(ps) if ps == paints) && s.property == slot.property());
    if let Some(s) = style {
        out.push(finding(
            n,
            Rule::HardCodedValue,
            slot.ref_key(),
            format!("{} matches style {}", slot.label(), fmt_style_quoted(s)),
            Fix::ApplyStyle { style: s.fix_key.clone() },
            fmt_style_apply_label(s),
        ));
    }
}

/// Text properties equal a text style's apart from the colour, which a text style does not carry.
fn same_text_style(own: &TextStyle, style: &TextStyle) -> bool {
    TextStyle { fills: own.fills.clone(), ..style.clone() } == *own
}

/// Whether the text style of an instance's layer is exactly its main component's, so a text style
/// finding belongs to the main component and not to the instance.
pub(super) fn text_style_inherited(n: &Node, src: Option<&Node>) -> bool {
    let (NodeKind::Text(t), Some(NodeKind::Text(st))) = (&n.kind, src.map(|s| &s.kind)) else { return false };
    same_text_style(&t.style, &st.style) && t.runs.len() == st.runs.len() && t.runs.iter().zip(&st.runs).all(|(a, b)| same_text_style(&a.style, &b.style))
}

fn check_typography_numbers(doc: &Document, index: &Index, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    let NodeKind::Text(_) = &n.kind else { return };
    if n.style_refs.contains_key("text") || index.by_num.is_empty() || text_style_inherited(n, src) {
        return;
    }
    check_single_number(doc, index, n, src, Rule::TypographyAndStroke, "fontSize", "Font size", true, out);
    check_single_number(doc, index, n, src, Rule::TypographyAndStroke, "lineHeight", "Line height", true, out);
}

impl Index {
    pub(super) fn find_matching_text_style(&self, t: &designos_doc::TextProps) -> Option<(String, String, String)> {
        let mut matches = self
            .styles
            .iter()
            .filter(|s| matches!(&s.style, Style::Text(ts) if same_text_style(&t.style, ts) && t.runs.iter().all(|r| same_text_style(&r.style, ts))));
        // Two equal styles leave the choice to the user, so none is offered.
        let s = matches.next().filter(|_| matches.next().is_none())?;
        Some((s.fix_key.clone(), fmt_style_quoted(s), fmt_style_apply_label(s)))
    }
}

fn effects_rule(index: &Index, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    if n.effects.is_empty() || n.style_refs.contains_key("effect") {
        return;
    }
    if src.is_some_and(|s| s.effects == n.effects) {
        return;
    }
    let style = index.styles.iter().find(|s| matches!(&s.style, Style::Effect(fx) if *fx == n.effects));
    if let Some(s) = style {
        out.push(finding(
            n,
            Rule::HardCodedValue,
            "effect",
            format!("Effects match style {}", fmt_style_quoted(s)),
            Fix::ApplyStyle { style: s.fix_key.clone() },
            fmt_style_apply_label(s),
        ));
    }
}

/// Concrete `NodeVariables::bindings` keys to write when applying a [`Fix::BindVariable`], or an
/// empty list when the finding is no longer current.
pub(super) fn keys_to_bind<'a>(doc: &Document, id: NodeId, key: &'a str, variable: &str) -> Vec<&'a str> {
    match key {
        "padding" => {
            if PADDING_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable)) {
                PADDING_KEYS.to_vec()
            } else {
                Vec::new()
            }
        }
        "cornerRadius" => {
            if RADIUS_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable)) {
                RADIUS_KEYS.to_vec()
            } else {
                Vec::new()
            }
        }
        other => {
            if bind_is_current(doc, id, other, variable) {
                vec![other]
            } else {
                Vec::new()
            }
        }
    }
}

/// Whether binding `variable` to `key` on `id` is still a pure re-link: the property is unbound and
/// its raw value already equals what the variable resolves to here.
pub(super) fn bind_is_current(doc: &Document, id: NodeId, key: &str, variable: &str) -> bool {
    if key == "padding" {
        return PADDING_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable));
    }
    if key == "cornerRadius" {
        return RADIUS_KEYS.iter().all(|k| bind_is_current(doc, id, k, variable));
    }
    let (Some(n), Some(v)) = (doc.get(id), doc.variables.get(variable)) else { return false };
    if variable_library_id(doc, v).is_some_and(|lib_id| enabled_library_name(doc, lib_id).is_none()) {
        return false;
    }
    if let Some((slot_name, rest)) = key.split_once('/') {
        let slot = match slot_name {
            "fills" => Slot::Fill,
            "strokes" => Slot::Stroke,
            _ => return false,
        };
        let Some(p) = rest.parse::<usize>().ok().and_then(|i| slot.paints(n).get(i)) else { return false };
        let PaintKind::Solid(c) = &p.kind else { return false };
        return p.visible && !n.variables.bindings.contains_key(key) && !n.style_refs.contains_key(slot.ref_key()) && variable_holds(doc, n, key, v, c);
    }
    let Some(val) = number_value_at(n, key) else { return false };
    !n.variables.bindings.contains_key(key) && number_variable_holds(doc, n, key, v, val)
}

/// Whether binding an unimported library variable `raw_var` in `snap` to `key` on `id` is current.
pub(super) fn library_bind_is_current(doc: &Document, snap: &LibrarySnapshot, id: NodeId, key: &str, raw_var: &str) -> bool {
    if key == "padding" {
        return PADDING_KEYS.iter().all(|k| library_bind_is_current(doc, snap, id, k, raw_var));
    }
    if key == "cornerRadius" {
        return RADIUS_KEYS.iter().all(|k| library_bind_is_current(doc, snap, id, k, raw_var));
    }
    if enabled_library_name(doc, &snap.library_id).is_none() || snap.asset(&format!("var:{raw_var}")).is_none() {
        return false;
    }
    let (Some(n), Some(v)) = (doc.get(id), snap.variables.get(raw_var)) else { return false };
    let store = VariableStore { variables: snap.variables.clone(), collections: snap.variable_collections.clone() };
    let resolved = resolve_in_snapshot(doc, snap, &store, id, v);
    if let Some((slot_name, rest)) = key.split_once('/') {
        let slot = match slot_name {
            "fills" => Slot::Fill,
            "strokes" => Slot::Stroke,
            _ => return false,
        };
        let Some(p) = rest.parse::<usize>().ok().and_then(|i| slot.paints(n).get(i)) else { return false };
        let PaintKind::Solid(c) = &p.kind else { return false };
        return p.visible
            && !n.variables.bindings.contains_key(key)
            && !n.style_refs.contains_key(slot.ref_key())
            && scope_allows(&v.scopes, key, &n.kind)
            && matches!(resolved, Some(VariableValue::Color(vc)) if same_colour(&vc, c));
    }
    let Some(val) = number_value_at(n, key) else { return false };
    !n.variables.bindings.contains_key(key)
        && scope_allows(&v.scopes, scope_key_for_number(key), &n.kind)
        && matches!(resolved, Some(VariableValue::Number(vx)) if same_number(vx, val))
}

fn style_matches_node(_doc: &Document, n: &Node, s: &Style, property: StyleProperty) -> bool {
    match s {
        Style::Fill(ps) => {
            let slot = if property == StyleProperty::Stroke { Slot::Stroke } else { Slot::Fill };
            let any_bound = n.variables.bindings.keys().any(|k| k.starts_with(slot.prefix()));
            !n.style_refs.contains_key(slot.ref_key()) && !any_bound && slot.paints(n) == ps.as_slice()
        }
        Style::Text(ts) => match &n.kind {
            NodeKind::Text(t) => !n.style_refs.contains_key("text") && same_text_style(&t.style, ts) && t.runs.iter().all(|r| same_text_style(&r.style, ts)),
            _ => false,
        },
        Style::Effect(fx) => !n.style_refs.contains_key("effect") && !n.effects.is_empty() && n.effects == *fx,
        Style::Grid(_) => false,
    }
}

/// Whether applying `style` to `id` is still a pure re-link: the layer is not linked yet and its
/// values, as painted in its mode, already equal the style's. This compares on the same resolved
/// view [`check`] matched on, so a value that equals the style only once a variable resolves in
/// the layer's mode stays fixable.
pub(super) fn style_is_current(doc: &Document, id: NodeId, style: &str) -> bool {
    let (Some(resolved), Some(s)) = (designos_doc::resolved_node(doc, id), doc.styles.get(style)) else { return false };
    let n: &Node = &resolved;
    if style_library_id(doc, style).is_some_and(|lib_id| enabled_library_name(doc, lib_id).is_none()) {
        return false;
    }
    style_matches_node(doc, n, s, style_property(doc, style))
}

/// Whether applying an unimported library style `raw_style` in `snap` to `id` is current.
pub(super) fn library_style_is_current(doc: &Document, snap: &LibrarySnapshot, id: NodeId, raw_style: &str) -> bool {
    if enabled_library_name(doc, &snap.library_id).is_none() || snap.asset(&format!("style:{raw_style}")).is_none() {
        return false;
    }
    let (Some(resolved), Some(s)) = (designos_doc::resolved_node(doc, id), snap.styles.get(raw_style)) else { return false };
    let n: &Node = &resolved;
    let property = snap.style_meta.get(raw_style).map(|m| m.property).unwrap_or_else(|| default_style_property(s));
    style_matches_node(doc, n, s, property)
}
