//! Editing a main component syncs its instances on pages the window has not opened, and opening
//! such a page expands what the file left unexpanded. Neither the edit nor an undo of it may leave
//! those instances with more or fewer children than a fresh load of the file has.

use super::undo_exact_layout::{differences, layout_of};
use super::*;
use designos_doc::{AutoLayout, Document, FrameProps, LayoutDirection, Node, NodeKind, Rect as DRect, Sizing, TextStyle};

const INSTANCES: usize = 3;
/// Enough nodes that the importer defers the pages it does not need up front.
const FILLER: usize = 25_000;

/// A `.fig` with a main component on the first page, instances of it on the second, and a big
/// third page, so that the second page stays unexpanded after the file is opened.
fn deferred_fig() -> &'static [u8] {
    static BYTES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    BYTES.get_or_init(|| {
        let mut d = Document::new("deferred");
        let main_page = d.add_page("Main");
        let uses = d.add_page("Uses");
        let filler = d.add_page("Filler");
        let hugging = |direction| AutoLayout { direction, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..AutoLayout::default() };
        let main_id = d.alloc_id();
        let mut main_node =
            Node::new(main_id, NodeKind::Component(FrameProps { auto_layout: Some(hugging(LayoutDirection::Vertical)), ..FrameProps::default() }));
        main_node.name = "Header".into();
        main_node.size = DV::new(120.0, 40.0);
        let main = d.insert(main_page, main_node).unwrap();
        for (name, y) in [("Title", 0.0), ("Subtitle", 20.0)] {
            let id = d.alloc_id();
            let mut text = Node::text(id, DRect::new(0.0, y, 80.0, 14.0), name, TextStyle::default());
            text.name = name.into();
            d.insert(main, text).unwrap();
        }
        let column_id = d.alloc_id();
        let mut column = Node::frame(column_id, DRect::new(0.0, 0.0, 400.0, 400.0));
        column.kind = NodeKind::Frame(FrameProps { auto_layout: Some(hugging(LayoutDirection::Vertical)), ..FrameProps::default() });
        let column = d.insert(uses, column).unwrap();
        for _ in 0..INSTANCES {
            let id = d.alloc_id();
            let kind = NodeKind::Instance {
                frame: FrameProps { auto_layout: Some(hugging(LayoutDirection::Vertical)), ..FrameProps::default() },
                component: Some(main),
                component_key: None,
            };
            let mut instance = Node::new(id, kind);
            instance.name = "Header".into();
            instance.size = DV::new(120.0, 40.0);
            d.insert(column, instance).unwrap();
        }
        for i in 0..FILLER {
            let id = d.alloc_id();
            d.insert(filler, Node::rectangle(id, DRect::new((i % 100) as f32 * 12.0, (i / 100) as f32 * 12.0, 10.0, 10.0))).unwrap();
        }
        designos_fig::export_fig(&d).expect("export the deferred fixture")
    })
}

/// The node count and the children of every instance, in id order: what a load shows.
fn shape_of(doc: &Document) -> (usize, Vec<usize>) {
    let instances = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.children.len()).collect();
    (doc.nodes.len(), instances)
}

/// What a fresh load of the file has once every page is expanded.
fn fresh_shape() -> (usize, Vec<usize>) {
    let mut fresh = designos_fig::import_fig(deferred_fig()).expect("import the deferred fixture");
    fresh.hydrate_all_pages();
    shape_of(&fresh)
}

struct TmpFig(std::path::PathBuf);

