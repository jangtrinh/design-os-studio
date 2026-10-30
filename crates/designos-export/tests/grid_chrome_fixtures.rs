//! Issue #484: pins the emitted CSS and Tailwind strings for the six Chrome-compared grid
//! fixture scenes (`equal_tracks`, `unequal_tracks`, `fixed_first_column`, `hugging_grid`,
//! `fixed_height_auto_rows`, `row_and_column_spans`) and dumps their solver boxes + CSS/Tailwind
//! as JSON for `scripts/qa/grid-chrome-compare.py` when `GRID_CHROME_DUMP_JSON=1` is set.

use designos_doc::{AutoLayout, Document, FrameProps, GridTrack, LayoutDirection, Node, NodeId, NodeKind, Rect, Sizing, Vec2};
use designos_export::{css_for, tailwind_for};

pub struct GridFixture {
    pub name: &'static str,
    pub doc: Document,
    pub grid: NodeId,
    pub items: Vec<NodeId>,
    pub expected_css: &'static str,
    pub expected_tailwind: &'static str,
}

fn new_grid(al: AutoLayout, size: Vec2) -> (Document, NodeId) {
    let mut doc = Document::new("grid-fixtures");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, size.x, size.y));
    frame.fills.clear();
    frame.kind =
        NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(AutoLayout { direction: LayoutDirection::Grid, ..al }), ..Default::default() });
    let grid = doc.insert(page, frame).unwrap();
    (doc, grid)
}

fn add_item(doc: &mut Document, grid: NodeId, w: f32, h: f32, horiz: Sizing, vert: Sizing) -> NodeId {
    let id = doc.alloc_id();
    let mut rect = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
    rect.fills.clear();
    rect.layout_child.horizontal = horiz;
    rect.layout_child.vertical = vert;
    doc.insert(grid, rect).unwrap()
}

pub fn build_fixtures() -> Vec<GridFixture> {
    vec![
        scene_equal_tracks(),
        scene_unequal_tracks(),
        scene_fixed_first_column(),
        scene_hugging_grid(),
        scene_fixed_height_auto_rows(),
        scene_row_and_column_spans(),
    ]
}

fn scene_equal_tracks() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 12.0,
        counter_spacing: 12.0,
        padding: [12.0, 12.0, 12.0, 12.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(324.0, 180.0));
    let mut items = Vec::new();
    for _ in 0..6 {
        items.push(add_item(&mut doc, grid, 80.0, 36.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    GridFixture {
        name: "equal_tracks",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 324px;\nheight: 180px;\npadding: 12px;\ngrid-template-columns: repeat(3, minmax(0, 1fr));\ngap: 12px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-3 content-start gap-3 p-3 w-[324px] h-[180px]\">\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n</div>",
    }
}

fn scene_unequal_tracks() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 140.0));
    let items = vec![
        add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed),
    ];
    doc.relayout_subtree(grid);
    GridFixture {
        name: "unequal_tracks",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 200px;\nheight: 140px;\ngrid-template-columns: 100px 40px 60px;\ngap: 10px 0px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_60px] content-start gap-y-2.5 w-[200px] h-[140px]\">\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n  <div class=\"w-15 h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>",
    }
}

fn scene_fixed_first_column() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(200.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(440.0, 180.0));
    let mut items = Vec::new();
    for _ in 0..6 {
        items.push(add_item(&mut doc, grid, 50.0, 40.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    GridFixture {
        name: "fixed_first_column",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 440px;\nheight: 180px;\ngrid-template-columns: 200px repeat(2, minmax(0, 1fr));\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[200px_repeat(2,minmax(0,1fr))] content-start gap-2.5 w-[440px] h-[180px]\">\n  <div class=\"h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"h-10\"></div>\n</div>",
    }
}

fn scene_hugging_grid() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 8.0,
        counter_spacing: 8.0,
        padding: [8.0, 8.0, 8.0, 8.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let items = vec![
        add_item(&mut doc, grid, 100.0, 28.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 24.0, 28.0, Sizing::Fixed, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed),
    ];
    doc.relayout_subtree(grid);
    GridFixture {
        name: "hugging_grid",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 212px;\nheight: 72px;\npadding: 8px;\ngrid-template-columns: 100px 40px 40px;\ngap: 8px;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_40px] gap-2 p-2 w-[212px] h-18\">\n  <div class=\"w-[100px] h-7\"></div>\n  <div class=\"w-6 h-7\"></div>\n  <div class=\"w-6 h-7\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>",
    }
}

fn scene_fixed_height_auto_rows() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 12.0,
        counter_spacing: 16.0,
        padding: [16.0, 16.0, 16.0, 16.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(260.0, 240.0));
    let items = vec![
        add_item(&mut doc, grid, 50.0, 24.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 24.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 36.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 36.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 28.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 50.0, 28.0, Sizing::Fill, Sizing::Fixed),
    ];
    doc.relayout_subtree(grid);
    GridFixture {
        name: "fixed_height_auto_rows",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 260px;\nheight: 240px;\npadding: 16px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngap: 16px 12px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start gap-x-3 gap-y-4 p-4 w-[260px] h-60\">\n  <div class=\"h-6\"></div>\n  <div class=\"h-6\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-7\"></div>\n  <div class=\"h-7\"></div>\n</div>",
    }
}

