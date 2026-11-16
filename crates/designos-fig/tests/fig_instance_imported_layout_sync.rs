//! A sync of a main keeps the geometry an imported `.fig` stored for a resized instance while that sync would not
//! change Studio's own layout of the instance, and is exactly the record-free sync otherwise.
//!
//! "Record-free" is the same document with the imported-layout record removed (through the native JSON, so this file
//! also builds against a reader without the record): what Studio did before keeping stored geometry, and the promise
//! is never to do worse than it. The layout fields a kept record writes back are size, position, the auto-layout
//! stretch record, baked outlines and a vector's paths; every other field must always match the record-free sync.
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::import_fig;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, guid, matrix, msg, node, solid, st, vec2};
use serde_json::Value;

// ------------------------------------------------------------------ fixtures

fn fig(changes: Vec<V>, blobs: Vec<Vec<u8>>) -> Vec<u8> {
    let schema = figma_like_schema();
    let root = schema.find("Message").expect("Message in schema");
    let mut fields = vec![("nodeChanges", V::Array(changes))];
    if !blobs.is_empty() {
        fields.push(("blobs", V::Array(blobs.into_iter().map(|b| st(vec![("bytes", V::Bytes(b))])).collect())));
    }
    let body = schema.encode_message(root, &msg(fields)).expect("encode fig message");
    container::write_container(&schema.encode(), &body, Compression::Zstd, 106)
}

fn page() -> Vec<V> {
    vec![node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]), node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![])]
}

/// One derived-data entry: the stored size and position of the layer at `path` (main layer guids, outermost first).
fn derived(path: &[(u32, u32)], size: (f32, f32), at: (f32, f32)) -> V {
    msg(vec![
        ("guidPath", msg(vec![("guids", V::Array(path.iter().map(|&(s, l)| guid(s, l)).collect()))])),
        ("size", vec2(size.0, size.1)),
        ("transform", matrix(at.0, at.1)),
    ])
}

/// A resized instance of `main` named `name`, with its stored derived data. The instance's own size is stated in its
/// override record, as Figma writes for a resized instance, so a sync keeps it.
fn instance(id: (u32, u32), name: &str, main: (u32, u32), size: (f32, f32), at: (f32, f32), stored: Vec<V>, extra: Vec<(&str, V)>) -> V {
    instance_on((0, 1), id, name, main, size, at, stored, extra)
}

/// [`instance`] on page `page`.
#[allow(clippy::too_many_arguments)]
fn instance_on(page: (u32, u32), id: (u32, u32), name: &str, main: (u32, u32), size: (f32, f32), at: (f32, f32), stored: Vec<V>, extra: Vec<(&str, V)>) -> V {
    let mut fields = vec![
        ("size", vec2(size.0, size.1)),
        ("transform", matrix(at.0, at.1)),
        (
            "symbolData",
            msg(vec![
                ("symbolID", guid(main.0, main.1)),
                ("symbolOverrides", V::Array(vec![msg(vec![("guidPath", msg(vec![("guids", V::Array(vec![]))])), ("size", vec2(size.0, size.1))])])),
            ]),
        ),
        ("derivedSymbolData", V::Array(stored)),
    ];
    fields.extend(extra);
    node(id, page, "b", "INSTANCE", name, fields)
}

fn auto_layout(dir: &str, spacing: f32, pad: f32) -> Vec<(&'static str, V)> {
    vec![
        ("stackMode", en(dir)),
        ("stackSpacing", fl(spacing)),
        ("stackHorizontalPadding", fl(pad)),
        ("stackPaddingRight", fl(pad)),
        ("stackVerticalPadding", fl(pad)),
        ("stackPaddingBottom", fl(pad)),
    ]
}

