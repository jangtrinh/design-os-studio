//! Editing blocks: only the edited block is rewritten, the rewrite reads back as what was written, and neighbours
//! (lists, line endings, table cells) are not disturbed.

mod common;

use common::{names, parse, FIXTURES, UNKNOWN};
use designos_docs::{BlockKind, Group, Inline, ListItem, Page};

fn text(s: &str) -> Vec<Inline> {
    vec![Inline::Text(s.into())]
}

fn reparsed_names(page: &Page) -> Vec<&'static str> {
    names(&parse(&page.render()))
}

#[test]
fn editing_one_block_leaves_every_other_byte_alone() {
    let mut page = parse(UNKNOWN);
    assert_eq!(page.blocks[0].kind().name(), "heading");
    *page.blocks[0].kind_mut() = BlockKind::Heading { level: 2, content: vec![Inline::Text("Renamed".into())] };
    let expected = UNKNOWN.replacen("# Known heading", "## Renamed", 1);
    assert_eq!(page.render(), expected);
}

#[test]
fn inserting_and_removing_blocks_keeps_blank_line_hygiene() {
    let mut page = parse(FIXTURES[0].1);
    page.insert_block(1, BlockKind::Paragraph(vec![Inline::Text("Between.".into())]));
    assert_eq!(page.render(), "# Heading one\n\nBetween.\n\n## Heading two\n\n### Heading three\n");
    page.remove_block(0).unwrap();
    assert_eq!(page.render(), "Between.\n\n## Heading two\n\n### Heading three\n");
    page.remove_block(2).unwrap();
    assert_eq!(page.render(), "Between.\n\n## Heading two\n");
}

#[test]
fn new_pages_and_front_matter_edits_are_written_canonically() {
    let mut page = Page::default();
    page.set_title(Some("My page"));
    page.set_description(Some("Has: a colon"));
    page.set_group(Some(Group::Spec));
    page.push_block(BlockKind::Heading { level: 1, content: vec![Inline::Text("My page".into())] });
    let text = page.render();
    assert_eq!(text, "---\ntitle: My page\ndescription: \"Has: a colon\"\ngroup: spec\n---\n\n# My page\n");
    let again = parse(&text);
    assert_eq!(again.front_matter().and_then(|f| f.description()), Some("Has: a colon"));
    assert_eq!(again.render(), text);

    page.set_title(None);
    assert!(!page.render().contains("title:"));
}

#[test]
fn a_rewritten_list_next_to_another_list_does_not_merge_with_it() {
    let mut page = parse("- a\n\n* b\n\n1. one\n\n2) two\n");
    assert_eq!(names(&page), ["bullet_list", "bullet_list", "numbered_list", "numbered_list"]);
    *page.blocks[1].kind_mut() = BlockKind::BulletList(vec![ListItem::new(vec![Inline::Text("B".into())])]);
    *page.blocks[3].kind_mut() = BlockKind::NumberedList { start: 2, items: vec![ListItem::new(vec![Inline::Text("Two".into())])] };
    for text in [page.render(), page.render_canonical()] {
        assert_eq!(names(&parse(&text)), ["bullet_list", "bullet_list", "numbered_list", "numbered_list"], "{text:?}");
    }
}

#[test]
fn heading_text_ending_in_hashes_and_odd_fence_info_survive_a_rewrite() {
    // A closing sequence is dropped on read; text that merely ends in `#` is escaped on write so it is kept.
    assert_eq!(parse("## C \\#\n\n### Issue ####\n").render_canonical(), "## C \\#\n\n### Issue\n");
    let mut page = Page::default();
    page.push_block(BlockKind::Heading { level: 3, content: vec![Inline::Text("Tags ###".into())] });
    let again = parse(&page.render());
    let BlockKind::Heading { content, .. } = again.blocks[0].kind() else { panic!("heading expected") };
    assert_eq!(content, &[Inline::Text("Tags ##\\#".into())]);

    let page = parse("```a\\&b\nx\n```\n");
    assert_eq!(page.blocks[0].kind(), &BlockKind::Code { info: "a\\&b".into(), code: "x".into() });
    assert_eq!(page.render_canonical(), "```a\\&b\nx\n```\n");
}

