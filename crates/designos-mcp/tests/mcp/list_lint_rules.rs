//! `designos-mcp --list-lint-rules` prints the linter registry and the vendored-skills checks read the same ids.
use designos_doc::lint::{all_rule_ids, canonical_rule_id};
use std::{collections::BTreeSet, path::PathBuf, process::Command};

fn workspace_file(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative)
}

fn registry_ids() -> Vec<String> {
    let mut ids: Vec<String> = all_rule_ids().into_iter().map(str::to_string).collect();
    ids.sort_unstable();
    ids
}

fn run_list_lint_rules() -> (String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_designos-mcp")).arg("--list-lint-rules").output().expect("run designos-mcp --list-lint-rules");
    assert!(out.status.success(), "designos-mcp --list-lint-rules failed: {:?}", out.status);
    (String::from_utf8(out.stdout).expect("stdout is UTF-8"), String::from_utf8(out.stderr).expect("stderr is UTF-8"))
}

#[test]
fn list_lint_rules_prints_every_registry_id_once_in_sorted_order() {
    let (stdout, stderr) = run_list_lint_rules();
    assert!(stderr.is_empty(), "the listing goes to stdout only: {stderr:?}");
    assert!(stdout.ends_with('\n'), "every id is on its own newline-terminated line");
    let listed: Vec<String> = stdout.lines().map(str::to_string).collect();
    assert_eq!(listed, registry_ids(), "the listing must be exactly the linter registry ids, sorted");
    assert!(listed.iter().all(|id| !id.trim().is_empty() && id.trim() == id), "no blank or padded lines");
}

#[test]
fn list_lint_rules_never_prints_an_alias() {
    let (stdout, _) = run_list_lint_rules();
    let listed: BTreeSet<&str> = stdout.lines().collect();
    for id in &listed {
        assert_eq!(canonical_rule_id(id), Some(*id), "`{id}` is not a canonical registry id");
    }
    // The legacy name still lints (it resolves to the canonical rule) but is not listed.
    assert_eq!(canonical_rule_id("overflowing-children"), Some("overflow-parent"));
    assert!(!listed.contains("overflowing-children"));
}

/// The string literals between `NAME: Set[str] = {` and the closing brace of a Python set.
fn python_set_literals(source: &str, name: &str) -> BTreeSet<String> {
    let start = source.find(&format!("{name}: Set[str] = {{")).unwrap_or_else(|| panic!("{name} not found in check_vendor_skills.py"));
    let body = &source[start..];
    let body = &body[..body.find("\n}").expect("the set literal is closed")];
    body.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.trim().strip_prefix('"')?.strip_suffix("\",").map(str::to_string))
        .collect()
}

#[test]
fn vendored_skills_checker_fallback_ids_are_the_registry_ids() {
    // CI runs the vendored-skills check before it builds designos-mcp, so the checker falls back to this set.
    let source = std::fs::read_to_string(workspace_file("scripts/check_vendor_skills.py")).expect("read scripts/check_vendor_skills.py");
    let fallback = python_set_literals(&source, "CANONICAL_LINT_RULE_IDS");
    let registry: BTreeSet<String> = registry_ids().into_iter().collect();
    assert_eq!(fallback, registry, "CANONICAL_LINT_RULE_IDS in scripts/check_vendor_skills.py must equal the linter registry ids");
}

#[test]
fn vendored_skills_rule_map_targets_registry_ids() {
    let map = std::fs::read_to_string(workspace_file("skills/vendor/jakubkrehel-skills/rule-map.toml")).expect("read rule-map.toml");
    let registry: BTreeSet<String> = registry_ids().into_iter().collect();
    let targets: Vec<&str> = map.lines().filter_map(|line| line.strip_prefix("map = \"lint:")?.strip_suffix('"')).collect();
    assert!(!targets.is_empty(), "rule-map.toml maps some rules to lint ids");
    for id in targets {
        assert!(
            registry.contains(id),
            "rule-map.toml maps to `lint:{id}`, which is not a canonical registry id (an alias maps to the same rule, but the map names canonical ids)"
        );
    }
}
