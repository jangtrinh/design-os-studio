//! Markdown block model through the public API: every v1 block type round-trips byte for byte, unknown
//! syntax survives verbatim, and the canonical writer reproduces files Studio itself writes.

mod common;

use common::{names, parse, FIXTURES, UNKNOWN};
use designos_docs::{Align, BlockKind, Inline, Page};

#[test]
fn every_v1_fixture_round_trips_byte_identical() {
    for (name, text) in FIXTURES {
        assert_eq!(parse(text).render(), *text, "fixture {name}");
    }
}

#[test]
fn canonical_writer_reproduces_files_studio_writes() {
    // Rebuilding every block from its typed content (ignoring the retained source) gives the same bytes, so the
    // typed model really carries the whole file and an edited block is written the way Studio writes it.
    for (name, text) in FIXTURES {
        assert_eq!(parse(text).render_canonical(), *text, "fixture {name}");
    }
}

#[test]
fn each_v1_block_type_is_recognised() {
    assert_eq!(names(&parse(FIXTURES[0].1)), ["heading", "heading", "heading"]);
    assert_eq!(names(&parse(FIXTURES[2].1)), ["bullet_list", "numbered_list"]);
    assert_eq!(names(&parse(FIXTURES[3].1)), ["checklist"]);
    assert_eq!(names(&parse(FIXTURES[4].1)), ["callout", "callout"]);
    assert_eq!(names(&parse(FIXTURES[5].1)), ["table"]);
    assert_eq!(names(&parse(FIXTURES[6].1)), ["paragraph", "frame_embed", "paragraph"]);
    assert_eq!(names(&parse(FIXTURES[7].1)), ["mermaid"]);
    assert_eq!(names(&parse(FIXTURES[8].1)), ["code", "code", "code"]);
    assert_eq!(
        names(&parse(FIXTURES[9].1)),
        ["heading", "paragraph", "heading", "bullet_list", "numbered_list", "checklist", "callout", "table", "frame_embed", "mermaid", "code"]
    );
}

#[test]
fn escapes_at_the_edges_of_blocks_stay_typed_blocks_and_keep_their_backslashes() {
    // If these fell back to Raw, the canonical round-trip above would pass for the wrong reason.
    let page = parse(FIXTURES[10].1);
    assert_eq!(names(&page), ["paragraph", "heading", "bullet_list", "numbered_list", "callout", "table"]);
    let BlockKind::Paragraph(inlines) = page.blocks[0].kind() else { panic!("paragraph expected") };
    assert_eq!(inlines, &[Inline::Text("\\*not italic\\* at paragraph start and a trailing escape\\#".into())]);
    let BlockKind::Table { header, rows, .. } = page.blocks[5].kind() else { panic!("table expected") };
    assert_eq!(header[0], [Inline::Text("\\|a".into())]);
    assert_eq!(header[1], [Inline::Text("b\\\\".into())]);
    assert_eq!(rows[1][0], Vec::<Inline>::new());
}

#[test]
fn block_content_is_structured() {
    let page = parse(FIXTURES[0].1);
    let levels: Vec<u8> = page
        .blocks
        .iter()
        .map(|b| match b.kind() {
            BlockKind::Heading { level, .. } => *level,
            other => panic!("not a heading: {other:?}"),
        })
        .collect();
    assert_eq!(levels, [1, 2, 3]);

    let page = parse(FIXTURES[1].1);
    let BlockKind::Paragraph(inlines) = page.blocks[0].kind() else { panic!("paragraph expected") };
    assert!(inlines.contains(&Inline::Bold(vec![Inline::Text("bold".into())])));
    assert!(inlines.contains(&Inline::Italic(vec![Inline::Text("italic".into())])));
    assert!(inlines.contains(&Inline::Code("code".into())));
    assert!(inlines.contains(&Inline::Link { content: vec![Inline::Text("link".into())], target: "https://example.com/a?b=1".into() }));

    let page = parse(FIXTURES[6].1);
    assert_eq!(page.blocks[1].kind(), &BlockKind::FrameEmbed { name: "Checkout screen".into(), file_id: "file_01HZ".into(), node_id: "42".into() });

    let page = parse(FIXTURES[5].1);
    let BlockKind::Table { aligns, header, rows } = page.blocks[0].kind() else { panic!("table expected") };
    assert_eq!(aligns, &[Align::None, Align::Center, Align::Right]);
    assert_eq!(header.len(), 3);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2][1], Vec::<Inline>::new(), "an empty cell stays an empty cell");

    let page = parse(FIXTURES[3].1);
    let BlockKind::Checklist(items) = page.blocks[0].kind() else { panic!("checklist expected") };
    assert_eq!(items.iter().map(|i| i.checked).collect::<Vec<_>>(), [false, true, false]);

    let page = parse(FIXTURES[7].1);
    let BlockKind::Mermaid { source } = page.blocks[0].kind() else { panic!("mermaid expected") };
    assert!(source.starts_with("flowchart TD\n") && source.ends_with("B -->|no| A"), "got {source:?}");
}

