use crate::branch_diff::{diff3, Conflict, ConflictKind, Diff3, EntityChange, EntityConflict, EntityKey, Group, NodeChange};
use crate::{
    Affine, Color, Comment, CommentAnchor, CommentThread, DevStatus, Document, Effect, ExportConstraint, ExportFileFormat, ExportPreset, Flow, FrameProps,
    Node, NodeId, NodeKind, Overflow, Paint, Rect, SlideProps, Stroke, StrokeAlign, StrokeCap, StrokeJoin, Style, Variable, VariableCollection, VariableKind,
    VariableMode, VariableValue, Vec2,
};
use std::collections::{BTreeMap, BTreeSet};

fn base_fixture_with_frame_and_children() -> (Document, NodeId, NodeId, NodeId, NodeId, NodeId, NodeId) {
    let mut doc = Document::new("Base");
    let page = doc.add_page("Page 1");
    let frame_f = doc.alloc_id();
    let frame_g = doc.alloc_id();
    let a = doc.alloc_id();
    let b = doc.alloc_id();
    let c = doc.alloc_id();

    let mut f_node = Node::frame(frame_f, Rect::new(0.0, 0.0, 400.0, 300.0));
    f_node.name = "F".into();
    doc.insert(page, f_node).unwrap();

    let mut g_node = Node::frame(frame_g, Rect::new(500.0, 0.0, 400.0, 300.0));
    g_node.name = "G".into();
    doc.insert(page, g_node).unwrap();

    for (id, name, x) in [(a, "a", 10.0), (b, "b", 60.0), (c, "c", 110.0)] {
        let mut rect = Node::rectangle(id, Rect::new(x, 10.0, 40.0, 40.0));
        rect.name = name.into();
        doc.insert(frame_f, rect).unwrap();
    }

    (doc, page, frame_f, frame_g, a, b, c)
}

#[test]
fn classifies_add_remove_move_change() {
    let (base, _page, frame_f, frame_g, a, b, c) = base_fixture_with_frame_and_children();
    let mut ours = base.clone();
    let mut theirs = base.clone();

    // Ours adds `d` under F and removes `b`.
    let d = ours.alloc_id();
    let mut d_node = Node::rectangle(d, Rect::new(160.0, 10.0, 40.0, 40.0));
    d_node.name = "d".into();
    ours.insert(frame_f, d_node).unwrap();
    ours.remove(b).unwrap();

    // Theirs moves `c` to `G` and recolours `a`.
    theirs.reparent(c, frame_g, 0).unwrap();
    theirs.get_mut(a).unwrap().fills = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];

    let diff = diff3(&base, &ours, &theirs);

    assert_eq!(diff.ours.nodes.get(&d), Some(&NodeChange::Added));
    assert_eq!(diff.ours.nodes.get(&b), Some(&NodeChange::Removed));
    assert_eq!(diff.ours.nodes.len(), 2, "only d and b changed on ours: {:?}", diff.ours.nodes);

    assert_eq!(diff.theirs.nodes.get(&c), Some(&NodeChange::Moved { from: (frame_f, 2), to: (frame_g, 0) }));
    let mut expected_a_groups = BTreeSet::new();
    expected_a_groups.insert(Group::Fills);
    assert_eq!(diff.theirs.nodes.get(&a), Some(&NodeChange::Changed { groups: expected_a_groups }));
    assert_eq!(diff.theirs.nodes.len(), 2, "only c and a changed on theirs: {:?}", diff.theirs.nodes);

    assert!(diff.conflicts.is_empty(), "expected no conflicts, got {:?}", diff.conflicts);
    assert!(diff.id_collisions.is_empty());
}

#[test]
fn conflict_only_same_property() {
    let (base, _page, _frame_f, _frame_g, a, _b, _c) = base_fixture_with_frame_and_children();

    // Case 1: both recolour `a` differently -> conflict {a, [fills]}
    let mut ours = base.clone();
    let mut theirs = base.clone();
    ours.get_mut(a).unwrap().fills = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];
    theirs.get_mut(a).unwrap().fills = vec![Paint::solid(Color::rgb(0x00, 0x00, 0xFF))];

    let diff = diff3(&base, &ours, &theirs);
    let mut expected_groups = BTreeSet::new();
    expected_groups.insert(Group::Fills);
    assert_eq!(diff.conflicts, vec![Conflict { node: a, groups: expected_groups, kind: ConflictKind::ModifyModify }]);

    // Case 2: ours recolours `a`, theirs renames `a` -> no conflict
    let mut ours2 = base.clone();
    let mut theirs2 = base.clone();
    ours2.get_mut(a).unwrap().fills = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];
    theirs2.get_mut(a).unwrap().name = "a-renamed".into();

    let diff2 = diff3(&base, &ours2, &theirs2);
    assert!(diff2.conflicts.is_empty(), "different groups must not conflict: {:?}", diff2.conflicts);
}