/// Row (auto layout, 100 wide) holding a Badge and a growing Inner frame; the instance is 160 wide and Figma stored
/// Inner one pixel narrower than Studio's solve gives (111 instead of 112), as rounding in a real file does.
fn row_fig() -> Vec<u8> {
    let mut changes = page();
    let mut row = vec![("size", vec2(100.0, 40.0)), ("transform", matrix(0.0, 0.0)), ("stackPrimarySizing", en("FIXED"))];
    row.extend(auto_layout("HORIZONTAL", 8.0, 10.0));
    changes.push(node((1, 1), (0, 1), "a", "SYMBOL", "Row", row));
    changes.push(node(
        (1, 2),
        (1, 1),
        "a",
        "RECTANGLE",
        "Badge",
        vec![("size", vec2(20.0, 24.0)), ("transform", matrix(10.0, 8.0)), ("fillPaints", V::Array(vec![solid(0.2, 0.3, 0.4)]))],
    ));
    changes.push(node(
        (1, 3),
        (1, 1),
        "b",
        "FRAME",
        "Inner",
        vec![("size", vec2(52.0, 24.0)), ("transform", matrix(38.0, 8.0)), ("stackChildPrimaryGrow", fl(1.0))],
    ));
    let mut inst_extra = vec![("stackPrimarySizing", en("FIXED"))];
    inst_extra.extend(auto_layout("HORIZONTAL", 8.0, 10.0));
    changes.push(instance(
        (2, 1),
        "Row instance",
        (1, 1),
        (160.0, 40.0),
        (24.0, 32.0),
        vec![derived(&[(1, 2)], (20.0, 24.0), (10.0, 8.0)), derived(&[(1, 3)], (111.0, 24.0), (38.0, 8.0))],
        inst_extra,
    ));
    fig(changes, vec![])
}

/// Row (auto layout, 100 wide, hugging its height) > Card (fills the width, hugs its height) > Inner (fills Card's
/// width, hugs its height) > Box. The instance is 160 wide; Figma stored Card and Inner one pixel narrower than
/// Studio's solve gives.
fn card_fig() -> Vec<u8> {
    let mut changes = page();
    let mut row = vec![
        ("size", vec2(100.0, 52.0)),
        ("transform", matrix(0.0, 0.0)),
        ("stackPrimarySizing", en("FIXED")),
        ("stackCounterSizing", en("RESIZE_TO_FIT_WITH_IMPLICIT_SIZE")),
    ];
    row.extend(auto_layout("HORIZONTAL", 0.0, 8.0));
    changes.push(node((1, 1), (0, 1), "a", "SYMBOL", "Row", row));
    let mut card = vec![("size", vec2(84.0, 36.0)), ("transform", matrix(8.0, 8.0)), ("stackChildPrimaryGrow", fl(1.0))];
    card.extend(auto_layout("VERTICAL", 0.0, 4.0));
    changes.push(node((1, 2), (1, 1), "a", "FRAME", "Card", card));
    let mut inner = vec![("size", vec2(76.0, 28.0)), ("transform", matrix(4.0, 4.0)), ("stackChildAlignSelf", en("STRETCH"))];
    inner.extend(auto_layout("VERTICAL", 0.0, 4.0));
    changes.push(node((1, 3), (1, 2), "a", "FRAME", "Inner", inner));
    changes.push(node((1, 4), (1, 3), "a", "RECTANGLE", "Box", vec![("size", vec2(20.0, 20.0)), ("transform", matrix(4.0, 4.0))]));
    let mut inst_extra = vec![("stackPrimarySizing", en("FIXED")), ("stackCounterSizing", en("RESIZE_TO_FIT_WITH_IMPLICIT_SIZE"))];
    inst_extra.extend(auto_layout("HORIZONTAL", 0.0, 8.0));
    changes.push(instance(
        (2, 1),
        "Row instance",
        (1, 1),
        (160.0, 52.0),
        (24.0, 32.0),
        vec![derived(&[(1, 2)], (143.0, 36.0), (8.0, 8.0)), derived(&[(1, 3)], (135.0, 28.0), (4.0, 4.0)), derived(&[(1, 4)], (20.0, 20.0), (4.0, 4.0))],
        inst_extra,
    ));
    fig(changes, vec![])
}

/// A square outline blob in Figma's path command format.
fn square_blob(side: f32) -> Vec<u8> {
    let mut b = Vec::new();
    for (op, pts) in [(1u8, [0.0f32, 0.0]), (2, [side, 0.0]), (2, [side, side]), (2, [0.0, side])] {
        b.push(op);
        for p in pts {
            b.extend(p.to_le_bytes());
        }
    }
    b.push(0);
    b
}