#[test]
fn unknown_syntax_is_kept_verbatim() {
    let page = parse(UNKNOWN);
    assert_eq!(page.render(), UNKNOWN);
    let raw: Vec<&str> = page
        .blocks
        .iter()
        .filter_map(|b| match b.kind() {
            BlockKind::Raw(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for needle in [
        "Setext heading\n==============",
        "#### Level four heading",
        "<div class=\"custom\">\n  <b>raw html</b>\n</div>",
        "---",
        "[ref]: https://example.com \"Reference\"",
        "    indented code block",
        "> plain quote\n> over two lines",
    ] {
        assert!(raw.contains(&needle), "{needle:?} must be a raw block, raw blocks: {raw:?}");
    }
    // Canonical output keeps raw blocks verbatim too: nothing unknown is lost when a page is normalised.
    for needle in ["<b>raw html</b>", "[^1]: The footnote text.", "indented code block", "~~strike~~", "![alt](pic.png)"] {
        assert!(parse(UNKNOWN).render_canonical().contains(needle), "{needle:?} lost in canonical output");
    }
}

#[test]
fn crlf_and_missing_final_newline_survive() {
    for text in [
        "# A\r\n\r\npara one\r\nstill one\r\n\r\n- x\r\n- y\r\n",
        "# A\n\nno newline at end",
        "\n\n# Leading blank lines\n\n\n\nTwo blank gaps\n\n\n",
        "",
        "\n",
        "   \n",
    ] {
        assert_eq!(parse(text).render(), text, "{text:?}");
    }
}

#[test]
fn every_prefix_of_every_fixture_is_lossless_and_never_panics() {
    // Truncating at each char boundary produces unterminated fences, half tables, broken links and unclosed
    // emphasis: the nastiest inputs a half-written file can hold.
    let mut seen = 0;
    for (name, text) in FIXTURES.iter().chain(std::iter::once(&("unknown", UNKNOWN))) {
        for (end, _) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
            let prefix = &text[..end];
            let page = Page::parse(prefix);
            assert_eq!(page.render(), prefix, "{name} cut at {end}");
            page.render_canonical();
            seen += 1;
        }
    }
    assert!(seen > 1000, "swept {seen} prefixes");
}

#[test]
fn text_with_markdown_lookalikes_is_not_misread_as_structure() {
    for text in [
        "![x](designos:#)\n",
        "![x](designos:file#)\n",
        "![x](designos:file#a b)\n",
        "![x](designos:file#1) trailing words\n",
        "> [!note] has a title\n> body\n",
        "> [!note]\n>\n> two\n>\n> paragraphs\n",
        "```mermaid extra\ncode\n```\n",
        "| a | b |\n| - | - |\n| 1 |\n",
        "1. one\n3. skipped\n",
        "- [ ] a\n- plain\n",
        "*not closed\n",
        "[unclosed](http://x\n",
        "[ref link][ref]\n\n[ref]: http://x\n",
        "<http://autolink.example>\n",
    ] {
        let page = parse(text);
        assert_eq!(page.render(), text, "{text:?}");
        assert!(!page.render_canonical().is_empty());
    }
    // Malformed embeds and titled/multi-paragraph callouts stay ordinary paragraphs or raw blocks.
    assert_eq!(names(&parse("![x](designos:file#)\n")), ["paragraph"]);
    assert_eq!(names(&parse("> [!note] has a title\n> body\n")), ["raw"]);
    assert_eq!(names(&parse("> [!note]\n>\n> two\n>\n> paragraphs\n")), ["raw"]);
    assert_eq!(names(&parse("- [ ] a\n- plain\n")), ["raw"]);
}

#[test]
fn a_checklist_with_blank_lines_between_items_is_still_a_checklist() {
    let text = "- [ ] first\n\n- [x] second\n";
    let page = parse(text);
    assert_eq!(names(&page), ["checklist"]);
    let BlockKind::Checklist(items) = page.blocks[0].kind() else { panic!("checklist expected") };
    assert_eq!(
        items.iter().map(|i| (i.checked, designos_docs::plain_text(&i.content))).collect::<Vec<_>>(),
        [(false, "first".to_string()), (true, "second".to_string())]
    );
    assert_eq!(page.render(), text);
}

#[test]
fn headings_and_plain_text_serve_the_reader() {
    let page = parse(FIXTURES[9].1);
    let outline: Vec<(usize, u8, String)> = page.headings().into_iter().map(|h| (h.block_index, h.level, h.text)).collect();
    assert_eq!(outline, [(0, 1, "Checkout".to_string()), (2, 2, "Goals".to_string())]);

    let page = parse(FIXTURES[1].1);
    let texts: Vec<String> = page
        .blocks
        .iter()
        .map(|b| match b.kind() {
            BlockKind::Paragraph(inlines) => designos_docs::plain_text(inlines),
            other => panic!("paragraph expected, got {other:?}"),
        })
        .collect();
    assert_eq!(texts[0], "Plain text with bold, italic, code and a link.");
    assert_eq!(texts[2], "Escapes stay as typed: 2 * 3 and a [bracket] and snake_case_word.");
    assert_eq!(texts[3], "Code with a backtick: a ` b and a link with code.");
}

#[test]
fn a_table_row_with_text_beyond_the_header_columns_is_kept_raw_so_no_cell_is_lost() {
    for src in [
        "| a | b |\n|---|---|\n| 1 | 2 | 3 |\n",
        "| flag | values |\n|---|---|\n| `--format` | `json|yaml` |\n",
        "| a | b |\n|---|---|\n| 1 | 2 | 3 |\n| 4 | 5 |\n",
    ] {
        let page = parse(src);
        assert_eq!(names(&page), ["raw"], "{src:?}");
        assert_eq!(page.render(), src);
        assert_eq!(page.render_canonical(), src, "raw keeps every character even in canonical output");
    }
    // A trailing empty cell (`| 1 | 2 |  |` would be 3 cells; a closing pipe alone is not one).
    assert_eq!(names(&parse("| a | b |\n|---|---|\n| 1 | 2 |\n")), ["table"]);
}
