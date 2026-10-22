//! Undo and redo of an editor edit give back exactly the geometry before and after it, on the page
//! in view and on pages the window is not showing. A main component edited on one page syncs its
//! instances on another page whose children are not loaded yet; undoing the edit used to solve auto
//! layout again outside the undo step, which collapsed those instances and moved the frames around
//! them.

use super::*;
use designos_doc::{AutoLayout, Document, FrameProps, LayoutDirection, Node, NodeKind, PageHydrator, Rect as DRect, Sizing, TextStyle};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// What the window reports of an unopened page: it is not hydrated, and nothing here hydrates it.
#[derive(Debug)]
struct UnopenedPage(NodeId);

impl PageHydrator for UnopenedPage {
    fn hydrate_page(&mut self, _doc: &mut Document, _page: NodeId) -> bool {
        false
    }
    fn hydrate_all(&mut self, _doc: &mut Document) -> bool {
        false
    }
    fn is_hydrated(&self, page: NodeId) -> bool {
        page != self.0
    }
}

/// Size and position of every node: what a canvas draws.
pub(super) type Layout = BTreeMap<NodeId, (DV, f32, f32)>;

pub(super) fn layout_of(s: &Sim) -> Layout {
    s.app().doc.nodes.values().map(|n| (n.id, (n.size, n.transform.tx, n.transform.ty))).collect()
}

/// Every node that is added, gone, resized or moved between `want` and the layout now, by name.
pub(super) fn differences(s: &Sim, want: &Layout) -> Vec<String> {
    let now = layout_of(s);
    let name = |id: &NodeId| s.app().doc.get(*id).map(|n| n.name.clone()).unwrap_or_else(|| format!("{id:?}"));
    let mut out = Vec::new();
    for (id, (size, x, y)) in want {
        match now.get(id) {
            Some((s2, x2, y2)) if (s2, x2, y2) == (size, x, y) => {}
            Some((s2, x2, y2)) => out.push(format!("{} {id:?}: {}x{} at ({x},{y}) became {}x{} at ({x2},{y2})", name(id), size.x, size.y, s2.x, s2.y)),
            None => out.push(format!("{id:?} is gone")),
        }
    }
    out.extend(now.keys().filter(|id| !want.contains_key(id)).map(|id| format!("{} {id:?} is new", name(id))));
    out
}

fn hugging(direction: LayoutDirection, counter: Sizing) -> AutoLayout {
    AutoLayout { direction, primary_sizing: Sizing::Hug, counter_sizing: counter, ..AutoLayout::default() }
}

struct Scene {
    sim: Sim,
    title: NodeId,
    instance: NodeId,
    after_page: NodeId,
}

/// A main component "Header" with a title text on the page in view, and on a second page, which
/// the window has not opened, a column holding an instance of it and a sibling below. As in a
/// `.fig` file, the instance has no children yet, the stored sizes are the file's (375x1332 for
/// the instance, a main sized 120x30 around a 80x14 title), and none of them is what the solver
/// would write.
fn scene() -> Scene {
    let mut sim = Sim::new();
    let page = sim.app().page;
    let doc = &mut sim.shell.app.doc;
    let main_id = doc.alloc_id();
    let mut main_node =
        Node::new(main_id, NodeKind::Component(FrameProps { auto_layout: Some(hugging(LayoutDirection::Vertical, Sizing::Hug)), ..FrameProps::default() }));
    main_node.size = DV::new(120.0, 30.0);
    main_node.transform.tx = 9000.0;
    let main = doc.insert(page, main_node).unwrap();
    let title_id = doc.alloc_id();
    let title = doc.insert(main, Node::text(title_id, DRect::new(0.0, 0.0, 80.0, 14.0), "Title", TextStyle::default())).unwrap();

    let after_page = doc.add_page("About");
    let column_id = doc.alloc_id();
    let mut column_node = Node::frame(column_id, DRect::new(0.0, 0.0, 375.0, 1432.0));
    column_node.kind = NodeKind::Frame(FrameProps { auto_layout: Some(hugging(LayoutDirection::Vertical, Sizing::Fixed)), ..FrameProps::default() });
    let column = doc.insert(after_page, column_node).unwrap();
    let instance_id = doc.alloc_id();
    let mut instance_node = Node::new(
        instance_id,
        NodeKind::Instance {
            frame: FrameProps { auto_layout: Some(hugging(LayoutDirection::Vertical, Sizing::Fixed)), ..FrameProps::default() },
            component: Some(main),
            component_key: None,
        },
    );
    instance_node.size = DV::new(375.0, 1332.0);
    let instance = doc.insert(column, instance_node).unwrap();
    let sibling_id = doc.alloc_id();
    doc.insert(column, Node::frame(sibling_id, DRect::new(0.0, 1332.0, 375.0, 100.0))).unwrap();
    doc.page_hydrator = Some(Arc::new(Mutex::new(UnopenedPage(after_page))));
    sim.frame(vec![]);
    Scene { sim, title, instance, after_page }
}

