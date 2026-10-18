//! Dev mode data: "Ready for dev" status, dev links and annotations, plus the
//! change tracking behind "Compare changes".
//!
//! A frame or section marked *Ready for dev* remembers a fingerprint of its
//! subtree and a snapshot to diff against. When anything in the subtree
//! changes afterwards, [`refresh_statuses`] flips the status to *Changed*; it
//! flips back when the edit is undone. The snapshot lives either in the
//! file's version history (`snapshot_version`, the app saves a named version)
//! or, for files outside the library, inline as a serialized subtree
//! (`snapshot`).

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DevStatus {
    ReadyForDev,
    Completed,
    Changed,
}

impl DevStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadyForDev => "Ready for dev",
            Self::Completed => "Completed",
            Self::Changed => "Changed",
        }
    }
}

/// A URL attached to a layer (GitHub, Storybook, Jira, ...).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DevLink {
    pub label: String,
    pub url: String,
}

/// A note pinned to a layer in Dev mode.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub text: String,
    /// Pinned property labels ("Width", "Fill", "Font", ...), see [`ANNOTATION_PROPERTIES`].
    pub properties: Vec<String>,
    pub category: Option<String>,
}

/// Property labels an annotation can pin, with the matching Figma REST annotation property type.
pub const ANNOTATION_PROPERTIES: [(&str, &str); 22] = [
    ("Width", "width"),
    ("Height", "height"),
    ("Min width", "minWidth"),
    ("Max width", "maxWidth"),
    ("Min height", "minHeight"),
    ("Max height", "maxHeight"),
    ("Fill", "fills"),
    ("Stroke", "strokes"),
    ("Effects", "effects"),
    ("Stroke weight", "strokeWeight"),
    ("Corner radius", "cornerRadius"),
    ("Text style", "textStyleId"),
    ("Text align", "textAlignHorizontal"),
    ("Font", "fontFamily"),
    ("Font style", "fontStyle"),
    ("Font size", "fontSize"),
    ("Font weight", "fontWeight"),
    ("Line height", "lineHeight"),
    ("Letter spacing", "letterSpacing"),
    ("Gap", "itemSpacing"),
    ("Padding", "padding"),
    ("Opacity", "opacity"),
];

/// Figma REST annotation property type for a label (unknown labels pass through).
pub fn annotation_property_type(label: &str) -> String {
    ANNOTATION_PROPERTIES.iter().find(|(l, _)| *l == label).map(|(_, t)| t.to_string()).unwrap_or_else(|| label.to_string())
}

/// Label for a Figma REST annotation property type (unknown types pass through).
pub fn annotation_property_label(ty: &str) -> String {
    ANNOTATION_PROPERTIES.iter().find(|(_, t)| *t == ty).map(|(l, _)| l.to_string()).unwrap_or_else(|| ty.to_string())
}

/// Dev mode data of a node; empty for most nodes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DevInfo {
    pub status: Option<DevStatus>,
    /// RFC 3339 time of the last status change.
    pub status_updated: Option<String>,
    pub links: Vec<DevLink>,
    pub annotations: Vec<Annotation>,
    /// Fingerprint of the subtree when it was marked Ready for dev.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// Version number in the file's version history that holds the marking snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_version: Option<u32>,
    /// Serialized subtree (JSON array of nodes, root first) taken at marking time,
    /// used when the file has no version history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

impl DevInfo {
    pub fn is_empty(&self) -> bool {
        *self == DevInfo::default()
    }

    /// Whether this node has a dev status that should list it in the Dev mode panel.
    pub fn is_tracked(&self) -> bool {
        self.status.is_some()
    }
}

/// 64-bit FNV-1a, stable across Rust versions (fingerprints are persisted).
fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Copy of `id` and its descendants (root first, pre-order) with the dev data cleared,
/// so dev edits never count as design changes.
pub fn subtree_nodes(doc: &Document, id: NodeId) -> Vec<Node> {
    let mut out = Vec::new();
    doc.walk(id, |n, _| {
        let mut c = n.clone();
        c.dev = DevInfo::default();
        out.push(c);
    });
    if let Some(root) = out.first_mut() {
        root.parent = None;
    }
    out
}

