//! A grid whose row gap, padding or included stroke weight is bound to a variable is laid out from
//! the resolved value, and so are the pixel row tracks the web exports pin for levelled implicit
//! rows and the cell sizes the native exports solve. Each scene stores a different number than the
//! variable resolves to, so a row list solved from the stored inset would not fit the box.

use designos_doc::*;
use designos_export::{android_xml_for, compose_for, css_for, swiftui_for, tailwind_for, uikit_for};

const BOX: f32 = 200.0;
const ITEM_H: f32 = 20.0;

/// A resolved inset the scene binds: the variable id, and the number it resolves to.
const GAP: (&str, f32) = ("gap", 10.0);
const PAD: (&str, f32) = ("pad", 20.0);
const WEIGHT: (&str, f32) = ("weight", 6.0);

fn number_variable(id: &str, px: f32) -> Variable {
    Variable {
        id: id.into(),
        name: format!("Space/{id}"),
        collection: "c".into(),
        kind: VariableKind::Number,
        values: [("m".to_string(), VariableValue::Number(px))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

/// A fixed 200 x 200 two-column grid whose implicit rows are flex auto tracks, so the solver levels
/// them across the fixed height, holding four items of the same height (two implicit rows). `al`
/// carries the stored inset, `bindings` the variables bound over it.
fn levelled_grid(al: AutoLayout, bindings: &[(&str, (&str, f32))]) -> (Document, NodeId, Vec<NodeId>) {
    let mut doc = Document::new("resolved-row-tracks");
    let page = doc.add_page("Page 1");
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Mode".into() }],
            default_mode: "m".into(),
            variables: [GAP, PAD, WEIGHT].iter().map(|(id, _)| id.to_string()).collect(),
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    for (id, px) in [GAP, PAD, WEIGHT] {
        doc.variables.insert(id.into(), number_variable(id, px));
    }
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, BOX, BOX));
    frame.fills.clear();
    let al = AutoLayout {
        direction: LayoutDirection::Grid,
        grid_columns: 2,
        grid_rows: 0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_auto_tracks: GridAutoTracks::Track(GridTrack::Flex(1.0)),
        ..al
    };
    frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), ..Default::default() });
    for (key, (var, _)) in bindings {
        frame.variables.bindings.insert((*key).into(), (*var).into());
    }
    let grid = doc.insert(page, frame).unwrap();
    let mut items = Vec::new();
    for _ in 0..4 {
        let id = doc.alloc_id();
        let mut rect = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, ITEM_H));
        rect.fills.clear();
        rect.layout_child.horizontal = Sizing::Fill;
        rect.layout_child.vertical = Sizing::Fixed;
        items.push(doc.insert(grid, rect).unwrap());
    }
    doc.relayout_subtree(grid);
    (doc, grid, items)
}

fn px_list(rows: &[f32]) -> String {
    rows.iter().map(|r| format!("{r}px")).collect::<Vec<_>>().join(" ")
}

/// The value of declaration `prop` in a CSS block.
fn decl(css: &str, prop: &str) -> String {
    let pre = format!("{prop}: ");
    css.lines().find_map(|l| l.strip_prefix(&pre)).map(|v| v.trim_end_matches(';').to_string()).unwrap_or_else(|| format!("<no {prop} in {css}>"))
}

