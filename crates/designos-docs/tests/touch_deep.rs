//! Editing a list must never change its neighbours or its own structure. Found by the second review: an indented
//! list swallowed the blank line in front of the next paragraph, and nested child lists folded into text.

use designos_docs::{BlockKind, Inline, ListItem, Page};

fn touch(page: &mut Page, i: usize) {
    let kind = page.blocks[i].kind().clone();
    *page.blocks[i].kind_mut() = kind;
}

/// Touches every typed block and returns what changed: the page must keep the same blocks, kind for kind and
/// deeply (children included), not just the same top-level names.
fn touch_drift(src: &str) -> Vec<String> {
    let page = Page::parse(src);
    let mut drift = Vec::new();
    for i in 0..page.blocks.len() {
        if matches!(page.blocks[i].kind(), BlockKind::Raw(_)) {
            continue;
        }
        let mut touched = page.clone();
        touch(&mut touched, i);
        let out = touched.render();
        let again = Page::parse(&out);
        let same = again.blocks.len() == page.blocks.len() && again.blocks.iter().zip(&page.blocks).all(|(a, b)| a.kind() == b.kind());
        if !same {
            drift.push(format!("touching block {i} of {src:?} gave {out:?}"));
        }
    }
    drift
}

#[test]
fn touching_an_indented_list_keeps_the_paragraph_after_it() {
    for src in ["  - three\n\n  para\n", " - three\n\n para\n", "  1. three\n\n  para\n", "  - a\n  - b\n\n  para\n\nlast\n"] {
        let page = Page::parse(src);
        let mut touched = page.clone();
        touch(&mut touched, 0);
        let out = touched.render();
        assert!(out.contains("para"), "{src:?} lost its paragraph: {out:?}");
        assert!(!out.contains("threepara"), "{src:?} glued the paragraph to the list: {out:?}");
        assert!(touch_drift(src).is_empty(), "{:?}", touch_drift(src));
    }
}

#[test]
fn touching_a_nested_list_keeps_it_a_list() {
    for src in [
        "- a\n\n  3. b\n",
        "1. a\n\n   para\n\n   2. b\n",
        "- a\n\n  -\n",
        "- a\n  -\n",
        "- a\n\n  - b\n\n- c\n",
        "- a\n  - b\n\n  para\n",
        "- [ ] a\n  1. x\n",
        "- a\n  1) x\n  1. y\n",
    ] {
        let drift = touch_drift(src);
        assert!(drift.is_empty(), "{drift:?}");
    }
}

#[test]
fn a_nested_list_that_cannot_interrupt_a_paragraph_is_built_with_a_blank_line() {
    let mut item = ListItem::new(vec![Inline::Text("a".into())]);
    item.children.push(BlockKind::NumberedList { start: 3, items: vec![ListItem::new(vec![Inline::Text("b".into())])] });
    let mut page = Page::parse("");
    page.push_block(BlockKind::BulletList(vec![item]));
    let again = Page::parse(&page.render());
    assert_eq!(again.blocks.len(), 1);
    assert_eq!(again.blocks[0].kind(), page.blocks[0].kind(), "{:?}", page.render());
}

/// Random nested-list soup, compared deeply. `DOCS_NESTED_CASES` raises the count.
#[test]
fn random_nested_list_soup_keeps_its_structure_when_touched() {
    const PIECES: &[&str] = &[
        "- a\n",
        "* b\n",
        "1. x\n",
        "2) y\n",
        "- [ ] t\n",
        "  - n\n",
        "    - nn\n",
        "   1. nnum\n",
        "  3. three\n",
        "  -\n",
        "\n",
        "  para\n",
        "lazy\n",
        "# H\n",
        "p\n",
        "  - [x] c\n",
        "\t- tab\n",
        "      - deep\n",
        "10. ten\n",
        "    - under ten\n",
    ];
    let mut seed: u64 = 0x1234_5678_9abc_def1;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let cases: usize = std::env::var("DOCS_NESTED_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(30_000);
    let mut drift = Vec::new();
    for _ in 0..cases {
        let len = 1 + next(7);
        let src: String = (0..len).map(|_| PIECES[next(PIECES.len())]).collect();
        assert_eq!(Page::parse(&src).render(), src, "lossless: {src:?}");
        drift.extend(touch_drift(&src));
    }
    assert!(drift.is_empty(), "{} touches changed the page, first: {:#?}", drift.len(), &drift[..drift.len().min(6)]);
}
