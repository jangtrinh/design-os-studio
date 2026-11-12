//! Chrome-compared grid scenes for strokes (Inside, Center, per-side weights, v1) and for track
//! cases the exporter once derived itself instead of reading the solver (Fixed row tracks without
//! explicit rows, Fill in an `auto` column, `grid-auto-rows`, Hug columns in a fixed-width grid).

use super::{add_item, new_grid, GridFixture};
use designos_doc::{
    AutoLayout, BlendMode, Color, GradientStop, GridAutoTracks, GridTrack, Node, NodeKind, Paint, PaintKind, Sizing, StrokeAlign, Variable, VariableCollection,
    VariableKind, VariableMode, VariableValue, Vec2,
};

fn stroke_2px(n: &mut Node, align: StrokeAlign) {
    n.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
    n.stroke.weight = 2.0;
    n.stroke.align = align;
}

fn base(cols: u32, gap: f32, primary: Sizing, counter: Sizing) -> AutoLayout {
    AutoLayout { grid_columns: cols, item_spacing: gap, counter_spacing: gap, primary_sizing: primary, counter_sizing: counter, ..Default::default() }
}

fn four_fill_items(doc: &mut designos_doc::Document, grid: designos_doc::NodeId) -> Vec<designos_doc::NodeId> {
    (0..4).map(|_| add_item(doc, grid, 50.0, 20.0, Sizing::Fill, Sizing::Fixed)).collect()
}

pub fn scene_c1_stroke_v0_center_unpadded_hug() -> GridFixture {
    let mut al = base(3, 0.0, Sizing::Hug, Sizing::Hug);
    al.layout_version = 0;
    al.stroke_included = true;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    stroke_2px(doc.get_mut(grid).unwrap(), StrokeAlign::Center);
    let a = add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "c1_stroke_v0_center_unpadded_hug", doc, grid, items: vec![a, b, c], expected_css: "display: grid;\nwidth: 202px;\nheight: 32px;\npadding: 1px;\nbox-sizing: border-box;\ngrid-template-columns: 100px 40px 60px;\nalign-content: start;\noutline: 2px solid #333;\noutline-offset: -1px;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_60px] content-start box-border p-px w-[202px] h-8 outline outline-2 outline-[#333333] outline-offset-[-1px]\">\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n  <div class=\"w-15 h-[30px]\"></div>\n</div>", }
}

pub fn scene_c2_stroke_v0_inside_sides_unpadded_hug() -> GridFixture {
    let mut al = base(3, 0.0, Sizing::Hug, Sizing::Hug);
    al.layout_version = 0;
    al.stroke_included = true;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Inside);
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    let a = add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "c2_stroke_v0_inside_sides_unpadded_hug", doc, grid, items: vec![a, b, c], expected_css: "display: grid;\nwidth: 204px;\nheight: 36px;\nbox-sizing: border-box;\ngrid-template-columns: 100px 40px 60px;\nalign-content: start;\nborder-width: 4px 1px 2px 3px;\nborder-style: solid;\nborder-color: #333;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_60px] content-start box-border w-[204px] h-9 border-t-4 border-r border-b-2 border-l-[3px] border-[#333333]\">\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n  <div class=\"w-15 h-[30px]\"></div>\n</div>", }
}

pub fn scene_n6d_stroke_v1_padded_fixed() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Hug);
    al.layout_version = 1;
    al.padding = [8.0; 4];
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 50.0));
    stroke_2px(doc.get_mut(grid).unwrap(), StrokeAlign::Inside);
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "n6d_stroke_v1_padded_fixed", doc, grid, items: vec![a, b], expected_css: "display: grid;\nwidth: 200px;\nheight: 46px;\npadding: 8px;\nbox-sizing: border-box;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\noutline: 2px solid #333;\noutline-offset: -2px;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start box-border p-2 w-[200px] h-[46px] outline outline-2 outline-[#333333] outline-offset-[-2px]\">\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>", }
}

