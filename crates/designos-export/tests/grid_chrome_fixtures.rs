//! Issue #484: pins the emitted CSS and Tailwind strings for the six Chrome-compared grid
//! fixture scenes (`equal_tracks`, `unequal_tracks`, `fixed_first_column`, `hugging_grid`,
//! `fixed_height_auto_rows`, `row_and_column_spans`) and dumps their solver boxes + CSS/Tailwind
//! as JSON for `scripts/qa/grid-chrome-compare.py` when `GRID_CHROME_DUMP_JSON=1` is set.

use designos_doc::{
    AlignSelf, AutoLayout, Color, Document, FrameProps, GridAutoTracks, GridTrack, LayoutDirection, Node, NodeId, NodeKind, Paint, Rect, Sizing, StrokeAlign,
    Vec2,
};
use designos_export::{css_for, tailwind_for};

#[path = "grid_chrome_fixtures/stroke_and_track_scenes.rs"]
mod stroke_and_track_scenes;
use stroke_and_track_scenes as extra;

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

fn add_frame_item(doc: &mut Document, grid: NodeId, w: f32, h: f32) -> NodeId {
    let id = doc.alloc_id();
    let mut f = Node::frame(id, Rect::new(0.0, 0.0, w, h));
    f.fills.clear();
    f.layout_child.horizontal = Sizing::Fixed;
    f.layout_child.vertical = Sizing::Fixed;
    doc.insert(grid, f).unwrap()
}

pub fn build_fixtures() -> Vec<GridFixture> {
    vec![
        scene_equal_tracks(),
        scene_unequal_tracks(),
        scene_fixed_first_column(),
        scene_hugging_grid(),
        scene_fixed_height_auto_rows(),
        scene_row_and_column_spans(),
        scene_s1_stretch_child_hug(),
        scene_s2_empty_hug_column(),
        scene_s3_hug_grid_max_w(),
        scene_s5_hug_height_min_h(),
        scene_s5b_hug_height_min_h_flex_auto(),
        scene_s7_explicit_rows_hug(),
        scene_n1_explicit_hug_rows_flex_auto(),
        scene_s9_auto_tracks_flex_unequal(),
        scene_n1b_explicit_hug_rows_flex_auto_overflow(),
        scene_n2_hug_equal_flex_two_empty_hug_cols(),
        scene_n2b_hug_equal_flex_one_empty_hug_col(),
        scene_n2c_hug_equal_flex_fill_only_hug_col(),
        scene_n6_stroke_v0_unpadded_hug(),
        scene_n6c_stroke_v0_unpadded_fixed(),
        scene_c7_single_hug_col_min_w_fill(),
        extra::scene_c1_stroke_v0_center_unpadded_hug(),
        extra::scene_c2_stroke_v0_inside_sides_unpadded_hug(),
        extra::scene_n6d_stroke_v1_padded_fixed(),
        extra::scene_c3_rows0_fixed_row_tracks_flex_auto(),
        extra::scene_n2d_fixed_fill_only_hug_col(),
        extra::scene_s6_explicit_rows_flex(),
        extra::scene_s8_auto_tracks_fixed(),
        extra::scene_s10_fixed_width_hug_tracks(),
        extra::scene_s6b_hug_height_explicit_rows_unequal(),
        extra::scene_c8_component_grid_center_stroke(),
        extra::scene_c9_stroke_v1_inside_sides_padded_fixed(),
        extra::scene_c10_stroke_v1_inside_sides_unpadded_fixed(),
        extra::scene_c10b_stroke_v1_inside_sides_unpadded_hug(),
        extra::scene_c12_row_span_over_content_sized_rows(),
        extra::scene_stroke_v1_inside_sides_unpadded_dashed(),
        extra::scene_stroke_v1_inside_sides_unpadded_gradient(),
        extra::scene_stroke_v1_inside_sides_unpadded_bound_width(),
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
        expected_css: "display: grid;\nwidth: 324px;\nheight: 180px;\npadding: 12px;\nbox-sizing: border-box;\ngrid-template-columns: repeat(3, minmax(0, 1fr));\ngap: 12px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-3 content-start gap-3 box-border p-3 w-[324px] h-[180px]\">\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n</div>",
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
        expected_css: "display: grid;\nwidth: 212px;\nheight: 72px;\npadding: 8px;\nbox-sizing: border-box;\ngrid-template-columns: 100px 40px 40px;\ngap: 8px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_40px] content-start gap-2 box-border p-2 w-[212px] h-18\">\n  <div class=\"w-[100px] h-7\"></div>\n  <div class=\"w-6 h-7\"></div>\n  <div class=\"w-6 h-7\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>",
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
        expected_css: "display: grid;\nwidth: 260px;\nheight: 240px;\npadding: 16px;\nbox-sizing: border-box;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngap: 16px 12px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start gap-x-3 gap-y-4 box-border p-4 w-[260px] h-60\">\n  <div class=\"h-6\"></div>\n  <div class=\"h-6\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-9\"></div>\n  <div class=\"h-7\"></div>\n  <div class=\"h-7\"></div>\n</div>",
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
        expected_css: "display: grid;\nwidth: 428px;\nheight: 220px;\npadding: 12px;\nbox-sizing: border-box;\ngrid-template-columns: repeat(4, minmax(0, 1fr));\ngrid-template-rows: 40px 40px 36px;\ngap: 12px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-4 grid-rows-[40px_40px_36px] content-start gap-3 box-border p-3 w-[428px] h-[220px]\">\n  <div class=\"col-span-2 row-span-2 h-[92px]\"></div>\n  <div class=\"col-span-2 h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"h-10\"></div>\n  <div class=\"col-start-1 col-span-4 row-start-3 h-9\"></div>\n</div>",
    }
}

