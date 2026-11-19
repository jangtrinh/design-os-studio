//! Golden for the texts that draw Figma's stored glyphs: an icon font glyph, a missing-font text of several glyphs
//! (one reused outline, one glyph without outline, two sizes) and an instance copy. The constants below were recorded
//! on the commit before the glyph data moved into a compact table, so this test cannot fail on that commit: it pins
//! the pixels and the outlines the table must keep reproducing. The outline hashes cover every coordinate bit for bit on
//! every platform; the pixel hashes depend on the rasteriser's vector units, so they are checked on the arm64 CI and
//! development hosts the constants come from.
#[path = "../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, Node, NodeId};
use designos_fig::import_fig;
use designos_render::{render_node, text_outlines_for_node, vectorops::text_flatten_runs};
use missing_font_fig::*;
use tiny_skia::PathSegment;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn named<'a>(doc: &'a Document, name: &str, nth: usize) -> &'a Node {
    let mut found: Vec<&Node> = doc.nodes.values().filter(|n| n.name == name).collect();
    found.sort_by_key(|n| n.id);
    found[nth]
}

/// Hash of the pixels of `id` rendered at 1x and 2x.
fn pixels(doc: &Document, id: NodeId) -> u64 {
    [1.0, 2.0].into_iter().fold(FNV_OFFSET, |h, scale| {
        let pm = render_node(doc, id, scale).expect("the node renders");
        fnv(fnv(h, &pm.width().to_le_bytes()), pm.data())
    })
}

/// Hash of every coordinate of every outline drawn for `node`, bit for bit.
fn outlines(node: &Node) -> u64 {
    let mut h = FNV_OFFSET;
    for outline in text_outlines_for_node(node) {
        h = fnv(h, b"outline");
        for seg in outline.path.segments() {
            let pts: &[tiny_skia::Point] = match &seg {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => std::slice::from_ref(p),
                PathSegment::QuadTo(a, b) => &[*a, *b],
                PathSegment::CubicTo(a, b, c) => &[*a, *b, *c],
                PathSegment::Close => &[],
            };
            h = fnv(h, &[pts.len() as u8]);
            for p in pts {
                h = fnv(fnv(h, &p.x.to_bits().to_le_bytes()), &p.y.to_bits().to_le_bytes());
            }
        }
    }
    h
}

/// Hash of the vectors Flatten makes of `node`.
fn flattened(doc: &Document, node: &Node) -> u64 {
    let mut h = FNV_OFFSET;
    for run in text_flatten_runs(doc, node) {
        h = fnv(h, format!("{:?}", run.path.cmds).as_bytes());
    }
    h
}

#[test]
fn stored_glyph_texts_draw_the_recorded_pixels_and_outlines() {
    let doc = import_fig(&golden_fixture()).unwrap();
    let frame = named(&doc, "Icon frame", 0).id;
    let icon = named(&doc, "Icon text", 0);
    let mixed = named(&doc, "Mixed glyphs", 0);
    let main = named(&doc, "Glyph source", 0);
    let copy = named(&doc, "Glyph source", 1);
    let instance = named(&doc, "Icon copy", 0).id;
    for (what, text) in [("icon", icon), ("mixed", mixed), ("component text", main), ("instance copy text", copy)] {
        assert!(text.fig_text_geometry.as_ref().is_some_and(|g| g.matches(text)), "{what} keeps Figma's glyphs");
    }
    let outline_hashes = [
        ("icon outlines", outlines(icon), 0x048a_cdd9_ee41_6c02),
        ("mixed outlines", outlines(mixed), 0x7462_f5a8_7360_63f8),
        ("component text outlines", outlines(main), 0x048a_cdd9_ee41_6c02),
        ("instance copy text outlines", outlines(copy), 0x048a_cdd9_ee41_6c02),
        ("icon flatten", flattened(&doc, icon), 0x85b2_d0c5_d9e7_9dc6),
        ("mixed flatten", flattened(&doc, mixed), 0xd288_c2e5_8cc4_f91c),
    ];
    let pixel_hashes = [
        ("icon frame pixels", pixels(&doc, frame), 0xa54f_49a4_722d_3a2d),
        ("mixed text pixels", pixels(&doc, mixed.id), 0x64f9_9b19_1625_9fd7),
        ("instance pixels", pixels(&doc, instance), 0x96e8_ec5a_a4d6_f699),
    ];
    for (what, got, want) in outline_hashes {
        assert_eq!(got, want, "{what}: recorded {want:#018x}, drawn {got:#018x}");
    }
    if cfg!(target_arch = "aarch64") {
        for (what, got, want) in pixel_hashes {
            assert_eq!(got, want, "{what}: recorded {want:#018x}, drawn {got:#018x}");
        }
    }
}