pub fn scene_c3_rows0_fixed_row_tracks_flex_auto() -> GridFixture {
    let mut al = base(2, 10.0, Sizing::Fixed, Sizing::Fixed);
    al.grid_auto_tracks = GridAutoTracks::Track(GridTrack::Flex(1.0));
    al.grid_row_tracks[0] = GridTrack::Fixed(40.0);
    al.grid_row_tracks[1] = GridTrack::Fixed(40.0);
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    let items = four_fill_items(&mut doc, grid);
    doc.relayout_subtree(grid);
    GridFixture { name: "c3_rows0_fixed_row_tracks_flex_auto", doc, grid, items, expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: 40px 40px;\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 grid-rows-[40px_40px] content-start gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>", }
}

pub fn scene_n2d_fixed_fill_only_hug_col() -> GridFixture {
    let mut al = base(3, 0.0, Sizing::Fixed, Sizing::Hug);
    al.grid_col_tracks[2] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 30.0));
    let a = add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 30.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "n2d_fixed_fill_only_hug_col", doc, grid, items: vec![a, b, c], expected_css: "display: grid;\nwidth: 200px;\nheight: 30px;\ngrid-template-columns: repeat(2, minmax(0, 1fr)) 30px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[repeat(2,minmax(0,1fr))_30px] content-start w-[200px] h-[30px]\">\n  <div class=\"w-[50px] h-[30px]\"></div>\n  <div class=\"w-[50px] h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>", }
}

pub fn scene_s6_explicit_rows_flex() -> GridFixture {
    let mut al = base(2, 10.0, Sizing::Fixed, Sizing::Fixed);
    al.grid_rows = 2;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    let items = four_fill_items(&mut doc, grid);
    doc.relayout_subtree(grid);
    GridFixture { name: "s6_explicit_rows_flex", doc, grid, items, expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>", }
}

pub fn scene_s8_auto_tracks_fixed() -> GridFixture {
    let mut al = base(2, 10.0, Sizing::Fixed, Sizing::Fixed);
    al.grid_auto_tracks = GridAutoTracks::Track(GridTrack::Fixed(50.0));
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    let items = four_fill_items(&mut doc, grid);
    doc.relayout_subtree(grid);
    GridFixture { name: "s8_auto_tracks_fixed", doc, grid, items, expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: 50px 50px;\ngap: 10px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 grid-rows-[50px_50px] content-start gap-2.5 w-[200px] h-[200px]\">\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n  <div class=\"h-5\"></div>\n</div>", }
}

pub fn scene_s10_fixed_width_hug_tracks() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Hug);
    al.grid_col_tracks[0] = GridTrack::Hug;
    al.grid_col_tracks[1] = GridTrack::Hug;
    let (mut doc, grid) = new_grid(al, Vec2::new(300.0, 20.0));
    let a = add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 50.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "s10_fixed_width_hug_tracks", doc, grid, items: vec![a, b], expected_css: "display: grid;\nwidth: 300px;\nheight: 20px;\ngrid-template-columns: 50px 50px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-[50px_50px] content-start w-[300px] h-5\">\n  <div class=\"w-[50px] h-5\"></div>\n  <div class=\"w-[50px] h-5\"></div>\n</div>", }
}

pub fn scene_s6b_hug_height_explicit_rows_unequal() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Hug);
    al.grid_rows = 2;
    let (mut doc, grid) = new_grid(al, Vec2::new(300.0, 500.0));
    let a = add_item(&mut doc, grid, 30.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 30.0, 20.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 30.0, 50.0, Sizing::Fixed, Sizing::Fixed);
    let d = add_item(&mut doc, grid, 30.0, 50.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "s6b_hug_height_explicit_rows_unequal", doc, grid, items: vec![a, b, c, d], expected_css: "display: grid;\nwidth: 300px;\nheight: 70px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start w-[300px] h-[70px]\">\n  <div class=\"w-[30px] h-5\"></div>\n  <div class=\"w-[30px] h-5\"></div>\n  <div class=\"w-[30px] h-[50px]\"></div>\n  <div class=\"w-[30px] h-[50px]\"></div>\n</div>", }
}

