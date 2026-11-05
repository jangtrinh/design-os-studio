//! Connectable-layer eligibility and home-container resolution for connectors.

use crate::{Document, NodeId, NodeKind};

fn strict_ancestors(doc: &Document, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut cur = doc.get(id).and_then(|n| n.parent);
    let limit = doc.nodes.len();
    while let Some(pid) = cur {
        if out.len() >= limit {
            break;
        }
        out.push(pid);
        cur = doc.get(pid).and_then(|n| n.parent);
    }
    out
}

fn is_inside_component_or_instance(doc: &Document, id: NodeId) -> bool {
    let mut cur = Some(id);
    let mut steps = 0usize;
    let limit = doc.nodes.len();
    while let Some(cid) = cur {
        if steps >= limit {
            return true;
        }
        steps += 1;
        let Some(node) = doc.get(cid) else {
            break;
        };
        if matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }) {
            return true;
        }
        cur = node.parent;
    }
    false
}

/// Whether `id` is a valid attached endpoint for a connector.
///
/// A layer is connectable when it exists in `doc`, is visible and unlocked, has a kind other than
/// `Page`, `Connector`, or `Unknown`, and has no `Instance`, `Component`, or `ComponentSet`
/// strict ancestor (a top-level `Instance`, `Component`, or `ComponentSet` is itself connectable).
pub fn connectable_node(doc: &Document, id: NodeId) -> bool {
    let Some(node) = doc.get(id) else {
        return false;
    };
    if !node.visible || node.locked {
        return false;
    }
    if matches!(node.kind, NodeKind::Page { .. } | NodeKind::Connector(_) | NodeKind::Unknown { .. }) {
        return false;
    }
    for anc in strict_ancestors(doc, id) {
        let Some(parent) = doc.get(anc) else {
            continue;
        };
        if matches!(parent.kind, NodeKind::Instance { .. } | NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
            return false;
        }
    }
    true
}

