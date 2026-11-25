//! A `.fig` save writes Figma's outline blobs and vector networks back on a layer nobody edited, and Studio's own on one that
//! was edited or resized.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, NodeId, NodeKind, Vec2};
use designos_fig::container;
use designos_fig::kiwi::{KiwiValue as V, Schema};
use designos_fig::testkit::{en, fl, msg, node, solid, vec2};
use designos_fig::{export_fig, export_fig_with_kept_report, import_fig};
use missing_font_fig::*;

/// The decoded message of a `.fig`.
fn message(bytes: &[u8]) -> V {
    let archive = container::open(bytes).expect("open the container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read the chunks");
    let schema = Schema::decode(&schema_bin).expect("decode the schema");
    let root = schema.root_message().expect("root message");
    schema.decode_message(root, &msg_bin).expect("decode the message")
}

fn records(bytes: &[u8]) -> Vec<V> {
    match message(bytes).take_field("nodeChanges") {
        Some(V::Array(a)) => a,
        _ => Vec::new(),
    }
}

/// The blob table of a `.fig`.
fn blobs(bytes: &[u8]) -> Vec<Vec<u8>> {
    match message(bytes).take_field("blobs") {
        Some(V::Array(a)) => a
            .iter()
            .map(|b| match b.get("bytes") {
                Some(V::Bytes(bytes)) => bytes.clone(),
                other => panic!("a blob holds {other:?}"),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn guid_of(record: &V) -> (u32, u32) {
    let g = record.get("guid").expect("a record has a guid");
    let n = |k: &str| match g.get(k) {
        Some(V::Uint(u)) => *u,
        other => panic!("guid.{k} is {other:?}"),
    };
    (n("sessionID"), n("localID"))
}

fn record_of(all: &[V], id: (u32, u32)) -> &V {
    all.iter().find(|r| guid_of(r) == id).unwrap_or_else(|| panic!("no record {id:?}"))
}

fn blob_number(v: &V) -> usize {
    match v {
        V::Uint(u) => *u as usize,
        other => panic!("a blob number is {other:?}"),
    }
}

/// An outline key of a record with each blob replaced by its bytes: every other field of each path (the winding rule, the style
/// id) and the commands.
fn outline(file: &[u8], id: (u32, u32), key: &str) -> Option<Vec<(String, Vec<u8>)>> {
    let all = records(file);
    let table = blobs(file);
    let paths = record_of(&all, id).get(key)?.as_array()?.to_vec();
    Some(
        paths
            .iter()
            .map(|p| {
                let V::Message(fields) = p else { panic!("a path is a message") };
                let rest: Vec<_> = fields.iter().filter(|(k, _)| &**k != "commandsBlob").collect();
                (format!("{rest:?}"), table[blob_number(p.get("commandsBlob").expect("a path has commands"))].clone())
            })
            .collect(),
    )
}

/// A vector's network blob, resolved.
fn network(file: &[u8], id: (u32, u32)) -> Option<Vec<u8>> {
    let all = records(file);
    let table = blobs(file);
    let vd = record_of(&all, id).get("vectorData")?;
    Some(table[blob_number(vd.get("vectorNetworkBlob")?)].clone())
}

fn normalized_size(file: &[u8], id: (u32, u32)) -> Option<V> {
    record_of(&records(file), id).get("vectorData")?.get("normalizedSize").cloned()
}

fn by_name(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).id
}

/// A path blob: M a, L b, L c, L d, Z.
fn rect_commands(w: f32, h: f32) -> Vec<u8> {
    let mut out = Vec::new();
    for (op, x, y) in [(1u8, 0.0f32, 0.0f32), (2, w, 0.0), (2, w, h), (2, 0.0, h)] {
        out.push(op);
        out.extend(x.to_le_bytes());
        out.extend(y.to_le_bytes());
    }
    out.push(0);
    out
}

/// A vector network blob in its normalised size: the vertices, a segment from each to the next (the last closes the loop),
/// and one filled region over all of them when `region` is set.
fn network_blob(vertices: &[(f32, f32)], region: bool) -> Vec<u8> {
    let n = vertices.len() as u32;
    let mut out = Vec::new();
    for count in [n, n, u32::from(region)] {
        out.extend(count.to_le_bytes());
    }
    for (x, y) in vertices {
        out.extend(0u32.to_le_bytes());
        out.extend(x.to_le_bytes());
        out.extend(y.to_le_bytes());
    }
    for i in 0..n {
        out.extend(0u32.to_le_bytes());
        out.extend(i.to_le_bytes());
        out.extend([0.0f32, 0.0].iter().flat_map(|f| f.to_le_bytes()));
        out.extend(((i + 1) % n).to_le_bytes());
        out.extend([0.0f32, 0.0].iter().flat_map(|f| f.to_le_bytes()));
    }
    if region {
        out.extend(1u32.to_le_bytes());
        out.extend(1u32.to_le_bytes());
        out.extend(n.to_le_bytes());
        for i in 0..n {
            out.extend(i.to_le_bytes());
        }
    }
    out
}

/// A path with a style id, which Studio's own outline blobs do not carry.
fn path(rule: &str, blob: u32) -> V {
    msg(vec![("windingRule", en(rule)), ("styleID", V::Uint(1)), ("commandsBlob", V::Uint(blob))])
}

/// More than 5,000 entries and a payload over 2 MB: the importer's large-file guards skip the outlines of a frame.
fn large(mut nodes: Vec<V>, mut table: Vec<Vec<u8>>) -> Vec<u8> {
    nodes.extend((1..=5_001u32).map(|i| node((9, i), (0, 1), "z", "FRAME", "Filler", vec![("size", vec2(4.0, 4.0))])));
    table.push(vec![0u8; 2_100_000]);
    fig_file(nodes, table)
}

const FRAME: (u32, u32) = (2, 1);
const VECTOR: (u32, u32) = (2, 2);
const FLAGGED: (u32, u32) = (2, 3);
const STROKED: (u32, u32) = (2, 4);

/// Blobs of the vector fixtures: a decoy first, so a blob number kept as it was would name the wrong bytes.
const DECOY: u32 = 0;
const FRAME_FILL: u32 = 1;
const FRAME_STROKE: u32 = 2;
const VECTOR_NETWORK: u32 = 3;
const FLAGGED_NETWORK: u32 = 4;
const STROKED_NETWORK: u32 = 5;
const STROKED_OUTLINE: u32 = 6;

fn triangle() -> [(f32, f32); 3] {
    [(0.0, 0.0), (200.0, 0.0), (100.0, 100.0)]
}

fn table() -> Vec<Vec<u8>> {
    vec![
        icon_blob(),
        rect_commands(40.0, 30.0),
        rect_commands(41.0, 31.0),
        network_blob(&triangle(), true),
        network_blob(&triangle(), false),
        network_blob(&triangle(), true),
        rect_commands(100.0, 50.0),
    ]
}

/// A frame with outlines, a filled vector, a vector the importer flags as region-less, and a stroked vector with an outline of its
/// stroke. Every network is stored in a normalised size twice the node's, which Studio's re-encode does not keep.
fn nodes() -> Vec<V> {
    let mut all = document_and_page();
    all.push(node(
        FRAME,
        (0, 1),
        "a",
        "FRAME",
        "Frame",
        vec![
            ("size", vec2(40.0, 30.0)),
            ("fillPaints", V::Array(vec![solid(0.5, 0.5, 0.5)])),
            ("fillGeometry", V::Array(vec![path("NONZERO", FRAME_FILL)])),
            ("strokePaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
            ("strokeWeight", fl(1.0)),
            ("strokeAlign", en("INSIDE")),
            ("strokeGeometry", V::Array(vec![path("ODD", FRAME_STROKE)])),
        ],
    ));
    let vector = |id, pos: &str, name: &str, blob: u32, extra: Vec<(&str, V)>| {
        let mut fields = vec![
            ("size", vec2(100.0, 50.0)),
            ("fillPaints", V::Array(vec![solid(0.2, 0.4, 0.6)])),
            ("vectorData", msg(vec![("vectorNetworkBlob", V::Uint(blob)), ("normalizedSize", vec2(200.0, 100.0))])),
        ];
        fields.extend(extra);
        node(id, (0, 1), pos, "VECTOR", name, fields)
    };
    all.push(vector(VECTOR, "b", "Filled", VECTOR_NETWORK, vec![("cornerRadius", fl(3.0))]));
    all.push(vector(FLAGGED, "c", "Flagged", FLAGGED_NETWORK, vec![]));
    all.push(vector(
        STROKED,
        "d",
        "Stroked",
        STROKED_NETWORK,
        vec![
            ("strokePaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
            ("strokeWeight", fl(2.0)),
            ("strokeAlign", en("CENTER")),
            ("strokeGeometry", V::Array(vec![path("NONZERO", STROKED_OUTLINE)])),
        ],
    ));
    all
}

fn small_file() -> Vec<u8> {
    fig_file(nodes(), table())
}

fn large_file() -> Vec<u8> {
    large(nodes(), table())
}

/// The vector of `doc` named `name`, to edit.
fn vector_mut<'a>(doc: &'a mut Document, name: &str) -> &'a mut designos_doc::Node {
    let id = by_name(doc, name);
    doc.get_mut(id).expect("the vector")
}

/// A document as the file is read and then edited, and the same document with no store: the file Studio wrote before the store.
fn edited_pair(file: &[u8], edit: impl Fn(&mut Document)) -> (Vec<u8>, Vec<u8>) {
    let mut with_store = import_fig(file).expect("imports");
    let mut without = import_fig(file).expect("imports");
    without.fig_kept = Default::default();
    edit(&mut with_store);
    edit(&mut without);
    (export_fig(&with_store).expect("saves"), export_fig(&without).expect("saves"))
}

#[test]
fn a_large_files_frame_keeps_the_outlines_figma_stored_for_it() {
    let original = large_file();
    let doc = import_fig(&original).expect("imports");
    let frame = doc.get(by_name(&doc, "Frame")).expect("the frame");
    assert!(frame.fill_geometry.is_none() && frame.stroke_geometry.is_none(), "the importer skips the outlines of a large file's frame");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(report.nodes_edited, 0, "{report:?}");
    for key in ["fillGeometry", "strokeGeometry"] {
        let was = outline(&original, FRAME, key).expect("the file has the outline");
        assert_eq!(outline(&saved, FRAME, key), Some(was), "{key}: Figma's commands and winding rule, through this save's own blob table");
    }
    let (fill, stroke) = (outline(&saved, FRAME, "fillGeometry").unwrap(), outline(&saved, FRAME, "strokeGeometry").unwrap());
    assert_eq!(fill[0].1, rect_commands(40.0, 30.0));
    assert_eq!(stroke[0].1, rect_commands(41.0, 31.0));
}

#[test]
fn a_vectors_network_is_figmas_after_a_save_with_no_edit() {
    for original in [small_file(), large_file()] {
        let doc = import_fig(&original).expect("imports");
        let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
        assert_eq!(report.nodes_edited, 0, "{report:?}");
        for id in [VECTOR, FLAGGED, STROKED] {
            assert_eq!(network(&saved, id), network(&original, id), "{id:?}: Figma's network bytes, in Figma's normalised size");
            assert_eq!(normalized_size(&saved, id), normalized_size(&original, id), "{id:?}: the size the blob is stored in");
        }
        assert_eq!(network(&saved, VECTOR), Some(network_blob(&triangle(), true)));
        assert_eq!(report.fields_dropped_by_reason.get("blob_missing"), None, "{report:?}");
    }
}

#[test]
fn an_outline_written_from_the_store_is_a_decoded_value_with_its_blob_added_to_the_table_of_this_save() {
    let original = small_file();
    let doc = import_fig(&original).expect("imports");
    let saved = export_fig(&doc).expect("saves");
    let table = blobs(&saved);
    let all = records(&saved);
    for (id, key) in [(FRAME, "fillGeometry"), (FRAME, "strokeGeometry"), (STROKED, "strokeGeometry")] {
        for p in record_of(&all, id).get(key).and_then(V::as_array).expect("the outline") {
            let number = blob_number(p.get("commandsBlob").expect("commands"));
            assert!(number < table.len(), "{id:?} {key}: blob {number} is in this save's table of {}", table.len());
        }
    }
    // the whole path is Figma's, the style id Studio's own outlines do not carry included
    for (id, key) in [(FRAME, "fillGeometry"), (FRAME, "strokeGeometry"), (STROKED, "strokeGeometry")] {
        assert_eq!(outline(&saved, id, key), outline(&original, id, key), "{id:?} {key}");
    }
    // the decoy of the file's table is not what the outline names
    let frame_fill = outline(&saved, FRAME, "fillGeometry").expect("the outline");
    assert_ne!(frame_fill[0].1, icon_blob());
    assert_eq!(DECOY, 0, "the file's blob 0 is a decoy: a number copied as it was would read it");
}

#[test]
fn a_vectors_outline_and_network_are_studios_after_a_vertex_nudge() {
    let original = small_file();
    let (saved, no_store) = edited_pair(&original, |doc| {
        let n = vector_mut(doc, "Filled");
        n.drop_stroke_outline();
        let NodeKind::Vector { paths, network: Some(net) } = &mut n.kind else { panic!("a network vector") };
        net.vertices[1].pos = Vec2::new(net.vertices[1].pos.x + 5.0, net.vertices[1].pos.y);
        *paths = net.to_paths();
    });
    assert_ne!(network(&saved, VECTOR), network(&original, VECTOR), "the nudge is saved");
    assert_eq!(network(&saved, VECTOR), network(&no_store, VECTOR), "Studio's own network, as it was written before the store");
    assert_eq!(normalized_size(&saved, VECTOR), normalized_size(&no_store, VECTOR));
    assert_eq!(outline(&saved, VECTOR, "fillGeometry"), outline(&no_store, VECTOR, "fillGeometry"));
    // a layer nobody touched keeps Figma's network in the same save
    assert_eq!(network(&saved, FLAGGED), network(&original, FLAGGED));
}

#[test]
fn a_flagged_vector_keeps_figmas_bytes_until_the_bucket_materialises_its_regions() {
    let original = small_file();
    let doc = import_fig(&original).expect("imports");
    let NodeKind::Vector { network: Some(net), .. } = &doc.get(by_name(&doc, "Flagged")).expect("the vector").kind else { panic!("a network vector") };
    assert!(net.fills_closed_loops_without_regions && net.regions.is_empty(), "the importer flags a network with no regions");
    let saved = export_fig(&doc).expect("saves");
    assert_eq!(network(&saved, FLAGGED), network(&original, FLAGGED), "unedited: Figma's bytes, no regions");

    let (saved, no_store) = edited_pair(&original, |doc| {
        let n = vector_mut(doc, "Flagged");
        let NodeKind::Vector { network: Some(net), .. } = &mut n.kind else { panic!("a network vector") };
        assert!(net.materialize_closed_loop_fill(), "the bucket materialises the loops");
        assert!(!net.regions.is_empty());
    });
    assert_ne!(network(&saved, FLAGGED), network(&original, FLAGGED), "materialised regions are Studio's");
    assert_eq!(network(&saved, FLAGGED), network(&no_store, FLAGGED));
}

#[test]
fn a_flagged_vector_whose_materialise_changed_nothing_is_still_an_edited_layer() {
    // a network with no closed loop has nothing to turn into a region, but dropping the flag is an edit of the layer
    let mut all = document_and_page();
    let open = {
        let mut out = Vec::new();
        for count in [2u32, 1, 0] {
            out.extend(count.to_le_bytes());
        }
        for (x, y) in [(0.0f32, 0.0f32), (200.0, 0.0)] {
            out.extend(0u32.to_le_bytes());
            out.extend(x.to_le_bytes());
            out.extend(y.to_le_bytes());
        }
        out.extend(0u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend([0.0f32, 0.0].iter().flat_map(|f| f.to_le_bytes()));
        out.extend(1u32.to_le_bytes());
        out.extend([0.0f32, 0.0].iter().flat_map(|f| f.to_le_bytes()));
        out
    };
    all.push(node(
        (2, 2),
        (0, 1),
        "b",
        "VECTOR",
        "Line",
        vec![
            ("size", vec2(100.0, 0.0)),
            ("strokePaints", V::Array(vec![solid(0.0, 0.0, 0.0)])),
            ("strokeWeight", fl(1.0)),
            ("vectorData", msg(vec![("vectorNetworkBlob", V::Uint(0)), ("normalizedSize", vec2(200.0, 0.0))])),
        ],
    ));
    let original = fig_file(all, vec![open]);
    let (saved, no_store) = edited_pair(&original, |doc| {
        let NodeKind::Vector { network: Some(net), .. } = &mut vector_mut(doc, "Line").kind else { panic!("a network vector") };
        net.materialize_closed_loop_fill();
    });
    assert_eq!(network(&saved, (2, 2)), network(&no_store, (2, 2)));
}

#[test]
fn an_edited_stroke_weight_or_alignment_writes_studios_stroke_outline() {
    let original = small_file();
    let was = outline(&original, STROKED, "strokeGeometry").expect("the file's outline");
    assert_eq!(outline(&small_file_saved_unedited(), STROKED, "strokeGeometry"), Some(was.clone()), "unedited: Figma's outline");
    let heavier: fn(&mut Document) = |doc| vector_mut(doc, "Stroked").stroke.weight = 6.0;
    let outside: fn(&mut Document) = |doc| vector_mut(doc, "Stroked").stroke.align = designos_doc::StrokeAlign::Outside;
    for edit in [heavier, outside] {
        let (saved, no_store) = edited_pair(&original, edit);
        assert_ne!(outline(&saved, STROKED, "strokeGeometry"), Some(was.clone()), "the outline of the old stroke is not written");
        assert_eq!(outline(&saved, STROKED, "strokeGeometry"), outline(&no_store, STROKED, "strokeGeometry"), "Studio's own, as before the store");
    }
}

fn small_file_saved_unedited() -> Vec<u8> {
    export_fig(&import_fig(&small_file()).expect("imports")).expect("saves")
}

#[test]
fn a_resized_layer_writes_studios_geometry() {
    for original in [small_file(), large_file()] {
        let (saved, no_store) = edited_pair(&original, |doc| {
            for name in ["Frame", "Filled"] {
                let n = vector_mut(doc, name);
                n.size = Vec2::new(n.size.x * 2.0, n.size.y);
            }
        });
        for id in [FRAME, VECTOR] {
            for key in ["fillGeometry", "strokeGeometry"] {
                assert_eq!(outline(&saved, id, key), outline(&no_store, id, key), "{id:?} {key}: Studio's, as before the store");
            }
        }
        assert_eq!(network(&saved, VECTOR), network(&no_store, VECTOR));
        assert_ne!(network(&saved, VECTOR), network(&original, VECTOR));
        assert_eq!(network(&saved, FLAGGED), network(&original, FLAGGED), "the layer nobody touched keeps Figma's network");
    }
}

#[test]
fn a_no_edit_save_of_a_vector_with_a_node_corner_radius_reopens_to_the_same_layer() {
    let first = import_fig(&small_file()).expect("imports");
    let saved = export_fig(&first).expect("saves");
    let second = import_fig(&saved).expect("reopens");
    for name in ["Filled", "Flagged", "Stroked", "Frame"] {
        let (a, b) = (first.get(by_name(&first, name)).unwrap(), second.get(by_name(&second, name)).unwrap());
        assert_eq!(
            designos_doc::branch_diff::node_content_hash(a),
            designos_doc::branch_diff::node_content_hash(b),
            "{name} reopens the same\n{}\n{}",
            serde_json::to_string(a).unwrap(),
            serde_json::to_string(b).unwrap()
        );
    }
}

#[test]
fn the_vector_record_keeps_the_node_radius_the_network_was_stored_with() {
    // the vertices of a network read back under the radius the record carries: both come from the file, or neither does
    let original = small_file();
    let saved = export_fig(&import_fig(&original).expect("imports")).expect("saves");
    let (was, now) = (records(&original), records(&saved));
    assert_eq!(record_of(&now, VECTOR).get("cornerRadius"), record_of(&was, VECTOR).get("cornerRadius"));
    assert_eq!(network(&saved, VECTOR), network(&original, VECTOR));
}

#[test]
fn a_layer_written_from_the_store_has_the_outlines_the_file_had_and_none_the_file_did_not_store() {
    // the filled vector has a network and no outline in the file; Studio would work out a fill outline for it
    let original = small_file();
    let doc = import_fig(&original).expect("imports");
    assert!(doc.get(by_name(&doc, "Filled")).expect("the vector").fill_geometry.is_none(), "the file stores no fill outline for it");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let all = records(&saved);
    for id in [VECTOR, FLAGGED] {
        assert!(record_of(&all, id).get("fillGeometry").is_none(), "{id:?}: no outline the file did not store");
        assert!(record_of(&all, id).get("strokeGeometry").is_none());
    }
    assert!(report.studio_fields_not_in_the_file > 0, "{report:?}");
}

#[test]
fn the_blobs_studio_worked_out_for_a_field_the_store_wrote_are_not_left_in_the_table() {
    let original = small_file();
    let saved = export_fig(&import_fig(&original).expect("imports")).expect("saves");
    let (all, table) = (records(&saved), blobs(&saved));
    let mut named = std::collections::BTreeSet::new();
    for r in &all {
        for key in ["fillGeometry", "strokeGeometry"] {
            for p in r.get(key).and_then(V::as_array).into_iter().flatten() {
                named.insert(blob_number(p.get("commandsBlob").expect("commands")));
            }
        }
        if let Some(vd) = r.get("vectorData") {
            named.insert(blob_number(vd.get("vectorNetworkBlob").expect("a network")));
        }
    }
    for (i, b) in table.iter().enumerate() {
        assert!(b.is_empty() || named.contains(&i), "blob {i} of {} bytes is named by no record", b.len());
    }
    // what Figma stored is all there
    assert_eq!(named.len(), 6, "the frame's two outlines, three networks and the stroked vector's outline: {named:?}");
}
