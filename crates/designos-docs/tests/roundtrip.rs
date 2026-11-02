//! Markdown block model through the public API: every v1 block type round-trips byte for byte, unknown
//! syntax survives verbatim, and the canonical writer reproduces files Studio itself writes.

use designos_docs::{Align, BlockKind, Group, Inline, Page};

const FIXTURES: &[(&str, &str)] = &[
    ("heading", include_str!("fixtures/heading.md")),
    ("paragraph-inline", include_str!("fixtures/paragraph-inline.md")),
    ("lists", include_str!("fixtures/lists.md")),
    ("checklist", include_str!("fixtures/checklist.md")),
    ("callout", include_str!("fixtures/callout.md")),
    ("table", include_str!("fixtures/table.md")),
    ("frame-embed", include_str!("fixtures/frame-embed.md")),
    ("mermaid", include_str!("fixtures/mermaid.md")),
    ("code", include_str!("fixtures/code.md")),
    ("front-matter-and-all-blocks", include_str!("fixtures/front-matter-and-all-blocks.md")),
    ("escapes-at-edges", include_str!("fixtures/escapes-at-edges.md")),
];
const UNKNOWN: &str = include_str!("fixtures/unknown-syntax.md");

fn names(page: &Page) -> Vec<&'static str> {
    page.blocks.iter().map(|b| b.kind().name()).collect()
}

fn parse(text: &str) -> Page {
    Page::parse(text).unwrap_or_else(|e| panic!("parse failed: {e}"))
}

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
fn front_matter_is_read_and_kept_verbatim() {
    let page = parse(FIXTURES[9].1);
    let fm = page.front_matter().expect("front matter");
    assert_eq!(fm.title(), Some("Checkout overview"));
    assert_eq!(fm.description(), Some("What this flow does: a summary"));
    assert_eq!(fm.group(), Some(Group::Overview));
}

#[test]
fn bad_front_matter_group_names_the_field_and_the_fix() {
    let err = Page::parse("---\ngroup: sketches\n---\n\n# T\n").unwrap_err().to_string();
    assert!(err.contains("group") && err.contains("overview | spec | screens | states | discovery"), "got: {err}");
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
        "- item\n  - nested item\n  - another nested",
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
    page.remove_block(0);
    assert_eq!(page.render(), "Between.\n\n## Heading two\n\n### Heading three\n");
    page.remove_block(2);
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
            match Page::parse(prefix) {
                Ok(page) => {
                    assert_eq!(page.render(), prefix, "{name} cut at {end}");
                    page.render_canonical();
                }
                // A cut inside the front matter can leave a group that is not one of the five: refused loudly.
                Err(e) => assert!(prefix.starts_with("---"), "{name} cut at {end}: {e}"),
            }
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

/// Small deterministic generator (no dependency): xorshift64*.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[(self.next() % items.len() as u64) as usize]
    }
}

/// Pieces whose canonical form may differ on a second pass, so only the lossless property is asserted for them:
/// a link definition glues to its neighbours; a tab before a line end changes how the parser reads the emphasis
/// in front of it; `_` / `__` emphasis is rewritten with `*`, which reads differently next to stray underscores.
/// Source-preserving output (`render`) is lossless for them all.
const LOSSLESS_ONLY: &[&str] = &["[x]: http://x\n", "* ", "2) ", "\t", "_", "__"];

const PIECES: &[&str] = &[
    "# ",
    "## ",
    "### ",
    "#### ",
    "- ",
    "1. ",
    "- [ ] ",
    "- [x] ",
    "> ",
    "> [!note]\n> ",
    "| a | b |\n| --- | :-: |\n",
    "| ",
    " | ",
    "\n",
    "\n\n",
    "\r\n",
    "  ",
    "    ",
    "```",
    "```mermaid\n",
    "````\n",
    "~~~\n",
    "**",
    "*",
    "`",
    "``",
    "[",
    "](",
    ")",
    "![img](designos:f#1)",
    "![",
    "<div>",
    "</div>",
    "---\n",
    "===\n",
    "\\",
    "\\*",
    "\\|",
    "|",
    "word",
    "two words",
    "é",
    "日本",
    "😀",
    "<!-- c -->",
    "&amp;",
    "http://a.b",
    "- a\n  - b\n",
    "1.",
    "-",
    "#",
    ">",
];

/// True when every backtick in `text` belongs to a fence at the start of a line. Stray backticks in running text
/// make a rewritten code span read differently next to them (known limit, see the README).
fn fences_only(text: &str) -> bool {
    text.lines().all(|line| {
        let t = line.trim_start();
        let rest = t.trim_start_matches('`');
        let fence = t.len() - rest.len();
        (fence == 0 || fence >= 3) && !rest.contains('`')
    })
}

#[test]
fn random_markdown_soup_is_always_lossless_and_the_canonical_form_is_stable() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut unstable = Vec::new();
    let cases: u32 = std::env::var("DOCS_SOUP_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(40_000);
    for case in 0..cases {
        let with_extras = case % 4 == 0;
        let n = 1 + (rng.next() % 14) as usize;
        let text: String = (0..n).map(|_| if with_extras && rng.next().is_multiple_of(5) { rng.pick(LOSSLESS_ONLY) } else { rng.pick(PIECES) }).collect();
        let Ok(page) = Page::parse(&text) else { continue };
        assert_eq!(page.render(), text, "case {case}: {text:?}");
        let canonical = page.render_canonical();
        let again = Page::parse(&canonical).unwrap_or_else(|e| panic!("case {case}: canonical output no longer parses ({e}): {canonical:?}"));
        assert_eq!(again.render(), canonical, "case {case}");
        if !with_extras && fences_only(&text) && again.render_canonical() != canonical {
            unstable.push((text, canonical));
        }
    }
    assert!(
        unstable.is_empty(),
        "{} inputs whose canonical form changes when read and written again, first: {:#?}",
        unstable.len(),
        &unstable[..unstable.len().min(10)]
    );
}

#[test]
fn a_rewritten_list_next_to_another_list_does_not_merge_with_it() {
    let mut page = parse("- a\n\n* b\n\n1. one\n\n2) two\n");
    assert_eq!(names(&page), ["bullet_list", "bullet_list", "numbered_list", "numbered_list"]);
    *page.blocks[1].kind_mut() = BlockKind::BulletList(vec![vec![Inline::Text("B".into())]]);
    *page.blocks[3].kind_mut() = BlockKind::NumberedList { start: 2, items: vec![vec![Inline::Text("Two".into())]] };
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