fn scene_s1_stretch_child_hug() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let a = add_item(&mut doc, grid, 20.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.get_mut(c).unwrap().layout_child.align_self = AlignSelf::Stretch;
    let e = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "s1_stretch_child_hug",
        doc,
        grid,
        items: vec![a, b, c, e],
        expected_css: "display: grid;\nwidth: 140px;\nheight: 60px;\ngrid-template-columns: 100px 40px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px] content-start w-[140px] h-15\">\n  <div class=\"w-5 h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n</div>",
    }
}

fn scene_s2_empty_hug_column() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al.grid_col_tracks[2] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let a = add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "s2_empty_hug_column",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 141px;\nheight: 30px;\ngrid-template-columns: 100px 40px 1px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_1px] content-start w-[141px] h-[30px]\">\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n</div>",
    }
}

fn scene_s3_hug_grid_max_w() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    doc.get_mut(grid).unwrap().layout_child.max_w = Some(150.0);
    let a = add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "s3_hug_grid_max_w",
        doc,
        grid,
        items: vec![a, b, c],
        expected_css: "display: grid;\nwidth: 150px;\nheight: 30px;\ngrid-template-columns: 100px 40px 60px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_60px] content-start w-[150px] h-[30px]\">\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n  <div class=\"w-15 h-[30px]\"></div>\n</div>",
    }
}

fn scene_s5_hug_height_min_h() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 50.0));
    doc.get_mut(grid).unwrap().layout_child.min_h = Some(200.0);
    let mut items = Vec::new();
    for _ in 0..4 {
        items.push(add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    GridFixture {
        name: "s5_hug_height_min_h",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>",
    }
}

fn scene_s5b_hug_height_min_h_flex_auto() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        grid_auto_tracks: GridAutoTracks::Track(GridTrack::Flex(1.0)),
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 50.0));
    doc.get_mut(grid).unwrap().layout_child.min_h = Some(200.0);
    let mut items = Vec::new();
    for _ in 0..4 {
        items.push(add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    GridFixture {
        name: "s5b_hug_height_min_h_flex_auto",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>",
    }
}

fn scene_s7_explicit_rows_hug() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    al.grid_row_tracks[0] = GridTrack::Hug;
    al.grid_row_tracks[1] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    let mut items = Vec::new();
    for _ in 0..4 {
        items.push(add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    GridFixture {
        name: "s7_explicit_rows_hug",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: auto auto;\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 grid-rows-[auto_auto] content-start gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>",
    }
}

fn scene_n1_explicit_hug_rows_flex_auto() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_auto_tracks: GridAutoTracks::Track(GridTrack::Flex(1.0)),
        ..Default::default()
    };
    al.grid_row_tracks[0] = GridTrack::Hug;
    al.grid_row_tracks[1] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    let mut items = Vec::new();
    for _ in 0..4 {
        items.push(add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed));
    }
    doc.relayout_subtree(grid);
    GridFixture {
        name: "n1_explicit_hug_rows_flex_auto",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: auto auto;\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 grid-rows-[auto_auto] content-start gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>",
    }
}

