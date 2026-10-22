//! Tests for compacting node ids that leave no allocation headroom.

use crate::{
    Action, Color, CommentAnchor, CommentThread, ComponentPropDef, ConnectorEnd, ConnectorProps, Document, ExprValue, Expression, ExternalAsset, Flow, Guide,
    GuideAxis, LibraryAssetKind, Magnet, Node, NodeId, NodeKind, PropKind, PropValue, Reaction, Rect, SlotPropConfig, Trigger, Vec2, MIN_ID_HEADROOM,
};
use std::collections::BTreeSet;

const MAX: u64 = u64::MAX;
const LAST_SAFE: u64 = u64::MAX - MIN_ID_HEADROOM;

fn rect(id: u64) -> Node {
    Node::rectangle(NodeId(id), Rect::new(0.0, 0.0, 1.0, 1.0))
}

/// A document whose every NodeId-bearing field points at an id above the allocation limit.
/// Returns it with the ids that must disappear.
fn document_with_every_reference_on_high_ids() -> (Document, Vec<u64>) {
    let mut d = Document::new("Exhausted");
    let page = d.add_page("P");
    let (big, kid, comp, lib_page) = (MAX, MAX - 1, MAX - 2, MAX - 3);

    d.insert(page, Node::frame(NodeId(big), Rect::new(0.0, 0.0, 8.0, 8.0))).unwrap();
    d.insert(NodeId(big), rect(kid)).unwrap();
    d.nodes.insert(NodeId(lib_page), Node::new(NodeId(lib_page), NodeKind::Page { background: Color::WHITE }));
    d.library_page = Some(NodeId(lib_page));
    d.insert(NodeId(lib_page), Node::frame(NodeId(comp), Rect::new(0.0, 0.0, 4.0, 4.0))).unwrap();

    let inst = d.alloc_id();
    let mut instance = Node::new(inst, NodeKind::Instance { frame: Default::default(), component: Some(NodeId(comp)), component_key: None });
    instance.name = "instance".into();
    instance.component.source = Some(NodeId(kid));
    instance.component.props.push(ComponentPropDef {
        name: "swap".into(),
        kind: PropKind::InstanceSwap { preferred: vec![NodeId(comp)] },
        default: PropValue::Instance(NodeId(big)),
    });
    instance.component.values.insert("slot".into(), PropValue::Slot(vec![NodeId(kid)]));
    instance.component.props.push(ComponentPropDef { name: "body".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    instance.component.slot_configs.insert("body".into(), SlotPropConfig { preferred: vec![NodeId(comp)], ..Default::default() });
    instance.prototype.fixed_children = vec![NodeId(kid)];
    instance.prototype.reactions.push(Reaction {
        trigger: Trigger::OnClick,
        actions: vec![
            Action::Navigate { dest: NodeId(big), preserve_scroll: false },
            Action::Conditional {
                branches: vec![(Expression::Literal(ExprValue::Boolean(true)), vec![Action::ScrollTo { dest: NodeId(kid), offset: Vec2::ZERO }])],
                otherwise: vec![Action::ChangeTo { dest: NodeId(comp) }],
            },
        ],
        transition: None,
    });
    d.insert(page, instance).unwrap();

    let line = d.alloc_id();
    let props = ConnectorProps {
        start: ConnectorEnd::attached(NodeId(big), Magnet::Auto),
        end: ConnectorEnd::attached(NodeId(kid), Magnet::Auto),
        ..Default::default()
    };
    d.insert(page, Node::connector(line, props)).unwrap();

    d.flows.push(Flow { name: "flow".into(), page, start: NodeId(big), description: String::new() });
    d.guides.insert(page, vec![Guide { axis: GuideAxis::X, offset: 1.0, frame: Some(NodeId(big)) }]);
    d.guides.insert(NodeId(big), vec![Guide { axis: GuideAxis::Y, offset: 2.0, frame: None }]);
    d.comments.push(CommentThread {
        id: "t".into(),
        anchor: CommentAnchor { page, node: Some(NodeId(kid)), offset: Vec2::ZERO },
        messages: vec![],
        resolved: false,
    });
    d.library
        .external
        .insert("k".into(), ExternalAsset { library_id: "l".into(), version: 1, kind: LibraryAssetKind::Component, node: Some(NodeId(comp)), local_key: None });
    (d, vec![big, kid, comp, lib_page])
}

fn find(d: &Document, name: &str) -> NodeId {
    *d.nodes.iter().find(|(_, n)| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).0
}

#[test]
fn renumbering_rewrites_every_reference_and_leaves_no_old_id() {
    let (d, old_ids) = document_with_every_reference_on_high_ids();
    let json = d.to_json().unwrap();
    for id in &old_ids {
        assert!(json.contains(&id.to_string()), "fixture must really reference {id}");
    }
    let sane: BTreeSet<NodeId> = d.nodes.keys().copied().filter(|id| id.0 <= LAST_SAFE).collect();

    let (mut loaded, report) = Document::from_json_with_report(&json).unwrap();
    assert!(report.id_space_exhausted);
    assert!(loaded.nodes.contains_key(&NodeId(MAX)), "load-time repair only reports; it must not renumber (the MCP refuses on the report)");

    assert_eq!(loaded.renumber_ids_for_headroom(), old_ids.len());
    let renumbered = loaded.to_json().unwrap();
    for id in &old_ids {
        assert!(!renumbered.contains(&id.to_string()), "old id {id} still referenced");
    }
    assert!(sane.iter().all(|id| loaded.nodes.contains_key(id)), "ids that were fine keep their value");

    // Structure survives: a second repair finds nothing to fix and headroom is back.
    let again = loaded.validate_and_repair();
    assert!(again.is_clean(), "{again:?}");
    assert_eq!(loaded.renumber_ids_for_headroom(), 0);

    // Semantics survive: the references land on the nodes they used to name.
    let page = loaded.pages[0];
    let instance = find(&loaded, "instance");
    let big = loaded.nodes[&page].children[0];
    let kid = loaded.nodes[&big].children[0];
    let lib_page = loaded.library_page.unwrap();
    let comp = loaded.nodes[&lib_page].children[0];
    assert_eq!(loaded.nodes[&kid].parent, Some(big));
    let inst = &loaded.nodes[&instance];
    assert!(matches!(&inst.kind, NodeKind::Instance { component: Some(c), .. } if *c == comp));
    assert_eq!(inst.component.source, Some(kid));
    assert_eq!(inst.component.props[0].default, PropValue::Instance(big));
    assert_eq!(inst.component.props[0].kind, PropKind::InstanceSwap { preferred: vec![comp] });
    assert_eq!(inst.component.values["slot"], PropValue::Slot(vec![kid]));
    assert_eq!(inst.component.slot_configs["body"].preferred, vec![comp]);
    assert_eq!(inst.prototype.fixed_children, vec![kid]);
    let actions = &inst.prototype.reactions[0].actions;
    assert!(matches!(&actions[0], Action::Navigate { dest, .. } if *dest == big));
    match &actions[1] {
        Action::Conditional { branches, otherwise } => {
            assert!(matches!(&branches[0].1[0], Action::ScrollTo { dest, .. } if *dest == kid));
            assert!(matches!(&otherwise[0], Action::ChangeTo { dest } if *dest == comp));
        }
        other => panic!("expected the conditional, got {other:?}"),
    }
    let connector = loaded.nodes[&page].children.iter().find_map(|c| match &loaded.nodes[c].kind {
        NodeKind::Connector(p) => Some(p.clone()),
        _ => None,
    });
    let connector = connector.expect("connector survives");
    assert_eq!((connector.start.node, connector.end.node), (Some(big), Some(kid)));
    assert_eq!((loaded.flows[0].page, loaded.flows[0].start), (page, big));
    assert_eq!(loaded.guides[&page][0].frame, Some(big));
    assert!(loaded.guides.contains_key(&big), "guides keyed by a renumbered page follow it");
    assert_eq!((loaded.comments[0].anchor.page, loaded.comments[0].anchor.node), (page, Some(kid)));
    assert_eq!(loaded.library.external["k"].node, Some(comp));
}

#[test]
fn three_allocations_after_renumbering_are_distinct_and_the_tree_stays_valid() {
    let (d, _) = document_with_every_reference_on_high_ids();
    let (mut doc, report) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
    assert!(report.id_space_exhausted);
    doc.renumber_ids_for_headroom();
    let page = doc.pages[0];
    let before = doc.nodes.len();
    let largest = doc.nodes.keys().next_back().unwrap().0;

    let ids = [doc.alloc_id(), doc.alloc_id(), doc.alloc_id()];
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), 3, "{ids:?}");
    assert!(ids.iter().all(|id| id.0 > largest && !doc.nodes.contains_key(id)), "{ids:?} must sit above every existing id");
    for id in ids {
        doc.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    }
    assert_eq!(doc.nodes.len(), before + 3);
    let children = &doc.nodes[&page].children;
    assert_eq!(children.iter().collect::<BTreeSet<_>>().len(), children.len(), "no child is listed twice");
    assert!(doc.validate_and_repair().is_clean());
}