/// Icon (16 x 16) holding a Glyph vector; the instance is 24 x 24 and Figma stored the glyph's outline at that size.
fn icon_fig() -> Vec<u8> {
    let mut changes = page();
    changes.push(node((1, 1), (0, 1), "a", "SYMBOL", "Icon", vec![("size", vec2(16.0, 16.0)), ("transform", matrix(0.0, 0.0))]));
    changes.push(node(
        (1, 2),
        (1, 1),
        "a",
        "VECTOR",
        "Glyph",
        vec![
            ("size", vec2(16.0, 16.0)),
            ("transform", matrix(0.0, 0.0)),
            ("horizontalConstraint", en("SCALE")),
            ("verticalConstraint", en("SCALE")),
            ("fillPaints", V::Array(vec![solid(0.1, 0.2, 0.3)])),
            ("fillGeometry", V::Array(vec![msg(vec![("windingRule", en("NONZERO")), ("commandsBlob", V::Uint(0))])])),
        ],
    ));
    let stored = msg(vec![
        ("guidPath", msg(vec![("guids", V::Array(vec![guid(1, 2)]))])),
        ("size", vec2(24.0, 24.0)),
        ("transform", matrix(0.0, 0.0)),
        ("fillGeometry", V::Array(vec![msg(vec![("windingRule", en("NONZERO")), ("commandsBlob", V::Uint(1))])])),
    ]);
    changes.push(instance((2, 1), "Icon instance", (1, 1), (24.0, 24.0), (40.0, 10.0), vec![stored], vec![]));
    fig(changes, vec![square_blob(16.0), square_blob(24.0)])
}

/// Chip (a 20 x 20 dot) used inside Bar (auto layout, 100 wide) next to a growing Spacer; the Bar instance is 160 wide
/// and Figma stored the Spacer one pixel narrower than Studio's solve gives.
fn chip_bar_fig() -> Vec<u8> {
    let mut changes = page();
    changes.push(node((1, 1), (0, 1), "a", "SYMBOL", "Chip", vec![("size", vec2(20.0, 20.0)), ("transform", matrix(0.0, 0.0))]));
    changes.push(node((1, 2), (1, 1), "a", "RECTANGLE", "Dot", vec![("size", vec2(20.0, 20.0)), ("transform", matrix(0.0, 0.0))]));
    let mut bar = vec![("size", vec2(100.0, 40.0)), ("transform", matrix(0.0, 100.0)), ("stackPrimarySizing", en("FIXED"))];
    bar.extend(auto_layout("HORIZONTAL", 8.0, 10.0));
    changes.push(node((3, 1), (0, 1), "b", "SYMBOL", "Bar", bar));
    changes.push(node(
        (3, 2),
        (3, 1),
        "a",
        "INSTANCE",
        "Chip in Bar",
        vec![("size", vec2(20.0, 20.0)), ("transform", matrix(10.0, 10.0)), ("symbolData", msg(vec![("symbolID", guid(1, 1))]))],
    ));
    changes.push(node(
        (3, 3),
        (3, 1),
        "b",
        "FRAME",
        "Spacer",
        vec![("size", vec2(54.0, 20.0)), ("transform", matrix(38.0, 10.0)), ("stackChildPrimaryGrow", fl(1.0))],
    ));
    let mut inst_extra = vec![("stackPrimarySizing", en("FIXED"))];
    inst_extra.extend(auto_layout("HORIZONTAL", 8.0, 10.0));
    changes.push(instance(
        (2, 1),
        "Bar instance",
        (3, 1),
        (160.0, 40.0),
        (24.0, 200.0),
        vec![derived(&[(3, 2)], (20.0, 20.0), (10.0, 10.0)), derived(&[(3, 3)], (113.0, 20.0), (38.0, 10.0))],
        inst_extra,
    ));
    fig(changes, vec![])
}

fn find(doc: &Document, name: &str, instance_side: bool) -> NodeId {
    doc.nodes
        .values()
        .find(|n| n.name == name && (instance_root_of(doc, n.id).is_some() == instance_side))
        .unwrap_or_else(|| panic!("no {} layer named {name}", if instance_side { "instance" } else { "main" }))
        .id
}

fn main_of(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_))).expect("main").id
}