fn scene_s9_auto_tracks_flex_unequal() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_auto_tracks: GridAutoTracks::Track(GridTrack::Flex(1.0)),
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 300.0));
    let items = vec![
        add_item(&mut doc, grid, 100.0, 20.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 100.0, 20.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 100.0, 200.0, Sizing::Fill, Sizing::Fixed),
        add_item(&mut doc, grid, 100.0, 200.0, Sizing::Fill, Sizing::Fixed),
    ];
    doc.relayout_subtree(grid);
    GridFixture {
        name: "s9_auto_tracks_flex_unequal",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 200px;\nheight: 300px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: 100px 200px;",
        expected_tailwind: "<div class=\"grid grid-cols-2 grid-rows-[100px_200px] w-[200px] h-[300px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-[200px]\"></div>\n  <div class=\"h-[200px]\"></div>\n</div>",
    }
}

fn scene_n1b_explicit_hug_rows_flex_auto_overflow() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 2,
        grid_rows: 2,
        item_spacing: 10.0,
        counter_spacing: 10.0,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_auto_tracks: GridAutoTracks::Track(GridTrack::Flex(1.0)),
        ..Default::default()
    };
    al.grid_row_tracks[0] = GridTrack::Hug;
    al.grid_row_tracks[1] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    let items = (0..6).map(|_| add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed)).collect();
    doc.relayout_subtree(grid);
    GridFixture {
        name: "n1b_explicit_hug_rows_flex_auto_overflow",
        doc,
        grid,
        items,
        expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: 20px 20px 140px;\ngap: 10px;",
        expected_tailwind: "<div class=\"grid grid-cols-2 grid-rows-[20px_20px_140px] gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>",
    }
}

fn scene_n2_hug_equal_flex_two_empty_hug_cols() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 4,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al.grid_col_tracks[2] = GridTrack::Hug;
    al.grid_col_tracks[3] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let a = add_frame_item(&mut doc, grid, 50.0, 30.0);
    let b = add_frame_item(&mut doc, grid, 50.0, 30.0);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "n2_hug_equal_flex_two_empty_hug_cols",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 102px;\nheight: 30px;\ngrid-template-columns: 50px 50px 1px 1px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[50px_50px_1px_1px] content-start w-[102px] h-[30px]\">\n  <div class=\"w-[50px] h-[30px]\"></div>\n  <div class=\"w-[50px] h-[30px]\"></div>\n</div>",
    }
}

fn scene_n2b_hug_equal_flex_one_empty_hug_col() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al.grid_col_tracks[2] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let a = add_frame_item(&mut doc, grid, 50.0, 30.0);
    let b = add_frame_item(&mut doc, grid, 50.0, 30.0);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "n2b_hug_equal_flex_one_empty_hug_col",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 101px;\nheight: 30px;\ngrid-template-columns: 50px 50px 1px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[50px_50px_1px] content-start w-[101px] h-[30px]\">\n  <div class=\"w-[50px] h-[30px]\"></div>\n  <div class=\"w-[50px] h-[30px]\"></div>\n</div>",
    }
}

fn scene_n2c_hug_equal_flex_fill_only_hug_col() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al.grid_col_tracks[2] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let a = add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 30.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "n2c_hug_equal_flex_fill_only_hug_col",
        doc,
        grid,
        items: vec![a, b, c],
        expected_css: "display: grid;\nwidth: 130px;\nheight: 30px;\ngrid-template-columns: 50px 50px 30px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[50px_50px_30px] content-start w-[130px] h-[30px]\">\n  <div class=\"w-[50px] h-[30px]\"></div>\n  <div class=\"w-[50px] h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>",
    }
}