#[test]
fn fallback_allocation_never_repeats_an_id_before_insertion() {
    let mut d = Document::new("Max");
    let page = d.add_page("P");
    d.insert(page, rect(MAX)).unwrap();
    let mut v = serde_json::to_value(&d).unwrap();
    v["next_id"] = serde_json::json!(MAX);
    // Deliberately no renumbering: this is the last-resort path other loaders still reach.
    let (mut back, _) = Document::from_json_with_report(&v.to_string()).unwrap();

    let ids = [back.alloc_id(), back.alloc_id(), back.alloc_id()];
    assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), 3, "batch allocation must hand out distinct ids: {ids:?}");
    assert!(ids.iter().all(|id| !back.nodes.contains_key(id) && id.0 > 0));
    for id in ids {
        back.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();
    }
    let next = back.alloc_id();
    assert!(!back.nodes.contains_key(&next) && !ids.contains(&next));
}

#[test]
fn ids_that_do_not_fit_below_the_limit_are_compacted_from_one() {
    let mut d = Document::new("Packed");
    let page = d.add_page("P");
    d.insert(page, rect(LAST_SAFE)).unwrap();
    d.insert(page, rect(MAX)).unwrap();

    assert_eq!(d.renumber_ids_for_headroom(), 2, "page keeps 1; the other two move");
    assert_eq!(d.nodes.keys().map(|id| id.0).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(d.nodes[&page].children, vec![NodeId(2), NodeId(3)], "order is kept");
    assert!(d.validate_and_repair().is_clean());
    assert_eq!(d.alloc_id(), NodeId(4));
}

#[test]
fn an_exhausted_counter_over_sane_ids_is_reset_without_touching_ids() {
    let mut d = Document::new("Counter");
    let page = d.add_page("P");
    d.insert(page, rect(10)).unwrap();
    let mut v = serde_json::to_value(&d).unwrap();
    v["next_id"] = serde_json::json!(MAX);
    let (mut back, report) = Document::from_json_with_report(&v.to_string()).unwrap();
    assert!(!report.id_space_exhausted, "no node id is too high");

    assert_eq!(back.renumber_ids_for_headroom(), 0);
    assert_eq!(back.alloc_id(), NodeId(11));
}

#[test]
fn renumber_ids_remaps_ignore_keys() {
    let mut d = Document::new("IgnoreRemap");
    let page = d.add_page("P");
    let high = d.insert(page, rect(MAX)).unwrap();
    d.check_designs.ignored.insert(crate::IgnoreKey { node: high, rule: "RECOMMEND_VARIABLES".into(), property: "fills/0".into() });
    assert_eq!(d.renumber_ids_for_headroom(), 1);
    let remapped = d.nodes[&page].children[0];
    assert_ne!(remapped, high);
    assert!(d.check_designs.ignored.contains(&crate::IgnoreKey { node: remapped, rule: "RECOMMEND_VARIABLES".into(), property: "fills/0".into() }));
}