// ------------------------------------------------------------------ helpers

/// Names a reader has used for the record a sync consults to keep stored geometry.
const RECORD_KEYS: [&str; 2] = ["imported_layout", "derived"];

/// The same document without any imported-layout record: what a sync did before records existed.
fn record_free(doc: &Document) -> Document {
    let mut v: Value = serde_json::from_str(&doc.to_json().expect("to json")).expect("json");
    if let Some(nodes) = v.get_mut("nodes").and_then(Value::as_object_mut) {
        for n in nodes.values_mut() {
            if let Some(c) = n.get_mut("component").and_then(Value::as_object_mut) {
                for key in RECORD_KEYS {
                    c.remove(key);
                }
            }
        }
    }
    Document::from_json(&v.to_string()).expect("from json")
}

fn subtree(doc: &Document, root: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        out.push(id);
        stack.extend(doc.get(id).map(|n| n.children.iter().rev().copied().collect::<Vec<_>>()).unwrap_or_default());
    }
    out
}

fn json_without_record(n: &Node) -> Value {
    let mut v = serde_json::to_value(n).expect("node json");
    if let Some(c) = v.get_mut("component").and_then(Value::as_object_mut) {
        for key in RECORD_KEYS {
            c.remove(key);
        }
    }
    v
}

/// The fields a kept record may write back.
#[derive(Debug, PartialEq)]
struct Layout {
    size: Vec2,
    transform: Affine,
    stretched: StretchedAxes,
    fill_geometry: Option<Vec<Path>>,
    stroke_geometry: Option<Vec<Path>>,
    vector: Option<NodeKind>,
}

fn layout(n: &Node, root: bool) -> Layout {
    Layout {
        size: n.size,
        transform: if root { Affine::IDENTITY } else { n.transform },
        stretched: n.layout_child.stretched,
        fill_geometry: n.fill_geometry.clone(),
        stroke_geometry: n.stroke_geometry.clone(),
        vector: matches!(n.kind, NodeKind::Vector { .. }).then(|| n.kind.clone()),
    }
}

/// Everything but the layout fields (and the record).
fn content(n: &Node) -> Value {
    let mut m = n.clone();
    m.size = Vec2::ZERO;
    m.transform = Affine::IDENTITY;
    m.layout_child.stretched = StretchedAxes::default();
    m.fill_geometry = None;
    m.stroke_geometry = None;
    m.stroke_geometry_for = None;
    if let NodeKind::Vector { paths, network } = &mut m.kind {
        paths.clear();
        *network = None;
    }
    // A grid span of 0 and of 1 mean the same and are written alike; the record-free copy went through the JSON.
    let one = LayoutChild::default();
    if m.layout_child.col_span <= 1 {
        m.layout_child.col_span = one.col_span;
    }
    if m.layout_child.row_span <= 1 {
        m.layout_child.row_span = one.row_span;
    }
    json_without_record(&m)
}

/// The geometry of every layer of `inst`, root position left out.
fn geometry(doc: &Document, inst: NodeId) -> Vec<(NodeId, Layout)> {
    subtree(doc, inst).into_iter().map(|id| (id, layout(doc.get(id).unwrap(), id == inst))).collect()
}

fn sync(doc: &mut Document, main: NodeId) {
    sync_instances(doc, main).apply(doc).expect("sync");
}

/// What a sync of `main` did to instance `inst`, compared with the record-free sync of the same document.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// The instance kept the geometry it held before the sync; the record-free sync moved it.
    KeptStored,
    /// The instance is exactly what the record-free sync made of it.
    RecordFree,
}

/// Syncs `main` on `doc` and on its record-free copy and checks the promise: every non-layout field matches the
/// record-free sync, and the layout fields are either all the record-free sync's or all as before the sync.
fn sync_and_compare(doc: &mut Document, main: NodeId, inst: NodeId) -> Outcome {
    let before = geometry(doc, inst);
    let mut free = record_free(doc);
    sync(doc, main);
    sync(&mut free, main);
    let ids = subtree(doc, inst);
    assert_eq!(ids, subtree(&free, inst), "the sync builds the same layers with and without the record");
    for id in &ids {
        assert_eq!(content(doc.get(*id).unwrap()), content(free.get(*id).unwrap()), "layer {id:?}: non-layout fields match the record-free sync");
    }
    let after = geometry(doc, inst);
    let free_after = geometry(&free, inst);
    if after == free_after {
        Outcome::RecordFree
    } else {
        assert_eq!(after, before, "a sync that does not match the record-free sync keeps every layer as it was");
        Outcome::KeptStored
    }
}