fn apply_inside_stroke_2px(doc: &mut Document, grid: NodeId) {
    let n = doc.get_mut(grid).unwrap();
    n.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
    n.stroke.weight = 2.0;
    n.stroke.align = StrokeAlign::Inside;
}

fn scene_n6_stroke_v0_unpadded_hug() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 3,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        layout_version: 0,
        stroke_included: true,
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    apply_inside_stroke_2px(&mut doc, grid);
    let a = add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "n6_stroke_v0_unpadded_hug",
        doc,
        grid,
        items: vec![a, b, c],
        expected_css: "display: grid;\nwidth: 204px;\nheight: 34px;\nbox-sizing: border-box;\ngrid-template-columns: 100px 40px 60px;\nalign-content: start;\nborder: 2px solid #333;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_60px] content-start box-border w-[204px] h-[34px] border-2 border-[#333333]\">\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n  <div class=\"w-15 h-[30px]\"></div>\n</div>",
    }
}

fn scene_n6c_stroke_v0_unpadded_fixed() -> GridFixture {
    let al = AutoLayout {
        grid_columns: 2,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        layout_version: 0,
        stroke_included: true,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 50.0));
    apply_inside_stroke_2px(&mut doc, grid);
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "n6c_stroke_v0_unpadded_fixed",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 200px;\nheight: 34px;\nbox-sizing: border-box;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\nborder: 2px solid #333;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start box-border w-[200px] h-[34px] border-2 border-[#333333]\">\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>",
    }
}

fn scene_c7_single_hug_col_min_w_fill() -> GridFixture {
    let mut al = AutoLayout {
        grid_columns: 1,
        grid_rows: 0,
        item_spacing: 0.0,
        counter_spacing: 0.0,
        padding: [0.0, 0.0, 0.0, 0.0],
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    doc.get_mut(grid).unwrap().layout_child.min_w = Some(200.0);
    let a = add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 80.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "c7_single_hug_col_min_w_fill",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 200px;\nheight: 60px;\ngrid-template-columns: 80px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[80px] content-start w-[200px] h-15\">\n  <div class=\"h-[30px]\"></div>\n  <div class=\"w-20 h-[30px]\"></div>\n</div>",
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

/// What the Chrome harness expects of a scene beyond box positions: the drift it accepts (a visible
/// stroke wins over exact geometry) and whether a child that fills its cell must leave the stroke
/// visible on all four sides.
fn harness_expectation(name: &str) -> (f32, bool) {
    match name {
        "c9_stroke_v1_inside_sides_padded_fixed" | "c10_stroke_v1_inside_sides_unpadded_fixed" | "c10b_stroke_v1_inside_sides_unpadded_hug" => (0.0, true),
        _ => (0.0, false),
    }
}

/// The colour Studio's renderer shows at the middle of each edge (top, bottom, left, right) of the grid when
/// every child is filled blue: the child's colour where a child covers the stroke (Studio paints a frame's
/// stroke under its children), the stroke colour where none does.
fn studio_edge_colours(f: &GridFixture) -> [[u8; 3]; 4] {
    let mut doc = f.doc.clone();
    for &id in &f.items {
        doc.get_mut(id).unwrap().fills = vec![Paint::solid(Color::rgb(0, 0, 255))];
    }
    let pm = designos_render::render_node(&doc, f.grid, 1.0).expect("the scene renders");
    let (w, h) = (pm.width(), pm.height());
    [(w / 2, 0), (w / 2, h - 1), (0, h / 2), (w - 1, h / 2)].map(|(x, y)| {
        let c = pm.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue()]
    })
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
        let (accepted_drift, stroke_probe) = harness_expectation(f.name);
        out.push_str(&format!("    \"accepted_drift\": {accepted_drift},\n    \"stroke_probe\": {stroke_probe},\n"));
        if stroke_probe {
            let [top, bottom, left, right] = studio_edge_colours(f);
            out.push_str(&format!("    \"studio_edges\": {{\"top\": {top:?}, \"bottom\": {bottom:?}, \"left\": {left:?}, \"right\": {right:?}}},\n"));
        }
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

#[test]
fn s1_stretch_child_hug_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s1_stretch_child_hug");
}

#[test]
fn s2_empty_hug_column_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s2_empty_hug_column");
}

