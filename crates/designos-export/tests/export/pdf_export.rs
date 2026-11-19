//! Selected frames export as one multi-page PDF through the same `ExportSetting`
//! entry point the inspector uses for PNG / JPG / SVG.

use designos_doc::{Color, Document, Node, NodeId, Paint, Rect};
use designos_export::{ExportFormat, ExportScale, ExportSetting};

fn pdf_setting() -> ExportSetting {
    ExportSetting { scale: ExportScale::Factor(1.0), format: ExportFormat::Pdf }
}

fn doc_with_frames(sizes: &[(f32, f32)]) -> (Document, Vec<NodeId>) {
    let mut d = Document::new("Screens");
    let page = d.add_page("P");
    let mut ids = Vec::new();
    for (i, (w, h)) in sizes.iter().enumerate() {
        let id = d.alloc_id();
        let mut n = Node::frame(id, Rect::new(i as f32 * 500.0, 0.0, *w, *h));
        n.fills = vec![Paint::solid(Color::rgb(0, 128, 255))];
        d.insert(page, n).unwrap();
        ids.push(id);
    }
    (d, ids)
}

/// Page objects only: `/Type /Pages` (the tree root) has an `s` after `Page`.
fn page_count(pdf: &[u8]) -> usize {
    let needle = b"/Type /Page ";
    pdf.windows(needle.len()).filter(|w| *w == needle).count()
}

#[test]
fn pdf_is_listed_with_label_and_extension_and_has_no_scale_suffix() {
    assert!(ExportFormat::ALL.contains(&ExportFormat::Pdf));
    assert_eq!(ExportFormat::Pdf.label(), "PDF");
    assert_eq!(ExportFormat::Pdf.extension(), "pdf");
    assert_eq!(pdf_setting().file_name("Hero"), "Hero.pdf");
}

#[test]
fn two_frames_make_one_two_page_pdf() {
    let (d, ids) = doc_with_frames(&[(40.0, 20.0), (20.0, 40.0)]);
    let pdf = pdf_setting().render_many(&d, &ids).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(page_count(&pdf), 2);
    assert!(String::from_utf8_lossy(&pdf).contains("/Count 2"));
}

#[test]
fn one_frame_makes_a_one_page_pdf() {
    let (d, ids) = doc_with_frames(&[(40.0, 20.0)]);
    let pdf = pdf_setting().render_many(&d, &ids).unwrap();
    assert_eq!(page_count(&pdf), 1);
    // the single-node path used by the inspector row writes the same document
    assert_eq!(pdf_setting().render(&d, ids[0]), pdf);
}

#[test]
fn oversized_frame_is_an_error_not_a_panic() {
    let (d, ids) = doc_with_frames(&[(40.0, 20.0), (20_000.0, 100.0)]);
    assert!(pdf_setting().render_many(&d, &ids).is_err());
    // the single-node path yields "nothing rendered" like the raster formats
    assert!(pdf_setting().render(&d, ids[1]).is_empty());
}

#[test]
fn page_limit_and_empty_selection_are_errors() {
    let (d, ids) = doc_with_frames(&[(10.0, 10.0)]);
    assert!(pdf_setting().render_many(&d, &vec![ids[0]; 501]).is_err());
    assert!(pdf_setting().render_many(&d, &[]).is_err());
}

#[test]
fn non_pdf_formats_take_exactly_one_layer() {
    let (d, ids) = doc_with_frames(&[(10.0, 10.0), (10.0, 10.0)]);
    let png = ExportSetting { scale: ExportScale::Factor(1.0), format: ExportFormat::Png };
    assert!(png.render_many(&d, &ids).is_err());
    let one = png.render_many(&d, &ids[..1]).unwrap();
    assert_eq!(&one[..4], &[0x89, b'P', b'N', b'G']);
}