#[test]
fn same_change_no_conflict() {
    let (base, _page, _frame_f, _frame_g, a, _b, _c) = base_fixture_with_frame_and_children();
    let mut ours = base.clone();
    let mut theirs = base.clone();

    ours.get_mut(a).unwrap().opacity = 0.5;
    theirs.get_mut(a).unwrap().opacity = 0.5;

    let diff = diff3(&base, &ours, &theirs);
    assert!(diff.conflicts.is_empty(), "equal changes on both sides must not conflict");
    assert!(diff.ours.nodes.contains_key(&a));
    assert!(diff.theirs.nodes.contains_key(&a));
}

#[test]
fn delete_vs_modify_conflict() {
    let (base, _page, _frame_f, _frame_g, a, _b, _c) = base_fixture_with_frame_and_children();
    let mut ours = base.clone();
    let mut theirs = base.clone();

    ours.remove(a).unwrap();
    theirs.get_mut(a).unwrap().name = "modified-on-theirs".into();

    let diff = diff3(&base, &ours, &theirs);
    assert_eq!(diff.conflicts.len(), 1);
    assert_eq!(diff.conflicts[0].node, a);
    assert_eq!(diff.conflicts[0].kind, ConflictKind::DeleteModify);
    assert!(diff.conflicts[0].groups.contains(&Group::Name));

    // Deleting a container on one side while the other side adds a child under it is also DeleteModify.
    let mut ours_del_f = base.clone();
    let mut theirs_add_child = base.clone();
    let frame_f = base.get(a).unwrap().parent.unwrap();
    ours_del_f.remove(frame_f).unwrap();
    let new_child = theirs_add_child.alloc_id();
    theirs_add_child.insert(frame_f, Node::rectangle(new_child, Rect::new(0.0, 0.0, 20.0, 20.0))).unwrap();
    let diff_container = diff3(&base, &ours_del_f, &theirs_add_child);
    assert!(
        diff_container.conflicts.iter().any(|c| c.node == frame_f && c.kind == ConflictKind::DeleteModify),
        "removing a container while the other side adds a child under it must report DeleteModify on the container: {:?}",
        diff_container.conflicts
    );
}

#[test]
fn id_collision_detected() {
    let (base, _page, frame_f, frame_g, _a, _b, _c) = base_fixture_with_frame_and_children();
    let mut ours = base.clone();
    let mut theirs = base.clone();

    let collision_id = NodeId(900);
    let mut n_ours = Node::rectangle(collision_id, Rect::new(0.0, 0.0, 50.0, 50.0));
    n_ours.name = "ours-900".into();
    ours.insert(frame_f, n_ours).unwrap();

    let mut n_theirs = Node::ellipse(collision_id, Rect::new(20.0, 20.0, 80.0, 80.0));
    n_theirs.name = "theirs-900".into();
    theirs.insert(frame_g, n_theirs).unwrap();

    let diff = diff3(&base, &ours, &theirs);
    assert_eq!(diff.id_collisions, vec![NodeId(900)]);
    assert!(diff.conflicts.is_empty(), "id collisions are resolved by remap in T10.3, not shown as user conflicts: {:?}", diff.conflicts);
}

