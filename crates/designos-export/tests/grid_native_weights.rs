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
    let fixed = numbers(between(out, "val fixed = listOf(", ")"), ".dp");
    let weights = numbers(between(out, "val weights = listOf(", ")"), "f");
    assert_eq!(fixed.len(), n_cols);
    let gaps = (n_cols - 1) as f32;
    let free = WIDTH - 2.0 * PAD - GAP * gaps - fixed.iter().sum::<f32>();
    let total: f32 = weights.iter().sum();
    fixed.iter().zip(&weights).map(|(f, w)| f + w * free / total).collect()
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
        let (Some(col), None) = (attr("layout_column"), attr("layout_columnSpan")) else { continue };
        if attr("layout_row").is_none() {
            continue;
        }
        let w = attr("layout_width").unwrap();
        // The column-filler `Space` has height 0dp and sits in row 0; cells carry the width too.
        let weight = attr("layout_columnWeight").map(|v| v.parse::<f32>().unwrap());
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

#[test]
fn swiftui_unequal_weights_fall_back_to_solved_pixels_with_a_comment() {
    // `Grid` has no per-column weight, so unequal flex weights keep the solver's widths.
    let s = mixed(2.0, 1.0);
    let out = swiftui_for(&s.doc, s.grid);
    assert!(!out.contains("minWidth:"), "{out}");
    for w in solver_widths(&s) {
        let ws = format!("{}", (w * 1000.0).round() / 1000.0);
        assert!(out.contains(&format!("width: {ws}, height:")), "missing width {ws}:\n{out}");
    }
    assert!(out.contains("unequal column weights"), "{out}");
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