/// The Hug twin of C10: the grid is exactly as tall as its children, so they reach the far-side stroke.
pub fn scene_c10b_stroke_v1_inside_sides_unpadded_hug() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Hug);
    al.layout_version = 1;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 60.0));
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Inside);
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "c10b_stroke_v1_inside_sides_unpadded_hug", doc, grid, items: vec![a, b], expected_css: "display: grid;\nwidth: 200px;\nheight: 30px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\nbox-shadow: inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start w-[200px] h-[30px] shadow-[inset_0px_4px_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]\">\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>" }
}

pub fn scene_c10_stroke_v1_inside_sides_unpadded_fixed() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Fixed);
    al.layout_version = 1;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 60.0));
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Inside);
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "c10_stroke_v1_inside_sides_unpadded_fixed", doc, grid, items: vec![a, b], expected_css: "display: grid;\nwidth: 200px;\nheight: 60px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\nbox-shadow: inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start w-[200px] h-15 shadow-[inset_0px_4px_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]\">\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>", }
}

/// A grid component (not a plain frame) with a Center stroke: the container kinds share one stroke model.
pub fn scene_c8_component_grid_center_stroke() -> GridFixture {
    let mut al = base(3, 0.0, Sizing::Hug, Sizing::Hug);
    al.layout_version = 0;
    al.stroke_included = true;
    let (mut doc, grid) = new_grid(al, Vec2::new(500.0, 500.0));
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Center);
    if let NodeKind::Frame(props) = std::mem::replace(&mut n.kind, NodeKind::Rectangle) {
        n.kind = NodeKind::Component(props);
    }
    let a = add_item(&mut doc, grid, 100.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 40.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 60.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "c8_component_grid_center_stroke", doc, grid, items: vec![a, b, c], expected_css: "display: grid;\nwidth: 202px;\nheight: 32px;\npadding: 1px;\nbox-sizing: border-box;\ngrid-template-columns: 100px 40px 60px;\nalign-content: start;\noutline: 2px solid #333;\noutline-offset: -1px;",
        expected_tailwind: "<div class=\"grid grid-cols-[100px_40px_60px] content-start box-border p-px w-[202px] h-8 outline outline-2 outline-[#333333] outline-offset-[-1px]\">\n  <div class=\"w-[100px] h-[30px]\"></div>\n  <div class=\"w-10 h-[30px]\"></div>\n  <div class=\"w-15 h-[30px]\"></div>\n</div>", }
}

/// Version 1 (the solver never insets for a stroke), Inside stroke with sides 4/1/2/3 and padding 8: the
/// stroke is inset shadows, which take no space, so the padding stays 8.
pub fn scene_c9_stroke_v1_inside_sides_padded_fixed() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Hug);
    al.layout_version = 1;
    al.padding = [8.0; 4];
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 60.0));
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Inside);
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture { name: "c9_stroke_v1_inside_sides_padded_fixed", doc, grid, items: vec![a, b], expected_css: "display: grid;\nwidth: 200px;\nheight: 46px;\npadding: 8px;\nbox-sizing: border-box;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\nbox-shadow: inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start box-border p-2 w-[200px] h-[46px] shadow-[inset_0px_4px_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]\">\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>", }
}

/// A child spans rows 0 and 1 and is taller (30) than the content of both rows (10 and 2). The solver
/// levels the rows to 15 each; CSS `auto` rows would split the extra unevenly (19 and 11).
pub fn scene_c12_row_span_over_content_sized_rows() -> GridFixture {
    let al = base(2, 0.0, Sizing::Fixed, Sizing::Fixed);
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 200.0));
    let a = add_item(&mut doc, grid, 50.0, 30.0, Sizing::Fixed, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 50.0, 10.0, Sizing::Fixed, Sizing::Fixed);
    let c = add_item(&mut doc, grid, 50.0, 2.0, Sizing::Fixed, Sizing::Fixed);
    let lc = &mut doc.get_mut(a).unwrap().layout_child;
    lc.grid_column = Some(0);
    lc.grid_row = Some(0);
    lc.row_span = 2;
    let lc = &mut doc.get_mut(b).unwrap().layout_child;
    lc.grid_column = Some(1);
    lc.grid_row = Some(0);
    let lc = &mut doc.get_mut(c).unwrap().layout_child;
    lc.grid_column = Some(1);
    lc.grid_row = Some(1);
    doc.relayout_subtree(grid);
    GridFixture { name: "c12_row_span_over_content_sized_rows", doc, grid, items: vec![a, b, c], expected_css: "display: grid;\nwidth: 200px;\nheight: 200px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\ngrid-template-rows: 15px 15px;\nalign-content: start;",
        expected_tailwind: "<div class=\"grid grid-cols-2 grid-rows-[15px_15px] content-start w-[200px] h-[200px]\">\n  <div class=\"col-start-1 row-start-1 row-span-2 w-[50px] h-[30px]\"></div>\n  <div class=\"col-start-2 row-start-1 w-[50px] h-2.5\"></div>\n  <div class=\"col-start-2 row-start-2 w-[50px] h-0.5\"></div>\n</div>" }
}

