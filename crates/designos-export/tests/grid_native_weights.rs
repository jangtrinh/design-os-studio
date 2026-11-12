//! Native generators export a flexible grid track as the platform's flexible unit (a Compose weight
//! list, an Android `layout_columnWeight`, an equal-share SwiftUI cell, a UIKit proportional
//! constraint) and a fixed track as fixed size. Each test rebuilds the column widths from the emitted
//! text, at the design's base width, and compares them with the layout solver's.

use designos_doc::layout_solver::{grid_column_widths, ChildIn, SizeLimits};
use designos_doc::{AutoLayout, Color, Document, FrameProps, GridTrack, LayoutDirection, Node, NodeId, NodeKind, Paint, Rect, Sizing};
use designos_export::{android_xml_for, compose_for, swiftui_for, uikit_for};

const PAD: f32 = 10.0;
const GAP: f32 = 10.0;
const WIDTH: f32 = 400.0;

struct Scene {
    doc: Document,
    grid: NodeId,
}

fn scene(tracks: &[GridTrack], primary: Sizing, item_w: &[f32]) -> Scene {
    let mut al = AutoLayout {
        grid_columns: tracks.len() as u32,
        grid_rows: 0,
        item_spacing: GAP,
        counter_spacing: GAP,
        padding: [PAD; 4],
        primary_sizing: primary,
        counter_sizing: Sizing::Fixed,
        direction: LayoutDirection::Grid,
        ..Default::default()
    };
    for (i, t) in tracks.iter().enumerate() {
        al.grid_col_tracks[i] = *t;
    }
    let mut doc = Document::new("grid-native-weights");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, WIDTH, 120.0));
    frame.name = "GridFrame".into();
    frame.fills.clear();
    frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), ..Default::default() });
    let grid = doc.insert(page, frame).unwrap();
    for (i, w) in item_w.iter().enumerate() {
        let cid = doc.alloc_id();
        let mut rect = Node::rectangle(cid, Rect::new(0.0, 0.0, *w, 30.0));
        rect.name = format!("Item {}", i + 1);
        rect.fills = vec![Paint::solid(Color::rgb(0x35, 0x82, 0xFF))];
        // Flex and Hug columns hold Fill items; a fixed column holds a fixed-size item.
        let fill = !matches!(tracks[i % tracks.len()], GridTrack::Fixed(_));
        rect.layout_child.horizontal = if fill { Sizing::Fill } else { Sizing::Fixed };
        rect.layout_child.vertical = Sizing::Fixed;
        doc.insert(grid, rect).unwrap();
    }
    doc.relayout_subtree(grid);
    Scene { doc, grid }
}

/// Fixed 80, Flex 2, Flex 1, Hug (50 wide item) in a 400 wide frame.
fn mixed(w2: f32, w1: f32) -> Scene {
    scene(&[GridTrack::Fixed(80.0), GridTrack::Flex(w2), GridTrack::Flex(w1), GridTrack::Hug], Sizing::Fixed, &[80.0, 20.0, 20.0, 50.0])
}

fn solver_widths(s: &Scene) -> Vec<f32> {
    let n = s.doc.get(s.grid).unwrap();
    let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    let kids: Vec<ChildIn> = n.children.iter().filter_map(|id| s.doc.flow_child_input(*id, true)).collect();
    grid_column_widths(&al.with_stroke(&n.stroke), n.size, SizeLimits::from(&n.layout_child), &kids)
}

fn numbers(list: &str, strip: &str) -> Vec<f32> {
    list.split(',').map(|t| t.trim().trim_end_matches(strip).parse::<f32>().unwrap_or_else(|_| panic!("number in {list:?}"))).collect()
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> &'a str {
    let a = text.find(open).unwrap_or_else(|| panic!("missing {open:?} in:\n{text}")) + open.len();
    let b = a + text[a..].find(close).unwrap_or_else(|| panic!("missing {close:?} after {open:?} in:\n{text}"));
    &text[a..b]
}