fn imported(bytes: &[u8], instance: &str, main: &str) -> (Document, NodeId, NodeId) {
    let doc = import_fig(bytes).expect("import");
    let inst = find(&doc, instance, true);
    let main = main_of(&doc, main);
    (doc, inst, main)
}

fn edit(doc: &mut Document, id: NodeId, f: impl FnOnce(&mut Node)) {
    f(doc.get_mut(id).expect("layer to edit"));
}

/// Changing `main` makes the record-free sync move at least one layer of `inst`; the leak tests below would pass
/// vacuously otherwise.
fn assert_record_free_sync_moves(doc: &Document, main: NodeId, inst: NodeId) {
    let mut free = record_free(doc);
    let before = geometry(&free, inst);
    sync(&mut free, main);
    assert_ne!(geometry(&free, inst), before, "the edit must move the instance in a record-free sync");
}

// ------------------------------------------------------------------ the original report

/// #1919: importing resized instances and syncing their main with no change left them as imported.
#[test]
fn no_change_sync_leaves_imported_resized_instances_as_imported() {
    // Studio's own sync moves the first two (it solves Inner and Card one pixel wider than Figma stored them); the icon
    // it already lays out as stored.
    let cases = [
        (row_fig(), "Row instance", "Row", Outcome::KeptStored),
        (card_fig(), "Row instance", "Row", Outcome::KeptStored),
        (icon_fig(), "Icon instance", "Icon", Outcome::RecordFree),
    ];
    for (bytes, instance, main, expected) in cases {
        let (mut doc, inst, main) = imported(&bytes, instance, main);
        let before: Vec<Value> = subtree(&doc, inst).iter().map(|id| json_without_record(doc.get(*id).unwrap())).collect();
        let layers = doc.nodes.len();
        assert_eq!(sync_and_compare(&mut doc, main, inst), expected, "{instance}");
        let after: Vec<Value> = subtree(&doc, inst).iter().map(|id| json_without_record(doc.get(*id).unwrap())).collect();
        assert_eq!(doc.nodes.len(), layers);
        assert_eq!(after, before, "{instance}: every layer is exactly as imported");
        // and again: a kept record stays usable
        assert_eq!(sync_and_compare(&mut doc, main, inst), expected, "{instance}, second sync");
    }
}

/// A main fill edit reaches the instance and moves nothing.
#[test]
fn main_fill_edit_updates_fill_and_keeps_stored_geometry() {
    let (mut doc, inst, main) = imported(&row_fig(), "Row instance", "Row");
    let badge = find(&doc, "Badge", false);
    let fill = vec![Paint::solid(Color::rgba(25, 204, 77, 255))];
    edit(&mut doc, badge, |n| n.fills = fill.clone());
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::KeptStored);
    assert_eq!(doc.get(find(&doc, "Badge", true)).unwrap().fills, fill);
    assert_eq!(doc.get(find(&doc, "Inner", true)).unwrap().size, Vec2::new(111.0, 24.0));
}

// ------------------------------------------------------------------ the leaks earlier models had

/// A stroke paint's opacity decides whether an inside stroke takes space in auto layout.
#[test]
fn stroke_paint_opacity_that_changes_the_inset_gives_the_record_free_sync() {
    let (mut doc, inst, main) = imported(&row_fig(), "Row instance", "Row");
    edit(&mut doc, main, |n| {
        let mut paint = Paint::solid(Color::BLACK);
        paint.opacity = 0.0;
        n.stroke.paints = vec![paint];
        n.stroke.weight = 8.0;
        n.stroke.align = StrokeAlign::Inside;
        let al = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()).expect("auto layout");
        al.stroke_included = true;
        al.layout_version = 0;
    });
    // an invisible stroke takes no space: nothing moves
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::KeptStored);
    edit(&mut doc, main, |n| n.stroke.paints[0].opacity = 1.0);
    assert_record_free_sync_moves(&doc, main, inst);
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::RecordFree);
}