#[test]
fn s3_hug_grid_max_w_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s3_hug_grid_max_w");
}

#[test]
fn s5_hug_height_min_h_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s5_hug_height_min_h");
}

#[test]
fn s5b_hug_height_min_h_flex_auto_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s5b_hug_height_min_h_flex_auto");
}

#[test]
fn s7_explicit_rows_hug_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s7_explicit_rows_hug");
}

#[test]
fn n1_explicit_hug_rows_flex_auto_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n1_explicit_hug_rows_flex_auto");
}

#[test]
fn s9_auto_tracks_flex_unequal_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s9_auto_tracks_flex_unequal");
}

#[test]
fn n1b_explicit_hug_rows_flex_auto_overflow_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n1b_explicit_hug_rows_flex_auto_overflow");
}

#[test]
fn n2_hug_equal_flex_two_empty_hug_cols_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n2_hug_equal_flex_two_empty_hug_cols");
}

#[test]
fn n2b_hug_equal_flex_one_empty_hug_col_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n2b_hug_equal_flex_one_empty_hug_col");
}

#[test]
fn n2c_hug_equal_flex_fill_only_hug_col_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n2c_hug_equal_flex_fill_only_hug_col");
}

#[test]
fn n6_stroke_v0_unpadded_hug_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n6_stroke_v0_unpadded_hug");
}

#[test]
fn n6c_stroke_v0_unpadded_fixed_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n6c_stroke_v0_unpadded_fixed");
}

#[test]
fn c7_single_hug_col_min_w_fill_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c7_single_hug_col_min_w_fill");
}

#[test]
fn c1_stroke_v0_center_unpadded_hug_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c1_stroke_v0_center_unpadded_hug");
}

#[test]
fn c2_stroke_v0_inside_sides_unpadded_hug_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c2_stroke_v0_inside_sides_unpadded_hug");
}

#[test]
fn n6d_stroke_v1_padded_fixed_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n6d_stroke_v1_padded_fixed");
}

#[test]
fn c3_rows0_fixed_row_tracks_flex_auto_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c3_rows0_fixed_row_tracks_flex_auto");
}

#[test]
fn n2d_fixed_fill_only_hug_col_css_and_tailwind() {
    assert_fixture_css_and_tailwind("n2d_fixed_fill_only_hug_col");
}

#[test]
fn s6_explicit_rows_flex_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s6_explicit_rows_flex");
}

#[test]
fn s8_auto_tracks_fixed_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s8_auto_tracks_fixed");
}

#[test]
fn s10_fixed_width_hug_tracks_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s10_fixed_width_hug_tracks");
}

#[test]
fn s6b_hug_height_explicit_rows_unequal_css_and_tailwind() {
    assert_fixture_css_and_tailwind("s6b_hug_height_explicit_rows_unequal");
}

#[test]
fn c8_component_grid_center_stroke_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c8_component_grid_center_stroke");
}

#[test]
fn c9_stroke_v1_inside_sides_padded_fixed_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c9_stroke_v1_inside_sides_padded_fixed");
}

#[test]
fn c10_stroke_v1_inside_sides_unpadded_fixed_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c10_stroke_v1_inside_sides_unpadded_fixed");
}

#[test]
fn stroke_v1_inside_sides_unpadded_dashed_css_and_tailwind() {
    assert_fixture_css_and_tailwind("stroke_v1_inside_sides_unpadded_dashed");
}

#[test]
fn stroke_v1_inside_sides_unpadded_gradient_css_and_tailwind() {
    assert_fixture_css_and_tailwind("stroke_v1_inside_sides_unpadded_gradient");
}

#[test]
fn stroke_v1_inside_sides_unpadded_bound_width_css_and_tailwind() {
    assert_fixture_css_and_tailwind("stroke_v1_inside_sides_unpadded_bound_width");
}

