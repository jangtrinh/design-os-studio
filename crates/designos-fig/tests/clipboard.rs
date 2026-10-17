//! "Paste from Figma": clipboard HTML -> `FigClipboard` -> `Document`, on payloads built from the
//! same fig-kiwi sample archive the `.fig` tests use (zip-wrapped, bare container, partial copy).

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use designos_doc::{Document, Node, NodeKind, Vec2};
use designos_fig::container::{write_container, Compression};
use designos_fig::kiwi::KiwiValue;
use designos_fig::testkit::{figma_like_schema, sample_fig, sample_message};
use designos_fig::{import_figma_clipboard, looks_like_figma_clipboard, parse_figma_clipboard_html};

const META_JSON: &str = r#"{"fileKey":"abc","pasteID":42,"dataType":"scene"}"#;

/// The wrapper Figma.app writes, with `buffer` as the `(figma)` payload.
fn figma_html(buffer: &[u8]) -> String {
    format!(
        "<meta charset=\"utf-8\"><div><span data-metadata=\"<!--(figmeta){}(/figmeta)-->\"></span><span data-buffer=\"<!--(figma){}(/figma)-->\"></span></div><span style=\"white-space:pre-wrap;\">Card</span>",
        STANDARD.encode(META_JSON),
        STANDARD.encode(buffer)
    )
}

fn escaped(html: &str) -> String {
    html.replace("<!--", "&lt;!--").replace("-->", "--&gt;")
}

fn paste(html: &str) -> Document {
    let clip = parse_figma_clipboard_html(html).expect("parse");
    import_figma_clipboard(&clip).expect("import")
}

fn top_level(doc: &Document) -> Vec<&Node> {
    doc.pages.iter().flat_map(|p| doc.get(*p).unwrap().children.iter().map(|c| doc.get(*c).unwrap())).collect()
}