fn assert_close(got: &[f32], want: &[f32], what: &str, out: &str) {
    assert_eq!(got.len(), want.len(), "{what}: column count\n{out}");
    for (c, (g, w)) in got.iter().zip(want).enumerate() {
        assert!((g - w).abs() <= 0.5, "{what}: column {c} is {g}, solver {w} (all {got:?} vs {want:?})\n{out}");
    }
}

fn compose_widths(out: &str, n_cols: usize) -> Vec<f32> {
    assert!(out.contains(".coerceAtLeast(0)"), "compose unit must be coerced to >= 0:\n{out}");
    assert!(out.contains("+ 0.5f).toInt()"), "compose column share must round instead of truncating:\n{out}");
    let fixed = numbers(between(out, "val fixed = listOf(", ")"), ".dp");
    let weights = numbers(between(out, "val weights = listOf(", ")"), "f");
    assert_eq!(fixed.len(), n_cols);
    let gaps = (n_cols - 1) as f32;
    let free = (WIDTH - 2.0 * PAD - GAP * gaps - fixed.iter().sum::<f32>()).max(0.0);
    let total: f32 = weights.iter().sum();
    let unit = free / total;
    fixed.iter().zip(&weights).map(|(&f, &w)| (f as i32 + (w * unit + 0.5) as i32) as f32).collect()
}

#[test]
fn compose_flex_tracks_are_weights_and_fixed_tracks_are_fixed() {
    let s = mixed(2.0, 1.0);
    let out = compose_for(&s.doc, s.grid);
    assert!(out.contains("val weights = listOf(0f, 2f, 1f, 0f)"), "{out}");
    assert!(out.contains("val fixed = listOf(80.dp, 0.dp, 0.dp, 50.dp)"), "{out}");
    assert_close(&compose_widths(&out, 4), &solver_widths(&s), "compose", &out);
    assert!(out.contains("// Hug column 4 is exported as its solved 50.dp"), "{out}");
}

#[test]
fn compose_hugging_grid_keeps_solved_pixels() {
    let s = scene(&[GridTrack::Flex(1.0), GridTrack::Flex(1.0)], Sizing::Hug, &[100.0, 100.0, 100.0, 100.0]);
    let out = compose_for(&s.doc, s.grid);
    assert!(!out.contains("val weights"), "{out}");
    assert!(out.contains("listOf("), "{out}");
}

fn android_columns(xml: &str, n_cols: usize) -> Vec<(String, Option<f32>)> {
    let mut cols: Vec<Option<(String, Option<f32>)>> = vec![None; n_cols];
    for tag in xml.split('<').skip(1) {
        let head = tag.split('>').next().unwrap();
        let attr = |name: &str| head.split(&format!("android:{name}=\"")).nth(1).map(|r| r.split('"').next().unwrap().to_string());
        if attr("layout_row").is_none() {
            continue;
        }
        let w = attr("layout_width").unwrap();
        let weight = attr("layout_columnWeight").map(|v| v.parse::<f32>().unwrap());
        if head.starts_with("FrameLayout") && (w == "0dp" || weight.is_some()) {
            assert_eq!(
                attr("layout_gravity").as_deref(),
                Some("fill_horizontal"),
                "weighted Android cell FrameLayout must have android:layout_gravity=\"fill_horizontal\":\n{xml}"
            );
        }
        if attr("layout_columnSpan").is_some() {
            assert!(weight.is_none(), "spanning Android cell FrameLayout must not publish layout_columnWeight:\n{xml}");
            continue;
        }
        let Some(col) = attr("layout_column") else { continue };
        // The column-filler `Space` has height 0dp and sits in row 0; span-1 cells carry the width too.
        let c: usize = col.parse().unwrap();
        if cols[c].is_none() {
            cols[c] = Some((w, weight));
        }
    }
    cols.into_iter().enumerate().map(|(c, v)| v.unwrap_or_else(|| panic!("no cell or space in column {c}:\n{xml}"))).collect()
}