/// The pixel list of a CSS declaration such as `grid-template-rows: 40px 40px`, `None` when absent or
/// when any entry is not a pixel value.
fn px_list(css: &str, prop: &str) -> Option<Vec<f32>> {
    let value = css.lines().find_map(|l| l.strip_prefix(&format!("{prop}: ")))?.trim_end_matches(';');
    value.split(' ').map(|t| t.strip_suffix("px").and_then(|n| n.parse::<f32>().ok())).collect()
}

fn solver_inputs(f: &GridFixture) -> (AutoLayout, designos_doc::layout_solver::SizeLimits, Vec2, Vec<designos_doc::layout_solver::ChildIn>) {
    let n = f.doc.get(f.grid).unwrap();
    let al = n.kind.frame_props().and_then(|p| p.auto_layout.clone()).unwrap();
    let kids = n.children.iter().filter_map(|id| f.doc.flow_child_input(*id, true)).collect();
    (al.with_stroke(&n.stroke), designos_doc::layout_solver::SizeLimits::from(&n.layout_child), n.size, kids)
}

#[test]
fn exported_pixel_rows_equal_the_rows_the_solver_lays_out() {
    for name in ["c3_rows0_fixed_row_tracks_flex_auto", "s8_auto_tracks_fixed"] {
        let f = fixture_named(name);
        let (al, own, size, kids) = solver_inputs(&f);
        let solved = designos_doc::layout_solver::grid_row_heights(&al, size, own, &kids);
        let css = css_for(&f.doc, f.grid);
        assert_eq!(px_list(&css, "grid-template-rows"), Some(solved.clone()), "{name}: {css}");
        let tw = tailwind_for(&f.doc, f.grid);
        let want = format!("grid-rows-[{}]", solved.iter().map(|h| format!("{h}px")).collect::<Vec<_>>().join("_"));
        assert!(tw.contains(&want), "{name}: {want} missing in {tw}");
    }
}

#[test]
fn exported_hug_columns_equal_the_widths_the_solver_lays_out() {
    for (name, hug_columns) in [("n2d_fixed_fill_only_hug_col", vec![2usize]), ("s10_fixed_width_hug_tracks", vec![0, 1])] {
        let f = fixture_named(name);
        let (al, own, size, kids) = solver_inputs(&f);
        let solved = designos_doc::layout_solver::grid_column_widths(&al, size, own, &kids);
        let css = css_for(&f.doc, f.grid);
        let tracks = css.lines().find_map(|l| l.strip_prefix("grid-template-columns: ")).unwrap().trim_end_matches(';');
        let mut written = tracks.replace("repeat(2, minmax(0, 1fr))", "fr fr").split(' ').map(String::from).collect::<Vec<_>>();
        written.retain(|t| !t.is_empty());
        for c in hug_columns {
            assert_eq!(written[c], format!("{}px", solved[c]), "{name}: column {c} in {css}");
        }
    }
}

#[test]
fn exported_stroke_padding_plus_border_equals_the_inset_the_solver_reserves() {
    // c1: a Center stroke is an outline, so the whole half-weight inset is padding.
    let f = fixture_named("c1_stroke_v0_center_unpadded_hug");
    let n = f.doc.get(f.grid).unwrap();
    let reserved = n.kind.frame_props().and_then(|p| p.auto_layout.as_ref()).unwrap().effective_padding(&n.stroke);
    let css = css_for(&f.doc, f.grid);
    assert_eq!(px_list(&css, "padding"), Some(vec![reserved[0]]), "{css}");
    assert!(css.contains("outline: 2px solid") && !css.contains("border:"), "{css}");
    // c2: sided Inside strokes the solver reserves are the border, with no padding of their own.
    let f = fixture_named("c2_stroke_v0_inside_sides_unpadded_hug");
    let css = css_for(&f.doc, f.grid);
    assert!(css.contains("border-width: 4px 1px 2px 3px;") && !css.contains("padding:"), "{css}");
    // n6d: version 1 reserves nothing, so the padding is the stored one and the stroke is an outline.
    let f = fixture_named("n6d_stroke_v1_padded_fixed");
    let css = css_for(&f.doc, f.grid);
    assert_eq!(px_list(&css, "padding"), Some(vec![8.0]), "{css}");
    assert!(css.contains("outline-offset: -2px;"), "{css}");
}