#[test]
fn lcs_avoids_false_moves() {
    let (base, _page, frame_f, _frame_g, a, b, c) = base_fixture_with_frame_and_children();
    let mut ours = base.clone();
    let theirs = base.clone();

    // Insert one new sibling at index 0 of frame_f on `ours`.
    let inserted = ours.alloc_id();
    ours.insert(frame_f, Node::rectangle(inserted, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    ours.reparent(inserted, frame_f, 0).unwrap();
    assert_eq!(ours.get(frame_f).unwrap().children, vec![inserted, a, b, c]);

    let diff = diff3(&base, &ours, &theirs);
    assert_eq!(diff.ours.nodes.get(&inserted), Some(&NodeChange::Added));
    assert!(
        !diff.ours.nodes.contains_key(&a) && !diff.ours.nodes.contains_key(&b) && !diff.ours.nodes.contains_key(&c),
        "inserting a sibling at index 0 must not mark existing siblings as moved: {:?}",
        diff.ours.nodes
    );
}

#[test]
fn edge_cases_tolerance_kind_reorder_instance_and_variables() {
    let (mut base, page, frame_f, _frame_g, a, b, c) = base_fixture_with_frame_and_children();

    // Setup a variable with two modes ("light", "dark") and an instance with a mirror node.
    base.variable_collections.insert(
        "col-1".into(),
        VariableCollection {
            id: "col-1".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["var-1".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let mut values = BTreeMap::new();
    values.insert("light".into(), VariableValue::Number(8.0));
    values.insert("dark".into(), VariableValue::Number(8.0));
    base.variables.insert(
        "var-1".into(),
        Variable {
            id: "var-1".into(),
            name: "Spacing/Sm".into(),
            collection: "col-1".into(),
            kind: VariableKind::Number,
            values,
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: BTreeMap::new(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );

    // Add an instance with a mirror child to verify §8.3 instance conflict grouping.
    let inst_id = base.alloc_id();
    let mirror_id = base.alloc_id();
    let inst_node = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: None, component_key: None });
    base.insert(page, inst_node).unwrap();
    base.insert(inst_id, Node::rectangle(mirror_id, Rect::new(0.0, 0.0, 30.0, 30.0))).unwrap();

    let mut ours = base.clone();
    let mut theirs = base.clone();

    // Edge case 1 (§8.1): frame_f -> Component on ours, fills change on frame_f on theirs -> no conflict on frame_f.
    let fp = ours.get(frame_f).unwrap().kind.frame_props().unwrap().clone();
    ours.get_mut(frame_f).unwrap().kind = NodeKind::Component(fp);
    theirs.get_mut(frame_f).unwrap().fills = vec![Paint::solid(Color::rgb(0x11, 0x22, 0x33))];

    // Edge case 4 (§8.4): reordering children `[a, b, c]` differently on both sides -> position conflict.
    ours.get_mut(frame_f).unwrap().children = vec![c, a, b];
    theirs.get_mut(frame_f).unwrap().children = vec![b, c, a];

    // Edge case 5 (§8.5): float noise < 1e-3 on `a.transform` and `a.size` -> treated as equal.
    ours.get_mut(a).unwrap().transform = Affine::translate(10.0004, 10.0);
    ours.get_mut(a).unwrap().size = Vec2::new(40.0003, 40.0);

    // Edge case 6 (§8.6): variable changed in different modes on each side -> no conflict.
    ours.variables.get_mut("var-1").unwrap().values.insert("light".into(), VariableValue::Number(12.0));
    theirs.variables.get_mut("var-1").unwrap().values.insert("dark".into(), VariableValue::Number(16.0));

    // Edge case 3 (§8.3): mirror node inside `inst_id` conflicts on both sides -> grouped under `inst_id` in stats.
    ours.get_mut(mirror_id).unwrap().opacity = 0.2;
    theirs.get_mut(mirror_id).unwrap().opacity = 0.8;

    let diff = diff3(&base, &ours, &theirs);

    // 1. frame_f has Kind on ours and Fills on theirs -> no conflict on frame_f.
    assert!(!diff.conflicts.iter().any(|cf| cf.node == frame_f), "Kind vs Fills on frame_f must not conflict: {:?}", diff.conflicts);
    // 2. Float drift < 1e-3 on `a` was ignored.
    assert_eq!(diff.ours.nodes.get(&a), None, "float noise < 1e-3 must not mark `a` as changed");
    // 3. Reordering children differently produced a `Position` conflict.
    assert!(
        diff.conflicts.iter().any(|cf| cf.groups.contains(&Group::Position)),
        "conflicting sibling reorder must report Group::Position: {:?}",
        diff.conflicts
    );
    // 4. Variable changed in different modes has no entity conflict.
    assert_eq!(diff.ours.variables.get("var-1"), Some(&EntityChange::Changed));
    assert_eq!(diff.theirs.variables.get("var-1"), Some(&EntityChange::Changed));
    assert!(
        !diff.entity_conflicts.iter().any(|ec| ec.key == EntityKey::Variable("var-1".into())),
        "variable changes in distinct modes must not conflict: {:?}",
        diff.entity_conflicts
    );
    // 5. Mirror conflict is grouped under `inst_id` in `stats.instance_conflicts`.
    assert_eq!(diff.stats.instance_conflicts.get(&inst_id), Some(&vec![mirror_id]));

    // Also verify non-node entity conflicts when the same style/flow/comment is edited differently.
    ours.styles.insert("S:primary".into(), Style::Fill(vec![Paint::solid(Color::rgb(1, 2, 3))]));
    theirs.styles.insert("S:primary".into(), Style::Fill(vec![Paint::solid(Color::rgb(4, 5, 6))]));
    ours.flows.push(Flow { name: "Main".into(), page, start: frame_f, description: "A".into() });
    theirs.flows.push(Flow { name: "Branch".into(), page, start: frame_f, description: "B".into() });
    ours.comments.push(CommentThread {
        id: "t1".into(),
        anchor: CommentAnchor { page, node: Some(frame_f), offset: Vec2::ZERO },
        messages: vec![Comment {
            id: "m1".into(),
            author: "Alice".into(),
            author_id: None,
            author_color: None,
            text: "Ours".into(),
            created: "2026-10-19T00:00:00Z".into(),
            edited: None,
        }],
        resolved: false,
    });
    theirs.comments.push(CommentThread {
        id: "t1".into(),
        anchor: CommentAnchor { page, node: Some(frame_f), offset: Vec2::ZERO },
        messages: vec![Comment {
            id: "m1".into(),
            author: "Alice".into(),
            author_id: None,
            author_color: None,
            text: "Theirs".into(),
            created: "2026-10-19T00:00:00Z".into(),
            edited: None,
        }],
        resolved: false,
    });
    let diff_entities = diff3(&base, &ours, &theirs);
    assert!(diff_entities.entity_conflicts.contains(&EntityConflict { key: EntityKey::Style("S:primary".into()), kind: ConflictKind::ModifyModify }));
    assert!(diff_entities
        .entity_conflicts
        .contains(&EntityConflict { key: EntityKey::Flow(format!("{}:{}", page.0, frame_f.0)), kind: ConflictKind::ModifyModify }));
    assert!(diff_entities.entity_conflicts.contains(&EntityConflict { key: EntityKey::Comment("t1".into()), kind: ConflictKind::ModifyModify }));
}

/// Applies `edit` to the stroke of `id` in `doc`.
fn edit_stroke(doc: &mut Document, id: NodeId, edit: impl FnOnce(&mut Stroke)) {
    edit(&mut doc.get_mut(id).unwrap().stroke);
}

/// A default rectangle has an empty paint list and weight 1, so none of the stroke edits below touch paints
/// or weight: the equal-hash short-circuit must still see them.
#[test]
fn unpainted_stroke_edits_are_changes_in_every_field() {
    let (base, _page, _f, _g, a, _b, _c) = base_fixture_with_frame_and_children();
    type StrokeEdit = fn(&mut Stroke);
    let edits: [(&str, StrokeEdit); 5] = [
        ("align", |s| s.align = StrokeAlign::Outside),
        ("cap", |s| s.cap = StrokeCap::Round),
        ("join", |s| s.join = StrokeJoin::Round),
        ("miter_limit", |s| s.miter_limit = 8.0),
        ("dashes", |s| s.dashes = vec![4.0, 2.0]),
    ];
    for (field, edit) in edits {
        let mut ours = base.clone();
        edit_stroke(&mut ours, a, edit);
        let diff = diff3(&base, &ours, &base);
        let expected = NodeChange::Changed { groups: BTreeSet::from([Group::Stroke]) };
        assert_eq!(diff.ours.nodes.get(&a), Some(&expected), "stroke.{field}-only edit was not reported: {:?}", diff.ours.nodes);
        assert_eq!(diff.ours.nodes.len(), 1, "stroke.{field}: only the edited node may change");
        assert!(diff.theirs.nodes.is_empty());
    }
}

#[test]
fn unpainted_stroke_align_conflict_when_sides_differ() {
    let (base, _page, _f, _g, a, _b, _c) = base_fixture_with_frame_and_children();
    let mut ours = base.clone();
    let mut theirs = base.clone();
    edit_stroke(&mut ours, a, |s| s.align = StrokeAlign::Outside);
    edit_stroke(&mut theirs, a, |s| s.align = StrokeAlign::Center);

    let diff = diff3(&base, &ours, &theirs);
    assert_eq!(diff.conflicts, vec![Conflict { node: a, groups: BTreeSet::from([Group::Stroke]), kind: ConflictKind::ModifyModify }]);

    // Both sides making the same edit is not a conflict.
    let mut same = base.clone();
    edit_stroke(&mut same, a, |s| s.align = StrokeAlign::Outside);
    assert!(diff3(&base, &ours, &same).conflicts.is_empty());
}

/// Every property group whose value the node hash skips while it is at its default must still be seen once it
/// leaves the default: a single edit from the default state has to surface as `Changed` for that group.
#[test]
fn edits_leaving_the_default_state_of_each_hash_guarded_group_are_changes() {
    type Edit = fn(&mut Node);
    let edits: Vec<(Group, &str, Edit)> = vec![
        (Group::Fills, "fills", |n| n.fills = vec![Paint::solid(Color::rgb(1, 2, 3))]),
        (Group::Effects, "effects", |n| {
            n.effects = vec![Effect::DropShadow {
                color: Color::rgb(0, 0, 0),
                offset: Vec2::new(0.0, 2.0),
                radius: 4.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
            }]
        }),
        (Group::StyleRefs, "style_refs", |n| {
            n.style_refs.insert("fill".into(), "S:primary".into());
        }),
        (Group::Vector, "fill_geometry", |n| n.fill_geometry = Some(Vec::new())),
        (Group::Component, "component.layout_cleared", |n| n.component.layout_cleared = true),
        (Group::Component, "component.description", |n| n.component.description = "d".into()),
        (Group::Prototype, "prototype.overflow", |n| n.prototype.overflow = Overflow::Both),
        (Group::Variables, "variables.modes", |n| {
            n.variables.modes.insert("c".into(), "m".into());
        }),
        (Group::ExportSettings, "export_settings", |n| {
            n.export_settings = vec![ExportPreset { format: ExportFileFormat::Png, constraint: ExportConstraint::Scale(2.0), suffix: "@2x".into() }]
        }),
        (Group::Dev, "dev.status", |n| n.dev.status = Some(DevStatus::ReadyForDev)),
        (Group::PluginData, "plugin_data", |n| {
            n.plugin_data.insert("k".into(), "v".into());
        }),
        (Group::Other, "hyperlink", |n| n.hyperlink = Some("https://example.com".into())),
        (Group::Other, "fig_metadata", |n| {
            n.fig_metadata.insert("k".into(), "v".into());
        }),
        (Group::Other, "source_id", |n| n.source_id = Some("1:2".into())),
        (Group::Other, "detached_symbol_id", |n| n.detached_symbol_id = Some("1:2".into())),
        (Group::Other, "slide", |n| n.slide = Some(SlideProps { notes: "n".into() })),
    ];
    let (base, _page, _f, _g, a, _b, _c) = base_fixture_with_frame_and_children();
    for (group, what, edit) in edits {
        let mut ours = base.clone();
        edit(ours.get_mut(a).unwrap());
        let diff = diff3(&base, &ours, &base);
        let expected = NodeChange::Changed { groups: BTreeSet::from([group]) };
        assert_eq!(diff.ours.nodes.get(&a), Some(&expected), "{what}: edit from the default state was hidden by the hash short-circuit");
    }
}

/// Synthetic large fixture (T10.2 §10 & §11): `total_scene_nodes` rectangles spread over 100 frames, with 1%
/// deterministic edits per side (every 100th leaf on `ours`, offset by 50 on `theirs`).
/// Returns `(base, ours, theirs, edited_ours, edited_theirs)`.
fn large_fixture(total_scene_nodes: usize) -> (Document, Document, Document, usize, usize) {
    const FRAMES: usize = 100;

    let mut base = Document::new("Synthetic large");
    let page = base.add_page("Page 1");

    let mut frame_ids = Vec::with_capacity(FRAMES);
    for f_idx in 0..FRAMES {
        let fid = NodeId((f_idx + 10) as u64);
        let mut frame = Node::frame(fid, Rect::new((f_idx as f32) * 500.0, 0.0, 480.0, 1000.0));
        frame.parent = Some(page);
        frame_ids.push(fid);
        base.nodes.insert(fid, frame);
    }
    base.nodes.get_mut(&page).unwrap().children = frame_ids.clone();

    let remaining = total_scene_nodes - FRAMES;
    let mut frame_children: Vec<Vec<NodeId>> = (0..FRAMES).map(|_| Vec::with_capacity(remaining / FRAMES + 2)).collect();
    let first_leaf_id = 1_000u64;
    for i in 0..remaining {
        let nid = NodeId(first_leaf_id + i as u64);
        let f_idx = i % FRAMES;
        let fid = frame_ids[f_idx];
        let mut rect = Node::rectangle(nid, Rect::new(0.0, 0.0, 24.0, 24.0));
        rect.parent = Some(fid);
        frame_children[f_idx].push(nid);
        base.nodes.insert(nid, rect);
    }
    for (f_idx, fid) in frame_ids.iter().enumerate() {
        base.nodes.get_mut(fid).unwrap().children = std::mem::take(&mut frame_children[f_idx]);
    }
    // 1 page + `total_scene_nodes` scene nodes.
    assert_eq!(base.nodes.len(), total_scene_nodes + 1);

    let mut ours = base.clone();
    let mut theirs = base.clone();
    let mut edited_ours = 0usize;
    let mut edited_theirs = 0usize;
    for i in (0..remaining).step_by(100) {
        let nid = NodeId(first_leaf_id + i as u64);
        ours.nodes.get_mut(&nid).unwrap().opacity = 0.75;
        edited_ours += 1;
    }
    for i in (50..remaining).step_by(100) {
        let nid = NodeId(first_leaf_id + i as u64);
        theirs.nodes.get_mut(&nid).unwrap().visible = false;
        edited_theirs += 1;
    }
    (base, ours, theirs, edited_ours, edited_theirs)
}

/// Runs `diff3` on a background thread, never the caller's, as the UI does.
fn diff3_off_thread(base: Document, ours: Document, theirs: Document) -> Diff3 {
    std::thread::spawn(move || diff3(&base, &ours, &theirs)).join().expect("background diff3 worker panicked")
}

#[test]
fn large_fixture_within_budget() {
    const TOTAL_SCENE_NODES: usize = 112_474;
    let (base, ours, theirs, edited_ours, edited_theirs) = large_fixture(TOTAL_SCENE_NODES);
    let diff = diff3_off_thread(base, ours, theirs);

    assert_eq!(diff.ours.nodes.len(), edited_ours);
    assert_eq!(diff.theirs.nodes.len(), edited_theirs);
    assert!(diff.conflicts.is_empty());
    eprintln!("large_fixture diff3 elapsed_ms={} scanned={}", diff.stats.elapsed_ms, diff.stats.scanned);
    // The 1.5 s spec budget binds optimized builds and is enforced there (`cargo test --release`). Unoptimized
    // debug builds run an order of magnitude slower, so only they get the 15x scale used by the other perf tests
    // in this repo (`designos-gpu/tests/large_file_perf.rs`); `diff3_scales_linearly_with_node_count` guards the
    // algorithmic cost there. A `CI` environment variable never relaxes the budget.
    let budget_scale = if cfg!(debug_assertions) { 15 } else { 1 };
    let budget_ms = 1_500 * budget_scale;
    assert!(diff.stats.elapsed_ms <= budget_ms, "112,474-node diff3 took {} ms (budget <= {budget_ms} ms)", diff.stats.elapsed_ms);
}

/// Diff cost must grow linearly with the node count: 4x the nodes may cost about 4x the time, never the
/// 16x of a quadratic pass. Machine speed cancels out of the ratio.
#[test]
fn diff3_scales_linearly_with_node_count() {
    let best_ms = |total: usize| {
        let (base, ours, theirs, _, _) = large_fixture(total);
        (0..3).map(|_| diff3(&base, &ours, &theirs).stats.elapsed_ms.max(1) as f64).fold(f64::MAX, f64::min)
    };
    let small = best_ms(28_000);
    let large = best_ms(112_000);
    let ratio = large / small;
    eprintln!("diff3_scaling small_ms={small} large_ms={large} ratio={ratio:.2}");
    assert!(ratio < 9.0, "4x nodes cost {ratio:.1}x diff time ({small} ms -> {large} ms); linear is ~4x, quadratic ~16x");
}