fn redo(s: &mut Sim) {
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
}

/// Opens a text session on `title` with the caret at the end, types "x" and commits with Esc.
fn type_x_and_commit(sim: &mut Sim, title: NodeId) {
    canvas::interact::begin_text_edit(&mut sim.shell.app, title);
    let session = sim.shell.app.canvas.text.as_mut().expect("a text session");
    session.anchor = session.caret;
    sim.type_text("x");
    sim.key(Key::Escape);
}

fn content(sim: &Sim, id: NodeId) -> String {
    sim.app().doc.get(id).and_then(|n| n.kind.any_text()).map(|t| t.content.clone()).unwrap()
}

#[test]
fn an_agent_edit_step_is_not_recorded_so_its_undo_keeps_solving_layout() {
    let (mut s, mut session, workspace) = super::live_agent_edits::live_sim();
    let hero = s.by_name("Hero");
    session.call("designos_update_node", serde_json::json!({ "node_id": hero.0, "name": "Hero (agent)" })).expect("agent rename");
    for _ in 0..10 {
        if s.app().doc.get(hero).unwrap().name == "Hero (agent)" {
            break;
        }
        s.frame(vec![]);
    }
    assert_eq!(s.app().doc.get(hero).unwrap().name, "Hero (agent)", "precondition: the agent edit landed");
    assert!(!s.app().hist.peek_undo_layout_recorded(), "the agent's layout is solved after its step, outside it");

    s.shell.app.edit_nodes(&[hero], |n| n.opacity = 0.5);
    assert!(s.app().hist.peek_undo_layout_recorded(), "an editor edit on top of it records its own layout");
    s.cmd_key(Key::Z);
    assert!(!s.app().hist.peek_undo_layout_recorded(), "the agent step is back on top and still not recorded");
    super::live_agent_edits::finish(s, workspace);
}

#[test]
fn undo_and_redo_of_a_main_component_text_edit_move_nothing_on_any_page() {
    let Scene { mut sim, title, instance, after_page } = scene();
    assert!(sim.app().doc.get(instance).unwrap().children.is_empty(), "precondition: the instance has no children yet");
    assert_eq!(sim.app().doc.page_of(instance), Some(after_page));
    let before = layout_of(&sim);

    type_x_and_commit(&mut sim, title);
    assert_eq!(content(&sim, title), "Titlex");
    assert_eq!(sim.app().hist.undo_depth(), 1, "the edit is one undo step");
    assert_eq!(sim.app().doc.get(instance).unwrap().children.len(), 1, "precondition: the edit synced the instance on the other page");
    let after = layout_of(&sim);
    assert_ne!(after, before, "precondition: the edit changed the layout");

    sim.cmd_key(Key::Z);
    assert_eq!(content(&sim, title), "Title");
    assert_eq!(differences(&sim, &before), Vec::<String>::new(), "undo leaves every node on every page exactly where it was before the edit");

    redo(&mut sim);
    assert_eq!(content(&sim, title), "Titlex");
    assert_eq!(differences(&sim, &after), Vec::<String>::new(), "redo gives back exactly the layout the edit made");

    sim.cmd_key(Key::Z);
    assert_eq!(differences(&sim, &before), Vec::<String>::new(), "a second undo is as exact as the first");
}
