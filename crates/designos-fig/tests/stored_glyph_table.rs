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

    let blobs = icon_blob().len() + curve_blob().len();
    let (stored, files) = (saved.wire.len(), saved.blob_files.len() * 4);
    assert!(stored > 0 && saved.blob_files.len() == 2, "Figma's data is kept as the wire bytes of the export schema, and the file's number of each blob");
    assert_eq!(saved.retained_bytes(), 400 * 24 + blobs + stored + files, "what the geometry keeps: the table, the blobs and the stored bytes, nothing else");
    let eager = eager_stored_path(&saved).len() * std::mem::size_of::<PathCmd>();
    eprintln!(
        "MEASURE retained {} bytes (table {} + blobs {blobs} + wire {stored}); the outlines decoded at import took {eager} bytes",
        saved.retained_bytes(),
        400 * 24
    );
    // these two blobs are a triangle and a curve, so this is the least a text saves: the wire bytes are the price of writing
    // Figma's data back, and a real face's outlines are far larger than the bytes that name them
    assert!(saved.retained_bytes() * 2 <= eager, "{} bytes retained against {eager} decoded", saved.retained_bytes());
}

#[test]
fn a_document_written_before_the_table_still_writes_the_same_derived_text() {
    let doc = import_fig(&golden_fixture()).unwrap();
    let mut legacy = doc.clone();
    // a document of that time kept `derivedTextData` as JSON in the node's metadata, which is what its save read
    let kept = designos_fig::decode_fig_to_json(&golden_fixture()).unwrap();
    let json_of = |name: &str| {
        kept["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == name).and_then(|n| n.get("derivedTextData")).map(|d| d.to_string())
    };
    for node in legacy.nodes.values_mut() {
        let Some(saved) = node.fig_text_geometry.as_deref() else { continue };
        if let Some(json) = json_of(&node.name) {
            node.fig_metadata.insert("derivedTextData".into(), json);
        }
        let old = FigTextGeometry {
            text: saved.text.clone(),
            size: saved.size,
            path: designos_doc::Path { cmds: eager_stored_path(saved), fill_rule: designos_doc::FillRule::NonZero },
            blobs: saved.blobs.clone(),
            glyph_blobs: saved.glyph_blob_slots().collect(),
            glyphs: Default::default(),
            ..Default::default()
        };
        node.fig_text_geometry = Some(std::sync::Arc::new(old));
    }
    assert!(legacy.nodes.values().any(|n| n.fig_text_geometry.as_ref().is_some_and(|g| g.glyphs.is_empty() && !g.path.cmds.is_empty())));
    assert!(legacy.nodes.values().filter_map(|n| n.fig_text_geometry.as_ref().map(|g| (n, g))).all(|(n, g)| g.matches(n)));
    // the same derived data in both files, whatever order a writer put the fields of a message in
    let derived = |bytes: &[u8]| {
        let all = designos_fig::decode_fig_to_json(bytes).unwrap();
        let mut texts: Vec<(String, serde_json::Value)> = all["message"]["nodeChanges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["type"] == "TEXT")
            .filter_map(|n| Some((n["name"].as_str()?.to_string(), n.get("derivedTextData")?.clone())))
            .collect();
        texts.sort_by(|a, b| a.0.cmp(&b.0));
        texts
    };
    let (old, new) = (derived(&export_fig(&legacy).unwrap()), derived(&export_fig(&doc).unwrap()));
    assert!(!new.is_empty());
    assert_eq!(old, new, "the legacy and the table form save the same derived data");
}
