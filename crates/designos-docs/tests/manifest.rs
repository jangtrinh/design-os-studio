//! `docs/docs-manifest.json`: strict validation with errors that name the field and the fix.

use designos_docs::{DocsError, Group, Manifest, ManifestError, Pin};

const VALID: &str = r#"{
  "schema_version": 1,
  "groups": [
    { "id": "overview", "pages": ["about.md"] },
    { "id": "spec", "pages": ["prd.md", "us/us-1.md"] },
    { "id": "screens", "pages": [] },
    { "id": "states", "pages": [] },
    { "id": "discovery", "pages": ["notes.md"] }
  ],
  "pins": [
    { "id": "p1", "file_id": "file_a", "node_id": "42", "name": "Checkout", "group": "screens" },
    { "id": "p2", "file_id": "file_a", "node_id": "43", "name": "Checkout error", "group": "states", "parent_screen": "p1" }
  ],
  "linked_specs": ["/Users/me/specs/checkout-spec"]
}"#;

fn err_of(json: &str) -> ManifestError {
    match Manifest::parse(json) {
        Err(DocsError::Manifest(e)) => e,
        other => panic!("expected a manifest error, got {other:?}"),
    }
}

/// Replaces one piece of the valid manifest, so each test changes exactly one thing.
fn with(from: &str, to: &str) -> String {
    assert!(VALID.contains(from), "test bug: {from:?} not in the valid manifest");
    VALID.replacen(from, to, 1)
}

#[test]
fn a_valid_manifest_parses_and_round_trips() {
    let m = Manifest::parse(VALID).unwrap();
    assert_eq!(m.groups.iter().map(|g| g.id).collect::<Vec<_>>(), Group::ALL);
    assert_eq!(m.group(Group::Spec).pages, ["prd.md", "us/us-1.md"]);
    assert_eq!(m.pins.len(), 2);
    assert_eq!(m.pins[1].parent_screen.as_deref(), Some("p1"));
    assert_eq!(m.linked_specs, ["/Users/me/specs/checkout-spec"]);
    assert_eq!(Manifest::parse(&m.to_json_pretty().unwrap()).unwrap(), m);
}

#[test]
fn groups_are_written_in_the_fixed_order() {
    let json = Manifest::new().to_json_pretty().unwrap();
    let at = |g: &str| json.find(&format!("\"{g}\"")).unwrap_or_else(|| panic!("{g} missing in {json}"));
    assert!(at("overview") < at("spec") && at("spec") < at("screens") && at("screens") < at("states") && at("states") < at("discovery"));
    assert_eq!(Manifest::parse(&json).unwrap(), Manifest::new());
}

#[test]
fn missing_groups_names_the_field_and_the_fix() {
    let json = r#"{ "schema_version": 1, "pins": [], "linked_specs": [] }"#;
    let e = err_of(json);
    assert_eq!(e.field, "groups");
    assert!(e.fix.contains("overview") && e.fix.contains("discovery"), "fix must list the five groups: {e}");
    let shown = DocsError::Manifest(e).to_string();
    assert!(shown.contains("docs-manifest.json") && shown.contains("`groups`") && shown.contains("fix:"), "{shown}");
}