#[test]
fn c10b_stroke_v1_inside_sides_unpadded_hug_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c10b_stroke_v1_inside_sides_unpadded_hug");
}

/// What Studio shows at the edges of the per-side stroke scenes: it paints the frame's stroke under the
/// children, so a child that touches an edge hides the stroke there. The harness compares Chrome with these.
#[test]
fn studio_hides_a_per_side_stroke_under_a_child_that_reaches_the_edge() {
    const STROKE: [u8; 3] = [0x33, 0x33, 0x33];
    const CHILD: [u8; 3] = [0, 0, 255];
    // c9: padding 8 keeps the 60px-tall cells off every edge band: all four show the stroke.
    assert_eq!(studio_edge_colours(&fixture_named("c9_stroke_v1_inside_sides_padded_fixed")), [STROKE; 4]);
    // c10: padding 0, 30px children in a 60px grid: they cover the top band only. The side probes sit at
    // mid height (y = 30), just below them, and the bottom band is free.
    assert_eq!(studio_edge_colours(&fixture_named("c10_stroke_v1_inside_sides_unpadded_fixed")), [CHILD, STROKE, STROKE, STROKE]);
    // c10b: the Hug twin is 30px tall, so a child reaches the bottom band too: Studio hides it there.
    assert_eq!(studio_edge_colours(&fixture_named("c10b_stroke_v1_inside_sides_unpadded_hug")), [CHILD; 4]);
}

#[test]
fn c12_row_span_over_content_sized_rows_css_and_tailwind() {
    assert_fixture_css_and_tailwind("c12_row_span_over_content_sized_rows");
}

#[test]
fn row_span_over_content_sized_rows_exports_the_solver_rows() {
    let f = fixture_named("c12_row_span_over_content_sized_rows");
    let (al, own, size, kids) = solver_inputs(&f);
    let solved = designos_doc::layout_solver::grid_row_heights(&al, size, own, &kids);
    assert_eq!(solved, vec![15.0, 15.0]);
    let css = css_for(&f.doc, f.grid);
    assert_eq!(px_list(&css, "grid-template-rows"), Some(solved), "{css}");
}