#[test]
fn android_flex_tracks_are_column_weights_and_fixed_tracks_are_dp() {
    let s = mixed(2.0, 1.0);
    let xml = android_xml_for(&s.doc, s.grid);
    let cols = android_columns(&xml, 4);
    assert_eq!(cols[0], ("80dp".into(), None), "{xml}");
    assert_eq!(cols[1], ("0dp".into(), Some(2.0)), "{xml}");
    assert_eq!(cols[2], ("0dp".into(), Some(1.0)), "{xml}");
    assert_eq!(cols[3], ("50dp".into(), None), "{xml}");
    // GridLayout shares the width left after fixed columns and margins by weight.
    let fixed: f32 = cols.iter().filter(|c| c.1.is_none()).map(|c| c.0.trim_end_matches("dp").parse::<f32>().unwrap()).sum();
    let free = WIDTH - 2.0 * PAD - GAP * 3.0 - fixed;
    let total: f32 = cols.iter().filter_map(|c| c.1).sum();
    let widths: Vec<f32> = cols.iter().map(|c| c.1.map_or_else(|| c.0.trim_end_matches("dp").parse().unwrap(), |w| free * w / total)).collect();
    assert_close(&widths, &solver_widths(&s), "android", &xml);
}

#[test]
fn android_hugging_grid_keeps_solved_pixels() {
    let s = scene(&[GridTrack::Flex(1.0), GridTrack::Flex(1.0)], Sizing::Hug, &[100.0, 100.0, 100.0, 100.0]);
    let xml = android_xml_for(&s.doc, s.grid);
    assert!(!xml.contains("columnWeight"), "{xml}");
}

#[test]
fn swiftui_equal_flex_tracks_share_the_free_width() {
    let s = scene(&[GridTrack::Fixed(80.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0), GridTrack::Hug], Sizing::Fixed, &[80.0, 20.0, 20.0, 50.0]);
    let out = swiftui_for(&s.doc, s.grid);
    assert_eq!(out.matches("minWidth: 0, maxWidth: .infinity").count(), 2, "{out}");
    assert!(out.contains(".frame(width: 80, height: 30, alignment: .topLeading)"), "{out}");
    assert!(out.contains(".frame(width: 50, height: 30, alignment: .topLeading)"), "{out}");
    let solver = solver_widths(&s);
    let share = (WIDTH - 2.0 * PAD - GAP * 3.0 - 80.0 - 50.0) / 2.0;
    assert_close(&[80.0, share, share, 50.0], &solver, "swiftui", &out);
}

/// The width of every cell and hole of a weighted SwiftUI grid, in order, evaluated at `width`.
fn swiftui_weighted_cell_widths(out: &str, width: f32, pad: f32) -> Vec<f32> {
    assert!(out.contains("GeometryReader { geo in"), "unequal weights need a GeometryReader:\n{out}");
    let fixed_and_gaps: f32 = between(out, "let unit = max(0, geo.size.width - ", ")").parse().unwrap();
    let total: f32 = between(out, ") / ", "\n").trim().parse().unwrap();
    let unit = (width - 2.0 * pad - fixed_and_gaps).max(0.0) / total;
    let eval = |expr: &str| -> f32 {
        expr.split(" + ")
            .map(|t| match t.strip_prefix("unit * ") {
                Some(w) => unit * w.parse::<f32>().unwrap(),
                None => t.parse::<f32>().unwrap(),
            })
            .sum()
    };
    // Only the cells inside the `GeometryReader`; the frame modifiers after its closing brace are the grid's own.
    let inside = out.split("\n}\n").next().unwrap();
    inside
        .lines()
        .filter(|l| l.contains("alignment: ") || l.contains("Color.clear"))
        .filter_map(|l| l.find("frame(width: ").map(|i| &l[i + "frame(width: ".len()..]))
        .map(|rest| eval(rest.split(", height:").next().unwrap()))
        .collect()
}