/// Fingerprint of a subtree, ignoring dev data.
pub fn subtree_fingerprint(doc: &Document, id: NodeId) -> String {
    let nodes = subtree_nodes(doc, id);
    let json = serde_json::to_vec(&nodes).unwrap_or_default();
    format!("{:016x}", fnv(&json))
}

/// Serialized snapshot of a subtree for [`DevInfo::snapshot`].
pub fn snapshot_json(doc: &Document, id: NodeId) -> String {
    serde_json::to_string(&subtree_nodes(doc, id)).unwrap_or_default()
}

/// Parses an inline snapshot.
pub fn parse_snapshot(json: &str) -> Option<Vec<Node>> {
    let v: Vec<Node> = serde_json::from_str(json).ok()?;
    (!v.is_empty()).then_some(v)
}

/// Whether a node can be marked Ready for dev (frames, sections, components).
pub fn can_mark(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Frame(_) | NodeKind::Section | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. } | NodeKind::Group)
}

/// The command that marks `id` Ready for dev: the status, a fresh fingerprint and a snapshot.
/// With `snapshot_version` the snapshot is the library version of that number; otherwise it is
/// stored inline.
pub fn mark_ready(doc: &Document, id: NodeId, now: &str, snapshot_version: Option<u32>) -> Option<Command> {
    let mut n = doc.get(id)?.clone();
    n.dev.status = Some(DevStatus::ReadyForDev);
    n.dev.status_updated = Some(now.to_string());
    n.dev.fingerprint = Some(subtree_fingerprint(doc, id));
    n.dev.snapshot_version = snapshot_version;
    n.dev.snapshot = if snapshot_version.is_some() { None } else { Some(snapshot_json(doc, id)) };
    Some(Command::Update { node: Box::new(n) })
}

/// The command that marks `id` Completed (keeps the snapshot for reference).
pub fn mark_completed(doc: &Document, id: NodeId, now: &str) -> Option<Command> {
    let mut n = doc.get(id)?.clone();
    n.dev.status = Some(DevStatus::Completed);
    n.dev.status_updated = Some(now.to_string());
    Some(Command::Update { node: Box::new(n) })
}

/// The command that removes the status (links and annotations stay).
pub fn clear_status(doc: &Document, id: NodeId) -> Option<Command> {
    let mut n = doc.get(id)?.clone();
    n.dev.status = None;
    n.dev.status_updated = None;
    n.dev.fingerprint = None;
    n.dev.snapshot = None;
    n.dev.snapshot_version = None;
    Some(Command::Update { node: Box::new(n) })
}

/// Replaces the dev data of `id` (annotation and link edits).
pub fn set_dev(doc: &Document, id: NodeId, dev: DevInfo) -> Option<Command> {
    let mut n = doc.get(id)?.clone();
    n.dev = dev;
    Some(Command::Update { node: Box::new(n) })
}

/// Re-evaluates Ready for dev / Changed for every tracked node. Returns the nodes whose
/// status flipped. This derives state from the content, so it is applied directly and not
/// recorded in the undo history: undoing an edit flips the status back by itself.
pub fn refresh_statuses(doc: &mut Document, now: &str) -> Vec<NodeId> {
    let tracked: Vec<(NodeId, DevStatus, String)> = doc
        .nodes
        .values()
        .filter_map(|n| match (n.dev.status, &n.dev.fingerprint) {
            (Some(s @ (DevStatus::ReadyForDev | DevStatus::Changed)), Some(fp)) => Some((n.id, s, fp.clone())),
            _ => None,
        })
        .collect();
    let mut flipped = Vec::new();
    for (id, status, fp) in tracked {
        let same = subtree_fingerprint(doc, id) == fp;
        let want = if same { DevStatus::ReadyForDev } else { DevStatus::Changed };
        if want != status {
            if let Some(n) = doc.get_mut(id) {
                n.dev.status = Some(want);
                n.dev.status_updated = Some(now.to_string());
                flipped.push(id);
            }
        }
    }
    flipped
}

