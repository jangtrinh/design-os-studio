use super::*;
use designos_docs::Page;
use egui::{Context, RawInput};

/// Lays `text` out at the column width inside a real egui pass (fonts only exist while one runs).
fn lay(text: &str) -> Laid {
    let ctx = Context::default();
    theme::install_fonts(&ctx);
    // The fonts take effect on the pass after they are installed.
    let _ = ctx.run_ui(RawInput::default(), |_| {});
    let mut out = None;
    let _ = ctx.run_ui(RawInput::default(), |ui| {
        let shape = |job: LayoutJob| ui.ctx().fonts_mut(|f| f.layout_job(job));
        out = Some(layout(&shape, "spec/page.md", &Page::parse(text), &TreeSnapshot::empty(), &[], 690.0));
    });
    out.expect("the pass ran")
}

const HEAD: &str = "---\ntitle: Payments\ndescription: How people pay.\n---\n";

#[test]
fn the_first_heading_sits_32_under_a_one_line_header_and_text_under_it_16_below_the_heading() {
    let laid = lay(&format!("{HEAD}\n## Goals\n\nPay by card.\n"));
    // 36 title + 12 + 26 description = 74, then the H2's 32 top margin.
    assert_eq!(laid.headings.len(), 1);
    assert_eq!(laid.headings[0].y, 106.0);
    assert_eq!(laid.headings[0].level, 2);
    assert_eq!(laid.headings[0].slug, "goals");
    // The paragraph starts 28 (heading line) + 16 below the heading's top.
    assert_eq!(laid.items[2].y, 150.0);
}

#[test]
fn a_table_is_followed_by_48_before_the_next_h2_and_gets_a_scroll_bar_when_it_is_wider_than_the_column() {
    let cell = "one two three four five six";
    let wide = format!("| A | B | C | D | E | F |\n| --- | --- | --- | --- | --- | --- |\n| {cell} | {cell} | {cell} | {cell} | {cell} | {cell} |\n");
    let laid = lay(&format!("{HEAD}\n## Table\n\n{wide}\n## After\n\nText.\n"));
    let table = laid.items.iter().find(|i| i.prims.iter().any(|p| matches!(p, Prim::HBar { .. }))).expect("a scroll bar under the wide table");
    let after = laid.headings.iter().find(|h| h.text == "After").unwrap();
    assert_eq!(after.y - (table.y + table.h), 48.0, "table then H2");
    let bar = table.prims.iter().find_map(|p| if let Prim::HBar { track, content_w, .. } = p { Some((*track, *content_w)) } else { None }).unwrap();
    assert!(bar.1 > 690.0, "the table is wider than the column: {}", bar.1);
    assert_eq!(bar.0.width(), 690.0);
}

#[test]
fn a_narrow_table_has_no_scroll_bar() {
    let laid = lay(&format!("{HEAD}\n## Table\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n"));
    assert!(!laid.items.iter().any(|i| i.prims.iter().any(|p| matches!(p, Prim::HBar { .. }))));
}

#[test]
fn a_comment_only_raw_block_renders_nothing_but_other_raw_text_does() {
    let quiet = lay(&format!("{HEAD}\n<!-- a note for editors -->\n"));
    assert_eq!(quiet.items.len(), 1, "only the header");
    let loud = lay(&format!("{HEAD}\n<div>kept as written</div>\n"));
    assert_eq!(loud.items.len(), 2, "the header and the raw block");
}

#[test]
fn the_first_h1_becomes_the_title_when_front_matter_has_none() {
    let laid = lay("# Heading as title\n\nBody text.\n");
    assert!(laid.headings.is_empty(), "the consumed H1 is not in the contents");
    assert_eq!(laid.items.len(), 2);
}