#[test]
fn swiftui_unequal_flex_tracks_share_the_free_width_by_weight() {
    let s = mixed(2.0, 1.0);
    let out = swiftui_for(&s.doc, s.grid);
    assert!(!out.contains("minWidth:"), "{out}");
    assert!(!out.contains("unequal column weights"), "{out}");
    let widths = swiftui_weighted_cell_widths(&out, WIDTH, PAD);
    assert_close(&widths, &solver_widths(&s), "swiftui", &out);
}

#[test]
fn swiftui_unequal_weights_keep_spans_and_holes() {
    // Fixed 100 + Flex 1 + Flex 2 in 400: free width 300 -> columns 100, 100, 200.
    let tracks = [GridTrack::Fixed(100.0), GridTrack::Flex(1.0), GridTrack::Flex(2.0)];
    let spanning = custom_grid_scene(&tracks, 400.0, 0.0, 0.0, &[(200.0, Sizing::Fill, Some(0), 2), (200.0, Sizing::Fill, Some(2), 1)]);
    let out = swiftui_for(&spanning.doc, spanning.grid);
    assert!(out.contains(".gridCellColumns(2)"), "{out}");
    assert_close(&swiftui_weighted_cell_widths(&out, 400.0, 0.0), &[200.0, 200.0], "span", &out);

    // The same tracks with nothing in the 1fr column: a hole keeps its share.
    let holey = custom_grid_scene(&tracks, 400.0, 0.0, 0.0, &[(100.0, Sizing::Fixed, Some(0), 1), (200.0, Sizing::Fill, Some(2), 1)]);
    let out = swiftui_for(&holey.doc, holey.grid);
    assert!(out.contains("Color.clear.frame(width: unit * 1, height: 30)"), "{out}");
    assert_close(&swiftui_weighted_cell_widths(&out, 400.0, 0.0), &[100.0, 100.0, 200.0], "hole", &out);
}