/// Checks the two row tracks the exports pin against what Studio drew. `top`, `bottom` and `row_gap`
/// are the resolved insets; `rows` the track heights those insets leave.
fn assert_rows_match_studio(doc: &Document, grid: NodeId, items: &[NodeId], top: f32, bottom: f32, row_gap: f32, rows: [f32; 2]) {
    let eps = 1e-3;
    // The layout the document drew: the second row starts below the first row and the resolved gap.
    let drawn_row_1 = doc.get(items[2]).unwrap().transform.ty;
    assert!((drawn_row_1 - (top + rows[0] + row_gap)).abs() < eps, "Studio drew row 1 at {drawn_row_1}, expected {}", top + rows[0] + row_gap);
    // The tracks, the resolved gap and the resolved insets fill the box exactly.
    let used = top + rows[0] + row_gap + rows[1] + bottom;
    assert!((used - BOX).abs() < eps, "tracks, gap and padding take {used} of {BOX}");

    let css = css_for(doc, grid);
    assert_eq!(decl(&css, "grid-template-rows"), px_list(&rows), "CSS row tracks\n{css}");
    let tw = tailwind_for(doc, grid);
    let class = format!("grid-rows-[{}]", rows.map(|r| format!("{r}px")).join("_"));
    assert!(tw.contains(&class), "Tailwind lacks {class}\n{tw}");
}

#[test]
fn levelled_rows_follow_a_bound_item_spacing() {
    let al = AutoLayout { item_spacing: 4.0, ..Default::default() };
    let (doc, grid, items) = levelled_grid(al, &[("itemSpacing", GAP)]);
    // Free height is 200 - 10 = 190, so each of the two flex rows is 95.
    assert_rows_match_studio(&doc, grid, &items, 0.0, 0.0, GAP.1, [95.0, 95.0]);
}

#[test]
fn levelled_rows_follow_a_bound_row_gap() {
    let al = AutoLayout { item_spacing: 4.0, counter_spacing: 4.0, ..Default::default() };
    let (doc, grid, items) = levelled_grid(al, &[("counterAxisSpacing", GAP)]);
    assert_rows_match_studio(&doc, grid, &items, 0.0, 0.0, GAP.1, [95.0, 95.0]);
}

#[test]
fn levelled_rows_follow_bound_padding() {
    let al = AutoLayout { padding: [4.0; 4], ..Default::default() };
    let (doc, grid, items) = levelled_grid(al, &[("paddingTop", PAD), ("paddingBottom", PAD)]);
    // Free height is 200 - 20 - 20 = 160, so each flex row is 80.
    assert_rows_match_studio(&doc, grid, &items, PAD.1, PAD.1, 0.0, [80.0, 80.0]);
}

#[test]
fn levelled_rows_follow_a_bound_included_stroke_weight() {
    let al = AutoLayout { layout_version: 0, stroke_included: true, ..Default::default() };
    let (mut doc, grid, _) = levelled_grid(al, &[]);
    // The stroke is part of the solved box in the first layout version, so its weight insets every side.
    {
        let n = doc.get_mut(grid).unwrap();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
        n.stroke.weight = 2.0;
        n.stroke.align = StrokeAlign::Inside;
        n.variables.bindings.insert("strokeWeight".into(), WEIGHT.0.into());
    }
    doc.relayout_subtree(grid);
    let items: Vec<NodeId> = doc.get(grid).unwrap().children.clone();
    // Free height is 200 - 6 - 6 = 188, so each flex row is 94.
    assert_rows_match_studio(&doc, grid, &items, WEIGHT.1, WEIGHT.1, 0.0, [94.0, 94.0]);
}

#[test]
fn native_grid_cells_follow_a_bound_item_spacing() {
    let al = AutoLayout { item_spacing: 4.0, ..Default::default() };
    let (doc, grid, _) = levelled_grid(al, &[("itemSpacing", GAP)]);
    // Item spacing resolves to 10, so the two columns and the two levelled rows are 95 each.
    let expected = [
        ("android", android_xml_for(&doc, grid), "android:layout_width=\"95dp\"\n        android:layout_height=\"95dp\""),
        ("compose", compose_for(&doc, grid), "Box(Modifier.height(95.dp)"),
        ("swiftui", swiftui_for(&doc, grid), ".frame(width: 95, height: 95"),
        ("uikit", uikit_for(&doc, grid), "cell.heightAnchor.constraint(equalToConstant: 95)"),
    ];
    for (target, out, needle) in expected {
        assert!(out.contains(needle), "{target} cells do not follow the resolved spacing\n{out}");
    }
}
