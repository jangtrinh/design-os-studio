//! A reconnect replay keeps the table rule: a log line that would break a table is dropped alone,
//! and the unrelated lines around it stay applied.

use super::replay_unabsorbed_patches;
use designos_doc::live_patch::{live_ops_for_command, LiveOp, LivePatch, SiblingHint};
use designos_doc::table_ops::create_table;
use designos_doc::{check_table_positions, Command, Document, Node, NodeId, NodeKind, Rect, TableStyle, Vec2};

fn one_table() -> (Document, NodeId, NodeId) {
    let mut d = Document::new("r");
    let page = d.add_page("P");
    create_table(&d, page, Vec2::new(0.0, 0.0), 3, 3, &TableStyle::default()).unwrap().apply(&mut d).unwrap();
    let t = d.get(page).unwrap().children[0];
    (d, page, t)
}

fn line(ops: Vec<LiveOp>) -> String {
    serde_json::to_string(&LivePatch::new(ops).with_base(Some(10))).unwrap()
}

fn insert_rect(d: &Document, page: NodeId, id: u64) -> String {
    let mut r = Node::rectangle(NodeId(id), Rect::new(0.0, 0.0, 5.0, 5.0));
    r.parent = Some(page);
    line(vec![LiveOp::Insert { parent: page, at: SiblingHint::at(&d.get(page).unwrap().children, 0), nodes: vec![r] }])
}

fn replay(d: &mut Document, lines: &[String]) -> Result<usize, String> {
    let log = lines.join("\n") + "\n";
    replay_unabsorbed_patches(d, log.as_bytes(), Some(10))
}

#[test]
fn a_table_update_that_drops_a_row_is_dropped_and_the_table_stays_valid() {
    let (mut d, page, t) = one_table();
    let mut broken = d.get(t).unwrap().clone();
    if let NodeKind::Table(p) = &mut broken.kind {
        p.rows.pop();
    }
    let bad = line(live_ops_for_command(&d, &Command::Update { node: Box::new(broken) }).unwrap());
    let (before, after) = (insert_rect(&d, page, 5000), insert_rect(&d, page, 5001));
    replay(&mut d, &[before, bad, after]).unwrap();
    check_table_positions(&d, t).expect("the table is intact");
    assert!(d.get(NodeId(5000)).is_some() && d.get(NodeId(5001)).is_some(), "the unrelated patches around it stay applied");
}

#[test]
fn a_frame_inserted_under_a_table_loses_only_that_patch() {
    let (mut d, page, t) = one_table();
    let good = insert_rect(&d, page, 5000);
    let mut f = Node::frame(NodeId(6000), Rect::new(0.0, 0.0, 5.0, 5.0));
    f.parent = Some(t);
    let bad = line(vec![LiveOp::Insert { parent: t, at: SiblingHint::at(&d.get(t).unwrap().children, 0), nodes: vec![f] }]);
    let later = insert_rect(&d, page, 5001);
    replay(&mut d, &[good, bad, later]).expect("the replay goes on");
    assert!(d.get(NodeId(5000)).is_some(), "the earlier unrelated patch is kept");
    assert!(d.get(NodeId(5001)).is_some(), "the later unrelated patch is applied");
    assert!(d.get(NodeId(6000)).is_none());
    check_table_positions(&d, t).unwrap();
}

#[test]
fn removing_one_cell_is_dropped_and_the_table_stays_valid() {
    let (mut d, page, t) = one_table();
    let cell = d.get(t).unwrap().children[4];
    let (before, after) = (insert_rect(&d, page, 5000), insert_rect(&d, page, 5001));
    replay(&mut d, &[before, line(vec![LiveOp::Remove { id: cell }]), after]).unwrap();
    assert!(d.get(cell).is_some(), "the cell is still there");
    check_table_positions(&d, t).unwrap();
    assert!(d.get(NodeId(5000)).is_some() && d.get(NodeId(5001)).is_some());
}

#[test]
fn every_refused_patch_is_counted_with_its_reason() {
    let (mut d, page, t) = one_table();
    let cell = d.get(t).unwrap().children[4];
    let log = [insert_rect(&d, page, 5000), line(vec![LiveOp::Remove { id: cell }])].join("\n") + "\n";
    let (applied, refused) = super::replay_unabsorbed_patches_counted(&mut d, log.as_bytes(), Some(10)).unwrap();
    assert_eq!(applied, 1);
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].starts_with("patch 2:"), "{refused:?}");
}
