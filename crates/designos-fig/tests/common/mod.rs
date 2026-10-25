//! Documents shared by the `.fig` export tests.
#![allow(dead_code)]

use designos_doc::*;

pub const STUDIO_PLUGIN_ID: &str = "designos-studio";

pub fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

pub fn component_with_padded_children(doc: &mut Document, page: NodeId) -> NodeId {
    let card_id = doc.alloc_id();
    let layout = |padding: [f32; 4], direction: LayoutDirection| FrameProps {
        auto_layout: Some(AutoLayout { direction, item_spacing: 4.0, padding, ..AutoLayout::default() }),
        ..FrameProps::default()
    };
    let mut card = Node::new(card_id, NodeKind::Component(layout([10.0, 20.0, 30.0, 40.0], LayoutDirection::Vertical)));
    card.name = "Card".into();
    card.size = Vec2::new(300.0, 200.0);
    doc.insert(page, card).unwrap();

    let inner_id = doc.alloc_id();
    let mut inner = Node::new(inner_id, NodeKind::Frame(layout([1.0, 2.0, 3.0, 4.0], LayoutDirection::Horizontal)));
    inner.name = "Inner".into();
    inner.size = Vec2::new(100.0, 50.0);
    doc.insert(card_id, inner).unwrap();
    card_id
}

/// Auto-layout component with an instance that overrides a child's padding, a grid child, a board with a
/// connector and stickies, an instance carrying a slot content id, and plugin data under foreign ids.
pub fn rich_document() -> Document {
    let mut doc = Document::new("real-tags");
    let page = doc.add_page("Page");

    let card_id = component_with_padded_children(&mut doc, page);
    doc.get_mut(card_id).unwrap().plugin_data.extend([
        ("12345:note".to_string(), "ünï ✓ value".to_string()),
        ("12345:other".to_string(), String::new()),
        ("bare-key".to_string(), "no plugin id".to_string()),
        (format!("{STUDIO_PLUGIN_ID}:custom"), "kept".to_string()),
    ]);

    create_instance(&mut doc, card_id, page, Vec2::new(400.0, 0.0)).apply(&mut doc).expect("create_instance");
    let inst_id = instances_of(&doc, card_id)[0];
    doc.get_mut(inst_id).unwrap().name = "CardInstance".into();
    let inner_in_instance =
        doc.get(inst_id).unwrap().children.iter().copied().find(|c| doc.get(*c).is_some_and(|n| n.name == "Inner")).expect("Inner in instance");
    let inner = doc.get_mut(inner_in_instance).unwrap();
    let NodeKind::Frame(fp) = &mut inner.kind else { panic!("Inner is a frame") };
    fp.auto_layout.as_mut().unwrap().padding = [5.0, 6.0, 7.0, 8.0];
    inner.component.overrides.insert(OverrideField::Layout);

    create_instance(&mut doc, card_id, page, Vec2::new(800.0, 0.0)).apply(&mut doc).expect("create_instance");
    let slot_inst = *instances_of(&doc, card_id).iter().find(|i| **i != inst_id).expect("second instance");
    let slot_inst_node = doc.get_mut(slot_inst).unwrap();
    slot_inst_node.name = "SlotInstance".into();
    slot_inst_node.component.slot_content_id = Some("7:42".into());

    let grid_id = doc.alloc_id();
    let grid_props = FrameProps {
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Grid,
            grid_columns: 3,
            grid_rows: 2,
            padding: [9.0, 8.0, 7.0, 6.0],
            ..AutoLayout::default()
        }),
        ..FrameProps::default()
    };
    let mut grid = Node::new(grid_id, NodeKind::Frame(grid_props));
    grid.name = "Grid".into();
    grid.size = Vec2::new(600.0, 400.0);
    doc.insert(page, grid).unwrap();
    let cell_id = doc.alloc_id();
    let mut cell = Node::new(cell_id, NodeKind::Rectangle);
    cell.name = "Cell".into();
    cell.size = Vec2::new(100.0, 100.0);
    cell.layout_child.grid_column = Some(2);
    cell.layout_child.grid_row = Some(1);
    cell.layout_child.col_span = 2;
    doc.insert(grid_id, cell).unwrap();

    let a = doc.alloc_id();
    let a = doc.insert(page, Node::sticky(a, Rect::new(0.0, 500.0, 200.0, 200.0), "shown")).unwrap();
    let b = doc.alloc_id();
    let b = doc.insert(page, Node::sticky(b, Rect::new(400.0, 500.0, 200.0, 200.0), "hidden")).unwrap();
    for (id, name, show) in [(a, "AuthorShown", true), (b, "AuthorHidden", false)] {
        let n = doc.get_mut(id).unwrap();
        n.name = name.into();
        let NodeKind::Sticky(sp) = &mut n.kind else { panic!("sticky") };
        sp.show_author = show;
    }
    let c = doc.alloc_id();
    let props = ConnectorProps {
        start: ConnectorEnd::attached(a, Magnet::Right),
        end: ConnectorEnd::attached(b, Magnet::Left),
        line: ConnectorLine::Straight,
        start_cap: ConnectorCap::Arrow,
        end_cap: ConnectorCap::Triangle,
        label: None,
    };
    doc.insert(page, Node::connector(c, props)).unwrap();
    sync_connector(&mut doc, c);

    let round = doc.alloc_id();
    let props = ConnectorProps {
        start: ConnectorEnd::attached(b, Magnet::Top),
        end: ConnectorEnd::attached(a, Magnet::Bottom),
        line: ConnectorLine::Curved,
        start_cap: ConnectorCap::Circle,
        end_cap: ConnectorCap::Diamond,
        label: None,
    };
    let mut n = Node::connector(round, props);
    n.name = "RoundCaps".into();
    doc.insert(page, n).unwrap();
    sync_connector(&mut doc, round);
    doc
}

