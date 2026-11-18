//! `--export-spec` and `--export-variables` write where an agent names: a link at the final component, a linked folder or a linked
//! file inside a variables folder is never written through, and the name must end in the format's ending.
//!
//! An existing file is still replaced without `--overwrite`; the guards below pin that, so a change of it is a decision and not a
//! side effect.

#[cfg(unix)]
use crate::agent_cli_headless_writes_tests::{is_link, link};
use crate::agent_cli_headless_writes_tests::{run, text, Scratch, KEEP};
use std::path::PathBuf;

impl Scratch {
    /// A saved document named `kit.designos` with one collection (`_Primitives`, one mode `Style`) and one colour variable.
    pub(crate) fn document_with_variables(&self) -> PathBuf {
        let path = self.path("kit.designos");
        let mut doc = designos_doc::Document::new("Kit");
        doc.add_page("Page 1");
        doc.variable_collections.insert(
            "VariableCollectionId:10:1".into(),
            designos_doc::VariableCollection {
                id: "VariableCollectionId:10:1".into(),
                name: "_Primitives".into(),
                modes: vec![designos_doc::VariableMode { id: "10:m1".into(), name: "Style".into() }],
                default_mode: "10:m1".into(),
                variables: vec!["VariableID:10:v1".into()],
                key: None,
                source_library_key: None,
                extensions: std::collections::BTreeMap::new(),
            },
        );
        doc.variables.insert(
            "VariableID:10:v1".into(),
            designos_doc::Variable {
                id: "VariableID:10:v1".into(),
                name: "Colors/Brand/500".into(),
                collection: "VariableCollectionId:10:1".into(),
                kind: designos_doc::VariableKind::Color,
                values: std::collections::BTreeMap::from([("10:m1".into(), designos_doc::VariableValue::Color(designos_doc::Color::rgb(10, 10, 10)))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: std::collections::BTreeMap::new(),
                extensions: std::collections::BTreeMap::new(),
                mode_extensions: std::collections::BTreeMap::new(),
            },
        );
        crate::io::save_path(&doc, &path).unwrap();
        path
    }
}

// --- --export-spec -----------------------------------------------------------------------------------------------------

#[test]
fn export_spec_refuses_a_name_that_is_not_json_and_leaves_the_file_alone() {
    let s = Scratch::new("spec_extension");
    let doc = s.document();
    let wrong = s.path("out.txt");
    std::fs::write(&wrong, KEEP).unwrap();

    let err = run(&[text(&doc), "--export-spec", text(&wrong)]).unwrap_err();

    assert!(err.contains(".json"), "{err}");
    assert_eq!(std::fs::read(&wrong).unwrap(), KEEP);
}

#[cfg(unix)]
#[test]
fn export_spec_never_writes_through_a_link() {
    let s = Scratch::new("spec_link");
    let doc = s.document();
    let victim = s.path("victim.json");
    let linked = s.path("link.json");
    std::fs::write(&victim, KEEP).unwrap();
    link(&victim, &linked);

    let err = run(&[text(&doc), "--export-spec", text(&linked)]).unwrap_err();

    assert!(err.contains("symbolic link"), "{err}");
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
    assert!(is_link(&linked));
}

#[test]
fn export_spec_replaces_an_existing_file_without_overwrite() {
    let s = Scratch::new("spec_replace");
    let doc = s.document();
    for name in ["out.json", "OUT.JSON"] {
        let out = s.path(name);
        std::fs::write(&out, KEEP).unwrap();

        run(&[text(&doc), "--export-spec", text(&out)]).unwrap_or_else(|e| panic!("{name}: {e}"));

        assert!(serde_json::from_slice::<serde_json::Value>(&std::fs::read(&out).unwrap()).is_ok(), "{name} holds the spec");
    }
    assert!(!s.names().iter().any(|n| n.ends_with(".tmp") || n.starts_with(".designos")), "no temporary file stays behind: {:?}", s.names());
}

// --- --export-variables ------------------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn export_variables_never_writes_through_a_link() {
    let s = Scratch::new("vars_link");
    let doc = s.document_with_variables();
    let victim = s.path("victim.bin");
    std::fs::write(&victim, KEEP).unwrap();
    let zip_link = s.path("tokens.zip");
    let json_link = s.path("tokens.json");
    link(&victim, &zip_link);
    link(&victim, &json_link);

    for (target, extra) in [(&zip_link, vec![]), (&json_link, vec![]), (&json_link, vec!["--format", "dtcg", "--mode", "Style"])] {
        let mut args = vec![text(&doc), "--export-variables", text(target)];
        args.extend(extra);
        let err = run(&args).unwrap_err();
        assert!(err.contains("symbolic link"), "{args:?}: {err}");
    }
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
}

#[cfg(unix)]
#[test]
fn export_variables_never_writes_through_a_linked_folder_or_a_linked_entry() {
    let s = Scratch::new("vars_folder_link");
    let doc = s.document_with_variables();
    let victim = s.path("victim.json");
    std::fs::write(&victim, KEEP).unwrap();
    let outside = s.path("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let linked_folder = s.path("linked-folder");
    link(&outside, &linked_folder);

    let err = run(&[text(&doc), "--export-variables", text(&linked_folder), "--format", "dtcg"]).unwrap_err();
    assert!(err.contains("symbolic link"), "{err}");
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none(), "the folder behind the link was written into");

    let folder = s.path("bundle");
    std::fs::create_dir_all(&folder).unwrap();
    link(&victim, &folder.join("tokens.manifest.json"));
    let err = run(&[text(&doc), "--export-variables", text(&folder), "--format", "dtcg"]).unwrap_err();
    assert!(err.contains("symbolic link"), "{err}");
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
}

#[test]
fn export_variables_replaces_existing_files_without_overwrite() {
    let s = Scratch::new("vars_replace");
    let doc = s.document_with_variables();

    let zip = s.path("TOKENS.ZIP");
    std::fs::write(&zip, KEEP).unwrap();
    run(&[text(&doc), "--export-variables", text(&zip)]).unwrap();
    assert!(std::fs::read(&zip).unwrap().starts_with(b"PK"), "the zip replaced the old file");

    let json = s.path("tokens.json");
    std::fs::write(&json, KEEP).unwrap();
    run(&[text(&doc), "--export-variables", text(&json)]).unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&std::fs::read(&json).unwrap()).is_ok(), "the json replaced the old file");

    let mode_json = s.path("light.tokens.json");
    std::fs::write(&mode_json, KEEP).unwrap();
    run(&[text(&doc), "--export-variables", text(&mode_json), "--format", "dtcg", "--mode", "Style"]).unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&std::fs::read(&mode_json).unwrap()).is_ok(), "the mode export replaced the old file");

    let folder = s.path("bundle");
    run(&[text(&doc), "--export-variables", text(&folder), "--format", "dtcg"]).unwrap();
    let entry = std::fs::read_dir(&folder).unwrap().flatten().next().expect("the bundle has a file").path();
    std::fs::write(&entry, KEEP).unwrap();
    run(&[text(&doc), "--export-variables", text(&folder), "--format", "dtcg"]).unwrap();
    assert_ne!(std::fs::read(&entry).unwrap(), KEEP, "the entry was replaced");
}
