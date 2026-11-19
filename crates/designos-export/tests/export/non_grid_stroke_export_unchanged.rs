//! Strokes and padding of nodes that are not grid frames: a Center or Outside stroke is an `outline`
//! in both CSS and Tailwind, an Inside stroke a `border`, and the padding written is the stored
//! padding. Only a grid frame folds its stroke into its padding.

use designos_doc::*;
use designos_export::{css_for, tailwind_for};

struct Case {
    label: &'static str,
    css: &'static str,
    tailwind: &'static str,
}

fn frame(doc: &mut Document, parent: NodeId, direction: LayoutDirection, f: impl FnOnce(&mut Node, &mut AutoLayout)) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 100.0));
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
    n.stroke.weight = 2.0;
    let mut al = AutoLayout { direction, padding: [8.0, 8.0, 8.0, 8.0], layout_version: 0, stroke_included: true, ..Default::default() };
    f(&mut n, &mut al);
    n.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    doc.insert(parent, n).unwrap()
}

fn outputs() -> Vec<(&'static str, String, String)> {
    let mut doc = Document::new("non-grid");
    let page = doc.add_page("Page 1");
    let mut out = Vec::new();
    let mut record = |label: &'static str, doc: &Document, id: NodeId| out.push((label, css_for(doc, id), tailwind_for(doc, id)));

    let center = frame(&mut doc, page, LayoutDirection::Horizontal, |n, _| n.stroke.align = StrokeAlign::Center);
    let outside = frame(&mut doc, page, LayoutDirection::Vertical, |n, _| n.stroke.align = StrokeAlign::Outside);
    let inside = frame(&mut doc, page, LayoutDirection::Horizontal, |n, _| n.stroke.align = StrokeAlign::Inside);
    let sided = frame(&mut doc, page, LayoutDirection::Horizontal, |n, _| n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]));
    let v1_inside = frame(&mut doc, page, LayoutDirection::Horizontal, |_, al| al.layout_version = 1);
    let dashed_center = frame(&mut doc, page, LayoutDirection::Vertical, |n, _| {
        n.stroke.align = StrokeAlign::Center;
        n.stroke.dashes = vec![4.0, 2.0];
    });
    let neg_row = frame(&mut doc, page, LayoutDirection::Horizontal, |n, al| {
        n.stroke.paints.clear();
        al.padding = [-10.0, 20.0, 0.0, 20.0];
    });
    let neg_all = frame(&mut doc, page, LayoutDirection::Vertical, |n, al| {
        n.stroke.paints.clear();
        al.padding = [-10.0; 4];
    });
    let neg_grid = frame(&mut doc, page, LayoutDirection::Grid, |n, al| {
        n.stroke.paints.clear();
        al.padding = [-10.0, 20.0, 0.0, 20.0];
    });
    let rect = doc.alloc_id();
    let mut r = Node::rectangle(rect, Rect::new(0.0, 0.0, 40.0, 40.0));
    r.fills.clear();
    r.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
    r.stroke.weight = 3.0;
    r.stroke.align = StrokeAlign::Center;
    let rect = doc.insert(page, r).unwrap();

    record("flex row, Center stroke", &doc, center);
    record("flex column, Outside stroke", &doc, outside);
    record("flex row, Inside stroke", &doc, inside);
    record("flex row, Inside stroke per side", &doc, sided);
    record("flex row, version 1 Inside stroke", &doc, v1_inside);
    record("flex column, dashed Center stroke", &doc, dashed_center);
    record("rectangle, Center stroke", &doc, rect);
    record("flex row, negative padding", &doc, neg_row);
    record("flex column, all sides negative padding", &doc, neg_all);
    record("grid without a stroke, negative padding", &doc, neg_grid);
    out
}

