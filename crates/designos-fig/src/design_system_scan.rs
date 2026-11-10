//! The design-system scan file: what `designos FILE --scan-design-system OUT.json` writes and what the DESIGN:OS
//! kernel (`ui ingest-figma-ds`) reads. The Figma plugin writes the same file from a live Figma session; this
//! module writes it from a [`Document`], with no window and no network.
//!
//! ```text
//! components[]  { id, key?, name, type, page, variantAxes?, section?, width?, boundVariables, instances, unresolvedInstances? }
//! tokens[]      { id, name, type, collection, valuesByMode }
//! styles[]      { id, name, type }
//! frames[]      { id, name, page, section?, w, h, instances, unresolvedInstances? }
//! counts        { components, tokens, styles }
//! sensing       { status: "complete" | "skipped", reason? }
//! ```
//!
//! Ids are the Figma ids kept from the file (`Node::source_id`, `Variable::id`), never Studio's own numeric ids.
//! `tokens` holds the variables of local collections only. `boundVariables`, `instances` and `frames` are
//! *sensing*: they are added after the other fields are complete, and a failure of that step leaves those fields
//! out and says so in `sensing` instead of losing the rest (see [`crate::design_system_scan_sensing`]).
//! The output is deterministic: stable ordering, no timestamps, no timings.

use crate::design_system_scan_sensing::{self as sensing, InstanceCount, ScanFrame};
use designos_doc::{library_asset_of_component, Document, Node, NodeId, NodeKind, PropKind, Style, StyleProperty, VariableValue};
use serde::ser::{SerializeMap, Serializer};
use serde::Serialize;
use serde_json::Value;

/// A scan could not be written because a part the file cannot do without has no Figma id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanError(pub String);

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScanError {}

/// A JSON object whose keys keep the order they were pushed in (the declared order of modes and variant axes).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ordered<V>(pub Vec<(String, V)>);