fn gap_variable(doc: &mut Document, main: NodeId, value: f32) {
    doc.variable_collections.insert(
        "c1".into(),
        VariableCollection {
            id: "c1".into(),
            name: "Spacing".into(),
            modes: vec![VariableMode { id: "m1".into(), name: "Compact".into() }],
            default_mode: "m1".into(),
            variables: vec!["v_gap".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "v_gap".into(),
        Variable {
            id: "v_gap".into(),
            name: "gap".into(),
            collection: "c1".into(),
            kind: VariableKind::Number,
            values: [("m1".to_string(), VariableValue::Number(value))].into_iter().collect(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    edit(doc, main, |n| {
        n.variables.bindings.insert("itemSpacing".into(), "v_gap".into());
    });
}

/// A gap bound to a variable is laid out with the variable's value, which lives outside every layer.
#[test]
fn variable_value_behind_a_bound_gap_gives_the_record_free_sync() {
    let (mut doc, inst, main) = imported(&row_fig(), "Row instance", "Row");
    // bound to a variable holding the gap the main already has: nothing moves
    gap_variable(&mut doc, main, 8.0);
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::KeptStored);
    doc.variables.get_mut("v_gap").unwrap().values.insert("m1".into(), VariableValue::Number(20.0));
    assert_record_free_sync_moves(&doc, main, inst);
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::RecordFree);
}

/// A box two hug frames down grows: both frames grow in the record-free sync.
#[test]
fn box_growing_two_hug_frames_down_gives_the_record_free_sync() {
    let (mut doc, inst, main) = imported(&card_fig(), "Row instance", "Row");
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::KeptStored);
    let main_box = find(&doc, "Box", false);
    edit(&mut doc, main_box, |n| n.size = Vec2::new(20.0, 40.0));
    doc.relayout_subtree(main);
    assert_eq!(doc.get(find(&doc, "Card", false)).unwrap().size.y, 56.0, "the main's own hug frames refit");
    let card_before = doc.get(find(&doc, "Card", true)).unwrap().size;
    let mut free = record_free(&doc);
    sync(&mut free, main);
    assert_ne!(free.get(find(&free, "Card", true)).unwrap().size.y, card_before.y, "the record-free sync grows the hug frame two levels up");
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::RecordFree);
}

/// The main's vector gets a new outline in the same box.
#[test]
fn vector_path_change_in_the_same_box_gives_the_record_free_sync() {
    let (mut doc, inst, main) = imported(&icon_fig(), "Icon instance", "Icon");
    let glyph = find(&doc, "Glyph", false);
    let triangle = vec![Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(8.0, 0.0)), PathCmd::LineTo(Vec2::new(16.0, 16.0)), PathCmd::LineTo(Vec2::new(0.0, 16.0)), PathCmd::Close],
        ..Default::default()
    }];
    edit(&mut doc, glyph, |n| {
        n.kind = NodeKind::Vector { paths: triangle.clone(), network: None };
        n.fill_geometry = None;
    });
    assert_record_free_sync_moves(&doc, main, inst);
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::RecordFree);
    let NodeKind::Vector { paths, .. } = &doc.get(find(&doc, "Glyph", true)).unwrap().kind else { panic!("glyph is a vector") };
    assert_eq!(paths[0].cmds.len(), 4, "the instance draws the main's new outline");
}

/// The main of an instance inside the holder's main changes: the check runs after that instance was synced, so it
/// sees the change (a check before the sync would see the old chip and keep the stored layout).
#[test]
fn edited_main_inside_the_holders_main_gives_the_record_free_sync() {
    let (mut doc, inst, _bar) = imported(&chip_bar_fig(), "Bar instance", "Bar");
    let chip = main_of(&doc, "Chip");
    let dot = find(&doc, "Dot", false);
    edit(&mut doc, dot, |n| n.size = Vec2::new(30.0, 20.0));
    edit(&mut doc, chip, |n| n.size = Vec2::new(30.0, 20.0));
    assert_record_free_sync_moves(&doc, chip, inst);
    assert_eq!(sync_and_compare(&mut doc, chip, inst), Outcome::RecordFree);
}

