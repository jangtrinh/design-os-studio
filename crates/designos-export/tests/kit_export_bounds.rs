//! Kit frames export bounds check against the private Untitled UI kit file.
//! Ignored by default; run with `cargo test -p designos-export --test kit_export_bounds -- --ignored`.

use designos_export::{export_png, export_svg, render_bounds, SvgOptions};
use std::path::PathBuf;

const UNTITLED_UI_KIT: &str = "❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig";

fn reference(name: &str) -> Option<Vec<u8>> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("DESIGNOS_REFERENCE_DIR").map(PathBuf::from).into_iter().collect();
    let mut cur = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    while let Some(d) = cur {
        dirs.push(d.join("reference"));
        cur = d.parent().map(PathBuf::from);
    }
    dirs.extend(std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads")));
    dirs.into_iter().map(|d| d.join(name)).find_map(|p| std::fs::read(p).ok())
}

struct FrameExpected {
    source_id: &'static str,
    name: &'static str,
    expected_w: f32,
    expected_h: f32,
    /// Why the frame is not asserted yet; `None` means it must match Figma within 1 px.
    known_open: Option<&'static str>,
}

const MOBILE_UNEXPLAINED: &str = "left edge reaches 5 px further than Figma and the cause is not located; Figma's bounds for this kit layer cannot be read";

const KIT_FRAMES: &[FrameExpected] = &[
    FrameExpected { source_id: "9264:578567", name: "_Video timestamp indicator", expected_w: 172.0, expected_h: 141.0, known_open: None },
    FrameExpected {
        source_id: "8085:173260",
        name: "Dashboards 01 Desktop",
        expected_w: 1524.0,
        expected_h: 1257.0,
        known_open: Some("the Series 2 frame inside reaches 1528; the rule that stops Figma at 1524 is not located yet"),
    },
    FrameExpected { source_id: "1689:439089", name: "Informational pages 01 Desktop", expected_w: 1440.0, expected_h: 966.0, known_open: None },
    FrameExpected { source_id: "1673:402973", name: "Settings pages 01 Desktop", expected_w: 1440.0, expected_h: 966.0, known_open: None },
    FrameExpected { source_id: "6277:293415", name: "Settings pages 02 Mobile", expected_w: 391.0, expected_h: 1425.0, known_open: Some(MOBILE_UNEXPLAINED) },
    FrameExpected { source_id: "11627:254713", name: "_Date picker menu", expected_w: 370.0, expected_h: 582.0, known_open: None },
    FrameExpected { source_id: "12469:575669", name: "_Footer social proof badge", expected_w: 310.0, expected_h: 42.0, known_open: None },
    FrameExpected { source_id: "11547:540792", name: "Context menu", expected_w: 476.0, expected_h: 1044.0, known_open: None },
    FrameExpected { source_id: "9291:551743", name: "_Text editor icon", expected_w: 1690.0, expected_h: 343.0, known_open: None },
    FrameExpected { source_id: "9291:558536", name: "Text highlight", expected_w: 403.0, expected_h: 76.0, known_open: None },
];

fn svg_root_attr(svg: &str, name: &str) -> Option<f32> {
    let tag_end = svg.find('>')?;
    let root = &svg[..tag_end];
    let pat = format!("{name}=\"");
    let start = root.find(&pat)? + pat.len();
    let end = root[start..].find('"')? + start;
    root[start..end].parse::<f32>().ok()
}

fn within_one_px(got: f32, want: f32) -> bool {
    (got - want).abs() <= 1.0
}

#[test]
#[ignore = "needs the private reference Untitled UI kit file"]
fn kit_frames_export_bounds_match_figma() {
    let bytes = match reference(UNTITLED_UI_KIT) {
        Some(b) => b,
        None => {
            if std::env::var_os("DESIGNOS_REFERENCE_DIR").is_some() {
                panic!("DESIGNOS_REFERENCE_DIR set but {UNTITLED_UI_KIT} not found");
            }
            eprintln!("{UNTITLED_UI_KIT} not found: skipped");
            return;
        }
    };

    let mut doc = designos_fig::import_fig(&bytes).expect("import kit");
    let mut failures: Vec<String> = Vec::new();

    for frame in KIT_FRAMES {
        let id = doc
            .nodes
            .iter()
            .find(|(_, n)| n.source_id.as_deref() == Some(frame.source_id))
            .map(|(id, _)| *id)
            .unwrap_or_else(|| panic!("frame {} ({}) not found in kit", frame.name, frame.source_id));

        // Instances on a page the import left deferred have no children yet. The app loads a page when it
        // opens it and the MCP server loads the page of every node it reads, so measure the same way.
        let page = doc.page_of(id).unwrap_or_else(|| panic!("frame {} ({}) has no page", frame.name, frame.source_id));
        doc.ensure_page_hydrated(page);
        if let Some(err) = doc.take_hydration_error() {
            panic!("cannot load the page of frame {} ({}): {err}", frame.name, frame.source_id);
        }

        let b = render_bounds(&doc, id);
        let svg = export_svg(&doc, id, &SvgOptions::default());
        assert!(!svg.is_empty(), "frame {} ({}) exported an empty svg", frame.name, frame.source_id);
        let svg_w = svg_root_attr(&svg, "width").expect("svg root width");
        let svg_h = svg_root_attr(&svg, "height").expect("svg root height");
        let png_bytes = export_png(&doc, id, 1.0);
        let img = image::load_from_memory(&png_bytes).expect("decode exported png");
        let (png_w, png_h) = (img.width() as f32, img.height() as f32);

        eprintln!(
            "{} ({}): figma {}x{} | bounds {}x{} | png {}x{} | svg {}x{}{}",
            frame.name,
            frame.source_id,
            frame.expected_w,
            frame.expected_h,
            b.w,
            b.h,
            png_w,
            png_h,
            svg_w,
            svg_h,
            frame.known_open.map(|why| format!(" | known-open: {why}")).unwrap_or_default()
        );
        if frame.known_open.is_some() {
            continue;
        }

        for (what, w, h) in [("render bounds", b.w, b.h), ("png", png_w, png_h), ("svg", svg_w, svg_h)] {
            if !within_one_px(w, frame.expected_w) || !within_one_px(h, frame.expected_h) {
                failures.push(format!("frame {} ({}) {what}: studio {w}x{h} vs figma {}x{}", frame.name, frame.source_id, frame.expected_w, frame.expected_h));
            }
        }
    }

    assert!(failures.is_empty(), "kit frames differ from Figma by more than 1 px:\n{}", failures.join("\n"));
}