/// The weighted SwiftUI export compiles against the macOS SDK. Skipped where `xcrun` or the SDK is missing.
#[test]
fn swiftui_weighted_grid_typechecks_with_swiftc() {
    let Ok(sdk) = std::process::Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]).output() else {
        eprintln!("swiftui typecheck skipped: xcrun is not installed");
        return;
    };
    if !sdk.status.success() {
        eprintln!("swiftui typecheck skipped: no macosx SDK");
        return;
    }
    let sdk = String::from_utf8_lossy(&sdk.stdout).trim().to_string();
    let dir = std::env::temp_dir().join(format!("designos-swiftui-weights-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tracks = [GridTrack::Fixed(100.0), GridTrack::Flex(1.0), GridTrack::Flex(2.0)];
    let scenes = [mixed(2.0, 1.0), custom_grid_scene(&tracks, 400.0, 0.0, 0.0, &[(200.0, Sizing::Fill, Some(0), 2), (200.0, Sizing::Fill, Some(2), 1)])];
    for (i, s) in scenes.iter().enumerate() {
        let body: String = swiftui_for(&s.doc, s.grid).lines().map(|l| format!("        {l}\n")).collect();
        let source = format!("import SwiftUI\n\nstruct Weighted: View {{\n    var body: some View {{\n{body}    }}\n}}\n");
        let path = dir.join(format!("weighted_grid_{i}.swift"));
        std::fs::write(&path, &source).unwrap();
        let out = std::process::Command::new("swiftc")
            .args(["-typecheck", "-sdk", &sdk, "-target", "arm64-apple-macos13.0"])
            .arg(&path)
            .output()
            .expect("run swiftc");
        assert!(out.status.success(), "swiftc rejected the SwiftUI export:\n{}\n{source}", String::from_utf8_lossy(&out.stderr));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn uikit_flex_tracks_are_proportional_constraints_and_fixed_tracks_are_constants() {
    let s = mixed(2.0, 1.0);
    let out = uikit_for(&s.doc, s.grid);
    let unit_line =
        out.lines().find(|l| l.contains("colUnit.widthAnchor.constraint(equalTo: ") && l.contains("multiplier: 1.0 / ")).unwrap_or_else(|| panic!("{out}"));
    let total: f32 = between(unit_line, "multiplier: 1.0 / ", ",").trim().parse().unwrap();
    let minus: f32 = between(unit_line, "constant: -", " / ").trim().parse().unwrap();
    let unit = WIDTH / total - minus / total;
    let mut widths = Vec::new();
    for line in out.lines().filter(|l| l.contains("widthAnchor.constraint(equalTo: colUnit.widthAnchor")) {
        if line.contains("colUnit.widthAnchor.constraint") && line.trim_start().starts_with("colUnit") {
            continue;
        }
        let m: f32 = between(line, "multiplier: ", ",").trim().parse().unwrap();
        let px: f32 = between(line, "constant: ", ")").trim().parse().unwrap();
        widths.push(unit * m + px);
    }
    let want = solver_widths(&s);
    // Every flexible column (2 and 1 units) appears as a proportional width.
    for flex in &want[1..3] {
        assert!(widths.iter().any(|w| (w - flex).abs() <= 0.5), "no proportional cell for {flex} in {widths:?}\n{out}");
    }
    for fixed in [80.0, 50.0] {
        assert!(out.contains(&format!("widthAnchor.constraint(equalToConstant: {fixed})")), "{out}");
    }
}

#[test]
fn uikit_hugging_grid_keeps_solved_pixels() {
    let s = scene(&[GridTrack::Flex(1.0), GridTrack::Flex(1.0)], Sizing::Hug, &[100.0, 100.0, 100.0, 100.0]);
    let out = uikit_for(&s.doc, s.grid);
    assert!(!out.contains("colUnit"), "{out}");
}

/// The weighted UIKit export compiles against the iOS SDK. Skipped where `xcrun` or the SDK is missing.
#[test]
fn uikit_weighted_grid_typechecks_with_swiftc() {
    let Ok(sdk) = std::process::Command::new("xcrun").args(["--sdk", "iphonesimulator", "--show-sdk-path"]).output() else {
        eprintln!("uikit typecheck skipped: xcrun is not installed");
        return;
    };
    if !sdk.status.success() {
        eprintln!("uikit typecheck skipped: no iphonesimulator SDK");
        return;
    }
    let sdk = String::from_utf8_lossy(&sdk.stdout).trim().to_string();
    let dir = std::env::temp_dir().join(format!("designos-uikit-weights-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let s = mixed(2.0, 1.0);
    let source = format!("import UIKit\nfunc build() -> UIView {{\n{}\nreturn gridFrame\n}}\n", uikit_for(&s.doc, s.grid));
    let path = dir.join("weighted_grid.swift");
    std::fs::write(&path, &source).unwrap();
    let out = std::process::Command::new("swiftc")
        .args(["-typecheck", "-sdk", &sdk, "-target", "arm64-apple-ios17.0-simulator"])
        .arg(&path)
        .output()
        .expect("run swiftc");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "swiftc rejected the UIKit export:\n{}\n{source}", String::from_utf8_lossy(&out.stderr));
}

fn custom_grid_scene(tracks: &[GridTrack], width: f32, pad: f32, gap: f32, items: &[(f32, Sizing, Option<u16>, u16)]) -> Scene {
    let mut al = AutoLayout {
        grid_columns: tracks.len() as u32,
        grid_rows: 0,
        item_spacing: gap,
        counter_spacing: gap,
        padding: [pad; 4],
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        direction: LayoutDirection::Grid,
        ..Default::default()
    };
    for (i, t) in tracks.iter().enumerate() {
        al.grid_col_tracks[i] = *t;
    }
    let mut doc = Document::new("grid-native-weights-custom");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, width, 120.0));
    frame.name = "GridFrame".into();
    frame.fills.clear();
    frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), ..Default::default() });
    let grid = doc.insert(page, frame).unwrap();
    for (i, &(w, horiz, col, col_span)) in items.iter().enumerate() {
        let cid = doc.alloc_id();
        let mut rect = Node::rectangle(cid, Rect::new(0.0, 0.0, w, 30.0));
        rect.name = format!("Item {}", i + 1);
        rect.fills = vec![Paint::solid(Color::rgb(0x35, 0x82, 0xFF))];
        rect.layout_child.horizontal = horiz;
        rect.layout_child.vertical = Sizing::Fixed;
        rect.layout_child.grid_column = col;
        rect.layout_child.col_span = col_span;
        doc.insert(grid, rect).unwrap();
    }
    doc.relayout_subtree(grid);
    Scene { doc, grid }
}

