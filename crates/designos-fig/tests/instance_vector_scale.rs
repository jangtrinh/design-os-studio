//! Regression tests for #1242: vectors inside a resized instance must use
//! scaled geometry (from `derivedSymbolData` when present, or scaled by
//! `instance_child_size / main_child_size` when absent) while preserving
//! `stroke.weight`.

use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::import_fig;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{en, figma_like_schema, fl, guid, matrix, msg, node, solid, st, vec2};

fn path_blob(cmds: &[(u8, &[f32])]) -> Vec<u8> {
    let mut b = Vec::new();
    for &(op, pts) in cmds {
        b.push(op);
        for &p in pts {
            b.extend(p.to_le_bytes());
        }
    }
    b
}

/// Closed stroke-outline polygon for the top chevron `(0, h) -> (w/2, 0) -> (w, h)` with stroke 1.5 (half = 0.75).
/// Bounds span `x: -0.75 .. w + 0.75`, `y: -0.75 .. h + 0.75`.
fn chevron_up_stroke_blob(w: f32, h: f32) -> Vec<u8> {
    let hw = w * 0.5;
    path_blob(&[(1, &[-0.75, h]), (2, &[hw, -0.75]), (2, &[w + 0.75, h]), (2, &[w - 0.75, h + 0.75]), (2, &[hw, 1.5]), (2, &[0.75, h + 0.75]), (0, &[])])
}

/// Closed stroke-outline polygon for the bottom chevron `(0, 0) -> (w/2, h) -> (w, 0)` with stroke 1.5 (half = 0.75).
/// Bounds span `x: -0.75 .. w + 0.75`, `y: -0.75 .. h + 0.75`.
fn chevron_down_stroke_blob(w: f32, h: f32) -> Vec<u8> {
    let hw = w * 0.5;
    path_blob(&[(1, &[-0.75, 0.0]), (2, &[0.75, -0.75]), (2, &[hw, h - 1.5]), (2, &[w - 0.75, -0.75]), (2, &[w + 0.75, 0.0]), (2, &[hw, h + 0.75]), (0, &[])])
}

/// Open 3-vertex centerline `vectorNetworkBlob`: `v0 -> v1 -> v2`.
fn centerline_network_blob(v0: (f32, f32), v1: (f32, f32), v2: (f32, f32)) -> Vec<u8> {
    let mut b = Vec::new();
    for cnt in [3u32, 2u32, 0u32] {
        b.extend(cnt.to_le_bytes());
    }
    for (x, y) in [v0, v1, v2] {
        b.extend(0u32.to_le_bytes());
        b.extend(x.to_le_bytes());
        b.extend(y.to_le_bytes());
    }
    for (a, e) in [(0u32, 1u32), (1u32, 2u32)] {
        b.extend(0u32.to_le_bytes());
        b.extend(a.to_le_bytes());
        b.extend(0.0f32.to_le_bytes());
        b.extend(0.0f32.to_le_bytes());
        b.extend(e.to_le_bytes());
        b.extend(0.0f32.to_le_bytes());
        b.extend(0.0f32.to_le_bytes());
    }
    b
}

fn path_of(guids: &[(u32, u32)]) -> V {
    msg(vec![("guids", V::Array(guids.iter().map(|g| guid(g.0, g.1)).collect()))])
}

fn ov(path: &[(u32, u32)], mut fields: Vec<(&str, V)>) -> V {
    let mut f = vec![("guidPath", path_of(path))];
    f.append(&mut fields);
    msg(f)
}

fn geo(rule: &str, blob: u32) -> V {
    V::Array(vec![msg(vec![("windingRule", en(rule)), ("commandsBlob", V::Uint(blob))])])
}

fn vec_data(blob: u32, nw: f32, nh: f32) -> V {
    msg(vec![("vectorNetworkBlob", V::Uint(blob)), ("normalizedSize", vec2(nw, nh))])
}