#[test]
fn every_required_field_is_required_with_no_default_by_omission() {
    for (field, json) in [
        ("schema_version", r#"{ "groups": [], "pins": [], "linked_specs": [] }"#.to_string()),
        ("pins", with("\"pins\": [", "\"pinz\": [")),
        ("linked_specs", with("\"linked_specs\"", "\"linked\"")),
        ("groups[1].pages", with("{ \"id\": \"spec\", \"pages\": [\"prd.md\", \"us/us-1.md\"] }", "{ \"id\": \"spec\" }")),
        ("pins[0].file_id", with("\"file_id\": \"file_a\", \"node_id\": \"42\"", "\"node_id\": \"42\"")),
        ("pins[0].name", with("\"name\": \"Checkout\", ", "")),
    ] {
        let e = err_of(&json);
        assert_eq!(e.field, field, "{e}");
        assert!(!e.fix.is_empty());
    }
}

#[test]
fn the_group_list_must_be_the_five_groups_in_fixed_order() {
    let swapped = with(
        "{ \"id\": \"screens\", \"pages\": [] },\n    { \"id\": \"states\", \"pages\": [] },",
        "{ \"id\": \"states\", \"pages\": [] },\n    { \"id\": \"screens\", \"pages\": [] },",
    );
    let e = err_of(&swapped);
    assert_eq!(e.field, "groups[2].id");
    assert!(e.fix.contains("screens"), "{e}");

    let four = with("    { \"id\": \"discovery\", \"pages\": [\"notes.md\"] }\n", "");
    let four = four.replace("},\n  ],", "}\n  ],");
    assert_eq!(err_of(&four).field, "groups");

    assert_eq!(err_of(&with("\"id\": \"spec\"", "\"id\": \"specs\"")).field, "groups[1].id");
}

#[test]
fn page_paths_are_relative_markdown_files_listed_once() {
    for bad in ["../secret.md", "/etc/passwd.md", "notes.txt", "a\\\\b.md", "", ".hidden.md", "a//b.md"] {
        let e = err_of(&with("\"about.md\"", &format!("\"{bad}\"")));
        assert_eq!(e.field, "groups[0].pages[0]", "{bad:?}: {e}");
    }
    let e = err_of(&with("\"notes.md\"", "\"about.md\""));
    assert_eq!(e.field, "groups[4].pages[0]");
    assert!(e.problem.contains("about.md") && e.problem.contains("overview"), "{e}");
}

#[test]
fn a_bad_pin_names_the_pin_field_and_the_fix() {
    let e = err_of(&with("\"group\": \"screens\"", "\"group\": \"spec\""));
    assert_eq!(e.field, "pins[0].group");
    assert!(e.fix.contains("screens") && e.fix.contains("states"), "{e}");

    let e = err_of(&with("\"parent_screen\": \"p1\"", "\"parent_screen\": \"nope\""));
    assert_eq!(e.field, "pins[1].parent_screen");
    assert!(e.problem.contains("nope"), "{e}");

    let e = err_of(&with(", \"parent_screen\": \"p1\"", ""));
    assert_eq!(e.field, "pins[1].parent_screen", "a state needs its screen: {e}");

    let e = err_of(&with("\"group\": \"screens\"", "\"group\": \"screens\", \"parent_screen\": \"p2\""));
    assert_eq!(e.field, "pins[0].parent_screen", "only a state has a parent screen: {e}");

    let e = err_of(&with("\"id\": \"p2\"", "\"id\": \"p1\""));
    assert_eq!(e.field, "pins[1].id");

    let e = err_of(&with("\"node_id\": \"42\"", "\"node_id\": \"\""));
    assert_eq!(e.field, "pins[0].node_id");

    let e = err_of(&with("\"node_id\": \"42\"", "\"node_id\": \"4 2\""));
    assert_eq!(e.field, "pins[0].node_id", "{e}");

    let e = err_of(&with("\"group\": \"screens\"", "\"group\": 7"));
    assert_eq!(e.field, "pins[0].group");

    let e = err_of(&with("\"parent_screen\": \"p1\"", "\"parent_screen\": \"p2\""));
    assert_eq!(e.field, "pins[1].parent_screen", "a state cannot be its own parent: {e}");
}

#[test]
fn schema_version_unknown_fields_and_bad_json_are_refused() {
    let e = err_of(&with("\"schema_version\": 1", "\"schema_version\": 2"));
    assert_eq!(e.field, "schema_version");
    assert!(e.fix.to_lowercase().contains("update studio"), "{e}");

    let e = err_of(&with("\"schema_version\": 1,", "\"schema_version\": 1,\n  \"extra\": true,"));
    assert_eq!(e.field, "extra", "a typo must not silently drop data: {e}");

    let e = err_of(&with("\"name\": \"Checkout\"", "\"name\": \"Checkout\", \"colour\": \"red\""));
    assert_eq!(e.field, "pins[0].colour");

    let e = err_of("{ not json");
    assert_eq!(e.field, "(file)");
    assert!(e.problem.contains("line 1"), "{e}");

    assert_eq!(err_of("[]").field, "(file)");
}

#[test]
fn building_an_invalid_manifest_in_code_is_caught_before_it_is_written() {
    let mut m = Manifest::new();
    m.pins.push(Pin { id: "s".into(), file_id: "f".into(), node_id: "1".into(), name: "S".into(), group: Group::States, parent_screen: None });
    let DocsError::Manifest(e) = m.to_json_pretty().unwrap_err() else { panic!("manifest error expected") };
    assert_eq!(e.field, "pins[0].parent_screen");

    let mut m = Manifest::new();
    m.group_mut(Group::Overview).pages.push("../x.md".into());
    assert!(matches!(m.validate(), Err(DocsError::Manifest(_))));
}

#[test]
fn linked_specs_must_be_real_distinct_paths() {
    let with_specs = |list: &str| with(r#"["/Users/me/specs/checkout-spec"]"#, list);
    for (list, needle) in
        [(r#"[""]"#, "is empty"), (r#"["../elsewhere"]"#, ".."), (r#"["/a/../b"]"#, ".."), (r#"["/a", "/a"]"#, "again"), (r#"["bad\u0000path"]"#, "control")]
    {
        let e = err_of(&with_specs(list));
        assert!(e.field.starts_with("linked_specs["), "{list}: {e}");
        assert!(e.to_string().contains(needle), "{list}: {e}");
        assert!(e.fix.contains("spec folder"), "{list}: {e}");
    }
    // Absolute and project-relative folders are both fine.
    assert_eq!(Manifest::parse(&with_specs(r#"["/a/b", "specs/c"]"#)).unwrap().linked_specs, ["/a/b", "specs/c"]);
}

#[test]
fn pages_that_differ_only_in_case_are_one_file_on_macos_and_are_refused() {
    let e = err_of(&with(r#""pages": ["notes.md"]"#, r#""pages": ["notes.md", "Notes.md"]"#));
    assert_eq!(e.field, "groups[4].pages[1]");
    assert!(e.to_string().contains("already listed"), "{e}");
    let e = err_of(&with(r#""pages": ["notes.md"]"#, r#""pages": ["About.md"]"#));
    assert!(e.to_string().contains("already listed under overview"), "{e}");
}
