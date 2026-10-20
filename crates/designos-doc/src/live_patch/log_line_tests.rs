use super::*;

#[test]
fn a_log_line_is_an_edit_or_a_document_switch() {
    let edit = serde_json::to_string(&LivePatch::new(vec![LiveOp::Remove { id: NodeId(7) }])).unwrap();
    assert!(matches!(LiveLogLine::parse(edit.as_bytes()), Ok(LiveLogLine::Patch(p)) if p.ops.len() == 1));
    let switch = serde_json::to_string(&DocEpoch::of(3, b"doc")).unwrap();
    assert_eq!(LiveLogLine::parse(switch.as_bytes()).unwrap(), LiveLogLine::DocEpoch(DocEpoch::of(3, b"doc")));
}

#[test]
fn an_unreadable_log_line_is_refused() {
    assert!(LiveLogLine::parse(b"{\"v\":1,\"ops\":\"nope\"}").is_err());
    assert!(LiveLogLine::parse(b"{\"doc_epoch\":1}").is_err(), "a switch without its stamp is not a switch");
    assert!(LiveLogLine::parse(b"not json").is_err());
}

#[test]
fn a_switch_matches_only_the_document_it_was_written_for() {
    let mark = DocEpoch::of(1, b"first document");
    assert!(mark.matches(b"first document"));
    assert!(!mark.matches(b"second document"));
    assert!(!mark.matches(b"first documenT"), "same length, other bytes");
}

#[test]
fn patch_label_is_skipped_when_none_and_roundtrips_when_set() {
    let plain = serde_json::to_string(&LivePatch::new(vec![LiveOp::Remove { id: NodeId(7) }])).unwrap();
    assert!(!plain.contains("\"label\""), "None label must be omitted: {plain}");
    let parsed = match LiveLogLine::parse(plain.as_bytes()).unwrap() {
        LiveLogLine::Patch(p) => p,
        other => panic!("expected patch, got {other:?}"),
    };
    assert_eq!(parsed.label, None);

    let labelled = serde_json::to_string(&LivePatch::new(vec![LiveOp::Remove { id: NodeId(7) }]).with_label(Some("Agent: run-7".into()))).unwrap();
    assert!(labelled.contains("\"label\":\"Agent: run-7\""), "{labelled}");
    let parsed_lbl = match LiveLogLine::parse(labelled.as_bytes()).unwrap() {
        LiveLogLine::Patch(p) => p,
        other => panic!("expected patch, got {other:?}"),
    };
    assert_eq!(parsed_lbl.label.as_deref(), Some("Agent: run-7"));
}