#[test]
fn android_weighted_cells_fill_horizontally_and_keep_inner_child_gravity() {
    let mut s =
        custom_grid_scene(&[GridTrack::Flex(1.0), GridTrack::Flex(1.0)], 400.0, 0.0, 0.0, &[(200.0, Sizing::Fill, None, 1), (120.0, Sizing::Fixed, None, 1)]);
    let second = s.doc.get(s.grid).unwrap().children[1];
    s.doc.get_mut(second).unwrap().layout_child.grid_align_h = designos_doc::GridChildAlign::Center;
    s.doc.get_mut(second).unwrap().layout_child.grid_align_v = designos_doc::GridChildAlign::End;
    s.doc.relayout_subtree(s.grid);

    let xml = android_xml_for(&s.doc, s.grid);
    let cols = android_columns(&xml, 2);
    assert_eq!(cols, vec![("0dp".into(), Some(1.0)), ("0dp".into(), Some(1.0))], "{xml}");
    for col in [0, 1] {
        let cell_tag = format!(
            "<FrameLayout\n        android:layout_width=\"0dp\"\n        android:layout_height=\"30dp\"\n        android:layout_row=\"0\"\n        android:layout_column=\"{col}\"\n        android:layout_columnWeight=\"1\"\n        android:layout_gravity=\"fill_horizontal\">"
        );
        assert!(xml.contains(&cell_tag), "missing weighted cell FrameLayout for col {col}:\n{xml}");
    }
    assert!(!xml.contains("android:layout_gravity=\"fill\""), "cell must use fill_horizontal so layout_height stays:\n{xml}");
    assert!(xml.contains("android:layout_gravity=\"top|start\""), "first inner child must keep its top|start gravity:\n{xml}");
    assert!(xml.contains("android:layout_gravity=\"bottom|center_horizontal\""), "second inner child must keep its alignment gravity:\n{xml}");
}