/// Resolves the container that should own a connector joining `a` and `b` on `page`.
///
/// Starts at the nearest common ancestor strictly above both attached ends (or the single
/// attached end's parent, or `page` when neither end is attached), climbs past `Group` and
/// `BooleanOp` containers, climbs out of any `Component`, `ComponentSet`, or `Instance` subtree,
/// and stops at the first `Frame`, `Section`, or `Page` (falling back to `page` when the ends
/// have no common container on the same page).
pub fn connector_home(doc: &Document, a: Option<NodeId>, b: Option<NodeId>, page: NodeId) -> NodeId {
    let a = a.filter(|&id| doc.get(id).is_some());
    let b = b.filter(|&id| doc.get(id).is_some());
    let start = match (a, b) {
        (Some(a_id), Some(b_id)) => {
            let anc_a = strict_ancestors(doc, a_id);
            let anc_b = strict_ancestors(doc, b_id);
            anc_a.into_iter().find(|id| anc_b.contains(id))
        }
        (Some(id), None) | (None, Some(id)) => doc.get(id).and_then(|n| n.parent),
        (None, None) => None,
    };
    let mut cur = start;
    let mut steps = 0usize;
    let limit = doc.nodes.len();
    while let Some(cid) = cur {
        if steps >= limit {
            break;
        }
        steps += 1;
        let Some(node) = doc.get(cid) else {
            break;
        };
        if matches!(node.kind, NodeKind::Frame(_) | NodeKind::Section | NodeKind::Page { .. }) && !is_inside_component_or_instance(doc, cid) {
            return cid;
        }
        cur = node.parent;
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BooleanOp, ConnectorProps, FrameProps, Node, NodeKind, Rect};

    fn add_child(doc: &mut Document, parent: NodeId, node: Node) -> NodeId {
        let id = node.id;
        doc.insert(parent, node).expect("insert child");
        id
    }

    fn make_frame(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 200.0));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_rect(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 60.0, 40.0));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_group(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Group);
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_boolean_op(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Union));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_section(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Section);
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_component(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Component(FrameProps::default()));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_component_set(doc: &mut Document, parent: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::ComponentSet(FrameProps::default()));
        n.name = name.into();
        add_child(doc, parent, n)
    }

    fn make_instance(doc: &mut Document, parent: NodeId, component: NodeId, name: &str) -> NodeId {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Instance { frame: FrameProps::default(), component: Some(component), component_key: None });
        n.name = name.into();
        add_child(doc, parent, n)
    }

    #[test]
    fn connectable_node_rejects_missing_id_and_accepts_standard_shapes() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let rect = make_rect(&mut doc, page, "Box");
        let frame = make_frame(&mut doc, page, "Card");

        assert!(!connectable_node(&doc, NodeId(999_999)), "missing node must not be connectable");
        assert!(connectable_node(&doc, rect), "visible unlocked rectangle on page is connectable");
        assert!(connectable_node(&doc, frame), "visible unlocked frame on page is connectable");
    }

    #[test]
    fn connectable_node_rejects_page_connector_and_unknown_kinds() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let rect = make_rect(&mut doc, page, "Box");

        let conn_id = doc.alloc_id();
        let conn = add_child(&mut doc, page, Node::connector(conn_id, ConnectorProps::default()));

        let unk_id = doc.alloc_id();
        let unk = add_child(&mut doc, page, Node::new(unk_id, NodeKind::Unknown { type_name: "CUSTOM".into() }));

        assert!(connectable_node(&doc, rect), "rectangle baseline must be connectable");
        assert!(!connectable_node(&doc, page), "Page is not connectable");
        assert!(!connectable_node(&doc, conn), "Connector is not connectable");
        assert!(!connectable_node(&doc, unk), "Unknown kind is not connectable");
    }

    #[test]
    fn connectable_node_allows_top_level_component_set_component_and_instance_but_rejects_their_descendants() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");

        let comp = make_component(&mut doc, page, "Button");
        let inside_comp = make_rect(&mut doc, comp, "Bg");

        let comp_set = make_component_set(&mut doc, page, "ButtonSet");
        let variant_comp = make_component(&mut doc, comp_set, "Primary");
        let inside_variant = make_rect(&mut doc, variant_comp, "Label");

        let inst = make_instance(&mut doc, page, comp, "ButtonInst");
        let inside_inst = make_rect(&mut doc, inst, "InstChild");

        assert!(connectable_node(&doc, comp), "top-level Component is connectable");
        assert!(connectable_node(&doc, comp_set), "top-level ComponentSet is connectable");
        assert!(connectable_node(&doc, inst), "top-level Instance is connectable");

        assert!(!connectable_node(&doc, inside_comp), "descendant of Component is not connectable");
        assert!(!connectable_node(&doc, variant_comp), "Component inside ComponentSet has ComponentSet ancestor and is not connectable");
        assert!(!connectable_node(&doc, inside_variant), "descendant inside ComponentSet is not connectable");
        assert!(!connectable_node(&doc, inside_inst), "descendant of Instance is not connectable");
    }

    #[test]
    fn connectable_node_rejects_hidden_or_locked_nodes() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let visible_unlocked = make_rect(&mut doc, page, "Open");
        let hidden = make_rect(&mut doc, page, "Hidden");
        doc.get_mut(hidden).unwrap().visible = false;

        let locked = make_rect(&mut doc, page, "Locked");
        doc.get_mut(locked).unwrap().locked = true;

        assert!(connectable_node(&doc, visible_unlocked), "visible unlocked node must be connectable");
        assert!(!connectable_node(&doc, hidden), "hidden node is not connectable");
        assert!(!connectable_node(&doc, locked), "locked node is not connectable");
    }

    #[test]
    fn connector_home_picks_nearest_common_frame_or_section_or_page() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let section = make_section(&mut doc, page, "FlowSection");
        let outer_frame = make_frame(&mut doc, section, "Board");
        let a = make_rect(&mut doc, outer_frame, "A");
        let b = make_rect(&mut doc, outer_frame, "B");
        let in_section = make_rect(&mut doc, section, "C");
        let on_page = make_rect(&mut doc, page, "D");

        assert_eq!(connector_home(&doc, Some(a), Some(b), page), outer_frame);
        assert_eq!(connector_home(&doc, Some(a), Some(in_section), page), section);
        assert_eq!(connector_home(&doc, Some(a), Some(on_page), page), page);
    }

    #[test]
    fn connector_home_is_strictly_above_both_ends_so_an_end_is_never_its_own_home() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let outer = make_frame(&mut doc, page, "Outer");
        let inner = make_frame(&mut doc, outer, "Inner");
        let child = make_rect(&mut doc, inner, "Child");

        // `inner` is an ancestor of `child`; the connector cannot live inside `inner`.
        assert_eq!(connector_home(&doc, Some(inner), Some(child), page), outer);
        assert_eq!(connector_home(&doc, Some(child), Some(inner), page), outer);
        // `outer` lives directly on `page`, so connecting `outer` to `child` homes on `page`.
        assert_eq!(connector_home(&doc, Some(outer), Some(child), page), page);
    }

    #[test]
    fn connector_home_climbs_past_group_and_boolean_op() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let frame = make_frame(&mut doc, page, "HostFrame");
        let group = make_group(&mut doc, frame, "G");
        let bool_op = make_boolean_op(&mut doc, group, "Union");
        let a = make_rect(&mut doc, group, "A");
        let b = make_rect(&mut doc, group, "B");
        let c = make_rect(&mut doc, bool_op, "C");
        let d = make_rect(&mut doc, bool_op, "D");

        assert_eq!(connector_home(&doc, Some(a), Some(b), page), frame, "climbs past Group to enclosing Frame");
        assert_eq!(connector_home(&doc, Some(c), Some(d), page), frame, "climbs past BooleanOp and Group to enclosing Frame");
    }

    #[test]
    fn connector_home_climbs_out_of_component_component_set_and_instance() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let frame = make_frame(&mut doc, page, "CanvasFrame");

        let comp = make_component(&mut doc, frame, "Comp");
        let comp_inner_frame = make_frame(&mut doc, comp, "CompInner");
        let ca = make_rect(&mut doc, comp_inner_frame, "CA");
        let cb = make_rect(&mut doc, comp_inner_frame, "CB");

        let comp_set = make_component_set(&mut doc, frame, "Set");
        let v1 = make_component(&mut doc, comp_set, "V1");
        let v2 = make_component(&mut doc, comp_set, "V2");

        let inst = make_instance(&mut doc, frame, comp, "Inst");
        let inst_inner = make_frame(&mut doc, inst, "InstInner");
        let ia = make_rect(&mut doc, inst_inner, "IA");
        let ib = make_rect(&mut doc, inst_inner, "IB");

        assert_eq!(connector_home(&doc, Some(ca), Some(cb), page), frame, "climbs out of Component");
        assert_eq!(connector_home(&doc, Some(v1), Some(v2), page), frame, "climbs out of ComponentSet");
        assert_eq!(connector_home(&doc, Some(ia), Some(ib), page), frame, "climbs out of Instance");
    }

    #[test]
    fn connector_home_handles_single_attached_end_both_free_and_cross_page_fallback() {
        let mut doc = Document::new("Test");
        let page1 = doc.add_page("Page 1");
        let frame = make_frame(&mut doc, page1, "Frame1");
        let group = make_group(&mut doc, frame, "Group1");
        let a = make_rect(&mut doc, group, "A");

        let page2 = doc.add_page("Page 2");
        let b_other_page = make_rect(&mut doc, page2, "B");

        // One attached end uses its parent (and climbs past Group to Frame1).
        assert_eq!(connector_home(&doc, Some(a), None, page1), frame);
        assert_eq!(connector_home(&doc, None, Some(a), page1), frame);
        // Both free ends use the given `page`.
        assert_eq!(connector_home(&doc, None, None, page1), page1);
        assert_eq!(connector_home(&doc, None, None, page2), page2);
        // Ends on different pages have no common ancestor and fall back to `page`.
        assert_eq!(connector_home(&doc, Some(a), Some(b_other_page), page1), page1);
    }
}