/// Nodes with a dev status on `page`, in document order.
pub fn items_on_page(doc: &Document, page: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    doc.walk(page, |n, _| {
        if n.dev.is_tracked() {
            out.push(n.id);
        }
    });
    out
}

/// Whether any node carries a dev status (cheap pre-check for [`refresh_statuses`]).
pub fn has_tracked(doc: &Document) -> bool {
    doc.nodes.values().any(|n| matches!(n.dev.status, Some(DevStatus::ReadyForDev | DevStatus::Changed)) && n.dev.fingerprint.is_some())
}

// ------------------------------------------------------------------ compare

/// One changed property of a node.
#[derive(Clone, Debug, PartialEq)]
pub struct PropChange {
    pub property: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified(Vec<PropChange>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeChange {
    pub id: NodeId,
    pub name: String,
    pub kind: ChangeKind,
}

fn n2(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn paints_text(p: &[Paint]) -> String {
    if p.is_empty() {
        return "none".into();
    }
    p.iter()
        .map(|p| {
            let base = match &p.kind {
                PaintKind::Solid(c) => format!("#{}", c.to_hex()),
                PaintKind::LinearGradient { .. } => "linear gradient".into(),
                PaintKind::RadialGradient { .. } => "radial gradient".into(),
                PaintKind::AngularGradient { .. } => "angular gradient".into(),
                PaintKind::DiamondGradient { .. } => "diamond gradient".into(),
                PaintKind::Image { .. } => "image".into(),
            };
            let mut s = base;
            if !p.visible {
                s.push_str(" (hidden)");
            }
            if p.opacity < 0.999 {
                s = format!("{s} {}%", n2(p.opacity * 100.0));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn effects_text(e: &[Effect]) -> String {
    if e.is_empty() {
        return "none".into();
    }
    e.iter()
        .map(|e| match e {
            Effect::DropShadow { offset, radius, .. } => format!("drop shadow {} {} {}", n2(offset.x), n2(offset.y), n2(*radius)),
            Effect::InnerShadow { offset, radius, .. } => format!("inner shadow {} {} {}", n2(offset.x), n2(offset.y), n2(*radius)),
            Effect::LayerBlur { radius, .. } => format!("layer blur {}", n2(*radius)),
            Effect::BackgroundBlur { radius, .. } => format!("background blur {}", n2(*radius)),
            Effect::Noise(_) => "noise".into(),
            Effect::Texture(_) => "texture".into(),
            Effect::Glass(_) => "glass".into(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn radii_text(r: [f32; 4]) -> String {
    if r.iter().all(|v| *v == r[0]) {
        n2(r[0])
    } else {
        r.iter().map(|v| n2(*v)).collect::<Vec<_>>().join(" ")
    }
}

fn kind_name(k: &NodeKind) -> String {
    k.default_name().to_string()
}

/// Property-level differences between two versions of the same node.
pub fn diff_node(old: &Node, new: &Node) -> Vec<PropChange> {
    let mut out = Vec::new();
    let mut add = |property: &str, before: String, after: String| {
        if before != after {
            out.push(PropChange { property: property.into(), before, after });
        }
    };
    add("Name", old.name.clone(), new.name.clone());
    add("Type", kind_name(&old.kind), kind_name(&new.kind));
    add("Visible", old.visible.to_string(), new.visible.to_string());
    add("Locked", old.locked.to_string(), new.locked.to_string());
    add("Opacity", n2(old.opacity * 100.0) + "%", n2(new.opacity * 100.0) + "%");
    add("Blend mode", format!("{:?}", old.blend), format!("{:?}", new.blend));
    add("Width", n2(old.size.x), n2(new.size.x));
    add("Height", n2(old.size.y), n2(new.size.y));
    add("X", n2(old.transform.tx), n2(new.transform.tx));
    add("Y", n2(old.transform.ty), n2(new.transform.ty));
    add("Rotation", n2(old.transform.rotation_rad().to_degrees()), n2(new.transform.rotation_rad().to_degrees()));
    add("Corner radius", radii_text(old.corner_radii), radii_text(new.corner_radii));
    add("Fill", paints_text(&old.fills), paints_text(&new.fills));
    add("Stroke", paints_text(&old.stroke.paints), paints_text(&new.stroke.paints));
    add("Stroke weight", n2(old.stroke.weight), n2(new.stroke.weight));
    add("Effects", effects_text(&old.effects), effects_text(&new.effects));
    add("Mask", old.is_mask.to_string(), new.is_mask.to_string());
    if let (Some(a), Some(b)) = (old.kind.frame_props(), new.kind.frame_props()) {
        add("Clip content", a.clips_content.to_string(), b.clips_content.to_string());
        match (&a.auto_layout, &b.auto_layout) {
            (Some(x), Some(y)) => {
                add("Direction", format!("{:?}", x.direction), format!("{:?}", y.direction));
                add("Gap", n2(x.item_spacing), n2(y.item_spacing));
                add("Padding", x.padding.map(n2).join(" "), y.padding.map(n2).join(" "));
                add("Primary alignment", format!("{:?}", x.primary_align), format!("{:?}", y.primary_align));
                add("Counter alignment", format!("{:?}", x.counter_align), format!("{:?}", y.counter_align));
                add("Primary sizing", format!("{:?}", x.primary_sizing), format!("{:?}", y.primary_sizing));
                add("Counter sizing", format!("{:?}", x.counter_sizing), format!("{:?}", y.counter_sizing));
            }
            (None, None) => {}
            (x, y) => add("Auto layout", if x.is_some() { "on" } else { "off" }.into(), if y.is_some() { "on" } else { "off" }.into()),
        }
    }
    if let (NodeKind::Text(a), NodeKind::Text(b)) = (&old.kind, &new.kind) {
        add("Text", a.content.clone(), b.content.clone());
        add("Font", a.style.font_family.clone(), b.style.font_family.clone());
        add("Font size", n2(a.style.font_size), n2(b.style.font_size));
        add("Font weight", a.style.font_weight.to_string(), b.style.font_weight.to_string());
        add("Text color", paints_text(&a.style.fills), paints_text(&b.style.fills));
        add("Line height", format!("{:?}", a.style.line_height), format!("{:?}", b.style.line_height));
        add("Letter spacing", n2(a.style.letter_spacing), n2(b.style.letter_spacing));
        add("Text align", format!("{:?}", a.style.align_h), format!("{:?}", b.style.align_h));
    }
    add(
        "Constraints",
        format!("{:?}/{:?}", old.constraints.horizontal, old.constraints.vertical),
        format!("{:?}/{:?}", new.constraints.horizontal, new.constraints.vertical),
    );
    out
}

/// Differences between an earlier snapshot of a subtree and the subtree now.
///
/// Nodes are matched by id: ids only in the snapshot are *Removed*, ids only in the document
/// are *Added*. Results are in document order (removed nodes last).
pub fn diff_subtree(old: &[Node], doc: &Document, root: NodeId) -> Vec<NodeChange> {
    let old_by_id: BTreeMap<NodeId, &Node> = old.iter().map(|n| (n.id, n)).collect();
    let mut seen = Vec::new();
    let mut out = Vec::new();
    doc.walk(root, |n, _| {
        seen.push(n.id);
        match old_by_id.get(&n.id) {
            None => out.push(NodeChange { id: n.id, name: n.name.clone(), kind: ChangeKind::Added }),
            Some(o) => {
                let mut changes = diff_node(o, n);
                // moving a child within or between parents shows up as an order change
                if o.children != n.children && o.children.iter().all(|c| n.children.contains(c)) && n.children.iter().all(|c| o.children.contains(c)) {
                    changes.push(PropChange { property: "Layer order".into(), before: "earlier".into(), after: "changed".into() });
                }
                if !changes.is_empty() {
                    out.push(NodeChange { id: n.id, name: n.name.clone(), kind: ChangeKind::Modified(changes) });
                }
            }
        }
    });
    for o in old {
        if !seen.contains(&o.id) {
            out.push(NodeChange { id: o.id, name: o.name.clone(), kind: ChangeKind::Removed });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Document, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let rect = d.insert(frame, Node::rectangle(rid, Rect::new(10.0, 10.0, 20.0, 20.0))).unwrap();
        (d, frame, rect)
    }

    fn exec(d: &mut Document, h: &mut History, f: impl FnOnce(&Document) -> Option<Command>) {
        let c = f(d).unwrap();
        h.exec(d, c).unwrap();
    }

    #[test]
    fn marking_sets_status_fingerprint_and_inline_snapshot_and_is_undoable() {
        let (mut d, frame, _) = setup();
        let mut h = History::default();
        exec(&mut d, &mut h, |d| mark_ready(d, frame, "2026-01-01T00:00:00Z", None));
        let n = d.get(frame).unwrap();
        assert_eq!(n.dev.status, Some(DevStatus::ReadyForDev));
        assert_eq!(n.dev.status_updated.as_deref(), Some("2026-01-01T00:00:00Z"));
        assert!(n.dev.fingerprint.is_some());
        assert_eq!(parse_snapshot(n.dev.snapshot.as_ref().unwrap()).unwrap().len(), 2);
        h.undo(&mut d).unwrap();
        assert!(d.get(frame).unwrap().dev.is_empty());
    }

    #[test]
    fn library_marking_stores_a_version_number_instead_of_a_snapshot() {
        let (mut d, frame, _) = setup();
        let c = mark_ready(&d, frame, "t", Some(7)).unwrap();
        c.apply(&mut d).unwrap();
        let n = d.get(frame).unwrap();
        assert_eq!((n.dev.snapshot_version, n.dev.snapshot.is_none()), (Some(7), true));
    }

    #[test]
    fn edits_flip_to_changed_and_undo_flips_back() {
        let (mut d, frame, rect) = setup();
        let mut h = History::default();
        exec(&mut d, &mut h, |d| mark_ready(d, frame, "t0", None));
        assert!(refresh_statuses(&mut d, "t1").is_empty());

        let mut r = d.get(rect).unwrap().clone();
        r.size.x = 44.0;
        exec(&mut d, &mut h, |_| Some(Command::Update { node: Box::new(r) }));
        assert_eq!(refresh_statuses(&mut d, "t2"), vec![frame]);
        assert_eq!(d.get(frame).unwrap().dev.status, Some(DevStatus::Changed));
        assert_eq!(d.get(frame).unwrap().dev.status_updated.as_deref(), Some("t2"));
        // idempotent
        assert!(refresh_statuses(&mut d, "t3").is_empty());

        h.undo(&mut d).unwrap();
        assert_eq!(refresh_statuses(&mut d, "t4"), vec![frame]);
        assert_eq!(d.get(frame).unwrap().dev.status, Some(DevStatus::ReadyForDev));
    }

    #[test]
    fn dev_edits_and_completed_do_not_count_as_changes() {
        let (mut d, frame, rect) = setup();
        let mut h = History::default();
        exec(&mut d, &mut h, |d| mark_ready(d, frame, "t0", None));
        // an annotation on a child is not a design change
        let mut dev = d.get(rect).unwrap().dev.clone();
        dev.annotations.push(Annotation { text: "Use 8px radius".into(), properties: vec!["Width".into()], category: None });
        exec(&mut d, &mut h, |d| set_dev(d, rect, dev));
        assert!(refresh_statuses(&mut d, "t1").is_empty());
        assert_eq!(d.get(frame).unwrap().dev.status, Some(DevStatus::ReadyForDev));
        // completed items stay completed even when edited
        exec(&mut d, &mut h, |d| mark_completed(d, frame, "t2"));
        d.get_mut(rect).unwrap().size.x = 99.0;
        assert!(refresh_statuses(&mut d, "t3").is_empty());
        assert_eq!(d.get(frame).unwrap().dev.status, Some(DevStatus::Completed));
    }

    #[test]
    fn re_marking_after_a_change_takes_a_new_baseline() {
        let (mut d, frame, rect) = setup();
        mark_ready(&d, frame, "t0", None).unwrap().apply(&mut d).unwrap();
        d.get_mut(rect).unwrap().size.x = 50.0;
        refresh_statuses(&mut d, "t1");
        assert_eq!(d.get(frame).unwrap().dev.status, Some(DevStatus::Changed));
        mark_ready(&d, frame, "t2", None).unwrap().apply(&mut d).unwrap();
        assert!(refresh_statuses(&mut d, "t3").is_empty());
        assert_eq!(d.get(frame).unwrap().dev.status, Some(DevStatus::ReadyForDev));
    }

    #[test]
    fn items_listed_per_page() {
        let (mut d, frame, _) = setup();
        let p2 = d.add_page("P2");
        let fid = d.alloc_id();
        let f2 = d.insert(p2, Node::frame(fid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        mark_ready(&d, frame, "t", None).unwrap().apply(&mut d).unwrap();
        mark_completed(&d, f2, "t").unwrap().apply(&mut d).unwrap();
        let page = d.page_of(frame).unwrap();
        assert_eq!(items_on_page(&d, page), vec![frame]);
        assert_eq!(items_on_page(&d, p2), vec![f2]);
        assert!(has_tracked(&d));
    }

    #[test]
    fn compare_reports_modified_added_and_removed() {
        let (mut d, frame, rect) = setup();
        let snap = subtree_nodes(&d, frame);
        d.get_mut(rect).unwrap().size.x = 44.0;
        d.get_mut(rect).unwrap().fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        let nid = d.alloc_id();
        let added = d.insert(frame, Node::ellipse(nid, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
        let diff = diff_subtree(&snap, &d, frame);
        assert_eq!(diff.len(), 2);
        let ChangeKind::Modified(props) = &diff[0].kind else { panic!("{diff:?}") };
        assert_eq!(diff[0].id, rect);
        assert_eq!(props[0], PropChange { property: "Width".into(), before: "20".into(), after: "44".into() });
        assert!(props.iter().any(|p| p.property == "Fill" && p.after == "#FF0000" && p.before == "#D9D9D9"));
        assert_eq!((diff[1].id, &diff[1].kind), (added, &ChangeKind::Added));
        // delete the rect: it becomes Removed
        d.remove(rect).unwrap();
        let diff = diff_subtree(&snap, &d, frame);
        assert!(diff.iter().any(|c| c.id == rect && c.kind == ChangeKind::Removed));
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let (d, frame, _) = setup();
        let json = snapshot_json(&d, frame);
        let nodes = parse_snapshot(&json).unwrap();
        assert_eq!(nodes, subtree_nodes(&d, frame));
        assert!(diff_subtree(&nodes, &d, frame).is_empty());
    }

    #[test]
    fn dev_info_serializes_only_when_present() {
        let (mut d, frame, _) = setup();
        let json = d.to_json().unwrap();
        assert!(!json.contains("\"dev\""));
        d.get_mut(frame).unwrap().dev.links.push(DevLink { label: "PR".into(), url: "https://github.com/a/b/pull/1".into() });
        let back = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back.get(frame).unwrap().dev.links.len(), 1);
    }

    #[test]
    fn property_type_mapping() {
        assert_eq!(annotation_property_type("Fill"), "fills");
        assert_eq!(annotation_property_label("fontFamily"), "Font");
        assert_eq!(annotation_property_label("gridRowGap"), "gridRowGap");
    }
}