/// Builds a `.fig` message containing:
/// - `chevron-selector-vertical` (24×24 main component) with a `10×16` `vector` frame and two `10×5` `Icon`
///   open-path stroked vectors (`strokeWeight = 1.5`, `ROUND` cap/join, `SCALE` constraints, `overrideKey`s).
/// - `chevron-12px-derived`: 12×12 instance with `derivedSymbolData` carrying scaled `strokeGeometry`.
/// - `chevron-12px-no-derived`: 12×12 instance with no `derivedSymbolData` entry.
/// - `chevron-12px-size-only-derived`: 12×12 instance whose `derivedSymbolData` overrides `size` without `strokeGeometry`.
/// - `Button`: 100×40 auto-layout button component containing a nested 24×24 `ButtonIcon` instance.
/// - `Button-small-derived`: resized button instance where `derivedSymbolData` resizes the nested `ButtonIcon` to 12×12.
/// - `Button-scaled`: non-auto-layout button component (100×40) with a Scale-constrained 24×24 `ButtonIcon`, instantiated at 50×20 with no derived data.
/// - `chevron-24px-unchanged`: 24×24 instance at the main component's own size.
fn build_chevron_fixture_fig(pad_large: bool) -> Vec<u8> {
    let doc = node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]);
    let page =
        node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![("backgroundColor", st(vec![("r", fl(1.0)), ("g", fl(1.0)), ("b", fl(1.0)), ("a", fl(1.0))]))]);

    // Main component: 24x24 chevron-selector-vertical
    let main_comp = node((1, 100), (0, 1), "a", "SYMBOL", "chevron-selector-vertical", vec![("size", vec2(24.0, 24.0)), ("transform", matrix(0.0, 0.0))]);
    let vec_frame = node(
        (1, 101),
        (1, 100),
        "a",
        "FRAME",
        "vector",
        vec![
            ("overrideKey", guid(90, 101)),
            ("size", vec2(10.0, 16.0)),
            ("transform", matrix(7.0, 4.0)),
            ("horizontalConstraint", en("SCALE")),
            ("verticalConstraint", en("SCALE")),
        ],
    );
    let icon_top = node(
        (1, 102),
        (1, 101),
        "a",
        "VECTOR",
        "IconTop",
        vec![
            ("overrideKey", guid(90, 102)),
            ("size", vec2(10.0, 5.0)),
            ("transform", matrix(0.0, 0.0)),
            ("horizontalConstraint", en("SCALE")),
            ("verticalConstraint", en("SCALE")),
            ("strokePaints", V::Array(vec![solid(0.09, 0.09, 0.11)])),
            ("strokeWeight", fl(1.5)),
            ("strokeAlign", en("CENTER")),
            ("strokeCap", en("ROUND")),
            ("strokeJoin", en("ROUND")),
            ("vectorData", vec_data(4, 10.0, 5.0)),
            ("strokeGeometry", geo("NONZERO", 0)),
        ],
    );
    let icon_bot = node(
        (1, 103),
        (1, 101),
        "b",
        "VECTOR",
        "IconBottom",
        vec![
            ("overrideKey", guid(90, 103)),
            ("size", vec2(10.0, 5.0)),
            ("transform", matrix(0.0, 11.0)),
            ("horizontalConstraint", en("SCALE")),
            ("verticalConstraint", en("SCALE")),
            ("strokePaints", V::Array(vec![solid(0.09, 0.09, 0.11)])),
            ("strokeWeight", fl(1.5)),
            ("strokeAlign", en("CENTER")),
            ("strokeCap", en("ROUND")),
            ("strokeJoin", en("ROUND")),
            ("vectorData", vec_data(5, 10.0, 5.0)),
            ("strokeGeometry", geo("NONZERO", 1)),
        ],
    );

    // 1. Resized 12x12 instance with derivedSymbolData carrying scaled strokeGeometry (addressed by overrideKey).
    let inst_derived = node(
        (2, 1),
        (0, 1),
        "b",
        "INSTANCE",
        "chevron-12px-derived",
        vec![
            ("size", vec2(12.0, 12.0)),
            ("transform", matrix(40.0, 6.0)),
            ("symbolData", msg(vec![("symbolID", guid(1, 100))])),
            (
                "derivedSymbolData",
                V::Array(vec![
                    // Child entries intentionally placed before parent container entry to test order independence.
                    ov(&[(90, 102)], vec![("size", vec2(5.0, 2.5)), ("transform", matrix(0.0, 0.0)), ("strokeGeometry", geo("NONZERO", 2))]),
                    ov(&[(90, 103)], vec![("size", vec2(5.0, 2.5)), ("transform", matrix(0.0, 5.5)), ("strokeGeometry", geo("NONZERO", 3))]),
                    ov(&[(90, 101)], vec![("size", vec2(5.0, 8.0)), ("transform", matrix(3.5, 2.0))]),
                ]),
            ),
        ],
    );

    // 2a. Resized 12x12 instance with NO derivedSymbolData at all.
    let inst_no_derived = node(
        (2, 2),
        (0, 1),
        "c",
        "INSTANCE",
        "chevron-12px-no-derived",
        vec![("size", vec2(12.0, 12.0)), ("transform", matrix(64.0, 6.0)), ("symbolData", msg(vec![("symbolID", guid(1, 100))]))],
    );

    // 2b. Component whose `vector` frame has MIN constraints, and an instance where derivedSymbolData only supplies `size` (no strokeGeometry).
    let main_min_comp = node((1, 200), (0, 1), "d", "SYMBOL", "chevron-min-frame", vec![("size", vec2(24.0, 24.0)), ("transform", matrix(0.0, 40.0))]);
    let min_vec_frame = node(
        (1, 201),
        (1, 200),
        "a",
        "FRAME",
        "vector",
        vec![
            ("overrideKey", guid(90, 201)),
            ("size", vec2(10.0, 16.0)),
            ("transform", matrix(7.0, 4.0)),
            ("horizontalConstraint", en("MIN")),
            ("verticalConstraint", en("MIN")),
        ],
    );
    let min_icon_top = node(
        (1, 202),
        (1, 201),
        "a",
        "VECTOR",
        "IconTop",
        vec![
            ("overrideKey", guid(90, 202)),
            ("size", vec2(10.0, 5.0)),
            ("transform", matrix(0.0, 0.0)),
            ("horizontalConstraint", en("MIN")),
            ("verticalConstraint", en("MIN")),
            ("strokePaints", V::Array(vec![solid(0.09, 0.09, 0.11)])),
            ("strokeWeight", fl(1.5)),
            ("strokeAlign", en("CENTER")),
            ("strokeCap", en("ROUND")),
            ("strokeJoin", en("ROUND")),
            ("vectorData", vec_data(4, 10.0, 5.0)),
            ("strokeGeometry", geo("NONZERO", 0)),
        ],
    );
    let inst_size_only_derived = node(
        (2, 3),
        (0, 1),
        "e",
        "INSTANCE",
        "chevron-12px-size-only-derived",
        vec![
            ("size", vec2(12.0, 12.0)),
            ("transform", matrix(40.0, 46.0)),
            ("symbolData", msg(vec![("symbolID", guid(1, 200))])),
            (
                "derivedSymbolData",
                V::Array(vec![
                    ov(&[(90, 201)], vec![("size", vec2(5.0, 8.0)), ("transform", matrix(3.5, 2.0))]),
                    ov(&[(90, 202)], vec![("size", vec2(5.0, 2.5)), ("transform", matrix(0.0, 0.0))]),
                ]),
            ),
        ],
    );

    // 3a. Nested case: an icon instance inside an auto-layout button component, resized to 12x12 in the button instance.
    let btn_al_comp = node(
        (1, 300),
        (0, 1),
        "f",
        "SYMBOL",
        "ButtonAutoLayout",
        vec![
            ("size", vec2(100.0, 40.0)),
            ("transform", matrix(0.0, 80.0)),
            ("stackMode", en("HORIZONTAL")),
            ("stackSpacing", fl(8.0)),
            ("stackHorizontalPadding", fl(12.0)),
            ("stackVerticalPadding", fl(8.0)),
        ],
    );
    let btn_al_icon = node(
        (1, 301),
        (1, 300),
        "a",
        "INSTANCE",
        "ButtonIcon",
        vec![
            ("overrideKey", guid(90, 301)),
            ("size", vec2(24.0, 24.0)),
            ("transform", matrix(12.0, 8.0)),
            ("symbolData", msg(vec![("symbolID", guid(1, 100))])),
        ],
    );
    let btn_al_inst = node(
        (2, 4),
        (0, 1),
        "g",
        "INSTANCE",
        "Button-small-derived",
        vec![
            ("size", vec2(60.0, 24.0)),
            ("transform", matrix(120.0, 80.0)),
            ("symbolData", msg(vec![("symbolID", guid(1, 300))])),
            (
                "derivedSymbolData",
                V::Array(vec![
                    // Resizes only the nested icon instance to 12x12; its child vectors must scale to 5x2.5!
                    ov(&[(90, 301)], vec![("size", vec2(12.0, 12.0)), ("transform", matrix(6.0, 6.0))]),
                ]),
            ),
        ],
    );

    // 3b. Nested case without derived data: non-auto-layout button (100x40) with Scale-constrained 24x24 icon instance, resized to 50x20.
    let btn_scale_comp = node((1, 400), (0, 1), "h", "SYMBOL", "ButtonScale", vec![("size", vec2(100.0, 40.0)), ("transform", matrix(0.0, 140.0))]);
    let btn_scale_icon = node(
        (1, 401),
        (1, 400),
        "a",
        "INSTANCE",
        "ButtonIconScaled",
        vec![
            ("overrideKey", guid(90, 401)),
            ("size", vec2(24.0, 24.0)),
            ("transform", matrix(8.0, 8.0)),
            ("horizontalConstraint", en("SCALE")),
            ("verticalConstraint", en("SCALE")),
            ("symbolData", msg(vec![("symbolID", guid(1, 100))])),
        ],
    );
    let btn_scale_inst = node(
        (2, 5),
        (0, 1),
        "i",
        "INSTANCE",
        "Button-scaled-no-derived",
        vec![("size", vec2(50.0, 20.0)), ("transform", matrix(120.0, 140.0)), ("symbolData", msg(vec![("symbolID", guid(1, 400))]))],
    );

    // 4. Unchanged case: 24x24 instance at the main component's own size.
    let inst_unchanged = node(
        (2, 6),
        (0, 1),
        "j",
        "INSTANCE",
        "chevron-24px-unchanged",
        vec![("size", vec2(24.0, 24.0)), ("transform", matrix(90.0, 0.0)), ("symbolData", msg(vec![("symbolID", guid(1, 100))]))],
    );

    let mut changes = vec![
        doc,
        page,
        main_comp,
        vec_frame,
        icon_top,
        icon_bot,
        inst_derived,
        inst_no_derived,
        main_min_comp,
        min_vec_frame,
        min_icon_top,
        inst_size_only_derived,
        btn_al_comp,
        btn_al_icon,
        btn_al_inst,
        btn_scale_comp,
        btn_scale_icon,
        btn_scale_inst,
        inst_unchanged,
    ];
    if pad_large {
        for i in 0..5_100u32 {
            changes.push(node((9, i), (0, 1), &format!("z{i:05}"), "RECTANGLE", &format!("Pad {i}"), vec![("size", vec2(2.0, 2.0))]));
        }
    }

    let blobs = V::Array(vec![
        st(vec![("bytes", V::Bytes(chevron_up_stroke_blob(10.0, 5.0)))]),
        st(vec![("bytes", V::Bytes(chevron_down_stroke_blob(10.0, 5.0)))]),
        st(vec![("bytes", V::Bytes(chevron_up_stroke_blob(5.0, 2.5)))]),
        st(vec![("bytes", V::Bytes(chevron_down_stroke_blob(5.0, 2.5)))]),
        st(vec![("bytes", V::Bytes(centerline_network_blob((0.0, 5.0), (5.0, 0.0), (10.0, 5.0))))]),
        st(vec![("bytes", V::Bytes(centerline_network_blob((0.0, 0.0), (5.0, 5.0), (10.0, 0.0))))]),
    ]);

    let message = msg(vec![("nodeChanges", V::Array(changes)), ("blobs", blobs)]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 101)
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn find_in_subtree<'a>(doc: &'a Document, root: NodeId, name: &str) -> &'a Node {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        if n.name == name {
            return n;
        }
        stack.extend(n.children.iter().rev().copied());
    }
    panic!("no node named {name} under {root:?}");
}