fn scene_row_and_column_spans() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 4,
        grid_rows: 0,
        item_spacing: 12.0,
        counter_spacing: 12.0,
        padding: [12.0, 12.0, 12.0, 12.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(428.0, 220.0));
    let hero = add_item(&mut doc, grid, 80.0, 40.0, Sizing::Fill, Sizing::Fill);
    doc.get_mut(hero).unwrap().layout_child.col_span = 2;
    doc.get_mut(hero).unwrap().layout_child.row_span = 2;

    let top_wide = add_item(&mut doc, grid, 80.0, 40.0, Sizing::Fill, Sizing::Fixed);
    doc.get_mut(top_wide).unwrap().layout_child.col_span = 2;

    let mid_a = add_item(&mut doc, grid, 60.0, 40.0, Sizing::Fill, Sizing::Fixed);
    let mid_b = add_item(&mut doc, grid, 60.0, 40.0, Sizing::Fill, Sizing::Fixed);

    let footer = add_item(&mut doc, grid, 120.0, 36.0, Sizing::Fill, Sizing::Fixed);
    doc.get_mut(footer).unwrap().layout_child.grid_column = Some(0);
    doc.get_mut(footer).unwrap().layout_child.grid_row = Some(2);
    doc.get_mut(footer).unwrap().layout_child.col_span = 4;
    doc.get_mut(footer).unwrap().layout_child.row_span = 1;

    doc.relayout_subtree(grid);
    GridFixture {
        name: "row_and_column_spans",
        doc,
        grid,
        items: vec![hero, top_wide, mid_a, mid_b, footer],
        expected_css: "display: grid;\nwidth: 428px;\nheight: 220px;\npadding: 12px;\ngrid-template-columns: repeat(4, minmax(0, 1fr));\ngap: 12px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-4 content-start gap-3 p-3 w-[428px] h-[220px]\">\n  <div class=\"col-span-2 row-span-2 h-[92px]\"></div>\n  <div class=\"col-span-2 h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"col-start-1 col-span-4 row-start-3 h-9\"></div>\n</div>",
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn dump_fixtures_json() -> String {
    let fixtures = build_fixtures();
    let mut out = String::from("[\n");
    for (si, f) in fixtures.iter().enumerate() {
        let gnode = f.doc.get(f.grid).unwrap();
        out.push_str("  {\n");
        out.push_str(&format!("    \"name\": {},\n", json_str(f.name)));
        out.push_str(&format!("    \"grid_width\": {},\n", gnode.size.x));
        out.push_str(&format!("    \"grid_height\": {},\n", gnode.size.y));
        out.push_str(&format!("    \"grid_css\": {},\n", json_str(&css_for(&f.doc, f.grid))));
        out.push_str(&format!("    \"tailwind_html\": {},\n", json_str(&tailwind_for(&f.doc, f.grid))));
        out.push_str("    \"items\": [\n");
        for (idx, &id) in f.items.iter().enumerate() {
            let n = f.doc.get(id).unwrap();
            let comma = if idx + 1 == f.items.len() { "" } else { "," };
            out.push_str(&format!(
                "      {{\"index\": {}, \"x\": {}, \"y\": {}, \"w\": {}, \"h\": {}, \"css\": {}}}{}\n",
                idx,
                n.transform.tx,
                n.transform.ty,
                n.size.x,
                n.size.y,
                json_str(&css_for(&f.doc, id)),
                comma
            ));
        }
        out.push_str("    ]\n");
        let scomma = if si + 1 == fixtures.len() { "" } else { "," };
        out.push_str(&format!("  }}{scomma}\n"));
    }
    out.push(']');
    out
}

fn fixture_named(name: &str) -> GridFixture {
    build_fixtures().into_iter().find(|f| f.name == name).unwrap_or_else(|| panic!("missing fixture {name}"))
}

fn assert_fixture_css_and_tailwind(name: &str) {
    if std::env::var_os("GRID_CHROME_DUMP_JSON").is_some() {
        println!("===GRID_CHROME_JSON_BEGIN===\n{}\n===GRID_CHROME_JSON_END===", dump_fixtures_json());
        return;
    }
    let f = fixture_named(name);
    let css = css_for(&f.doc, f.grid);
    let tw = tailwind_for(&f.doc, f.grid);
    assert_eq!(css, f.expected_css, "{name} CSS mismatch");
    assert_eq!(tw, f.expected_tailwind, "{name} Tailwind mismatch");
}

#[test]
fn equal_tracks_css_and_tailwind() {
    assert_fixture_css_and_tailwind("equal_tracks");
}

#[test]
fn unequal_tracks_css_and_tailwind() {
    assert_fixture_css_and_tailwind("unequal_tracks");
}

#[test]
fn fixed_first_column_css_and_tailwind() {
    assert_fixture_css_and_tailwind("fixed_first_column");
}

#[test]
fn hugging_grid_css_and_tailwind() {
    assert_fixture_css_and_tailwind("hugging_grid");
}

#[test]
fn fixed_height_auto_rows_css_and_tailwind() {
    assert_fixture_css_and_tailwind("fixed_height_auto_rows");
}

#[test]
fn row_and_column_spans_css_and_tailwind() {
    assert_fixture_css_and_tailwind("row_and_column_spans");
}