#[test]
fn android_spanning_cell_keeps_column_weights_and_omits_span_weight() {
    // 1. Flex(2) + Flex(1) with one cell spanning both -> no weight 3 anywhere, columns keep 2 and 1.
    let s1 = custom_grid_scene(&[GridTrack::Flex(2.0), GridTrack::Flex(1.0)], 300.0, 0.0, 0.0, &[(300.0, Sizing::Fill, Some(0), 2)]);
    let xml1 = android_xml_for(&s1.doc, s1.grid);
    assert!(!xml1.contains("android:layout_columnWeight=\"3\""), "spanning cell must not sum weights to 3:\n{xml1}");
    let cols1 = android_columns(&xml1, 2);
    assert_eq!(cols1, vec![("0dp".into(), Some(2.0)), ("0dp".into(), Some(1.0))], "{xml1}");
    assert!(
        xml1.contains(
            "<FrameLayout\n        android:layout_width=\"0dp\"\n        android:layout_height=\"30dp\"\n        android:layout_row=\"0\"\n        android:layout_column=\"0\"\n        android:layout_columnSpan=\"2\"\n        android:layout_gravity=\"fill_horizontal\">"
        ),
        "spanning cell over Flex(2)+Flex(1) must be 0dp + layout_columnSpan + fill_horizontal with no columnWeight:\n{xml1}"
    );

    // 2. Fixed(100) + Flex(1) with one cell spanning both -> fixed column carries no weight.
    let s2 = custom_grid_scene(&[GridTrack::Fixed(100.0), GridTrack::Flex(1.0)], 300.0, 0.0, 0.0, &[(300.0, Sizing::Fill, Some(0), 2)]);
    let xml2 = android_xml_for(&s2.doc, s2.grid);
    let cols2 = android_columns(&xml2, 2);
    assert_eq!(cols2, vec![("100dp".into(), None), ("0dp".into(), Some(1.0))], "{xml2}");
    assert_eq!(xml2.matches("android:layout_columnWeight=").count(), 1, "only the Flex(1) column Space may carry a weight:\n{xml2}");
    assert!(
        xml2.contains(
            "<FrameLayout\n        android:layout_width=\"0dp\"\n        android:layout_height=\"30dp\"\n        android:layout_row=\"0\"\n        android:layout_column=\"0\"\n        android:layout_columnSpan=\"2\"\n        android:layout_gravity=\"fill_horizontal\">"
        ),
        "spanning cell over Fixed(100)+Flex(1) must be 0dp + layout_columnSpan + fill_horizontal with no columnWeight:\n{xml2}"
    );
}

#[test]
fn swiftui_span_covering_fixed_or_hug_column_keeps_solved_width() {
    // 1. Width 400, Fixed(100) + Flex(1) + Flex(1), item spanning cols 0-1 + item in col 2 -> span frame 250, col 2 150.
    let s_fixed = custom_grid_scene(
        &[GridTrack::Fixed(100.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0)],
        400.0,
        0.0,
        0.0,
        &[(250.0, Sizing::Fill, Some(0), 2), (150.0, Sizing::Fill, Some(2), 1)],
    );
    assert_close(&solver_widths(&s_fixed), &[100.0, 150.0, 150.0], "solver", "");
    let out_fixed = swiftui_for(&s_fixed.doc, s_fixed.grid);
    assert!(
        out_fixed.contains(".frame(width: 250, height: 30, alignment: .topLeading)\n            .gridCellColumns(2)"),
        "span covering Fixed(100)+Flex(1) must keep solved 250pt frame:\n{out_fixed}"
    );
    assert!(
        out_fixed.contains(".frame(minWidth: 0, maxWidth: .infinity, minHeight: 30, maxHeight: 30, alignment: .topLeading)"),
        "all-flex col 2 must stretch to fill remaining 150pt:\n{out_fixed}"
    );
    assert!(!out_fixed.contains("minWidth: 100"), "span over Fixed(100) must not emit minWidth: 100, maxWidth: .infinity:\n{out_fixed}");

    // 2. Width 400, Hug + Flex(1) + Flex(1), only a span over cols 0-1 + item in col 2 -> span keeps solved pixel frame,
    //    and the Hug comment does not claim the Hug column's standalone width was emitted on a cell.
    let s_hug = custom_grid_scene(
        &[GridTrack::Hug, GridTrack::Flex(1.0), GridTrack::Flex(1.0)],
        400.0,
        0.0,
        0.0,
        &[(200.0, Sizing::Fill, Some(0), 2), (150.0, Sizing::Fill, Some(2), 1)],
    );
    let hug_widths = solver_widths(&s_hug);
    let span_w = hug_widths[0] + hug_widths[1];
    let out_hug = swiftui_for(&s_hug.doc, s_hug.grid);
    assert!(
        out_hug.contains(&format!(".frame(width: {span_w}, height: 30, alignment: .topLeading)\n            .gridCellColumns(2)")),
        "span covering Hug+Flex(1) must keep solved {span_w}pt frame:\n{out_hug}"
    );
    assert!(
        !out_hug.contains("// Hug column 1 is exported as its solved width"),
        "Hug column covered only by a span never emits its standalone width:\n{out_hug}"
    );
    assert!(
        out_hug.contains("// Hug column 1 keeps its solved width 1 in its span"),
        "Hug comment should explain that the Hug column width is kept inside its span:\n{out_hug}"
    );
}

