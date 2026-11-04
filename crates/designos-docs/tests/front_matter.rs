//! Front matter: detection, values, and problems that never hide the page body.

mod common;

use common::{parse, FIXTURES};
use designos_docs::{DocsStore, Group};

#[test]
fn front_matter_is_read_and_kept_verbatim() {
    let page = parse(FIXTURES[9].1);
    let fm = page.front_matter().expect("front matter");
    assert_eq!(fm.title(), Some("Checkout overview"));
    assert_eq!(fm.description(), Some("What this flow does: a summary"));
    assert_eq!(fm.group(), Some(Group::Overview));
}

#[test]
fn replacing_a_block_scalar_title_removes_its_continuation_lines() {
    let mut page = parse("---\ntitle: |\n  multi\n  line\ngroup: spec\n---\n# Body\n");
    page.set_title(Some("x"));
    let out = page.render();
    assert_eq!(out, "---\ntitle: x\ngroup: spec\n---\n# Body\n");
    let mut page = parse("---\ntitle: |\n  multi\n  line\ngroup: spec\n---\n# Body\n");
    page.set_title(None);
    assert_eq!(page.render(), "---\ngroup: spec\n---\n# Body\n");
}

#[test]
fn front_matter_after_a_byte_order_mark_is_found_and_not_duplicated() {
    let src = "\u{FEFF}---\ntitle: Hello\ngroup: spec\n---\n# H\n";
    let mut page = parse(src);
    let fm = page.front_matter().expect("front matter after a BOM");
    assert_eq!((fm.title(), fm.group()), (Some("Hello"), Some(Group::Spec)));
    assert_eq!(page.headings().len(), 1);
    assert_eq!(page.render(), src);
    page.set_title(Some("New"));
    let out = page.render();
    assert_eq!(out, "\u{FEFF}---\ntitle: New\ngroup: spec\n---\n# H\n");
    assert_eq!(out.matches("---").count(), 2, "one front matter block only");
}

#[test]
fn a_page_that_opens_with_a_thematic_break_is_not_swallowed_as_front_matter() {
    let src = "---\n\n# Intro\n\nBody text\n\n---\n\n## Next\n";
    let page = parse(src);
    assert!(page.front_matter().is_none());
    assert_eq!(page.headings().iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), ["Intro", "Next"]);
    assert_eq!(page.render(), src);
    // Real front matter next to a leading rule still works, and so does an empty block.
    assert!(parse("---\ntitle: x\n# a comment\n---\nbody\n").front_matter().is_some());
    assert!(parse("---\n---\nbody\n").front_matter().is_some());
}

// ------------------------------------------------------------------ M3 front matter never hides the body

#[test]
fn bad_front_matter_values_leave_the_key_unset_and_the_page_readable() {
    for src in ["---\ntitle: |\n  multi\n---\n# body\n", "---\ntitle: [a, b]\n---\n# body\n", "---\ngroup: sketches\ntitle: Fine\n---\n# body\n"] {
        let page = parse(src);
        assert_eq!(page.headings().len(), 1, "{src:?}");
        assert_eq!(page.render(), src);
    }
}

// ------------------------------------------------------------------ m3 CRLF

#[test]
fn front_matter_problems_are_reported_next_to_the_page_and_the_body_still_reads() {
    let page = parse("---\ntitle: |\n  multi line\ngroup: sketches\ndescription: Fine\n---\n# Body\n");
    let fm = page.front_matter().unwrap();
    assert_eq!((fm.title(), fm.group(), fm.description()), (None, None, Some("Fine")));
    let problems: Vec<(&str, bool)> = fm.problems().iter().map(|p| (p.field.as_str(), p.fix.contains("one line") || p.fix.contains(Group::list()))).collect();
    assert_eq!(problems, [("title", true), ("group", true)]);
    assert_eq!(page.headings().len(), 1);

    // Setting the key replaces the bad line and clears its problem.
    let mut page = page;
    page.set_group(Some(Group::Spec));
    assert_eq!(page.front_matter().unwrap().problems().len(), 1);
    assert_eq!(page.front_matter().unwrap().group(), Some(Group::Spec));
}

#[test]
fn valid_yaml_that_the_first_version_refused_is_read() {
    let page = parse("---\ntitle: \"Tab\\there \\u00e9 \\x41 \\/\"\ngroup: Spec\n---\nbody\n");
    let fm = page.front_matter().unwrap();
    assert_eq!(fm.title(), Some("Tab\there \u{e9} A /"));
    assert_eq!(fm.group(), Some(Group::Spec));
    assert!(fm.problems().is_empty(), "{:?}", fm.problems());
}

#[test]
fn reading_a_page_with_bad_front_matter_succeeds_and_reports_the_problem() {
    let tmp = tempfile::tempdir().unwrap();
    let store = DocsStore::at(tmp.path());
    std::fs::write(tmp.path().join("a.md"), "---\ngroup: nope\n---\n# Still here\n").unwrap();
    let read = store.read_page("a.md").expect("read_page must not fail on front matter");
    assert_eq!(read.value.headings().len(), 1);
    assert_eq!(read.value.front_matter().unwrap().problems()[0].field, "group");
}