// ------------------------------------------------------------------ never worse than dropping the record

/// For every edit, the sync either keeps the instance exactly as it was or is exactly the record-free sync
/// (checked inside [`sync_and_compare`]); edits that do not change Studio's layout keep the stored geometry.
#[test]
fn every_sync_is_the_record_free_sync_or_keeps_the_instance_as_it_was() {
    type Edit = fn(&mut Document);
    let cases: Vec<(&str, Edit, Outcome)> = vec![
        ("no change", |_| {}, Outcome::KeptStored),
        ("badge fill", |d| edit(d, find(d, "Badge", false), |n| n.fills = vec![Paint::solid(Color::rgba(1, 2, 3, 255))]), Outcome::KeptStored),
        ("badge renamed", |d| edit(d, find(d, "Badge", false), |n| n.name = "Pill".into()), Outcome::KeptStored),
        ("badge opacity", |d| edit(d, find(d, "Badge", false), |n| n.opacity = 0.5), Outcome::KeptStored),
        ("inner blend", |d| edit(d, find(d, "Inner", false), |n| n.blend = BlendMode::Multiply), Outcome::KeptStored),
        ("badge corner radius", |d| edit(d, find(d, "Badge", false), |n| n.corner_radii = [4.0; 4]), Outcome::RecordFree),
        ("badge wider", |d| edit(d, find(d, "Badge", false), |n| n.size = Vec2::new(30.0, 24.0)), Outcome::RecordFree),
        ("badge hidden", |d| edit(d, find(d, "Badge", false), |n| n.visible = false), Outcome::RecordFree),
        (
            "row padding",
            |d| edit(d, main_of(d, "Row"), |n| n.kind.frame_props_mut().unwrap().auto_layout.as_mut().unwrap().padding = [4.0; 4]),
            Outcome::RecordFree,
        ),
        (
            "row gap",
            |d| edit(d, main_of(d, "Row"), |n| n.kind.frame_props_mut().unwrap().auto_layout.as_mut().unwrap().item_spacing = 2.0),
            Outcome::RecordFree,
        ),
    ];
    for (what, change, expected) in cases {
        let (mut doc, inst, main) = imported(&row_fig(), "Row instance", "Row");
        change(&mut doc);
        assert_eq!(sync_and_compare(&mut doc, main, inst), expected, "{what}");
        // a second sync after the first, with no further change, keeps whatever the first one left
        let settled = geometry(&doc, inst);
        sync_and_compare(&mut doc, main, inst);
        assert_eq!(geometry(&doc, inst), settled, "{what}: a following no-change sync moves nothing");
    }
}

// ------------------------------------------------------------------ the record outside one sync

/// A native save and reopen keeps what the record needs.
#[test]
fn native_save_and_reopen_then_no_change_sync_keeps_stored_geometry() {
    let (doc, _, _) = imported(&row_fig(), "Row instance", "Row");
    let mut reopened = Document::from_json(&doc.to_json().expect("save")).expect("reopen");
    let inst = find(&reopened, "Row instance", true);
    let main = main_of(&reopened, "Row");
    assert_eq!(sync_and_compare(&mut reopened, main, inst), Outcome::KeptStored);
}

/// Undoing a sync that dropped the record brings the instance and its record back.
#[test]
fn undo_of_a_sync_that_re_laid_out_the_instance_restores_it() {
    let (mut doc, inst, main) = imported(&row_fig(), "Row instance", "Row");
    let before: Vec<Value> = subtree(&doc, inst).iter().map(|id| serde_json::to_value(doc.get(*id).unwrap()).unwrap()).collect();
    let badge = find(&doc, "Badge", false);
    edit(&mut doc, badge, |n| n.size = Vec2::new(30.0, 24.0));
    let undo = sync_instances(&mut doc, main).apply(&mut doc).expect("sync");
    undo.apply(&mut doc).expect("undo");
    let after: Vec<Value> = subtree(&doc, inst).iter().map(|id| serde_json::to_value(doc.get(*id).unwrap()).unwrap()).collect();
    assert_eq!(after, before);
    // and the kept path still works once the main is back
    edit(&mut doc, badge, |n| n.size = Vec2::new(20.0, 24.0));
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::KeptStored);
}