#[test]
fn one_column_flex_grid_keeps_minmax_0_1fr() {
    // 1-column Hugging grid with default Flex(1) track and min_w keeps repeat(1, minmax(0, 1fr)) / grid-cols-1.
    let al = AutoLayout { grid_columns: 1, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    doc.get_mut(grid).unwrap().layout_child.min_w = Some(200.0);
    add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, grid, 80.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(css.contains("grid-template-columns: repeat(1, minmax(0, 1fr));"), "{css}");
    assert!(tw.contains("grid-cols-1"), "{tw}");

    // 1-column Fixed grid with default Flex(1) track also keeps repeat(1, minmax(0, 1fr)) / grid-cols-1.
    let al_fixed = AutoLayout { grid_columns: 1, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Hug, ..Default::default() };
    let (mut doc_f, grid_f) = new_grid(al_fixed, Vec2::new(200.0, 500.0));
    add_item(&mut doc_f, grid_f, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc_f, grid_f, 80.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc_f.relayout_subtree(grid_f);
    let css_f = css_for(&doc_f, grid_f);
    let tw_f = tailwind_for(&doc_f, grid_f);
    assert!(css_f.contains("grid-template-columns: repeat(1, minmax(0, 1fr));"), "{css_f}");
    assert!(tw_f.contains("grid-cols-1"), "{tw_f}");
}

#[test]
fn mutation_r1_stroked_hug_grid_uses_effective_padding_for_column_tracks() {
    let al = AutoLayout {
        grid_columns: 3,
        layout_version: 0,
        stroke_included: true,
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    apply_inside_stroke_2px(&mut doc, grid);
    doc.get_mut(grid).unwrap().layout_child.min_w = Some(304.0);
    add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(css.contains("grid-template-columns: 133.333px 73.333px 93.333px;"), "{css}");
    assert!(tw.contains("grid-cols-[133.333px_73.333px_93.333px]"), "{tw}");
}

#[test]
fn mutation_r3_flex_zero_auto_tracks_packs_rows_at_top() {
    let al = AutoLayout {
        grid_columns: 2,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_auto_tracks: GridAutoTracks::Track(GridTrack::Flex(0.0)),
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    for _ in 0..4 {
        add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
    }
    doc.relayout_subtree(grid);
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(css.contains("align-content: start;"), "{css}");
    assert!(tw.contains("content-start"), "{tw}");
}

#[test]
fn mutation_m4_hugging_grid_min_w_distributes_extra_across_flex_columns() {
    let al = AutoLayout { grid_columns: 3, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    doc.get_mut(grid).unwrap().layout_child.min_w = Some(300.0);
    add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(css.contains("grid-template-columns: 133.333px 73.333px 93.333px;"), "{css}");
    assert!(tw.contains("grid-cols-[133.333px_73.333px_93.333px]"), "{tw}");
}

#[test]
fn mutation_m7_non_fill_child_width_wins_over_stale_wider_fill_child() {
    let al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let fill_col1 = add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    // Simulate a wider stored width on the Fill child in column 1; the non-Fill 40px child in
    // column 1 must still govern column 1's track width.
    doc.get_mut(fill_col1).unwrap().size.x = 90.0;
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(css.contains("grid-template-columns: 100px 40px;"), "{css}");
    assert!(tw.contains("grid-cols-[100px_40px]"), "{tw}");
}

#[test]
fn mutation_m11_fixed_height_grid_with_flex_auto_rows_omits_align_content_start() {
    let al = AutoLayout {
        grid_columns: 2,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_auto_tracks: GridAutoTracks::Track(GridTrack::Flex(1.0)),
        ..Default::default()
    };
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
    add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(!css.contains("align-content: start;"), "{css}");
    assert!(!tw.contains("content-start"), "{tw}");
}

#[test]
fn mutation_m13_fixed_width_grid_with_weighted_flex_columns_keeps_fractional_tracks() {
    let mut al = AutoLayout { grid_columns: 3, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
    al.grid_col_tracks[0] = GridTrack::Fixed(200.0);
    al.grid_col_tracks[1] = GridTrack::Flex(1.0);
    al.grid_col_tracks[2] = GridTrack::Flex(4.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 120.0));
    add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(css.contains("grid-template-columns: 200px minmax(0, 1fr) minmax(0, 4fr);"), "{css}");
    assert!(tw.contains("grid-cols-[200px_minmax(0,1fr)_minmax(0,4fr)]"), "{tw}");
}

#[test]
fn mutation_m14_flex_unit_tolerance_distinguishes_unequal_and_weighted_equal_columns() {
    // 60px vs 40px on Flex(1) columns -> unit diff is 20px (> 0.5, < 50.0): must emit pixel tracks.
    let al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() };
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    let css = css_for(&doc, grid);
    let tw = tailwind_for(&doc, grid);
    assert!(css.contains("grid-template-columns: 60px 40px;"), "{css}");
    assert!(tw.contains("grid-cols-[60px_40px]"), "{tw}");

    // 160px on Flex(2) and 80px on Flex(1) -> both have unit 80px: keeps fractional tracks.
    let mut al_w = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() };
    al_w.grid_col_tracks[0] = GridTrack::Flex(2.0);
    al_w.grid_col_tracks[1] = GridTrack::Flex(1.0);
    let (mut doc_w, grid_w) = new_grid(al_w, Vec2::new(500.0, 500.0));
    add_item(&mut doc_w, grid_w, 160.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    add_item(&mut doc_w, grid_w, 80.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc_w.relayout_subtree(grid_w);
    let css_w = css_for(&doc_w, grid_w);
    let tw_w = tailwind_for(&doc_w, grid_w);
    assert!(css_w.contains("grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);"), "{css_w}");
    assert!(tw_w.contains("grid-cols-[minmax(0,2fr)_minmax(0,1fr)]"), "{tw_w}");
}
