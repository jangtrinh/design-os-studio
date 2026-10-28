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
    fixed_by_per_side_rule: bool,
}

const KIT_FRAMES: &[FrameExpected] = &[
    FrameExpected { source_id: "9264:578567", name: "_Video timestamp indicator", expected_w: 172.0, expected_h: 141.0, fixed_by_per_side_rule: true },
    FrameExpected { source_id: "8085:173260", name: "Dashboards 01 Desktop", expected_w: 1524.0, expected_h: 1257.0, fixed_by_per_side_rule: false },
    FrameExpected { source_id: "1689:439089", name: "Informational pages 01 Desktop", expected_w: 1440.0, expected_h: 966.0, fixed_by_per_side_rule: false },
    FrameExpected { source_id: "1673:402973", name: "Settings pages 01 Desktop", expected_w: 1440.0, expected_h: 966.0, fixed_by_per_side_rule: false },
    FrameExpected { source_id: "6277:293415", name: "Settings pages 02 Mobile", expected_w: 391.0, expected_h: 1425.0, fixed_by_per_side_rule: false },
    FrameExpected { source_id: "11627:254713", name: "_Date picker menu", expected_w: 370.0, expected_h: 582.0, fixed_by_per_side_rule: true },
    FrameExpected { source_id: "12469:575669", name: "_Footer social proof badge", expected_w: 310.0, expected_h: 42.0, fixed_by_per_side_rule: true },
    FrameExpected { source_id: "11547:540792", name: "Context menu", expected_w: 476.0, expected_h: 1044.0, fixed_by_per_side_rule: false },
    FrameExpected { source_id: "9291:551743", name: "_Text editor icon", expected_w: 1690.0, expected_h: 343.0, fixed_by_per_side_rule: true },
    FrameExpected { source_id: "9291:558536", name: "Text highlight", expected_w: 403.0, expected_h: 76.0, fixed_by_per_side_rule: false },
];

fn svg_root_attr(svg: &str, name: &str) -> Option<f32> {
    let tag_end = svg.find('>')?;
    let root = &svg[..tag_end];
    let pat = format!("{name}=\"");
    let start = root.find(&pat)? + pat.len();
    let end = root[start..].find('"')? + start;
    root[start..end].parse::<f32>().ok()
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

    let doc = designos_fig::import_fig(&bytes).expect("import kit");

    for frame in KIT_FRAMES {
        let (id, _node) = doc
            .nodes
            .iter()
            .find(|(_, n)| n.source_id.as_deref() == Some(frame.source_id))
            .unwrap_or_else(|| panic!("frame {} ({}) not found in kit", frame.name, frame.source_id));

        let b = render_bounds(&doc, *id);
        let svg = export_svg(&doc, *id, &SvgOptions::default());
        assert!(!svg.is_empty());
        let svg_w = svg_root_attr(&svg, "width").expect("svg root width");
        let svg_h = svg_root_attr(&svg, "height").expect("svg root height");

        if !frame.fixed_by_per_side_rule {
            eprintln!(
                "known-open (cause 5): frame {} ({}) studio {}x{} (svg {}x{}) vs figma {}x{}",
                frame.name, frame.source_id, b.w, b.h, svg_w, svg_h, frame.expected_w, frame.expected_h
            );
            continue;
        }

        assert!(
            (b.w - frame.expected_w).abs() <= 1.0,
            "frame {} ({}) width mismatch: studio {} vs figma {}",
            frame.name,
            frame.source_id,
            b.w,
            frame.expected_w
        );
        assert!(
            (b.h - frame.expected_h).abs() <= 1.0,
            "frame {} ({}) height mismatch: studio {} vs figma {}",
            frame.name,
            frame.source_id,
            b.h,
            frame.expected_h
        );

        // Check PNG export at 1x
        let png_bytes = export_png(&doc, *id, 1.0);
        let img = image::load_from_memory(&png_bytes).expect("decode exported png");
        assert!(
            (img.width() as f32 - frame.expected_w).abs() <= 1.0,
            "frame {} ({}) png width mismatch: {} vs {}",
            frame.name,
            frame.source_id,
            img.width(),
            frame.expected_w
        );
        assert!(
            (img.height() as f32 - frame.expected_h).abs() <= 1.0,
            "frame {} ({}) png height mismatch: {} vs {}",
            frame.name,
            frame.source_id,
            img.height(),
            frame.expected_h
        );

        // Check SVG root width/height at 1x
        assert!((svg_w - frame.expected_w).abs() <= 1.0, "frame {} ({}) svg width mismatch: {} vs {}", frame.name, frame.source_id, svg_w, frame.expected_w);
        assert!((svg_h - frame.expected_h).abs() <= 1.0, "frame {} ({}) svg height mismatch: {} vs {}", frame.name, frame.source_id, svg_h, frame.expected_h);
    }
}