const GOLDEN: &[Case] = &[
    Case {
        label: "flex row, Center stroke",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\npadding: 8px;\nalign-items: flex-start;\noutline: 2px solid #333;\noutline-offset: -1px;",
        tailwind: "<div class=\"flex items-start p-2 w-[200px] h-[100px] outline outline-2 outline-[#333333] outline-offset-[-1px]\"></div>",
    },
    Case {
        label: "flex column, Outside stroke",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\npadding: 8px;\nflex-direction: column;\nalign-items: flex-start;\noutline: 2px solid #333;",
        tailwind: "<div class=\"flex flex-col items-start p-2 w-[200px] h-[100px] outline outline-2 outline-[#333333]\"></div>",
    },
    Case {
        label: "flex row, Inside stroke",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\npadding: 8px;\nalign-items: flex-start;\nborder: 2px solid #333;",
        tailwind: "<div class=\"flex items-start p-2 w-[200px] h-[100px] border-2 border-[#333333]\"></div>",
    },
    Case {
        label: "flex row, Inside stroke per side",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\npadding: 8px;\nalign-items: flex-start;\nborder-width: 4px 1px 2px 3px;\nborder-style: solid;\nborder-color: #333;",
        tailwind: "<div class=\"flex items-start p-2 w-[200px] h-[100px] border-t-4 border-r border-b-2 border-l-[3px] border-[#333333]\"></div>",
    },
    Case {
        label: "flex row, version 1 Inside stroke",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\npadding: 8px;\nalign-items: flex-start;\nborder: 2px solid #333;",
        tailwind: "<div class=\"flex items-start p-2 w-[200px] h-[100px] border-2 border-[#333333]\"></div>",
    },
    Case {
        label: "flex column, dashed Center stroke",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\npadding: 8px;\nflex-direction: column;\nalign-items: flex-start;\noutline: 2px dashed #333;\noutline-offset: -1px;",
        tailwind: "<div class=\"flex flex-col items-start p-2 w-[200px] h-[100px] outline outline-2 outline-dashed outline-[#333333] outline-offset-[-1px]\"></div>",
    },
    Case {
        label: "rectangle, Center stroke",
        css: "width: 40px;\nheight: 40px;\noutline: 3px solid #333;\noutline-offset: -1.5px;",
        tailwind: "<div class=\"w-10 h-10 outline outline-[3px] outline-[#333333] outline-offset-[-1.5px]\"></div>",
    },
    Case {
        label: "flex row, negative padding",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\npadding: -10px 20px 0px;\nalign-items: flex-start;",
        tailwind: "<div class=\"flex items-start px-5 pt-[-10px] w-[200px] h-[100px]\"></div>",
    },
    Case {
        label: "flex column, all sides negative padding",
        css: "display: flex;\nwidth: 200px;\nheight: 100px;\nflex-direction: column;\nalign-items: flex-start;",
        tailwind: "<div class=\"flex flex-col items-start p-[-10px] w-[200px] h-[100px]\"></div>",
    },
    Case {
        label: "grid without a stroke, negative padding",
        css: "display: grid;\nwidth: 200px;\nheight: 100px;\npadding: -10px 20px 0px;\nbox-sizing: border-box;\ngrid-template-columns: repeat(3, minmax(0, 1fr));\nalign-content: start;",
        tailwind: "<div class=\"grid grid-cols-3 content-start box-border px-5 pt-[-10px] w-[200px] h-[100px]\"></div>",
    },
];

#[test]
fn non_grid_strokes_and_padding_export_as_before() {
    let got = outputs();
    if std::env::var_os("NON_GRID_STROKE_DUMP").is_some() {
        for (label, css, tw) in &got {
            println!("=== {label}\ncss: {css:?}\ntailwind: {tw:?}");
        }
        return;
    }
    assert_eq!(got.len(), GOLDEN.len(), "one golden per case");
    for ((label, css, tw), want) in got.iter().zip(GOLDEN) {
        assert_eq!(*label, want.label);
        assert_eq!(css, want.css, "{label}: CSS");
        assert_eq!(tw, want.tailwind, "{label}: Tailwind");
    }
}
