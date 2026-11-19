use designos_doc::{ColorProfile, Document};

#[test]
fn a_new_document_is_srgb_and_leaves_the_key_out_of_the_file() {
    let doc = Document::new("Plain");
    assert_eq!(doc.color_profile, ColorProfile::Srgb);
    let json = doc.to_json().unwrap();
    assert!(!json.contains("color_profile"), "an sRGB file must stay byte-identical to older files");
}

#[test]
fn display_p3_survives_a_designos_round_trip() {
    let mut doc = Document::new("Wide");
    doc.color_profile = ColorProfile::DisplayP3;
    let json = doc.to_json().unwrap();
    assert!(json.contains("\"color_profile\":\"DisplayP3\""), "{json}");
    let back = Document::from_json(&json).unwrap();
    assert_eq!(back.color_profile, ColorProfile::DisplayP3);
}

#[test]
fn a_file_without_the_key_reads_as_srgb() {
    let json = Document::new("Old").to_json().unwrap();
    assert_eq!(Document::from_json(&json).unwrap().color_profile, ColorProfile::Srgb);
}

#[test]
fn a_cloned_document_keeps_its_profile() {
    let mut doc = Document::new("Wide");
    doc.color_profile = ColorProfile::DisplayP3;
    assert_eq!(doc.clone().color_profile, ColorProfile::DisplayP3);
    assert_eq!(doc.clone_without_images().color_profile, ColorProfile::DisplayP3);
}

#[test]
fn changing_the_profile_is_one_undo_step_and_dirties_the_whole_document() {
    use designos_doc::{Command, History};
    let mut doc = Document::new("t");
    doc.add_page("P");
    let mut h = History::default();
    let epoch = doc.mutation_epoch();
    h.exec(&mut doc, Command::SetColorProfile { profile: ColorProfile::DisplayP3 }).unwrap();
    assert_eq!(doc.color_profile, ColorProfile::DisplayP3);
    assert!(doc.mutation_epoch() > epoch, "a profile change must invalidate cached renders");
    h.undo(&mut doc).unwrap();
    assert_eq!(doc.color_profile, ColorProfile::Srgb);
    h.redo(&mut doc).unwrap();
    assert_eq!(doc.color_profile, ColorProfile::DisplayP3);
}
