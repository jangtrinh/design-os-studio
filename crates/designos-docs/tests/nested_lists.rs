//! Nested lists are part of the model (phase 04 indents and outdents items): unedited files stay byte-identical,
//! and a list built in code writes the same way it is read.

use designos_docs::{plain_text, BlockKind, ChecklistItem, Inline, ListItem, Page};

fn parse(text: &str) -> Page {
    Page::parse(text)
}

fn item(text: &str) -> ListItem {
    ListItem::new(vec![Inline::Text(text.into())])
}

fn bullet_items(page: &Page, block: usize) -> &Vec<ListItem> {
    match page.blocks[block].kind() {
        BlockKind::BulletList(items) => items,
        other => panic!("bullet list expected, got {other:?}"),
    }
}

#[test]
fn a_nested_bullet_list_is_a_list_with_children_and_round_trips() {
    let src = "- a\n  - b\n  - c\n- d\n";
    let page = parse(src);
    assert_eq!(page.blocks.iter().map(|b| b.kind().name()).collect::<Vec<_>>(), ["bullet_list"]);
    let items = bullet_items(&page, 0);
    assert_eq!(items.len(), 2);
    assert_eq!(plain_text(&items[0].content), "a");
    let [BlockKind::BulletList(sub)] = items[0].children.as_slice() else { panic!("one nested list expected: {:?}", items[0].children) };
    assert_eq!(sub.iter().map(|i| plain_text(&i.content)).collect::<Vec<_>>(), ["b", "c"]);
    assert!(items[1].children.is_empty());
    assert_eq!(page.render(), src);
    assert_eq!(page.render_canonical(), src);
}

#[test]
fn nesting_works_across_list_kinds_and_depth() {
    for src in [
        "1. one\n   - sub\n   - sub two\n2. two\n",
        "- [x] done\n  - [ ] sub task\n  - [x] sub done\n- [ ] open\n",
        "- a\n  - b\n    - c\n      - d\n",
        "- a\n  1. x\n  2. y\n- b\n",
        "10. ten\n    - sub\n11. eleven\n",
    ] {
        let page = parse(src);
        assert_eq!(page.blocks.len(), 1, "{src:?}");
        assert_ne!(page.blocks[0].kind().name(), "raw", "{src:?}");
        assert_eq!(page.render(), src);
        assert_eq!(page.render_canonical(), src, "typed model carries the whole list: {src:?}");
    }
}

#[test]
fn a_paragraph_inside_an_item_is_a_child_paragraph() {
    let src = "- a\n\n  second paragraph\n- b\n";
    let page = parse(src);
    let items = bullet_items(&page, 0);
    assert!(matches!(items[0].children.as_slice(), [BlockKind::Paragraph(_)]), "{:?}", items[0].children);
    assert_eq!(page.render(), src);
    assert_eq!(page.render_canonical(), src);
}

#[test]
fn indent_and_outdent_built_in_code_write_and_read_back_the_same() {
    let mut a = item("a");
    a.children.push(BlockKind::BulletList(vec![item("b")]));
    let mut page = Page::default();
    page.push_block(BlockKind::BulletList(vec![a, item("c")]));
    assert_eq!(page.render(), "- a\n  - b\n- c\n");
    let again = parse(&page.render());
    assert_eq!(again.render_canonical(), page.render());

    // Outdent "b": it becomes a top-level item between a and c.
    let BlockKind::BulletList(items) = page.blocks[0].kind_mut() else { unreachable!() };
    let moved = items[0].children.remove(0);
    let BlockKind::BulletList(mut sub) = moved else { unreachable!() };
    items.insert(1, sub.remove(0));
    assert_eq!(page.render(), "- a\n- b\n- c\n");

    // Numbered parent: children are indented by the width of the marker.
    let mut one = item("one");
    one.children.push(BlockKind::Checklist(vec![ChecklistItem::new(true, vec![Inline::Text("sub".into())])]));
    let mut page = Page::default();
    page.push_block(BlockKind::NumberedList { start: 9, items: vec![one, item("two")] });
    assert_eq!(page.render(), "9. one\n   - [x] sub\n10. two\n");
    assert_eq!(parse(&page.render()).render_canonical(), page.render());
}

#[test]
fn lists_with_content_the_model_cannot_hold_stay_raw() {
    for src in ["- a\n  ```\n  code\n  ```\n", "- a\n  > quote\n", "- a\n\n  | x | y |\n  | - | - |\n  | 1 | 2 |\n"] {
        let page = parse(src);
        assert_eq!(page.render(), src);
        assert_eq!(page.blocks.iter().map(|b| b.kind().name()).collect::<Vec<_>>(), ["raw"], "{src:?}");
    }
}

#[test]
fn a_rewritten_list_does_not_merge_with_either_neighbour() {
    // The middle list is rewritten; both neighbours keep the markers they were written with.
    for (src, expected) in
        [("- a\n\n* b\n\n+ c\n", 3), ("* a\n\n- b\n\n* c\n", 3), ("1. a\n\n1) b\n\n1. c\n", 3), ("- a\n\n1. b\n\n- c\n", 3), ("- a\n\n* b\n", 2)]
    {
        let page = parse(src);
        assert_eq!(page.blocks.len(), expected, "{src:?}");
        for i in 0..page.blocks.len() {
            let mut touched = page.clone();
            let kind = touched.blocks[i].kind().clone();
            *touched.blocks[i].kind_mut() = kind;
            let again = parse(&touched.render());
            assert_eq!(again.blocks.len(), expected, "touching block {i} of {src:?} gave {:?}", touched.render());
        }
    }
}
