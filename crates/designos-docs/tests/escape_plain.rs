//! Plain text typed by a person (or an agent) to Markdown source that reads back as exactly that text, in the
//! block it was typed into. This is what phase 04's editor and phase 06's MCP tools build text blocks from.

use designos_docs::{escape_plain, paragraphs_from_plain, plain_inlines, plain_text, BlockKind, ChecklistItem, Inline, ListItem, Page, TextContext};

fn parse(text: &str) -> Page {
    Page::parse(text)
}

fn one_block(kind: BlockKind) -> Page {
    let mut page = Page::default();
    page.push_block(kind);
    page
}

/// What the escaping promises to keep: lines trimmed of spaces and tabs, blank lines folded away.
fn normalised(s: &str) -> String {
    s.replace('\0', "").replace("\r\n", "\n").split('\n').map(|l| l.trim_matches([' ', '\t'])).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn flat(s: &str) -> String {
    normalised(s).replace('\n', " ")
}

#[test]
fn text_that_looks_like_markdown_stays_a_paragraph_with_the_same_text() {
    for typed in [
        "- not a list",
        "2024. A good year",
        "3) also not a list",
        "# tag",
        "> not a quote",
        "+ plus",
        "* star",
        "1. one\n2. two",
        "---",
        "===",
        "line\n---",
        "a\n===",
        "```rust\ncode?\n```",
        "~~~\nfence?\n~~~",
        "| a | b |\n| - | - |\n| 1 | 2 |",
        "[x]: http://x",
        "[ ] task?",
        "![img](designos:f#1)",
        "<div>html</div> &amp; <http://auto.link>",
        "snake_case and 2 * 3 and **not bold** and `not code` and [link](x)",
        "trailing backslash\\",
        "tab\tinside",
        "ends with hashes ###",
    ] {
        let page = one_block(BlockKind::Paragraph(plain_inlines(typed, TextContext::Paragraph)));
        let again = parse(&page.render());
        assert_eq!(again.blocks.len(), 1, "{typed:?} -> {:?}", page.render());
        let BlockKind::Paragraph(inlines) = again.blocks[0].kind() else {
            panic!("{typed:?} became {:?} via {:?}", again.blocks[0].kind().name(), page.render())
        };
        assert_eq!(plain_text(inlines), normalised(typed), "{typed:?} via {:?}", page.render());
    }
}

#[test]
fn a_newline_in_a_heading_or_cell_is_folded_and_a_blank_line_in_a_paragraph_splits_it() {
    let page = one_block(BlockKind::Heading { level: 2, content: plain_inlines("line1\nline2", TextContext::Heading) });
    let again = parse(&page.render());
    assert_eq!(again.blocks.len(), 1, "{:?}", page.render());
    let BlockKind::Heading { content, .. } = again.blocks[0].kind() else { panic!("heading expected") };
    assert_eq!(plain_text(content), "line1 line2");

    let blocks = paragraphs_from_plain("first paragraph\nsame paragraph\n\n\n- second\n");
    assert_eq!(blocks.len(), 2);
    let mut page = Page::default();
    for b in blocks {
        page.push_block(b);
    }
    let again = parse(&page.render());
    let texts: Vec<String> = again
        .blocks
        .iter()
        .map(|b| match b.kind() {
            BlockKind::Paragraph(i) => plain_text(i),
            other => panic!("paragraph expected, got {other:?}"),
        })
        .collect();
    assert_eq!(texts, ["first paragraph\nsame paragraph", "- second"]);
}

#[test]
fn escaped_text_is_safe_in_every_block_kind() {
    let nasty = "- a | b *c* _d_ `e` [f](g) \\ <h> &i; #j\nk. 1) l";
    let ctx_item = || plain_inlines(nasty, TextContext::ListItem);

    let page = one_block(BlockKind::BulletList(vec![ListItem::new(ctx_item()), ListItem::new(ctx_item())]));
    let again = parse(&page.render());
    let BlockKind::BulletList(items) = again.blocks[0].kind() else { panic!("{:?}", page.render()) };
    assert_eq!(items.len(), 2);
    assert_eq!(plain_text(&items[0].content), normalised(nasty));

    let page = one_block(BlockKind::Checklist(vec![ChecklistItem::new(true, ctx_item())]));
    let again = parse(&page.render());
    assert_eq!(again.blocks[0].kind().name(), "checklist", "{:?}", page.render());

    let page = one_block(BlockKind::Callout { kind: "note".into(), body: plain_inlines(nasty, TextContext::CalloutBody) });
    let again = parse(&page.render());
    let BlockKind::Callout { body, .. } = again.blocks[0].kind() else { panic!("{:?}", page.render()) };
    assert_eq!(plain_text(body), normalised(nasty));

    let cell = |s: &str| plain_inlines(s, TextContext::TableCell);
    let page = one_block(BlockKind::Table {
        aligns: vec![designos_docs::Align::None, designos_docs::Align::None],
        header: vec![cell("h | one"), cell("h\ntwo")],
        rows: vec![vec![cell("a | b"), cell("`c` | d")]],
    });
    let again = parse(&page.render());
    let BlockKind::Table { header, rows, .. } = again.blocks[0].kind() else { panic!("{:?} -> {:?}", page.render(), again.blocks[0].kind().name()) };
    assert_eq!((plain_text(&header[0]), plain_text(&header[1])), ("h | one".to_string(), "h two".to_string()));
    assert_eq!((plain_text(&rows[0][0]), plain_text(&rows[0][1])), ("a | b".to_string(), "`c` | d".to_string()));
}

#[test]
fn random_typed_text_reads_back_as_itself_in_each_context() {
    let pieces = [
        "-",
        "- ",
        "+ ",
        "* ",
        "#",
        "# ",
        "> ",
        "1.",
        "1. ",
        "2) ",
        "12.",
        "=",
        "===",
        "---",
        "***",
        "___",
        "~~~",
        "```",
        "`",
        "*",
        "_",
        "[",
        "]",
        "(",
        ")",
        "!",
        "<",
        ">",
        "&",
        "&amp;",
        "|",
        "\\",
        "\\\\",
        "\n",
        "\n\n",
        "\r\n",
        " ",
        "  ",
        "\t",
        "word",
        "two words",
        "é",
        "日本",
        "😀",
        "http://a.b",
        "[x]: u",
        "<!-- c -->",
        "$",
        "~",
        ":",
        "{",
        "}",
        "^",
        "%",
    ];
    let mut seed: u64 = 0xD1B5_4A32_D192_ED03;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    for case in 0..20_000 {
        let len = 1 + next(7);
        let typed: String = (0..len).map(|_| pieces[next(pieces.len())]).collect();
        let want = normalised(&typed);
        if want.is_empty() {
            continue;
        }
        let page = one_block(BlockKind::Paragraph(plain_inlines(&typed, TextContext::Paragraph)));
        let again = parse(&page.render());
        assert!(
            matches!(again.blocks.as_slice(), [b] if b.kind().name() == "paragraph"),
            "case {case}: {typed:?} -> {:?} -> {:?}",
            page.render(),
            again.blocks.iter().map(|b| b.kind().name()).collect::<Vec<_>>()
        );
        let BlockKind::Paragraph(i) = again.blocks[0].kind() else { unreachable!() };
        assert_eq!(plain_text(i), want, "case {case}: {typed:?} via {:?}", page.render());

        let want_flat = flat(&typed);
        let page = one_block(BlockKind::Heading { level: 3, content: plain_inlines(&typed, TextContext::Heading) });
        let again = parse(&page.render());
        let BlockKind::Heading { content, .. } = again.blocks[0].kind() else { panic!("case {case}: heading lost: {typed:?} -> {:?}", page.render()) };
        assert_eq!((again.blocks.len(), plain_text(content)), (1, want_flat.clone()), "case {case}: {typed:?} via {:?}", page.render());

        let page = one_block(BlockKind::BulletList(vec![ListItem::new(plain_inlines(&typed, TextContext::ListItem))]));
        let again = parse(&page.render());
        let BlockKind::BulletList(items) = again.blocks[0].kind() else { panic!("case {case}: list item lost: {typed:?} -> {:?}", page.render()) };
        assert_eq!((again.blocks.len(), items.len(), plain_text(&items[0].content)), (1, 1, want.clone()), "case {case}: {typed:?} via {:?}", page.render());

        let page = one_block(BlockKind::Table {
            aligns: vec![designos_docs::Align::None],
            header: vec![plain_inlines("h", TextContext::TableCell)],
            rows: vec![vec![plain_inlines(&typed, TextContext::TableCell)]],
        });
        let again = parse(&page.render());
        let BlockKind::Table { rows, .. } = again.blocks[0].kind() else { panic!("case {case}: table lost: {typed:?} -> {:?}", page.render()) };
        assert_eq!((again.blocks.len(), plain_text(&rows[0][0])), (1, want_flat), "case {case}: {typed:?} via {:?}", page.render());
    }
}

#[test]
fn escape_plain_is_the_text_of_plain_inlines() {
    assert_eq!(escape_plain("- a", TextContext::Paragraph), "\\- a");
    assert_eq!(plain_inlines("- a", TextContext::Paragraph), vec![Inline::Text("\\- a".into())]);
}