/// A copy of an imported instance carries the stored geometry: it keeps it as the original does.
#[test]
fn duplicated_instance_keeps_stored_geometry_like_the_original() {
    let (mut doc, inst, main) = imported(&row_fig(), "Row instance", "Row");
    let page = doc.get(inst).unwrap().parent.unwrap();
    let nodes = duplicate_subtree(&mut doc, inst);
    let copy = nodes[0].id;
    Command::Insert { parent: page, index: 0, nodes }.apply(&mut doc).expect("insert copy");
    assert_eq!(sync_and_compare(&mut doc, main, copy), Outcome::KeptStored);
    assert_eq!(doc.get(find_child(&doc, copy, "Inner")).unwrap().size, Vec2::new(111.0, 24.0));
}

fn find_child(doc: &Document, root: NodeId, name: &str) -> NodeId {
    subtree(doc, root).into_iter().find(|id| doc.get(*id).is_some_and(|n| n.name == name)).expect("child")
}

/// Row on a big first page (so the file loads its other pages later) and the resized Row instance on the third
/// page, plus a fill style with a sort position (which the reader sets after the import itself is done).
fn deferred_row_fig() -> Vec<u8> {
    let mut changes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Cover", vec![]),
        node((0, 2), (0, 0), "b", "CANVAS", "Page 2", vec![]),
        node((0, 3), (0, 0), "c", "CANVAS", "Later", vec![]),
    ];
    let mut row = vec![("size", vec2(100.0, 40.0)), ("transform", matrix(0.0, 0.0)), ("stackPrimarySizing", en("FIXED"))];
    row.extend(auto_layout("HORIZONTAL", 8.0, 10.0));
    changes.push(node((1, 1), (0, 1), "a", "SYMBOL", "Row", row));
    changes.push(node((1, 2), (1, 1), "a", "RECTANGLE", "Badge", vec![("size", vec2(20.0, 24.0)), ("transform", matrix(10.0, 8.0))]));
    changes.push(node(
        (1, 3),
        (1, 1),
        "b",
        "FRAME",
        "Inner",
        vec![("size", vec2(52.0, 24.0)), ("transform", matrix(38.0, 8.0)), ("stackChildPrimaryGrow", fl(1.0))],
    ));
    changes.push(node(
        (5, 1),
        (0, 1),
        "b",
        "RECTANGLE",
        "Brand",
        vec![("styleType", en("FILL")), ("sortPosition", V::String("a".into())), ("fillPaints", V::Array(vec![solid(0.9, 0.1, 0.1)]))],
    ));
    let mut inst_extra = vec![("stackPrimarySizing", en("FIXED"))];
    inst_extra.extend(auto_layout("HORIZONTAL", 8.0, 10.0));
    changes.push(instance_on(
        (0, 3),
        (2, 1),
        "Row instance",
        (1, 1),
        (160.0, 40.0),
        (24.0, 32.0),
        vec![derived(&[(1, 2)], (20.0, 24.0), (10.0, 8.0)), derived(&[(1, 3)], (111.0, 24.0), (38.0, 8.0))],
        inst_extra,
    ));
    for i in 0..25_000u32 {
        changes.push(node((9, i + 1), (0, 1), "c", "RECTANGLE", "F", vec![("size", vec2(4.0, 4.0)), ("transform", matrix(0.0, 0.0))]));
    }
    fig(changes, vec![])
}

/// An instance on a page the reader loads later is recorded when the page opens, and keeps its stored geometry.
#[test]
fn instance_on_a_page_opened_later_keeps_stored_geometry() {
    let mut doc = import_fig(&deferred_row_fig()).expect("import");
    let inst = doc.nodes.values().find(|n| n.name == "Row instance").expect("instance").id;
    assert!(doc.get(inst).unwrap().children.is_empty(), "the instance's page is loaded later");
    assert!(doc.hydrate_all_pages());
    let main = main_of(&doc, "Row");
    assert_eq!(doc.get(find_child(&doc, inst, "Inner")).unwrap().size, Vec2::new(111.0, 24.0));
    assert_eq!(sync_and_compare(&mut doc, main, inst), Outcome::KeptStored);
}