#[test]
fn pipes_typed_into_cells_and_code_spans_are_escaped_when_written() {
    let mut page = parse("| a | b |\n| --- | --- |\n| 1 | 2 |\n");
    let BlockKind::Table { rows, header, .. } = page.blocks[0].kind_mut() else { panic!("table expected") };
    rows[0][0] = text("x|y");
    rows[0][1] = vec![Inline::Code("p|q".into())];
    header[0] = text("h\\|already escaped");
    let written = page.render();
    let again = parse(&written);
    let BlockKind::Table { header, rows, .. } = again.blocks[0].kind() else { panic!("still a table, got {:?} from {written:?}", again.blocks[0].kind()) };
    assert_eq!((header.len(), rows.len(), rows[0].len()), (2, 1, 2), "{written:?}");
    assert_eq!(designos_docs::plain_text(&rows[0][0]), "x|y");
    assert_eq!(designos_docs::plain_text(&header[0]), "h|already escaped");
    assert!(written.contains("`p\\|q`"), "{written:?}");
}

// ------------------------------------------------------------------ M1 adjacent lists

#[test]
fn a_new_list_next_to_a_list_stays_a_separate_list() {
    let mut page = parse("- x\n");
    page.insert_block(0, BlockKind::BulletList(vec![ListItem::new(text("new"))]));
    assert_eq!(reparsed_names(&page), ["bullet_list", "bullet_list"], "{:?}", page.render());

    let mut page = parse("1. a\n");
    page.insert_block(0, BlockKind::NumberedList { start: 1, items: vec![ListItem::new(text("new"))] });
    assert_eq!(reparsed_names(&page), ["numbered_list", "numbered_list"], "{:?}", page.render());

    let mut page = parse("para\n\n- x\n");
    *page.blocks[0].kind_mut() = BlockKind::BulletList(vec![ListItem::new(text("para"))]);
    assert_eq!(reparsed_names(&page), ["bullet_list", "bullet_list"], "{:?}", page.render());

    let mut page = parse("- a\n\n* b\n");
    page.insert_block(1, BlockKind::BulletList(vec![ListItem::new(text("mid"))]));
    assert_eq!(reparsed_names(&page), ["bullet_list", "bullet_list", "bullet_list"], "{:?}", page.render());
}

#[test]
fn a_numbered_list_between_a_dot_list_and_a_paren_list_is_kept_apart_by_a_comment() {
    let mut page = parse("1. a\n\n1) b\n");
    page.insert_block(1, BlockKind::NumberedList { start: 1, items: vec![ListItem::new(text("mid"))] });
    let again = parse(&page.render());
    let lists: Vec<&BlockKind> = again.blocks.iter().map(|b| b.kind()).filter(|k| matches!(k, BlockKind::NumberedList { .. })).collect();
    assert_eq!(lists.len(), 3, "three separate lists must survive: {:?}", page.render());
    for (kind, expected) in lists.iter().zip(["a", "mid", "b"]) {
        let BlockKind::NumberedList { items, .. } = kind else { unreachable!() };
        assert_eq!(items.len(), 1, "{expected}: {:?}", page.render());
    }
}

// ------------------------------------------------------------------ m1, m2 front matter detection

#[test]
fn edited_and_inserted_blocks_keep_the_crlf_line_endings_of_the_file() {
    let src = "# A\r\n\r\n> [!note]\r\n> one\r\n> two\r\n\r\npara\r\n";
    let mut page = parse(src);
    assert_eq!(page.render(), src);
    let kind = page.blocks[1].kind().clone();
    *page.blocks[1].kind_mut() = kind;
    page.push_block(BlockKind::Paragraph(text("new")));
    page.push_block(BlockKind::BulletList(vec![ListItem::new(text("a")), ListItem::new(text("b"))]));
    let out = page.render();
    assert!(!out.replace("\r\n", "").contains('\n'), "lone LF in {out:?}");
    assert!(!page.render_canonical().replace("\r\n", "").contains('\n'));
}

// ------------------------------------------------------------------ m4 remove_block
