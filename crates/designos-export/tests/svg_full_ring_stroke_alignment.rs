//! A standalone full ring (an ellipse with an inner radius of 100%) has no fill area, but its stroke is
//! still aligned in SVG like the canvas does: the stroke path, the Inside clip and the Outside knockout
//! are all the single outer ellipse (the disc), never a 0.999 annulus.

use designos_doc::*;
use designos_export::{export_svg, render_export, SvgOptions};
use std::collections::HashMap;
use tiny_skia as sk;

const DISC_D: &str = "M100 50 C100 77.614 77.614 100 50 100 C22.386 100 0 77.614 0 50 C0 22.386 22.386 0 50 0 C77.614 0 100 22.386 100 50 Z";
const DONUT_D: &str = "M100 50 C100 77.614 77.614 100 50 100 C22.386 100 0 77.614 0 50 C0 22.386 22.386 0 50 0 C77.614 0 100 22.386 100 50 Z M75 50 C75 36.193 63.807 25 50 25 C36.193 25 25 36.193 25 50 C25 63.807 36.193 75 50 75 C63.807 75 75 63.807 75 50 Z";

const TOLERANCE: i32 = 8;
const MAX_MISMATCH: f64 = 0.02;

fn ellipse_doc(inner_radius: f32, align: StrokeAlign) -> (Document, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut n = Node::new(id, NodeKind::Ellipse(ArcData { start: 0.0, end: std::f32::consts::TAU, inner_radius }));
    n.size = Vec2::new(100.0, 100.0);
    n.stroke = Stroke { paints: vec![Paint::solid(Color::BLACK)], weight: 10.0, align, ..Default::default() };
    let id = doc.insert(page, n).unwrap();
    (doc, id)
}

fn ellipse_svg(inner_radius: f32, align: StrokeAlign) -> String {
    let (doc, id) = ellipse_doc(inner_radius, align);
    export_svg(&doc, id, &SvgOptions::default())
}

fn attr<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("{name}=\"");
    let start = line.find(&pat)? + pat.len();
    let end = start + line[start..].find('"')?;
    Some(&line[start..end])
}

fn block_between<'a>(svg: &'a str, open: &str, close: &str) -> &'a str {
    let s = svg.find(open).unwrap_or_else(|| panic!("missing {open} in:\n{svg}"));
    let rest = &svg[s..];
    let e = rest.find(close).unwrap_or_else(|| panic!("missing {close} in:\n{svg}")) + close.len();
    &rest[..e]
}

fn assert_single_closed_disc(d: &str, role: &str, svg: &str) {
    assert_eq!(d.matches('M').count(), 1, "{role} must have one subpath (no inner oval), got d={d:?} in:\n{svg}");
    assert_eq!(d.matches('Z').count(), 1, "{role} must be one closed ellipse, got d={d:?} in:\n{svg}");
    assert_eq!(d, DISC_D, "{role} must be the outer disc:\n{svg}");
}

fn parse_svg_path(d: &str) -> sk::Path {
    let mut pb = sk::PathBuilder::new();
    let mut tokens = d.split_whitespace().peekable();
    while let Some(tok) = tokens.next() {
        let (cmd, first) = tok.split_at(1);
        let mut next_f = |first_num: &str| -> f32 {
            if !first_num.is_empty() {
                first_num.parse().unwrap()
            } else {
                tokens.next().unwrap().parse().unwrap()
            }
        };
        match cmd {
            "M" => {
                let x = next_f(first);
                let y: f32 = tokens.next().unwrap().parse().unwrap();
                pb.move_to(x, y);
            }
            "L" => {
                let x = next_f(first);
                let y: f32 = tokens.next().unwrap().parse().unwrap();
                pb.line_to(x, y);
            }
            "C" => {
                let x1 = next_f(first);
                let y1: f32 = tokens.next().unwrap().parse().unwrap();
                let x2: f32 = tokens.next().unwrap().parse().unwrap();
                let y2: f32 = tokens.next().unwrap().parse().unwrap();
                let x: f32 = tokens.next().unwrap().parse().unwrap();
                let y: f32 = tokens.next().unwrap().parse().unwrap();
                pb.cubic_to(x1, y1, x2, y2, x, y);
            }
            "Z" => pb.close(),
            other => panic!("unexpected SVG path command {other:?} in {d:?}"),
        }
    }
    pb.finish().expect("valid SVG path")
}

