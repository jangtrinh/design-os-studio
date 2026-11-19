//! The stored glyphs of an imported text are a table of 24 bytes a glyph plus the command blobs of the file: no outline
//! is built at import. The importer still refuses the texts it refused when it built the outlines, and a document
//! written before the table existed is written back unchanged.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, FigTextGeometry, Node, NodeKind, PathCmd};
use designos_fig::{export_fig, import_fig};
use missing_font_fig::*;

fn text_named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Text(_))).unwrap_or_else(|| panic!("no text named {name}"))
}

fn geometry_of(doc: &Document, name: &str) -> Option<std::sync::Arc<FigTextGeometry>> {
    text_named(doc, name).fig_text_geometry.clone()
}

/// A file with one text `Probe` of `glyphs`, over the blobs `[triangle, curve]`.
fn probe(glyphs: &[(Option<u32>, f32, f32, f32)]) -> Vec<u8> {
    let mut nodes = document_and_page();
    nodes.push(mixed_glyph_text((1, 3), (0, 1), "a", "Probe", FAMILY, glyphs));
    fig_file(nodes, vec![icon_blob(), curve_blob()])
}

#[test]
fn a_text_keeps_a_24_byte_table_and_the_wire_blobs_instead_of_its_decoded_outlines() {
    let glyphs: Vec<_> = (0..400).map(|i| (Some(i % 2), i as f32 * 0.1, 20.0, 20.0)).collect();
    let doc = import_fig(&probe(&glyphs)).unwrap();
    let saved = geometry_of(&doc, "Probe").expect("the text keeps its glyphs");
    assert!(saved.path.cmds.is_empty(), "no outline is built at import");
    assert_eq!((saved.glyphs.len(), saved.blobs.len()), (400, 2), "a glyph is a table entry and the two outlines are two blobs");
    assert_eq!(std::mem::size_of_val(saved.glyphs.as_slice()), 400 * 24);

    let wire = icon_blob().len() + curve_blob().len();
    assert_eq!(saved.retained_bytes(), 400 * 24 + wire, "what the geometry keeps: the table and the blobs, nothing else");
    let eager = eager_stored_path(&saved).len() * std::mem::size_of::<PathCmd>();
    eprintln!("MEASURE retained {} bytes (table {} + wire {wire}); the outlines decoded at import took {eager} bytes", saved.retained_bytes(), 400 * 24);
    assert!(saved.retained_bytes() * 4 <= eager, "{} bytes retained against {eager} decoded", saved.retained_bytes());
}

#[test]
fn a_document_written_before_the_table_still_writes_the_same_derived_text() {
    let doc = import_fig(&golden_fixture()).unwrap();
    let mut legacy = doc.clone();
    for node in legacy.nodes.values_mut() {
        let Some(saved) = node.fig_text_geometry.as_deref() else { continue };
        let old = FigTextGeometry {
            text: saved.text.clone(),
            size: saved.size,
            path: designos_doc::Path { cmds: eager_stored_path(saved), fill_rule: designos_doc::FillRule::NonZero },
            blobs: saved.blobs.clone(),
            glyph_blobs: saved.glyph_blob_slots().collect(),
            glyphs: Default::default(),
        };
        node.fig_text_geometry = Some(std::sync::Arc::new(old));
    }
    assert!(legacy.nodes.values().any(|n| n.fig_text_geometry.as_ref().is_some_and(|g| g.glyphs.is_empty() && !g.path.cmds.is_empty())));
    assert!(legacy.nodes.values().filter_map(|n| n.fig_text_geometry.as_ref().map(|g| (n, g))).all(|(n, g)| g.matches(n)));
    assert_eq!(export_fig(&legacy).unwrap(), export_fig(&doc).unwrap(), "the legacy and the table form save the same file");
}