fn grid_frame(doc: &mut Document, parent: NodeId, name: &str, columns: u32, rows: u32, tweak: impl FnOnce(&mut AutoLayout)) -> NodeId {
    let id = doc.alloc_id();
    let mut al = AutoLayout {
        direction: LayoutDirection::Grid,
        grid_columns: columns,
        grid_rows: rows,
        item_spacing: 4.0,
        counter_spacing: 6.0,
        ..AutoLayout::default()
    };
    tweak(&mut al);
    let mut frame = Node::new(id, NodeKind::Frame(FrameProps { auto_layout: Some(al), ..FrameProps::default() }));
    frame.name = name.into();
    frame.size = Vec2::new(600.0, 400.0);
    doc.insert(parent, frame).unwrap();
    id
}

fn grid_cell(doc: &mut Document, parent: NodeId, name: &str, column: Option<u16>, row: Option<u16>) -> NodeId {
    let id = doc.alloc_id();
    let mut cell = Node::new(id, NodeKind::Rectangle);
    cell.name = name.into();
    cell.size = Vec2::new(50.0, 50.0);
    cell.layout_child.grid_column = column;
    cell.layout_child.grid_row = row;
    doc.insert(parent, cell).unwrap();
    id
}

/// Grids of every shape the exporter writes: tracks of each kind, an auto-track that is more than "rows", cells inside
/// and outside the tracks, a grid with more tracks than the exporter writes entries for, and an instance that moves a
/// cell of its component.
pub fn grid_document() -> Document {
    let mut doc = Document::new("grids");
    let page = doc.add_page("Page");

    let main = grid_frame(&mut doc, page, "Main", 3, 2, |al| {
        al.grid_col_tracks[0] = GridTrack::Fixed(120.0);
        al.grid_col_tracks[1] = GridTrack::Flex(2.0);
        al.grid_col_tracks[2] = GridTrack::Hug;
        al.grid_row_tracks[1] = GridTrack::Fixed(70.0);
        al.grid_auto_tracks = GridAutoTracks::Track(GridTrack::Fixed(50.0));
    });
    grid_cell(&mut doc, main, "InTracks", Some(2), Some(1));
    grid_cell(&mut doc, main, "PastTracks", Some(5), Some(4));
    grid_cell(&mut doc, main, "Unpinned", None, None);

    let wide = grid_frame(&mut doc, page, "Wide", 300, 0, |al| al.grid_auto_tracks = GridAutoTracks::Rows);
    grid_cell(&mut doc, wide, "FarCell", Some(299), Some(0));

    let comp = grid_frame(&mut doc, page, "GridComp", 3, 2, |_| {});
    let cell = grid_cell(&mut doc, comp, "Cell", Some(0), Some(0));
    {
        let c = doc.get_mut(comp).unwrap();
        c.kind = NodeKind::Component(c.kind.frame_props().cloned().expect("frame props"));
    }
    let _ = cell;
    create_instance(&mut doc, comp, page, Vec2::new(700.0, 0.0)).apply(&mut doc).expect("create_instance");
    let inst = instances_of(&doc, comp)[0];
    doc.get_mut(inst).unwrap().name = "GridInstance".into();
    let moved = doc.get(inst).unwrap().children.iter().copied().find(|c| doc.get(*c).is_some_and(|n| n.name == "Cell")).expect("Cell in instance");
    let m = doc.get_mut(moved).unwrap();
    m.layout_child.grid_column = Some(2);
    m.layout_child.grid_row = Some(1);
    m.component.overrides.insert(OverrideField::Layout);

    // A grid nested in a component: the instance moves a cell of it (its column names a track, its row is past the tracks)
    // but does not restate the grid.
    let outer = doc.alloc_id();
    let mut outer_node = Node::new(outer, NodeKind::Component(FrameProps::default()));
    outer_node.name = "Outer".into();
    outer_node.size = Vec2::new(300.0, 200.0);
    doc.insert(page, outer_node).unwrap();
    let inner = grid_frame(&mut doc, outer, "InnerGrid", 3, 2, |_| {});
    grid_cell(&mut doc, inner, "Item", Some(0), Some(0));
    create_instance(&mut doc, outer, page, Vec2::new(0.0, 700.0)).apply(&mut doc).expect("create_instance");
    let outer_inst = instances_of(&doc, outer)[0];
    doc.get_mut(outer_inst).unwrap().name = "OuterInstance".into();
    let inner_clone =
        doc.get(outer_inst).unwrap().children.iter().copied().find(|c| doc.get(*c).is_some_and(|n| n.name == "InnerGrid")).expect("InnerGrid clone");
    let item = doc.get(inner_clone).unwrap().children.iter().copied().find(|c| doc.get(*c).is_some_and(|n| n.name == "Item")).expect("Item clone");
    let i = doc.get_mut(item).unwrap();
    i.layout_child.grid_column = Some(1);
    i.layout_child.grid_row = Some(7); // past the component's two rows: no track to name
    i.component.overrides.insert(OverrideField::Layout);
    doc
}

