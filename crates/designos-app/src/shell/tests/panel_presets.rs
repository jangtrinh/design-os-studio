use super::*;
use crate::app::RightTab;
use crate::canvas::tools::Tool;
use crate::cli::{self, Options};
use designos_doc::{Action, Comment, CommentThread, Document, Easing, Node, NodeId, Reaction, Rect as DRect, Transition, TransitionKind, Trigger, Vec2 as DV};

fn build_shell_with_doc(doc: &Document, opts: Options) -> Shell {
    Sim::with_starter_prep(opts, |d| *d = doc.clone(), |_| {}).shell
}

fn add_test_frame(doc: &mut Document, parent: NodeId, id: NodeId, name: &str, x: f32) -> NodeId {
    let mut node = Node::frame(id, DRect::new(x, 0.0, 120.0, 80.0));
    node.name = name.into();
    doc.insert(parent, node).expect("insert frame")
}

fn nav_reaction(dest: NodeId) -> Reaction {
    Reaction {
        trigger: Trigger::OnClick,
        actions: vec![Action::Navigate { dest, preserve_scroll: false }],
        transition: Some(Transition { kind: TransitionKind::Dissolve, easing: Easing::EaseOut, duration_ms: 200 }),
    }
}

fn make_thread(doc: &Document, id: &str, page: NodeId, node: NodeId, resolved: bool) -> CommentThread {
    let anchor = crate::comments::model::make_anchor(doc, page, DV::new(24.0, 24.0), Some(node));
    CommentThread {
        id: id.into(),
        anchor,
        messages: vec![Comment {
            id: format!("{id}-0"),
            author: "Kir".into(),
            author_id: None,
            author_color: None,
            text: format!("Thread {id}"),
            created: "2026-10-27T00:00:00Z".into(),
            edited: None,
        }],
        resolved,
    }
}

#[test]
fn interaction_panel_opens_the_selected_nodes_first_reaction() {
    let mut doc = Document::new("PresetTest");
    let page = doc.add_page("Page 1");
    let first_node = add_test_frame(&mut doc, page, NodeId(10), "FirstCard", 0.0);
    let selected_node = add_test_frame(&mut doc, page, NodeId(20), "SelectedCard", 160.0);
    let dest_node = add_test_frame(&mut doc, page, NodeId(30), "DestCard", 320.0);

    doc.get_mut(first_node).unwrap().prototype.reactions.push(nav_reaction(dest_node));
    doc.get_mut(selected_node).unwrap().prototype.reactions.push(nav_reaction(dest_node));

    let opts = Options { select: Some(format!("id:{}", selected_node.0)), panel: Some("interaction".into()), ..Default::default() };
    let shell = build_shell_with_doc(&doc, opts);

    assert_eq!(shell.app.ui.right_tab, Some(RightTab::Prototype));
    assert_eq!(shell.app.selection, vec![selected_node]);
    assert_eq!(shell.app.proto.editing, Some((selected_node, 0)));
}

#[test]
fn interaction_panel_without_a_reacting_selection_picks_the_first_reaction_in_paint_order() {
    let mut doc = Document::new("PresetPaintOrderTest");
    let page = doc.add_page("Page 1");
    let inert_selected = add_test_frame(&mut doc, page, NodeId(10), "Inert", 0.0);
    // Create two reacting nodes in reverse ID order vs paint order:
    // `high_id_painted_first` is inserted first in `page.children` (paint order) with NodeId(200),
    // while `low_id_painted_second` is inserted second in `page.children` with NodeId(100).
    let high_id_painted_first = add_test_frame(&mut doc, page, NodeId(200), "HighIdFirst", 140.0);
    let low_id_painted_second = add_test_frame(&mut doc, page, NodeId(100), "LowIdSecond", 280.0);
    assert!(low_id_painted_second.0 < high_id_painted_first.0);

    doc.get_mut(low_id_painted_second).unwrap().prototype.reactions.push(nav_reaction(inert_selected));
    doc.get_mut(high_id_painted_first).unwrap().prototype.reactions.push(nav_reaction(inert_selected));

    let opts = Options { select: Some(format!("id:{}", inert_selected.0)), panel: Some("interaction".into()), ..Default::default() };
    let shell = build_shell_with_doc(&doc, opts);

    assert_eq!(shell.app.ui.right_tab, Some(RightTab::Prototype));
    assert_eq!(shell.app.selection, vec![high_id_painted_first]);
    assert_eq!(shell.app.proto.editing, Some((high_id_painted_first, 0)));
}

#[test]
fn interaction_panel_with_no_reaction_opens_the_prototype_tab_only() {
    let mut doc = Document::new("PresetNoReactionTest");
    let page = doc.add_page("Page 1");
    // Include a node named "Add" without reactions to ensure the legacy name lookup is gone.
    let _legacy_add = add_test_frame(&mut doc, page, NodeId(10), "Add", 0.0);

    let opts = Options { panel: Some("interaction".into()), ..Default::default() };
    let shell = build_shell_with_doc(&doc, opts);

    assert_eq!(shell.app.ui.right_tab, Some(RightTab::Prototype));
    assert_eq!(shell.app.proto.editing, None);
}

#[test]
fn comments_panel_opens_the_first_unresolved_thread_in_paint_order() {
    let mut doc = Document::new("CommentsPresetPaintOrderTest");
    let page = doc.add_page("Page 1");
    let high_id_painted_first = add_test_frame(&mut doc, page, NodeId(200), "FirstInPaint", 0.0);
    let low_id_painted_second = add_test_frame(&mut doc, page, NodeId(100), "SecondInPaint", 140.0);
    assert!(low_id_painted_second.0 < high_id_painted_first.0);

    let resolved_first = make_thread(&doc, "c0", page, high_id_painted_first, true);
    let later_node_low_thread = make_thread(&doc, "c1", page, low_id_painted_second, false);
    let first_node_higher_thread = make_thread(&doc, "c9", page, high_id_painted_first, false);
    let first_node_lower_thread = make_thread(&doc, "c2", page, high_id_painted_first, false);

    doc.comments = vec![resolved_first, later_node_low_thread, first_node_higher_thread, first_node_lower_thread];

    let opts = Options { panel: Some("comments".into()), ..Default::default() };
    let shell = build_shell_with_doc(&doc, opts);

    assert_eq!(shell.app.tool, Tool::Comment);
    assert_eq!(shell.app.comments.open.as_deref(), Some("c2"));
}

#[test]
fn comments_panel_with_no_thread_opens_comment_mode_only() {
    let mut doc = Document::new("CommentsPresetEmptyTest");
    let page = doc.add_page("Page 1");
    let node = add_test_frame(&mut doc, page, NodeId(10), "Card", 0.0);
    let resolved = make_thread(&doc, "c1", page, node, true);
    doc.comments = vec![resolved];

    let opts = Options { panel: Some("comments".into()), ..Default::default() };
    let shell = build_shell_with_doc(&doc, opts);

    assert_eq!(shell.app.tool, Tool::Comment);
    assert_eq!(shell.app.comments.open, None);
}

#[test]
fn cli_accepts_panel_comments() {
    let opts = cli::parse(["--panel", "comments"].into_iter().map(String::from)).expect("--panel comments must parse");
    assert_eq!(opts.panel.as_deref(), Some("comments"));
    assert!(cli::PANELS.contains(&"comments"));
    assert!(cli::USAGE.contains("comments"));
}