impl<V: Serialize> Serialize for Ordered<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanComponent {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub page: PageRef,
    #[serde(rename = "variantAxes", skip_serializing_if = "Option::is_none")]
    pub variant_axes: Option<Ordered<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<Value>,
    #[serde(rename = "boundVariables", skip_serializing_if = "Option::is_none")]
    pub bound_variables: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instances: Option<Vec<InstanceCount>>,
    /// Instances in the subtree that cannot be resolved; written only when above 0 (a set sums its variants).
    #[serde(rename = "unresolvedInstances", skip_serializing_if = "Option::is_none")]
    pub unresolved_instances: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanToken {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub collection: String,
    #[serde(rename = "valuesByMode")]
    pub values_by_mode: Ordered<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanStyle {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanCounts {
    pub components: usize,
    pub tokens: usize,
    pub styles: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sensing {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DesignSystemScan {
    pub components: Vec<ScanComponent>,
    pub tokens: Vec<ScanToken>,
    pub styles: Vec<ScanStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<ScanFrame>>,
    pub counts: ScanCounts,
    pub sensing: Sensing,
}

impl DesignSystemScan {
    /// The scan as the pretty JSON text written to the output file, ending in a newline.
    pub fn to_json_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("a scan holds only strings, numbers and lists");
        text.push('\n');
        text
    }
}

/// Scans `doc`. Fails only when a component or its page has no Figma id. A failure of the sensing step does not fail
/// the scan: those fields are left out and `sensing` says why.
pub fn scan_design_system(doc: &Document) -> Result<DesignSystemScan, ScanError> {
    let roots = component_roots(doc);
    let mut components = roots.iter().map(|&id| base_component(doc, id)).collect::<Result<Vec<_>, _>>()?;
    let tokens = scan_tokens(doc);
    let styles = scan_styles(doc);
    let counts = ScanCounts { components: components.len(), tokens: tokens.len(), styles: styles.len() };
    // Sensing runs on a finished result: whatever it does, the fields above stay as they are.
    let (frames, sensing) = match sensing::sense(doc, &roots) {
        Ok(sensed) => {
            for (c, found) in components.iter_mut().zip(sensed.components) {
                c.bound_variables = Some(found.bound);
                c.instances = Some(found.instances);
                c.unresolved_instances = (found.unresolved > 0).then_some(found.unresolved);
            }
            (Some(sensed.frames), Sensing { status: "complete", reason: None })
        }
        Err(reason) => (None, Sensing { status: "skipped", reason: Some(reason) }),
    };
    Ok(DesignSystemScan { components, tokens, styles, frames, counts, sensing })
}

/// Page ids and names for `page_node`, or an error naming the page when it has no Figma id.
pub(crate) fn page_ref(doc: &Document, page_node: NodeId) -> Result<PageRef, ScanError> {
    let page = doc.get(page_node).ok_or_else(|| ScanError(format!("page node {} is missing", page_node.0)))?;
    let id = figma_id(page, "page")?;
    Ok(PageRef { id, name: page.name.clone() })
}

pub(crate) fn figma_id(node: &Node, what: &str) -> Result<String, ScanError> {
    node.source_id.clone().filter(|s| !s.is_empty()).ok_or_else(|| {
        ScanError(format!(
            "{what} \"{}\" has no Figma id: the scan names nodes by the ids the .fig file gave them. Save the document as .fig and scan that file.",
            node.name
        ))
    })
}

/// Main components and component sets of every page, in document order. A variant is not listed: its set stands
/// for it. Instances are not entered: their content is copies, and a component cannot sit inside one.
fn component_roots(doc: &Document) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &page in &doc.pages {
        let mut stack = vec![page];
        while let Some(id) = stack.pop() {
            let Some(n) = doc.get(id) else { continue };
            let in_set = n.parent.and_then(|p| doc.get(p)).is_some_and(|p| matches!(p.kind, NodeKind::ComponentSet(_)));
            match n.kind {
                NodeKind::ComponentSet(_) => out.push(id),
                NodeKind::Component(_) if !in_set => out.push(id),
                NodeKind::Instance { .. } => continue,
                _ => {}
            }
            stack.extend(n.children.iter().rev().copied());
        }
    }
    out
}

/// Name of the innermost enclosing section of `id`, or `None` when it is not inside a section.
fn enclosing_section(doc: &Document, id: NodeId) -> Option<String> {
    let mut cur = doc.get(id)?.parent;
    for _ in 0..=doc.nodes.len() {
        let p = doc.get(cur?)?;
        if matches!(p.kind, NodeKind::Section) {
            return Some(p.name.clone());
        }
        cur = p.parent;
    }
    None
}

fn base_component(doc: &Document, id: NodeId) -> Result<ScanComponent, ScanError> {
    let n = doc.get(id).expect("component_roots lists existing nodes");
    let is_set = matches!(n.kind, NodeKind::ComponentSet(_));
    let page = doc.page_of(id).ok_or_else(|| ScanError(format!("component \"{}\" is on no page", n.name)))?;
    let axes: Vec<(String, Vec<String>)> = n
        .component
        .props
        .iter()
        .filter_map(|p| match &p.kind {
            PropKind::Variant { options } => Some((p.name.clone(), options.clone())),
            _ => None,
        })
        .collect();
    Ok(ScanComponent {
        id: figma_id(n, "component")?,
        key: component_key(doc, n),
        name: n.name.clone(),
        kind: if is_set { "COMPONENT_SET" } else { "COMPONENT" },
        page: page_ref(doc, page)?,
        variant_axes: (!axes.is_empty()).then_some(Ordered(axes)),
        section: enclosing_section(doc, id),
        width: Some(json_number(n.size.x)),
        bound_variables: None,
        instances: None,
        unresolved_instances: None,
    })
}

/// Whether `n` is the synthetic stand-in `link_remote_components` builds for an instance whose symbol is absent from
/// the file: `link_remote_component` leaves `source_id` empty and stores the missing `<session>:<local>` guid as its key.
pub(crate) fn is_missing_symbol_stand_in(n: &Node, key: &str) -> bool {
    n.source_id.is_none()
        && !n.fig_metadata.contains_key("componentKey")
        && key
            .split_once(':')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit()))
}

/// The publish key of a component or set: the one the file wrote, else the key of the library asset it stands in for.
pub(crate) fn component_key(doc: &Document, n: &Node) -> Option<String> {
    if let Some(k) = n.fig_metadata.get("componentKey").filter(|k| !k.is_empty()) {
        return Some(k.clone());
    }
    let (ekey, asset) = library_asset_of_component(doc, n.id).filter(|(_, e)| e.node == Some(n.id))?;
    let prefix = format!("{}/", asset.library_id);
    let key = ekey.strip_prefix(&prefix).unwrap_or(ekey);
    (!is_missing_symbol_stand_in(n, key)).then(|| key.to_string())
}

/// Shortest decimal text of a float, as a JSON number (an integral value is written without a fraction).
pub(crate) fn json_number(x: f32) -> Value {
    let x = if x.is_finite() { x } else { 0.0 };
    // `f32` Display is the shortest text that reads back to the same `f32`, so 0.2 stays 0.2 and not 0.20000000298.
    let wide: f64 = x.to_string().parse().unwrap_or(0.0);
    if wide.fract() == 0.0 && wide.abs() < 1e15 {
        Value::from(wide as i64)
    } else {
        serde_json::Number::from_f64(wide).map_or(Value::Null, Value::Number)
    }
}

fn token_value(v: &VariableValue) -> Value {
    match v {
        VariableValue::Color(c) => {
            let channel = |x: f32, neutral: f32| json_number(if x.is_finite() { x } else { neutral });
            serde_json::json!({"r": channel(c.r, 0.0), "g": channel(c.g, 0.0), "b": channel(c.b, 0.0), "a": channel(c.a, 1.0)})
        }
        VariableValue::Number(n) => json_number(*n),
        VariableValue::String(s) => Value::String(s.clone()),
        VariableValue::Boolean(b) => Value::Bool(*b),
        VariableValue::Alias(id) => serde_json::json!({"type": "VARIABLE_ALIAS", "id": id}),
    }
}

/// Variables of local collections: a collection that is not a subscribed copy of a library, and not deleted.
/// Collections come in id order, variables in the collection's display order, then any the list forgot, by id.
fn scan_tokens(doc: &Document) -> Vec<ScanToken> {
    let mut out = Vec::new();
    for coll in doc.variable_collections.values().filter(|c| !c.is_library() && !c.is_soft_deleted()) {
        let mut seen = std::collections::BTreeSet::new();
        let unlisted = doc.variables.values().filter(|v| v.collection == coll.id).map(|v| &v.id);
        for vid in coll.variables.iter().chain(unlisted) {
            let Some(v) = doc.variables.get(vid).filter(|v| v.collection == coll.id && !v.is_soft_deleted()) else { continue };
            if !seen.insert(&v.id) {
                continue;
            }
            let mut by_mode: Vec<(String, Value)> = Vec::new();
            for mode in &coll.modes {
                let Some(value) = v.values.get(&mode.id) else { continue };
                // Mode names are unique in Figma; a repeat would overwrite a value in the reader, so it keeps its id.
                let name = if by_mode.iter().any(|(k, _)| *k == mode.name) { format!("{} ({})", mode.name, mode.id) } else { mode.name.clone() };
                by_mode.push((name, token_value(value)));
            }
            let kind = match v.kind {
                designos_doc::VariableKind::Color => "COLOR",
                designos_doc::VariableKind::Number => "FLOAT",
                designos_doc::VariableKind::String => "STRING",
                designos_doc::VariableKind::Boolean => "BOOLEAN",
            };
            out.push(ScanToken { id: v.id.clone(), name: v.name.clone(), kind, collection: coll.name.clone(), values_by_mode: Ordered(by_mode) });
        }
    }
    out
}

/// Local paint, text and effect styles, in that order, each in the file's style order (grid styles are not scanned).
fn scan_styles(doc: &Document) -> Vec<ScanStyle> {
    let mut rows: Vec<(u8, Option<&str>, &String, ScanStyle)> = Vec::new();
    for (id, style) in &doc.styles {
        let Some(meta) = doc.style_meta.get(id) else { continue };
        if id.starts_with("key:") || designos_doc::is_library_style(doc, id) {
            continue;
        }
        let (order, kind) = match (style, meta.property) {
            (Style::Fill(_), StyleProperty::Fill | StyleProperty::Stroke) => (0, "PAINT"),
            (Style::Text(_), _) => (1, "TEXT"),
            (Style::Effect(_), _) => (2, "EFFECT"),
            _ => continue,
        };
        rows.push((order, meta.sort_position.as_deref(), id, ScanStyle { id: id.clone(), name: meta.name.clone(), kind }));
    }
    rows.sort_by(|a, b| (a.0, a.1.is_none(), a.1, a.2).cmp(&(b.0, b.1.is_none(), b.1, b.2)));
    rows.into_iter().map(|r| r.3).collect()
}