#[test]
fn compose_column_sizes_round_each_share_and_clamp_negative_unit() {
    // 3 equal Flex(1) columns in 200px free width (220 wide, pad 10, gap 0): each column is 66.667px.
    // Truncating `.toInt()` yields 66 (0.667px error > 0.5); rounding `+ 0.5f).toInt()` yields 67 (0.333px error <= 0.5).
    let s = custom_grid_scene(
        &[GridTrack::Flex(1.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0)],
        220.0,
        10.0,
        0.0,
        &[(50.0, Sizing::Fill, None, 1), (50.0, Sizing::Fill, None, 1), (50.0, Sizing::Fill, None, 1)],
    );
    let out = compose_for(&s.doc, s.grid);
    assert!(out.contains(".coerceAtLeast(0)"), "{out}");
    assert!(out.contains("+ 0.5f).toInt()"), "{out}");
    let fixed = numbers(between(&out, "val fixed = listOf(", ")"), ".dp");
    let weights = numbers(between(&out, "val weights = listOf(", ")"), "f");
    let free = (220.0 - 20.0 - fixed.iter().sum::<f32>()).max(0.0);
    let unit = free / weights.iter().sum::<f32>();
    let got: Vec<f32> = fixed.iter().zip(&weights).map(|(&f, &w)| (f as i32 + (w * unit + 0.5) as i32) as f32).collect();
    assert_close(&got, &solver_widths(&s), "compose rounded 3-col", &out);

    // Narrower than fixed tracks: unit is clamped to 0 so flex columns become 0, not negative.
    let narrow_free = (50.0f32 - 130.0f32).max(0.0);
    let narrow_unit = narrow_free / 3.0;
    assert_eq!((2.0 * narrow_unit + 0.5) as i32, 0);
}

#[test]
fn solver_floor_on_flex_columns_keeps_solved_pixels_in_all_generators() {
    // Width 200, Fixed(200) + Flex(1) + Flex(1): free width is 0, so the solver floors both flex columns to 1px.
    // Weight share `weight * free / totalWeight` is 0 != 1, so generators must keep solved pixels.
    let s = custom_grid_scene(
        &[GridTrack::Fixed(200.0), GridTrack::Flex(1.0), GridTrack::Flex(1.0)],
        200.0,
        0.0,
        0.0,
        &[(200.0, Sizing::Fixed, None, 1), (10.0, Sizing::Fill, None, 1), (10.0, Sizing::Fill, None, 1)],
    );
    assert_eq!(solver_widths(&s), vec![200.0, 1.0, 1.0]);

    let compose = compose_for(&s.doc, s.grid);
    assert!(!compose.contains("val weights"), "Compose must keep pixels when 1px floor changed flex widths:\n{compose}");
    assert!(compose.contains("listOf(200.dp, 1.dp, 1.dp)"), "{compose}");

    let xml = android_xml_for(&s.doc, s.grid);
    assert!(!xml.contains("columnWeight"), "Android must keep pixels when 1px floor changed flex widths:\n{xml}");
    assert!(xml.contains("android:layout_width=\"1dp\""), "{xml}");

    let swift = swiftui_for(&s.doc, s.grid);
    assert!(!swift.contains("minWidth:"), "SwiftUI must keep pixels when 1px floor changed flex widths:\n{swift}");
    assert!(swift.contains(".frame(width: 1, height: 30, alignment: .topLeading)"), "{swift}");

    let uikit = uikit_for(&s.doc, s.grid);
    assert!(!uikit.contains("colUnit"), "UIKit must keep pixels when 1px floor changed flex widths:\n{uikit}");
    assert!(uikit.contains("widthAnchor.constraint(equalToConstant: 1)"), "{uikit}");
}
