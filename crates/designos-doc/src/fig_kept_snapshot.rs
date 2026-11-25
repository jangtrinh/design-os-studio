//! The value of a node as it was when a `.fig` open finished, to tell later whether the node was edited.
//!
//! A snapshot is a value, not a flag: it is the hash of the node's property groups together with the nine layout floats
//! kept as they are. Undo, redo, an agent's edit and a replay all change the node or put it back, and the comparison
//! sees exactly that. The groups are the ones [`crate::branch_diff::node_group_eq`] compares, hashed once each; a later
//! slice combines them into one hash per family of fields.
//!
//! What a loader writes on its own is left out so that it never reads as an edit: the imported-layout record of a
//! component link (and the runtime-only flags the link does not serialize) and the glyphs Figma stored for a text, which
//! have a rule of their own.

use crate::branch_diff::{StableHasher, FLOAT_TOLERANCE};
use crate::{Node, NodeKind};

/// How many property groups a node is hashed in: the 27 of `branch_diff` less the three that are layout floats
/// (opacity, transform, size) and the position, which is the tree's.
const GROUPS: usize = 24;

/// A node's value when the snapshot was taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSnapshot {
    /// The groups other than the layout floats, hashed once each and folded into one value.
    hash: u64,
    /// Opacity, the six cells of the transform, width and height. The layout solver rewrites these by amounts below
    /// [`FLOAT_TOLERANCE`], so they are compared with the tolerance and not hashed.
    floats: [f32; 9],
}

fn layout_floats(node: &Node) -> [f32; 9] {
    let t = node.transform;
    [node.opacity, t.a, t.b, t.c, t.d, t.tx, t.ty, node.size.x, node.size.y]
}

fn close(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a - b).abs() < FLOAT_TOLERANCE
}

impl NodeSnapshot {
    pub fn of(node: &Node) -> Self {
        Self { hash: fold(&group_hashes(node)), floats: layout_floats(node) }
    }

    /// Whether `node` still has the value this snapshot took: every group equal and every layout float within
    /// [`FLOAT_TOLERANCE`].
    pub fn matches(&self, node: &Node) -> bool {
        let now = layout_floats(node);
        self.floats.iter().zip(&now).all(|(a, b)| close(*a, *b)) && self.hash == fold(&group_hashes(node))
    }
}

fn fold(groups: &[u64; GROUPS]) -> u64 {
    let mut h = StableHasher::new();
    for g in groups {
        h.word(*g);
    }
    h.finish()
}

/// One hash per group, each computed once.
fn group_hashes(node: &Node) -> [u64; GROUPS] {
    let mut out = [0u64; GROUPS];
    let mut at = 0;
    let mut group = |f: &dyn Fn(&mut StableHasher)| {
        let mut h = StableHasher::new();
        f(&mut h);
        out[at] = h.finish();
        at += 1;
    };
    group(&|h| h.add(&node.name));
    group(&|h| h.add(&node.visible));
    group(&|h| h.add(&node.locked));
    group(&|h| h.add(&node.blend));
    group(&|h| h.add(&node.fills));
    group(&|h| h.add(&node.stroke));
    group(&|h| h.add(&node.effects));
    group(&|h| {
        for r in node.corner_radii {
            h.float(r);
        }
        h.float(node.corner_smoothing);
    });
    group(&|h| h.add(&node.constraints));
    group(&|h| h.add(&node.layout_child));
    group(&|h| {
        h.add(&node.is_mask);
        h.add(&node.mask_type);
    });
    group(&|h| {
        h.add(&node.style_refs);
        h.add(&node.text_style_overrides);
    });
    group(&|h| h.add(&node.variables));
    group(&|h| hash_component(h, node));
    group(&|h| h.add(&node.prototype));
    group(&|h| h.add(&node.dev));
    group(&|h| h.add(&node.export_settings));
    group(&|h| hash_kind(h, &node.kind));
    group(&|h| h.add(&node.kind.any_text().map(|t| &t.content)));
    group(&|h| hash_text_style(h, node));
    group(&|h| h.add(&node.kind.frame_props()));
    group(&|h| hash_vector(h, node));
    group(&|h| h.add(&node.plugin_data));
    group(&|h| {
        h.add(&node.source_id);
        h.add(&node.slide);
        h.add(&node.lock_aspect);
        h.add(&node.hyperlink);
        h.add(&node.detached_symbol_id);
        h.add(&node.fig_metadata);
        h.add(&node.motion);
        h.add(&node.extra);
    });
    debug_assert_eq!(at, GROUPS, "every group is hashed");
    out
}