impl Drop for TmpFig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The fixture on disk, in a directory of its own that goes with the guard.
fn fixture_file() -> (std::path::PathBuf, TmpFig) {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir =
        std::env::temp_dir().join(format!("designos_deferred_page_instances_{}_{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("deferred.fig");
    std::fs::write(&path, deferred_fig()).unwrap();
    (path, TmpFig(dir))
}

/// The window with the fixture open and a text session on the main component's "Title".
fn open_with_title_session() -> (Sim, TmpFig) {
    let (path, guard) = fixture_file();
    let sim = Sim::with(Options { file: Some(path), edit_text: Some("Title".into()), select_text: Some((5, 5)), ..Default::default() });
    (sim, guard)
}

fn uses_page(s: &Sim) -> NodeId {
    let page = s.app().doc.pages.iter().copied().find(|p| s.app().doc.get(*p).is_some_and(|n| n.name == "Uses")).expect("the Uses page");
    page
}

fn hydrated(s: &Sim, page: NodeId) -> bool {
    s.app().doc.page_hydrator.as_ref().is_none_or(|h| h.lock().unwrap().is_hydrated(page))
}

fn type_x_and_commit(s: &mut Sim) {
    s.type_text("x");
    s.key(Key::Escape);
}

#[test]
fn a_main_component_text_edit_then_opening_the_instances_page_adds_no_children() {
    let (mut s, _tmp) = open_with_title_session();
    let uses = uses_page(&s);
    let (fresh_nodes, fresh_instances) = fresh_shape();
    assert!(!hydrated(&s, uses), "precondition: the importer deferred the page of the instances");
    assert_eq!(shape_of(&s.app().doc).1, vec![0; INSTANCES], "precondition: the instances on the unopened page have no children yet");
    assert_eq!(fresh_instances, vec![2; INSTANCES], "precondition: a fresh load gives each instance the main's two children");

    type_x_and_commit(&mut s);
    assert_eq!(s.app().hist.undo_depth(), 1);
    assert_eq!(shape_of(&s.app().doc).1, vec![2; INSTANCES], "precondition: the edit synced the unopened instances, so they hold the main's children");
    s.shell.app.switch_page(uses);
    assert!(hydrated(&s, uses), "the page was expanded when it was opened");
    assert_eq!(shape_of(&s.app().doc), (fresh_nodes, fresh_instances), "edit then open the page: the instances hold what a fresh load holds");
}

#[test]
fn a_main_component_text_edit_undo_then_opening_the_instances_page_adds_no_children() {
    let (mut s, _tmp) = open_with_title_session();
    let uses = uses_page(&s);
    let (fresh_nodes, fresh_instances) = fresh_shape();
    let before = layout_of(&s);

    type_x_and_commit(&mut s);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().hist.undo_depth(), 0);
    assert_eq!(differences(&s, &before), Vec::<String>::new(), "the undo moved nothing on the unopened page or anywhere else");
    assert_eq!(shape_of(&s.app().doc).1, vec![0; INSTANCES], "precondition: the undo took the synced children away again");
    s.shell.app.switch_page(uses);
    assert!(hydrated(&s, uses), "the page was expanded when it was opened");
    assert_eq!(shape_of(&s.app().doc), (fresh_nodes, fresh_instances), "edit, undo, then open the page: the instances hold what a fresh load holds");
}

/// `--select` shows the page it found the target on, and a page on screen holds what a fresh load of it holds: the
/// instances on it have their main's children, as they do when the page is opened from the Pages panel.
fn assert_the_selected_page_is_shown_expanded(s: &mut Sim, uses: NodeId) {
    for _ in 0..3 {
        s.frame(vec![]);
    }
    assert!(s.shell.select_miss.is_none(), "{:?}", s.shell.select_miss);
    assert_eq!(s.app().page, uses, "the window shows the page the selector named or sits on");
    assert!(hydrated(s, uses), "the page on screen is expanded");
    assert_eq!(shape_of(&s.app().doc).1, vec![2; INSTANCES], "the instances on screen hold their main's children");
}

#[test]
fn select_by_id_of_an_instance_on_a_deferred_page_shows_that_page_expanded() {
    let fresh = designos_fig::import_fig(deferred_fig()).expect("import the deferred fixture");
    let instance = fresh.nodes.values().find(|n| matches!(n.kind, NodeKind::Instance { .. })).expect("an instance").id;
    let (path, _tmp) = fixture_file();
    let mut s = Sim::with(Options { file: Some(path), select: Some(format!("id:{}", instance.0)), ..Default::default() });
    let uses = uses_page(&s);
    assert_eq!(s.app().selection, vec![instance], "the instance is selected");
    assert_the_selected_page_is_shown_expanded(&mut s, uses);
}

#[test]
fn select_by_page_name_of_a_deferred_page_shows_that_page_expanded() {
    let (path, _tmp) = fixture_file();
    let mut s = Sim::with(Options { file: Some(path), select: Some("Uses".into()), ..Default::default() });
    let uses = uses_page(&s);
    assert!(s.app().selection.is_empty(), "a page name selects no layer");
    assert_the_selected_page_is_shown_expanded(&mut s, uses);
}
