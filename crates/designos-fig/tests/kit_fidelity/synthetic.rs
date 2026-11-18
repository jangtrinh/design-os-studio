//! Each part, run on a tiny file with one known deviation, counts it as a mismatch, and a file without it matches.
//! With `SCOREBOARD_BROKEN_COMPARATOR=1` every comparator reports a match, and these tests fail.

use super::board::Tally;
use super::fixtures::*;
use super::keys::NodeKeys;
use super::raw::{self, Raw};
use super::layout;
use designos_doc::*;

fn load(bytes: &[u8]) -> (Raw, Document, NodeKeys) {
    let r = raw::decode(bytes);
    let mut doc = designos_fig::import_fig(bytes).expect("import");
    doc.hydrate_all_pages();
    let keys = NodeKeys::build(&doc);
    (r, doc, keys)
}

fn t(matched: u64, measured: u64) -> Tally {
    Tally { matched, measured }
}

#[test]
fn auto_layout_counts_a_child_off_its_solved_place() {
    let (_, doc, keys) = load(&auto_layout_fig(30.0));
    assert_eq!(layout::part(&doc, &keys).bucket("total")[0], t(2, 2), "children where the solve puts them");
    let (_, doc, keys) = load(&auto_layout_fig(33.0));
    assert_eq!(layout::part(&doc, &keys).bucket("total")[0], t(1, 2), "the second child stored 3 px off its solved place");
}