/// The component link without the record a loader writes, plus the main an instance names.
fn hash_component(h: &mut StableHasher, node: &Node) {
    if node.component.imported_layout.is_some() {
        let bare = crate::ComponentLink { imported_layout: None, ..node.component.clone() };
        h.add(&bare);
    } else {
        h.add(&node.component);
    }
    if let NodeKind::Instance { component, component_key, .. } = &node.kind {
        h.add(&(component, component_key));
    }
}

fn hash_text_style(h: &mut StableHasher, node: &Node) {
    let Some(t) = node.kind.any_text() else {
        h.word(0);
        return;
    };
    h.word(1);
    h.add(&t.style);
    h.add(&t.runs);
    h.add(&t.auto_resize);
    h.add(&t.max_lines);
    h.add(&t.paragraph);
    h.add(&t.lists);
    h.add(&t.direction);
    h.add(&t.auto_rename);
    h.add(&t.decoration_skip_ink);
    h.add(&t.text_wrap);
    h.add(&t.uniform_scale);
}

fn hash_vector(h: &mut StableHasher, node: &Node) {
    h.add(&node.fill_geometry);
    h.add(&node.stroke_geometry);
    h.add(&node.effective_stroke_geometry_stamp().as_deref());
    if let NodeKind::Vector { paths, network } = &node.kind {
        h.word(1);
        h.add(paths);
        h.add(network);
    } else {
        h.word(0);
    }
}

/// The discriminant of the kind and the scalars [`crate::branch_diff::node_group_eq`] compares for it; what the other
/// groups hold (frame properties, text, vector data, component data) is hashed by them.
fn hash_kind(h: &mut StableHasher, kind: &NodeKind) {
    h.word(u64::from(kind_tag(kind)));
    match kind {
        NodeKind::Page { background } => h.add(background),
        NodeKind::Ellipse(arc) => h.add(arc),
        NodeKind::Polygon { count } => h.add(count),
        NodeKind::Star { count, inner_ratio } => {
            h.add(count);
            h.add(inner_ratio);
        }
        NodeKind::BooleanOp(op) => h.add(op),
        NodeKind::Sticky(s) => {
            h.add(&s.color);
            h.add(&s.author);
            h.add(&s.show_author);
        }
        NodeKind::Connector(c) => {
            h.add(&c.start);
            h.add(&c.end);
            h.add(&c.line);
            h.add(&c.start_cap);
            h.add(&c.end_cap);
        }
        NodeKind::ShapeWithText(s) => h.add(&s.shape),
        NodeKind::Stamp { kind } => h.add(kind),
        NodeKind::Unknown { type_name } => h.add(type_name),
        NodeKind::Table(t) => h.add(t),
        NodeKind::TableCell(c) => {
            h.add(&c.row);
            h.add(&c.col);
        }
        _ => {}
    }
}

/// A number for each kind, fixed by this table and not by the compiler, so a snapshot stored in a file reads the same in
/// every build. A kind added to [`NodeKind`] stops this match from compiling until it is given a number here.
fn kind_tag(kind: &NodeKind) -> u8 {
    match kind {
        NodeKind::Page { .. } => 0,
        NodeKind::Frame(_) => 1,
        NodeKind::Group => 2,
        NodeKind::Section => 3,
        NodeKind::Rectangle => 4,
        NodeKind::Ellipse(_) => 5,
        NodeKind::Line => 6,
        NodeKind::Polygon { .. } => 7,
        NodeKind::Star { .. } => 8,
        NodeKind::Vector { .. } => 9,
        NodeKind::Text(_) => 10,
        NodeKind::BooleanOp(_) => 11,
        NodeKind::Component(_) => 12,
        NodeKind::ComponentSet(_) => 13,
        NodeKind::Instance { .. } => 14,
        NodeKind::Sticky(_) => 15,
        NodeKind::Connector(_) => 16,
        NodeKind::ShapeWithText(_) => 17,
        NodeKind::Stamp { .. } => 18,
        NodeKind::Table(_) => 19,
        NodeKind::TableCell(_) => 20,
        NodeKind::Unknown { .. } => 21,
    }
}