fn paths_bbox(paths: &[Path]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut touch = |p: Vec2| {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    };
    for path in paths {
        for cmd in &path.cmds {
            match *cmd {
                PathCmd::MoveTo(p) | PathCmd::LineTo(p) => touch(p),
                PathCmd::QuadTo(c, p) => {
                    touch(c);
                    touch(p);
                }
                PathCmd::CubicTo(c1, c2, p) => {
                    touch(c1);
                    touch(c2);
                    touch(p);
                }
                PathCmd::Close => {}
            }
        }
    }
    (x0, y0, x1, y1)
}

fn network_bbox(net: &VectorNetwork) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for v in &net.vertices {
        x0 = x0.min(v.pos.x);
        y0 = y0.min(v.pos.y);
        x1 = x1.max(v.pos.x);
        y1 = y1.max(v.pos.y);
    }
    (x0, y0, x1, y1)
}

#[test]
fn resized_instance_with_derived_stroke_geometry_applies_scaled_outline_in_small_and_large_files() {
    for pad_large in [false, true] {
        let doc = import_fig(&build_chevron_fixture_fig(pad_large)).unwrap();
        let inst = find(&doc, "chevron-12px-derived");
        assert_eq!(inst.size, Vec2::new(12.0, 12.0));

        let vec_frame = find_in_subtree(&doc, inst.id, "vector");
        assert_eq!(vec_frame.size, Vec2::new(5.0, 8.0), "pad_large={pad_large}");

        for name in ["IconTop", "IconBottom"] {
            let icon = find_in_subtree(&doc, inst.id, name);
            assert_eq!(icon.size, Vec2::new(5.0, 2.5), "pad_large={pad_large} {name}");
            assert_eq!(icon.stroke.weight, 1.5, "pad_large={pad_large} {name} stroke weight must stay 1.5");

            let sg = icon.stroke_geometry.as_ref().expect("derived stroke_geometry must be present");
            let (sx0, sy0, sx1, sy1) = paths_bbox(sg);
            assert!(
                (sx0 - (-0.75)).abs() < 1e-3 && (sy0 - (-0.75)).abs() < 1e-3 && (sx1 - 5.75).abs() < 1e-3 && (sy1 - 3.25).abs() < 1e-3,
                "pad_large={pad_large} {name}: expected derived stroke_geometry bounds (-0.75..5.75, -0.75..3.25), got ({sx0}..{sx1}, {sy0}..{sy1})"
            );

            let NodeKind::Vector { paths, network } = &icon.kind else { panic!("expected Vector") };
            let (px0, py0, px1, py1) = paths_bbox(paths);
            assert!(
                (px0 - (-0.75)).abs() < 1e-3 && (py0 - (-0.75)).abs() < 1e-3 && (px1 - 5.75).abs() < 1e-3 && (py1 - 3.25).abs() < 1e-3,
                "pad_large={pad_large} {name}: expected vector paths bounds (-0.75..5.75, -0.75..3.25), got ({px0}..{px1}, {py0}..{py1})"
            );

            let net = network.as_ref().expect("centerline network must be present");
            let (nx0, ny0, nx1, ny1) = network_bbox(net);
            assert!(
                nx0.abs() < 1e-3 && ny0.abs() < 1e-3 && (nx1 - 5.0).abs() < 1e-3 && (ny1 - 2.5).abs() < 1e-3,
                "pad_large={pad_large} {name}: expected centerline network bounds (0..5, 0..2.5), got ({nx0}..{nx1}, {ny0}..{ny1})"
            );
        }
    }
}

