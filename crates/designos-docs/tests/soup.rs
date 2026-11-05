//! Random Markdown soup: unedited output is always lossless, the canonical form is stable, and touching a block
//! without changing it never changes how many blocks the page has.

use designos_docs::{BlockKind, Page};

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
/// in front of it; `_` / `__` emphasis is rewritten with `*`, which reads differently next to stray underscores; a
/// file that mixes `\r\n` and `\n` is rewritten with the line ending of its first line.
/// Source-preserving output (`render`) is lossless for them all.
const LOSSLESS_ONLY: &[&str] = &["[x]: http://x\n", "* ", "2) ", "\t", "_", "__", "\r\n"];

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
        let page = Page::parse(&text);
        assert_eq!(page.render(), text, "case {case}: {text:?}");
        let canonical = page.render_canonical();
        let again = Page::parse(&canonical);
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

/// Whole-line pieces, so that blocks of every kind (and the ways they sit next to each other) come up often.
const BLOCK_PIECES: &[&str] = &[
    "# H\n",
    "para *a* **b** `c` [l](u)\n",
    "- a\n",
    "* b\n",
    "+ p\n",
    "1. x\n",
    "2) y\n",
    "- [ ] t\n",
    "- [x] d\n",
    "  - nested\n",
    "    - deeper\n",
    "   1. nested number\n",
    "> [!note]\n> body\n",
    "| a | b |\n|---|---|\n| 1 | 2 |\n",
    "| a | b |\n|---|---|\n| 1 | 2 | 3 |\n",
    "```\ncode\n```\n",
    "```mermaid\ngraph TD\n```\n",
    "![n](designos:f#1:2)\n",
    "\n",
    "\r\n",
    "\u{FEFF}",
    "\t- tab\n",
    "    indented\n",
    "  para\n",
    "  3. three\n",
    "  -\n",
    "<div>\n",
    "[ref]: http://x\n",
    "---\n",
    "===\n",
    "\u{e9}\u{6f22}\u{5b57}\n",
    "\\*esc\\*\n",
    "> quote\n",
    "~~~\ntilde\n~~~\n",
];

/// The invariant behind "touching a block without changing it": the rewritten block must still be the same kind of
/// block, and must neither merge with a neighbour nor split, whatever its neighbours are.
#[test]
fn touching_a_block_without_changing_it_leaves_the_blocks_as_they_were() {
    let cases: u32 = std::env::var("DOCS_TOUCH_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(30_000);
    let mut rng = Rng(0xA076_1D64_78BD_642F);
    let mut checked = 0u64;
    for case in 0..cases {
        let n = 1 + (rng.next() % 8) as usize;
        let src: String = (0..n).map(|_| BLOCK_PIECES[(rng.next() % BLOCK_PIECES.len() as u64) as usize]).collect();
        let page = Page::parse(&src);
        assert_eq!(page.render(), src, "case {case}");
        let before: Vec<&str> = page.blocks.iter().map(|b| b.kind().name()).collect();
        for i in 0..page.blocks.len() {
            if matches!(page.blocks[i].kind(), BlockKind::Raw(_)) {
                continue;
            }
            let mut touched = page.clone();
            let kind = touched.blocks[i].kind().clone();
            *touched.blocks[i].kind_mut() = kind;
            let after_text = touched.render();
            let after = Page::parse(&after_text);
            let names: Vec<&str> = after.blocks.iter().map(|b| b.kind().name()).collect();
            assert_eq!(names, before, "case {case}: touching block {i} of {src:?} gave {after_text:?}");
            // Deeply: items and their children are the same too, and so is every other block.
            // (A file that mixes line endings keeps stray carriage returns in text; compare those by name only.)
            let same = src.contains('\r') || after.blocks.iter().zip(&page.blocks).all(|(a, b)| a.kind() == b.kind());
            assert!(same, "case {case}: touching block {i} of {src:?} changed a block's content: {after_text:?}");
            checked += 1;
        }
    }
    assert!(checked > 1000, "only {checked} touches were checked");
}
