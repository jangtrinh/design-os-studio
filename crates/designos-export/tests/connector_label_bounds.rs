use designos_doc::{board_text, sync_connector, ConnectorEnd, ConnectorProps, Document, Magnet, Node, Rect, TextAlignH, TextAlignV};
use designos_export::{export_svg, render_bounds, render_export, SvgOptions};

#[test]
fn connector_label_is_inside_its_export_bounds() {
    let mut doc = Document::new("connector export bounds");
    let page = doc.add_page("Page 1");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(40.0, 40.0, 500.0, 420.0))).unwrap();
    let top_box = doc.alloc_id();
    doc.insert(frame, Node::rectangle(top_box, Rect::new(180.0, 20.0, 80.0, 60.0))).unwrap();
    let bottom_box = doc.alloc_id();
    doc.insert(frame, Node::rectangle(bottom_box, Rect::new(180.0, 280.0, 80.0, 60.0))).unwrap();

    let cid = doc.alloc_id();
    let mut label = board_text(14.0, TextAlignH::Center, TextAlignV::Center);
    label.content = "MMMMMM\nWWWWWW".into();
    let props = ConnectorProps {
        start: ConnectorEnd::attached(top_box, Magnet::Bottom),
        end: ConnectorEnd::attached(bottom_box, Magnet::Top),
        label: Some(label),
        ..Default::default()
    };
    let c = doc.insert(frame, Node::connector(cid, props)).unwrap();
    assert!(sync_connector(&mut doc, c));

    let conn_node = doc.get(c).unwrap();
    let local_extent = Rect::new(0.0, 0.0, conn_node.size.x, conn_node.size.y);
    let export_rect = render_bounds(&doc, c);
    let lowered = designos_render::board::lower(&doc, conn_node).expect("connector lowers");
    for child_id in &lowered.children {
        let child = designos_render::board::get(&doc, *child_id).expect("synthetic child exists");
        let child_rect = Rect::new(child.transform.tx, child.transform.ty, child.size.x, child.size.y);
        assert!(
            local_extent.x <= child_rect.x
                && local_extent.y <= child_rect.y
                && local_extent.x + local_extent.w >= child_rect.x + child_rect.w
                && local_extent.y + local_extent.h >= child_rect.y + child_rect.h,
            "connector_extent local box {local_extent:?} must contain lowered child {child_rect:?}"
        );
        assert!(
            export_rect.x <= child_rect.x
                && export_rect.y <= child_rect.y
                && export_rect.x + export_rect.w >= child_rect.x + child_rect.w
                && export_rect.y + export_rect.h >= child_rect.y + child_rect.h,
            "export_rect {export_rect:?} must contain lowered child {child_rect:?}"
        );
    }

    let png = render_export(&doc, c, 1.0).expect("connector exports to PNG");
    assert!(png.width() as f32 >= local_extent.w.floor() && png.height() as f32 >= local_extent.h.floor());

    let svg = export_svg(&doc, c, &SvgOptions { outline_text: false });
    assert!(svg.contains("MMMMMM"), "connector SVG export includes label line 1");
    assert!(svg.contains("WWWWWW"), "connector SVG export includes label line 2");
}
