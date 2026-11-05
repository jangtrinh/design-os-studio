//! Page-level API that phases 02 and 06 rely on: per-block hashes for block-level compare-and-swap, and errors
//! instead of panics.

mod common;

use common::parse;
use designos_docs::{BlockKind, ContentHash, DocsError, Inline};

#[test]
fn every_block_has_a_hash_of_its_text_for_block_level_checks() {
    let page = parse("# A\n\ntext\n\n- x\n");
    let hashes = page.block_hashes();
    assert_eq!(hashes, [ContentHash::of(b"# A"), ContentHash::of(b"text"), ContentHash::of(b"- x")]);
    assert_eq!(page.blocks[1].hash(), hashes[1]);

    // A block inserted above shifts indices but not the hashes of the blocks that did not change.
    let mut edited = page.clone();
    edited.insert_block(0, BlockKind::Paragraph(vec![Inline::Text("new".into())]));
    let after = edited.block_hashes();
    assert_eq!(&after[1..], &hashes[..]);
    assert_ne!(after[0], hashes[0]);

    // Editing a block changes only that block's hash.
    let mut edited = page.clone();
    *edited.blocks[1].kind_mut() = BlockKind::Paragraph(vec![Inline::Text("other".into())]);
    let after = edited.block_hashes();
    assert_eq!((after[0], after[2]), (hashes[0], hashes[2]));
    assert_eq!(after[1], ContentHash::of(b"other"));
}

#[test]
fn remove_block_reports_a_missing_index_as_an_error() {
    let mut page = parse("a\n\nb\n");
    let err = page.remove_block(5).unwrap_err();
    assert!(matches!(err, DocsError::BlockIndex { index: 5, len: 2 }), "{err:?}");
    assert!(err.to_string().contains("fix:"), "{err}");
    assert_eq!(page.blocks.len(), 2);
    assert!(page.remove_block(1).is_ok());
}