fn names(doc: &Document) -> Vec<String> {
    let mut v: Vec<String> = top_level(doc).iter().map(|n| n.name.clone()).collect();
    v.sort();
    v
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

#[test]
fn zip_archive_payload_pastes_frames_text_vectors_images_and_instances() {
    let html = figma_html(&sample_fig(Compression::Zstd, true));
    assert!(looks_like_figma_clipboard(&html));
    let clip = parse_figma_clipboard_html(&html).unwrap();
    assert_eq!(clip.meta.as_ref().unwrap()["pasteID"], 42);

    let doc = import_figma_clipboard(&clip).unwrap();
    let tops = names(&doc);
    assert!(tops.contains(&"Card".to_string()), "{tops:?}");
    assert!(tops.contains(&"Pie".to_string()), "{tops:?}");

    let card = named(&doc, "Card");
    assert_eq!(card.size, Vec2::new(200.0, 300.0));
    assert!(card.kind.frame_props().unwrap().auto_layout.is_some());
    let NodeKind::Text(t) = &named(&doc, "Title").kind else { panic!("Title is text") };
    assert_eq!(t.content, "Hi \u{1F600}!");
    assert!(matches!(named(&doc, "Tri").kind, NodeKind::Vector { .. }));
    assert!(matches!(named(&doc, "Button copy").kind, NodeKind::Instance { .. }));
    // the image hash bytes de ad be ef ship in the archive's images/ folder
    assert!(doc.images.contains_key("deadbeef"), "{:?}", doc.images.keys().collect::<Vec<_>>());
}

#[test]
fn bare_container_payload_with_escaped_markers_matches_the_zip_payload() {
    let zipped = paste(&figma_html(&sample_fig(Compression::Zstd, true)));
    let bare = paste(&escaped(&figma_html(&sample_fig(Compression::Deflate, false))));
    assert_eq!(names(&bare), names(&zipped));
    assert!(bare.images.is_empty(), "a bare container ships no image bytes");
}

/// A copy of part of a page: the roots point at a canvas that is not in the payload.
fn partial_copy_buffer() -> Vec<u8> {
    let mut msg = sample_message();
    let Some(KiwiValue::Array(changes)) = msg.get_mut("nodeChanges") else { panic!("sample has nodeChanges") };
    changes.retain(|c| !matches!(c.get("type"), Some(KiwiValue::Enum(t)) if matches!(&**t, "DOCUMENT" | "CANVAS")));
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    write_container(&schema.encode(), &schema.encode_message(root, &msg).unwrap(), Compression::Zstd, 101)
}

#[test]
fn partial_copy_without_a_canvas_lands_on_a_synthetic_page() {
    let doc = paste(&figma_html(&partial_copy_buffer()));
    assert_eq!(doc.pages.len(), 1);
    assert_eq!(names(&doc), ["Card", "Note", "Pie", "Star"]);
    // children of the copied roots stay attached
    let card = named(&doc, "Card");
    assert!(card.children.len() >= 3, "{}", card.children.len());
    assert_eq!(doc.get(card.parent.unwrap()).unwrap().name, "Pasted");
}

#[test]
fn malformed_payloads_are_errors_not_panics() {
    let good = sample_fig(Compression::Zstd, true);
    // valid base64 that is not a fig archive
    let junk = parse_figma_clipboard_html(&figma_html(b"definitely not a fig file")).unwrap();
    assert!(import_figma_clipboard(&junk).is_err());
    // archive cut short at many offsets
    for cut in [0, 1, 2, 10, good.len() / 4, good.len() / 2, good.len() - 1] {
        let clip = parse_figma_clipboard_html(&figma_html(&good[..cut])).unwrap();
        assert!(import_figma_clipboard(&clip).is_err(), "cut at {cut}");
    }
    // container whose message chunk is garbage
    let schema = figma_like_schema();
    let bad = write_container(&schema.encode(), &[0xff; 64], Compression::Deflate, 101);
    let clip = parse_figma_clipboard_html(&figma_html(&bad)).unwrap();
    assert!(import_figma_clipboard(&clip).is_err());
    // wrapper problems
    assert!(parse_figma_clipboard_html("<b>just html</b>").is_err());
    assert!(parse_figma_clipboard_html("<!--(figma)@@@(/figma)-->").is_err());
    assert!(parse_figma_clipboard_html("<!--(figma)aGVsbG8=").is_err());
}

/// The sample payload with `Star` and `Note` (children of the canvas) re-parented onto a guid the payload does not
/// hold, as a copy of single layers out of a page that also ships the component masters. `drop_document` keeps
/// only the canvases.
fn orphaned_roots_beside_a_canvas_buffer(drop_document: bool) -> Vec<u8> {
    let mut msg = sample_message();
    let Some(KiwiValue::Array(changes)) = msg.get_mut("nodeChanges") else { panic!("sample has nodeChanges") };
    if drop_document {
        changes.retain(|c| !matches!(c.get("type"), Some(KiwiValue::Enum(t)) if &**t == "DOCUMENT"));
    }
    for c in changes.iter_mut() {
        let is_copied_root = matches!(c.get("name"), Some(KiwiValue::String(n)) if n == "Star" || n == "Note");
        if !is_copied_root {
            continue;
        }
        let Some(KiwiValue::Struct(fields)) = c.get_mut("parentIndex") else { panic!("parentIndex") };
        for (k, v) in fields.iter_mut() {
            if &**k == "guid" {
                *v = designos_fig::testkit::guid(7, 7);
            }
        }
    }
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    write_container(&schema.encode(), &schema.encode_message(root, &msg).unwrap(), Compression::Zstd, 101)
}

fn page_names(doc: &Document) -> Vec<String> {
    doc.pages.iter().map(|p| doc.get(*p).unwrap().name.clone()).collect()
}

#[test]
fn orphaned_roots_beside_a_document_and_canvas_are_adopted_onto_a_pasted_page() {
    let doc = paste(&figma_html(&orphaned_roots_beside_a_canvas_buffer(false)));
    assert_eq!(page_names(&doc), ["Page 1", "Pasted"], "the synthetic page sits under the payload's document, after its canvases");
    assert_eq!(names(&doc), ["Card", "Note", "Pie", "Star"], "orphans are kept, the canvas content stays");
    let pasted = doc.pages.iter().map(|p| doc.get(*p).unwrap()).find(|p| p.name == "Pasted").unwrap();
    let on_pasted: Vec<&str> = pasted.children.iter().map(|c| doc.get(*c).unwrap().name.as_str()).collect();
    assert_eq!(on_pasted.len(), 2);
    assert!(on_pasted.contains(&"Star") && on_pasted.contains(&"Note"), "{on_pasted:?}");
    // component masters on the canvas still resolve for the instance
    let NodeKind::Instance { component, .. } = &named(&doc, "Button copy").kind else { panic!("Button copy is an instance") };
    assert_eq!(*component, Some(named(&doc, "Button").id));
}

#[test]
fn orphaned_roots_beside_a_canvas_without_a_document_are_adopted_too() {
    let doc = paste(&figma_html(&orphaned_roots_beside_a_canvas_buffer(true)));
    assert_eq!(names(&doc), ["Card", "Note", "Pie", "Star"]);
    assert_eq!(doc.pages.len(), 2, "{:?}", page_names(&doc));
}

#[test]
fn a_payload_without_orphans_gets_no_synthetic_page() {
    let doc = paste(&figma_html(&sample_fig(Compression::Zstd, true)));
    assert_eq!(page_names(&doc), ["Page 1"]);
}

#[test]
fn a_clipboard_payload_inflating_past_the_clipboard_budget_is_an_error_not_an_abort() {
    // 300 MiB of zeros deflate to ~300 KB: small on the clipboard, over the 256 MiB clipboard inflate budget
    // (a whole `.fig` file may inflate 4 GiB, so only the clipboard path rejects it).
    let schema = figma_like_schema();
    let bomb = write_container(&schema.encode(), &vec![0u8; 300 << 20], Compression::Deflate, 101);
    assert!(bomb.len() < 4 << 20, "the payload itself stays small: {} bytes", bomb.len());
    let clip = parse_figma_clipboard_html(&figma_html(&bomb)).unwrap();
    let err = import_figma_clipboard(&clip).unwrap_err().to_string();
    assert!(err.contains("too large"), "{err}");
}