pub fn auto_layout<'a>(doc: &'a Document, name: &str) -> &'a AutoLayout {
    find(doc, name).kind.frame_props().and_then(|f| f.auto_layout.as_ref()).unwrap_or_else(|| panic!("{name} has no auto layout"))
}

/// What `grid_document` holds, read back from `rt`: tracks, auto-track, cells inside and outside the tracks, the count
/// past the entry cap, and the cells an instance moves.
pub fn assert_grids_survived(rt: &Document) {
    let main = auto_layout(rt, "Main");
    assert_eq!((main.grid_columns, main.grid_rows), (3, 2));
    assert_eq!(&main.grid_col_tracks[..3], &[GridTrack::Fixed(120.0), GridTrack::Flex(2.0), GridTrack::Hug]);
    assert_eq!(main.grid_row_tracks[1], GridTrack::Fixed(70.0));
    assert_eq!(main.grid_auto_tracks, GridAutoTracks::Track(GridTrack::Fixed(50.0)));
    let anchors = |name: &str| (find(rt, name).layout_child.grid_column, find(rt, name).layout_child.grid_row);
    assert_eq!(anchors("InTracks"), (Some(2), Some(1)));
    assert_eq!(anchors("PastTracks"), (Some(5), Some(4)), "an anchor past the tracks is kept");
    assert_eq!(anchors("Unpinned"), (None, None));

    let wide = auto_layout(rt, "Wide");
    assert_eq!(wide.grid_columns, 300, "the count past the entry cap is kept");
    assert_eq!(wide.grid_auto_tracks, GridAutoTracks::Rows);
    assert_eq!(anchors("FarCell"), (Some(299), Some(0)));

    let inst = find(rt, "GridInstance");
    let moved = inst.children.iter().filter_map(|c| rt.get(*c)).find(|n| n.name == "Cell").expect("Cell in the instance");
    assert_eq!((moved.layout_child.grid_column, moved.layout_child.grid_row), (Some(2), Some(1)), "the override moves the cell");
    assert_eq!(anchors("Cell"), (Some(0), Some(0)), "the component's own cell stays");

    let outer = find(rt, "OuterInstance");
    let inner = outer.children.iter().filter_map(|c| rt.get(*c)).find(|n| n.name == "InnerGrid").expect("InnerGrid in the instance");
    let item = inner.children.iter().filter_map(|c| rt.get(*c)).find(|n| n.name == "Item").expect("Item in the instance");
    assert_eq!(
        (item.layout_child.grid_column, item.layout_child.grid_row),
        (Some(1), Some(7)),
        "an override under a grid the instance does not restate, one cell past its tracks"
    );
    assert_eq!(anchors("Item"), (Some(0), Some(0)), "the component's own item stays");
}

