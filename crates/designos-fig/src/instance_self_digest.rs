//! Tells an edit of an unopened instance from the state the file recorded for it.
//!
//! An instance on a page the loader has not opened has no children, and the file's record of what it overrides is
//! written back as the file stated it. That record also states the instance's own fields (its size, fills, opacity...).
//! When the user changes one of them before saving, the record's value is out of date. The loader therefore takes a
//! digest of every overridden field of the instance when the file is opened, and a save compares the node against it:
//! a field whose digest moved was edited. The size is digested for every instance that holds derived data, flagged or
//! not: derived data is true only for the size it was computed at, and an edit that flags nothing still changes it.
//!
//! The loader flags each of those fields on the instance, so that syncing a main component leaves them alone. Without
//! the flag a sync would reset a childless instance to its main's size and look like an edit.

use designos_doc::{Node, OverrideField};
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

/// The fields an instance's own record entry can state, and that a save can restate from the node.
pub(crate) const SELF_FIELDS: [OverrideField; 12] = [
    OverrideField::Name,
    OverrideField::Visible,
    OverrideField::Locked,
    OverrideField::LockAspect,
    OverrideField::Opacity,
    OverrideField::Blend,
    OverrideField::Size,
    OverrideField::Transform,
    OverrideField::Fills,
    OverrideField::Stroke,
    OverrideField::Effects,
    OverrideField::CornerRadii,
];

/// A digest of what `node` holds for `field`, covering every value a save writes for that field.
fn digest(node: &Node, field: OverrideField) -> u64 {
    let mut h = DefaultHasher::new();
    let bound = |prefix: &str| -> Vec<(&String, &String)> { node.variables.bindings.iter().filter(|(k, _)| k.starts_with(prefix)).collect() };
    let text = match field {
        OverrideField::Name => format!("{:?}", node.name),
        OverrideField::Visible => format!("{:?}", node.visible),
        OverrideField::Locked => format!("{:?}", node.locked),
        OverrideField::LockAspect => format!("{:?}", node.lock_aspect),
        OverrideField::Opacity => format!("{:?}", node.opacity),
        OverrideField::Blend => format!("{:?}", node.blend),
        OverrideField::Size => format!("{:?}", node.size),
        OverrideField::Transform => format!("{:?}", node.transform),
        // The style link goes with the paints it supplies: linking to another style, or detaching, can leave the paints
        // themselves as they were.
        OverrideField::Fills => format!("{:?}{:?}{:?}", node.fills, bound("fills/"), node.style_refs.get("fill")),
        OverrideField::Stroke => format!("{:?}{:?}{:?}{:?}", node.stroke, node.stroke_geometry, bound("strokes/"), node.style_refs.get("stroke")),
        OverrideField::Effects => format!("{:?}{:?}{:?}", node.effects, bound("effects/"), node.style_refs.get("effect")),
        OverrideField::CornerRadii => format!("{:?}{:?}", node.corner_radii, node.corner_smoothing),
        _ => String::new(),
    };
    text.hash(&mut h);
    h.finish()
}

/// The digest of `node`'s size, which is taken whether or not the instance overrides it.
pub(crate) fn size_digest(node: &Node) -> u64 {
    digest(node, OverrideField::Size)
}

/// Whether `node` has another size than it had when the file was opened, whether or not the instance overrides its
/// size. An edit that sets no override flag (the agent tools write the node's fields directly) is found this way.
pub(crate) fn size_moved(node: &Node, opened: &BTreeMap<OverrideField, u64>) -> bool {
    opened.get(&OverrideField::Size).is_some_and(|d| *d != size_digest(node))
}

/// The digest of each field of `node` that the instance overrides and a save can restate.
pub(crate) fn digests(node: &Node) -> BTreeMap<OverrideField, u64> {
    SELF_FIELDS.iter().copied().filter(|f| node.component.overrides.contains(f)).map(|f| (f, digest(node, f))).collect()
}

/// The fields of `node` that differ from `opened`, the digests taken when the file was opened: a field overridden now
/// that was not then counts as differing.
pub(crate) fn edited_fields(node: &Node, opened: &BTreeMap<OverrideField, u64>) -> Vec<OverrideField> {
    digests(node).into_iter().filter(|(f, d)| opened.get(f) != Some(d)).map(|(f, _)| f).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, NodeId, NodeKind, Paint, Vec2};

    fn node() -> Node {
        let mut n = Node::new(NodeId(1), NodeKind::Rectangle);
        n.component.overrides.extend([OverrideField::Size, OverrideField::Fills]);
        n
    }

    #[test]
    fn an_unchanged_node_has_no_edited_field() {
        let n = node();
        assert!(edited_fields(&n, &digests(&n)).is_empty());
    }

    #[test]
    fn only_the_changed_field_is_reported() {
        let n = node();
        let opened = digests(&n);
        let mut edited = n.clone();
        edited.size = Vec2::new(200.0, 60.0);
        assert_eq!(edited_fields(&edited, &opened), vec![OverrideField::Size]);
        let mut refilled = n.clone();
        refilled.fills = vec![Paint::solid(Color::rgba(255, 0, 0, 255))];
        assert_eq!(edited_fields(&refilled, &opened), vec![OverrideField::Fills]);
    }

    #[test]
    fn a_style_link_change_is_an_edit_of_the_field_it_supplies_even_when_the_values_stay() {
        let mut n = node();
        n.component.overrides.extend([OverrideField::Stroke, OverrideField::Effects]);
        for (slot, field) in [("fill", OverrideField::Fills), ("stroke", OverrideField::Stroke), ("effect", OverrideField::Effects)] {
            n.style_refs.insert(slot.into(), "style-one".into());
            let opened = digests(&n);
            assert!(edited_fields(&n, &opened).is_empty(), "{slot}: nothing changed");
            let mut relinked = n.clone();
            relinked.style_refs.insert(slot.into(), "style-two".into());
            assert_eq!(edited_fields(&relinked, &opened), vec![field], "{slot}: re-linked");
            let mut detached = n.clone();
            detached.style_refs.remove(slot);
            assert_eq!(edited_fields(&detached, &opened), vec![field], "{slot}: detached");
        }
    }

    #[test]
    fn a_field_flagged_after_the_file_was_opened_counts_as_edited() {
        let mut n = node();
        let opened = digests(&n);
        n.component.overrides.insert(OverrideField::Opacity);
        assert_eq!(edited_fields(&n, &opened), vec![OverrideField::Opacity]);
    }

    #[test]
    fn a_size_change_is_found_without_a_flag() {
        let mut n = Node::new(NodeId(1), NodeKind::Rectangle);
        let opened = BTreeMap::from([(OverrideField::Size, size_digest(&n))]);
        assert!(!size_moved(&n, &opened));
        n.size = Vec2::new(200.0, 60.0);
        assert!(size_moved(&n, &opened));
        assert!(edited_fields(&n, &opened).is_empty(), "only a flagged field is restated");
        assert!(!size_moved(&n, &BTreeMap::new()), "no size taken at open: nothing to compare with");
    }

    #[test]
    fn a_field_the_node_does_not_override_is_never_reported() {
        let n = node();
        let opened = digests(&n);
        let mut edited = n.clone();
        edited.opacity = 0.5;
        assert!(edited_fields(&edited, &opened).is_empty());
    }
}