#[test]
fn resized_instance_without_derived_geometry_scales_geometry_to_fit_child_size() {
    for pad_large in [false, true] {
        let doc = import_fig(&build_chevron_fixture_fig(pad_large)).unwrap();

        // Case A: no derivedSymbolData at all on a Scale-constrained instance.
        let inst_none = find(&doc, "chevron-12px-no-derived");
        for name in ["IconTop", "IconBottom"] {
            let icon = find_in_subtree(&doc, inst_none.id, name);
            assert_eq!(icon.size, Vec2::new(5.0, 2.5), "pad_large={pad_large} {name}");
            assert_eq!(icon.stroke.weight, 1.5, "pad_large={pad_large} {name}");
            let sg = icon.stroke_geometry.as_ref().expect("scaled stroke_geometry must be present");
            let (_, _, sx1, sy1) = paths_bbox(sg);
            assert!(sx1 <= 5.75 + 1e-3 && sy1 <= 3.25 + 1e-3, "pad_large={pad_large} {name}: stroke_geometry max=({sx1}, {sy1}) exceeds 5x2.5 child");
            let NodeKind::Vector { paths, network } = &icon.kind else { panic!("expected Vector") };
            let (_, _, px1, py1) = paths_bbox(paths);
            assert!(px1 <= 5.75 + 1e-3 && py1 <= 3.25 + 1e-3, "pad_large={pad_large} {name}: paths max=({px1}, {py1}) exceeds 5x2.5 child");
            let (nx0, ny0, nx1, ny1) = network_bbox(network.as_ref().unwrap());
            assert!(nx0.abs() < 1e-3 && ny0.abs() < 1e-3 && (nx1 - 5.0).abs() < 1e-3 && (ny1 - 2.5).abs() < 1e-3);
        }

        // Case B: derivedSymbolData overrides size to 5x2.5 without supplying strokeGeometry.
        let inst_size_only = find(&doc, "chevron-12px-size-only-derived");
        let icon = find_in_subtree(&doc, inst_size_only.id, "IconTop");
        assert_eq!(icon.size, Vec2::new(5.0, 2.5), "pad_large={pad_large}");
        assert_eq!(icon.stroke.weight, 1.5, "pad_large={pad_large}");
        let sg = icon.stroke_geometry.as_ref().expect("stroke_geometry must be scaled when missing from derived entry");
        let (_, _, sx1, sy1) = paths_bbox(sg);
        assert!(
            (sx1 - 5.375).abs() < 1e-3 && (sy1 - 2.875).abs() < 1e-3,
            "pad_large={pad_large}: expected scaled stroke_geometry max (5.375, 2.875), got ({sx1}, {sy1})"
        );
        let NodeKind::Vector { paths, network } = &icon.kind else { panic!("expected Vector") };
        let (_, _, px1, py1) = paths_bbox(paths);
        assert!(
            (px1 - 5.375).abs() < 1e-3 && (py1 - 2.875).abs() < 1e-3,
            "pad_large={pad_large}: expected scaled paths max (5.375, 2.875), got ({px1}, {py1})"
        );
        let (nx0, ny0, nx1, ny1) = network_bbox(network.as_ref().unwrap());
        assert!(
            nx0.abs() < 1e-3 && ny0.abs() < 1e-3 && (nx1 - 5.0).abs() < 1e-3 && (ny1 - 2.5).abs() < 1e-3,
            "pad_large={pad_large}: expected scaled network (0..5, 0..2.5), got ({nx0}..{nx1}, {ny0}..{ny1})"
        );
    }
}