/// Rasterizes the SVG emitted by `export_svg` (viewBox, `<clipPath>`, `<mask>`, stroked `<path>`) with tiny-skia.
fn rasterize_svg(svg: &str) -> sk::Pixmap {
    let root = svg.lines().find(|l| l.trim_start().starts_with("<svg ")).expect("svg root");
    let w: u32 = attr(root, "width").unwrap().parse().unwrap();
    let h: u32 = attr(root, "height").unwrap().parse().unwrap();
    let vb: Vec<f32> = attr(root, "viewBox").unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
    let ts = sk::Transform::from_translate(-vb[0], -vb[1]);

    let mut clips: HashMap<String, sk::Mask> = HashMap::new();
    let mut masks: HashMap<String, sk::Mask> = HashMap::new();

    if svg.contains("<defs>") {
        let defs = block_between(svg, "<defs>", "</defs>");
        let mut lines = defs.lines().map(str::trim);
        while let Some(line) = lines.next() {
            if let Some(id) = line.strip_prefix("<clipPath ").and_then(| _| attr(line, "id")) {
                let mut m = sk::Mask::new(w, h).unwrap();
                for inner in lines.by_ref() {
                    if inner == "</clipPath>" {
                        break;
                    }
                    if inner.starts_with("<path ") {
                        let path = parse_svg_path(attr(inner, "d").unwrap());
                        m.fill_path(&path, sk::FillRule::Winding, true, ts);
                    }
                }
                clips.insert(id.to_string(), m);
            } else if let Some(id) = line.strip_prefix("<mask ").and_then(|_| attr(line, "id")) {
                let mut pm = sk::Pixmap::new(w, h).unwrap();
                for inner in lines.by_ref() {
                    if inner == "</mask>" {
                        break;
                    }
                    let color = match attr(inner, "fill") {
                        Some("white") => sk::Color::WHITE,
                        Some("black") => sk::Color::BLACK,
                        other => panic!("unexpected mask fill {other:?}"),
                    };
                    let mut paint = sk::Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;
                    if inner.starts_with("<rect ") {
                        let rx: f32 = attr(inner, "x").unwrap().parse().unwrap();
                        let ry: f32 = attr(inner, "y").unwrap().parse().unwrap();
                        let rw: f32 = attr(inner, "width").unwrap().parse().unwrap();
                        let rh: f32 = attr(inner, "height").unwrap().parse().unwrap();
                        let rect = sk::Rect::from_xywh(rx, ry, rw, rh).unwrap();
                        pm.fill_rect(rect, &paint, ts, None);
                    } else if inner.starts_with("<path ") {
                        let path = parse_svg_path(attr(inner, "d").unwrap());
                        pm.fill_path(&path, &paint, sk::FillRule::Winding, ts, None);
                    }
                }
                let mut m = sk::Mask::new(w, h).unwrap();
                for (dst, px) in m.data_mut().iter_mut().zip(pm.pixels()) {
                    *dst = px.red();
                }
                masks.insert(id.to_string(), m);
            }
        }
    }

    let body = if let Some(pos) = svg.find("</defs>") { &svg[pos + "</defs>".len()..] } else { svg };
    let mut pm = sk::Pixmap::new(w, h).unwrap();
    let mut active_mask: Option<&sk::Mask> = None;
    for line in body.lines().map(str::trim) {
        if line.starts_with("<g ") {
            if let Some(url) = attr(line, "clip-path") {
                let id = url.strip_prefix("url(#").unwrap().strip_suffix(')').unwrap();
                active_mask = Some(&clips[id]);
            } else if let Some(url) = attr(line, "mask") {
                let id = url.strip_prefix("url(#").unwrap().strip_suffix(')').unwrap();
                active_mask = Some(&masks[id]);
            }
        } else if line == "</g>" {
            active_mask = None;
        } else if line.starts_with("<path ") {
            let path = parse_svg_path(attr(line, "d").unwrap());
            let sw: f32 = attr(line, "stroke-width").unwrap().parse().unwrap();
            let mut paint = sk::Paint::default();
            paint.set_color(sk::Color::BLACK);
            paint.anti_alias = true;
            let stroke = sk::Stroke { width: sw, ..Default::default() };
            pm.stroke_path(&path, &paint, &stroke, ts, active_mask);
        }
    }
    pm
}

fn mismatch(a: &sk::Pixmap, b: &sk::Pixmap) -> f64 {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let bad = a
        .data()
        .chunks_exact(4)
        .zip(b.data().chunks_exact(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| (*x as i32 - *y as i32).abs() > TOLERANCE))
        .count();
    bad as f64 / (a.width() * a.height()) as f64
}

#[test]
fn a_full_ring_keeps_its_inside_stroke_aligned() {
    let svg = ellipse_svg(1.0, StrokeAlign::Inside);
    let clip = block_between(&svg, "<clipPath id=\"clip0\">", "</clipPath>");
    let clip_path = clip.lines().map(str::trim).find(|l| l.starts_with("<path ")).expect("clip path element");
    assert_single_closed_disc(attr(clip_path, "d").unwrap(), "Inside clip-path shape", &svg);

    let group = block_between(&svg, "<g clip-path=\"url(#clip0)\">", "</g>");
    let stroked = group.lines().map(str::trim).find(|l| l.starts_with("<path ")).expect("stroked path element");
    assert_single_closed_disc(attr(stroked, "d").unwrap(), "Inside stroked path", &svg);
    assert_eq!(attr(stroked, "stroke-width"), Some("20"), "Inside stroke width is doubled:\n{svg}");
}