pub fn scene_stroke_v1_inside_sides_unpadded_dashed() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Fixed);
    al.layout_version = 1;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 60.0));
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Inside);
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    n.stroke.dashes = vec![4.0, 2.0];
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "stroke_v1_inside_sides_unpadded_dashed",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 200px;\nheight: 60px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\n/* CSS cannot draw a per-side dashed stroke without a border that would shift grid content; drawn as a solid inset box-shadow */\nbox-shadow: inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start w-[200px] h-15 shadow-[inset_0px_4px_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]\">\n  <!-- CSS cannot draw a per-side dashed stroke without a border that would shift grid content; drawn as a solid inset box-shadow -->\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>",
    }
}

pub fn scene_stroke_v1_inside_sides_unpadded_gradient() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Fixed);
    al.layout_version = 1;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 60.0));
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Inside);
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    n.stroke.paints = vec![Paint {
        kind: PaintKind::LinearGradient {
            stops: vec![GradientStop { pos: 0.0, color: Color::rgb(0x33, 0x33, 0x33) }, GradientStop { pos: 1.0, color: Color::rgb(0x99, 0x99, 0x99) }],
            handles: [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "stroke_v1_inside_sides_unpadded_gradient",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 200px;\nheight: 60px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\n/* CSS cannot draw a per-side gradient stroke without a border that would shift grid content; drawn as a solid inset box-shadow */\nbox-shadow: inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start w-[200px] h-15 shadow-[inset_0px_4px_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]\">\n  <!-- CSS cannot draw a per-side gradient stroke without a border that would shift grid content; drawn as a solid inset box-shadow -->\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>",
    }
}

pub fn scene_stroke_v1_inside_sides_unpadded_bound_width() -> GridFixture {
    let mut al = base(2, 0.0, Sizing::Fixed, Sizing::Fixed);
    al.layout_version = 1;
    let (mut doc, grid) = new_grid(al, Vec2::new(200.0, 60.0));
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Mode".into() }],
            default_mode: "m".into(),
            variables: vec!["w-top".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "w-top".into(),
        Variable {
            id: "w-top".into(),
            name: "Border/Top".into(),
            collection: "c".into(),
            kind: VariableKind::Number,
            values: [("m".to_string(), VariableValue::Number(4.0))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    let n = doc.get_mut(grid).unwrap();
    stroke_2px(n, StrokeAlign::Inside);
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    n.variables.bindings.insert("strokeTopWeight".into(), "w-top".into());
    let a = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    let b = add_item(&mut doc, grid, 10.0, 30.0, Sizing::Fill, Sizing::Fixed);
    doc.relayout_subtree(grid);
    GridFixture {
        name: "stroke_v1_inside_sides_unpadded_bound_width",
        doc,
        grid,
        items: vec![a, b],
        expected_css: "display: grid;\nwidth: 200px;\nheight: 60px;\ngrid-template-columns: repeat(2, minmax(0, 1fr));\nalign-content: start;\nbox-shadow: inset 0px var(--Border-Top, 4px) 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333;",
        expected_tailwind: "<div class=\"grid grid-cols-2 content-start w-[200px] h-15 shadow-[inset_0px_var(--Border-Top)_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]\">\n  <div class=\"h-[30px]\"></div>\n  <div class=\"h-[30px]\"></div>\n</div>",
    }
}