#[test]
fn nested_icon_instance_inside_resized_button_instance_scales_child_vectors() {
    for pad_large in [false, true] {
        let doc = import_fig(&build_chevron_fixture_fig(pad_large)).unwrap();

        for btn_name in ["Button-small-derived", "Button-scaled-no-derived"] {
            let btn = find(&doc, btn_name);
            for icon_name in ["IconTop", "IconBottom"] {
                let icon = find_in_subtree(&doc, btn.id, icon_name);
                assert_eq!(icon.size, Vec2::new(5.0, 2.5), "pad_large={pad_large} {btn_name}/{icon_name}");
                assert_eq!(icon.stroke.weight, 1.5, "pad_large={pad_large} {btn_name}/{icon_name}");

                let sg = icon.stroke_geometry.as_ref().expect("nested icon stroke_geometry must be present");
                let (_, _, sx1, sy1) = paths_bbox(sg);
                assert!(
                    sx1 <= 5.75 + 1e-3 && sy1 <= 3.25 + 1e-3,
                    "pad_large={pad_large} {btn_name}/{icon_name}: stroke_geometry max=({sx1}, {sy1}) remained unscaled"
                );

                let NodeKind::Vector { paths, network } = &icon.kind else { panic!("expected Vector") };
                let (_, _, px1, py1) = paths_bbox(paths);
                assert!(px1 <= 5.75 + 1e-3 && py1 <= 3.25 + 1e-3, "pad_large={pad_large} {btn_name}/{icon_name}: paths max=({px1}, {py1}) remained unscaled");
                let (nx0, ny0, nx1, ny1) = network_bbox(network.as_ref().unwrap());
                assert!(
                    nx0.abs() < 1e-3 && ny0.abs() < 1e-3 && (nx1 - 5.0).abs() < 1e-3 && (ny1 - 2.5).abs() < 1e-3,
                    "pad_large={pad_large} {btn_name}/{icon_name}: network bounds ({nx0}..{nx1}, {ny0}..{ny1}) remained unscaled"
                );
            }
        }
    }
}

#[test]
fn unchanged_instance_at_main_component_size_keeps_byte_identical_geometry() {
    for pad_large in [false, true] {
        let doc = import_fig(&build_chevron_fixture_fig(pad_large)).unwrap();
        let main_comp = find(&doc, "chevron-selector-vertical");
        let unchanged = find(&doc, "chevron-24px-unchanged");
        assert_eq!(unchanged.size, main_comp.size);

        for name in ["IconTop", "IconBottom"] {
            let main_icon = find_in_subtree(&doc, main_comp.id, name);
            let inst_icon = find_in_subtree(&doc, unchanged.id, name);
            assert_eq!(inst_icon.size, main_icon.size);
            assert_eq!(inst_icon.stroke.weight, 1.5);
            assert_eq!(inst_icon.fill_geometry, main_icon.fill_geometry);
            assert_eq!(inst_icon.stroke_geometry, main_icon.stroke_geometry);
            assert_eq!(inst_icon.kind, main_icon.kind);
        }
    }
}