#[test]
fn a_full_ring_keeps_its_outside_stroke_aligned() {
    let svg = ellipse_svg(1.0, StrokeAlign::Outside);
    let mask = block_between(&svg, "<mask id=\"outside0\"", "</mask>");
    let knockout = mask.lines().map(str::trim).find(|l| l.starts_with("<path ")).expect("mask knockout path element");
    assert_eq!(attr(knockout, "fill"), Some("black"), "mask knockout is black:\n{svg}");
    assert_single_closed_disc(attr(knockout, "d").unwrap(), "Outside mask knockout shape", &svg);

    let group = block_between(&svg, "<g mask=\"url(#outside0)\">", "</g>");
    let stroked = group.lines().map(str::trim).find(|l| l.starts_with("<path ")).expect("stroked path element");
    assert_single_closed_disc(attr(stroked, "d").unwrap(), "Outside stroked path", &svg);
    assert_eq!(attr(stroked, "stroke-width"), Some("20"), "Outside stroke width is doubled:\n{svg}");
}

#[test]
fn a_full_ring_svg_pixels_match_the_canvas() {
    for align in [StrokeAlign::Inside, StrokeAlign::Outside, StrokeAlign::Center] {
        let (doc, id) = ellipse_doc(1.0, align);
        let svg = export_svg(&doc, id, &SvgOptions::default());
        let svg_pm = rasterize_svg(&svg);
        let canvas_pm = render_export(&doc, id, 1.0).expect("canvas render");
        let canvas_ink = canvas_pm.pixels().iter().filter(|p| p.alpha() > 128).count();
        assert!(canvas_ink > 500, "{align:?}: canvas must draw the ring stroke, got {canvas_ink} inked pixels");
        let diff = mismatch(&svg_pm, &canvas_pm);
        assert!(diff <= MAX_MISMATCH, "{align:?}: SVG raster differs from CPU canvas on {:.2}% of pixels\n{svg}", diff * 100.0);
    }
}

#[test]
fn a_half_ring_exports_the_same_markup_as_main() {
    let expected_inside = format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"100\" height=\"100\" viewBox=\"0 0 100 100\" fill=\"none\">\n",
            "  <defs>\n",
            "    <clipPath id=\"clip0\">\n",
            "      <path d=\"{donut}\"/>\n",
            "    </clipPath>\n",
            "  </defs>\n",
            "  <g data-name=\"Ellipse\">\n",
            "    <g clip-path=\"url(#clip0)\">\n",
            "      <path d=\"{donut}\" fill=\"none\" stroke=\"#000000\" stroke-width=\"20\"/>\n",
            "    </g>\n",
            "  </g>\n",
            "</svg>\n",
        ),
        donut = DONUT_D
    );
    assert_eq!(ellipse_svg(0.5, StrokeAlign::Inside), expected_inside);

    let expected_outside = format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"120\" height=\"120\" viewBox=\"-10 -10 120 120\" fill=\"none\">\n",
            "  <defs>\n",
            "    <mask id=\"outside0\" maskUnits=\"userSpaceOnUse\" x=\"-21\" y=\"-21\" width=\"142\" height=\"142\">\n",
            "      <rect x=\"-21\" y=\"-21\" width=\"142\" height=\"142\" fill=\"white\"/>\n",
            "      <path d=\"{donut}\" fill=\"black\"/>\n",
            "    </mask>\n",
            "  </defs>\n",
            "  <g data-name=\"Ellipse\">\n",
            "    <g mask=\"url(#outside0)\">\n",
            "      <path d=\"{donut}\" fill=\"none\" stroke=\"#000000\" stroke-width=\"20\"/>\n",
            "    </g>\n",
            "  </g>\n",
            "</svg>\n",
        ),
        donut = DONUT_D
    );
    assert_eq!(ellipse_svg(0.5, StrokeAlign::Outside), expected_outside);

    let expected_center = format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"110\" height=\"110\" viewBox=\"-5 -5 110 110\" fill=\"none\">\n",
            "  <g data-name=\"Ellipse\">\n",
            "    <path d=\"{donut}\" fill=\"none\" stroke=\"#000000\" stroke-width=\"10\"/>\n",
            "  </g>\n",
            "</svg>\n",
        ),
        donut = DONUT_D
    );
    assert_eq!(ellipse_svg(0.5, StrokeAlign::Center), expected_center);
}