fn child_named<'a>(doc: &'a Document, parent: &Node, name: &str) -> &'a Node {
    parent.children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == name).unwrap_or_else(|| panic!("{} has no child {name}", parent.name))
}

/// What `rich_document` holds, read back from `rt`.
pub fn assert_rich_survived(rt: &Document) {
    assert_eq!(auto_layout(rt, "Card").padding, [10.0, 20.0, 30.0, 40.0], "component padding");
    assert_eq!(auto_layout(rt, "Grid").padding, [9.0, 8.0, 7.0, 6.0], "grid padding");
    let inst = find(rt, "CardInstance");
    assert_eq!(auto_layout(rt, "Inner").padding, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(
        child_named(rt, inst, "Inner").kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|a| a.padding),
        Some([5.0, 6.0, 7.0, 8.0]),
        "padding override on an instance child"
    );
    let cell = find(rt, "Cell");
    assert_eq!((cell.layout_child.grid_column, cell.layout_child.grid_row, cell.layout_child.col_span), (Some(2), Some(1), 2));

    let shows_author = |name: &str| match &find(rt, name).kind {
        NodeKind::Sticky(sp) => sp.show_author,
        other => panic!("{name} is {other:?}"),
    };
    assert!(shows_author("AuthorShown") && !shows_author("AuthorHidden"));
    assert_eq!(find(rt, "SlotInstance").component.slot_content_id.as_deref(), Some("7:42"));

    let caps = |name: &str| match &find(rt, name).kind {
        NodeKind::Connector(p) => (p.start_cap, p.end_cap, p.line),
        other => panic!("{name} is {other:?}"),
    };
    assert_eq!(caps("Connector"), (ConnectorCap::Arrow, ConnectorCap::Triangle, ConnectorLine::Straight));
    assert_eq!(caps("RoundCaps"), (ConnectorCap::Circle, ConnectorCap::Diamond, ConnectorLine::Curved));
    assert_eq!(find(rt, "Card").plugin_data.get("12345:note").map(String::as_str), Some("ünï ✓ value"));
}
