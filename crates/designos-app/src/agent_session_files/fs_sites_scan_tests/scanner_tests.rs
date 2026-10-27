//! The scan of shipped code, tested on source made for the test. Each test is a way the scan was once blind (or could be), written as
//! the smallest source that shows it: with the defect the line is not reported, without it the line is.

use super::{parse_allowlist, problems};
use crate::agent_session_files::fs_scan::shipped_files;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A crate made of `files` (path below `src`, source), scanned with an empty allowlist: every problem it has.
pub(super) fn scan(files: &[(&str, &str)]) -> Vec<String> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root: PathBuf = std::env::temp_dir().join(format!("designos-fs-scan-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    for (rel, source) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("make the folder");
        std::fs::write(&path, source).expect("write the source");
    }
    let found = problems(&shipped_files(&root), &parse_allowlist(""));
    std::fs::remove_dir_all(&root).ok();
    found
}

pub(super) fn flagged(found: &[String], text: &str) -> bool {
    found.iter().any(|problem| problem.contains(text))
}

/// A function that reads `agent-status.json` below `session_dir` with std, the kind of line the scan exists to find.
const READ: &str = "    let _ = std::fs::read_to_string(session_dir.join(\"agent-status.json\"));\n";

fn function_with(body: &str) -> String {
    format!("pub fn probe(session_dir: &std::path::Path) {{\n{body}}}\n")
}

#[test]
fn an_inner_cfg_test_inside_an_inline_module_does_not_hide_the_rest_of_the_file() {
    let source = format!("mod inner {{\n    #![cfg(test)]\n}}\n{}", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(flagged(&found, "std::fs::read_to_string(session_dir.join(\"agent-status.json\"))"), "{found:?}");
}

#[test]
fn an_inner_cfg_test_at_the_top_of_a_file_hides_that_whole_file() {
    let source = format!("#![cfg(test)]\n{}", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_cfg_test_on_a_field_does_not_hide_the_code_after_its_struct() {
    let source = format!("pub struct S {{\n    #[cfg(test)]\n    pub path_override: Option<u8>,\n    pub x: u8,\n}}\nimpl S {{\n{}}}\n", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(flagged(&found, "std::fs::read_to_string"), "{found:?}");
}

#[test]
fn a_cfg_test_on_a_private_field_with_a_generic_type_does_not_hide_the_code_after_its_struct() {
    let source = format!("pub struct S {{\n    #[cfg(test)]\n    map: std::collections::HashMap<u8, u8>,\n}}\n{}", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(flagged(&found, "std::fs::read_to_string"), "{found:?}");
}

#[test]
fn a_cfg_test_on_an_enum_variant_does_not_hide_the_code_after_its_enum() {
    let source = format!("pub enum E {{\n    #[cfg(test)]\n    A,\n    B,\n}}\n{}", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(flagged(&found, "std::fs::read_to_string"), "{found:?}");
}

#[test]
fn a_cfg_test_on_a_match_arm_does_not_hide_the_arm_after_it() {
    let source = "pub fn probe(x: u8, session_dir: &std::path::Path) {\n    match x {\n        #[cfg(test)]\n        1 => noop(),\n        _ => {\n    let _ = std::fs::read_to_string(session_dir.join(\"agent-status.json\"));\n        }\n    }\n}\n";
    let found = scan(&[("lib.rs", source)]);
    assert!(flagged(&found, "std::fs::read_to_string"), "{found:?}");
}

#[test]
fn a_cfg_test_item_is_still_skipped_and_so_is_the_code_it_holds() {
    let source = format!("#[cfg(test)]\nmod tests {{\n{}}}\n#[cfg(test)]\nfn only_in_tests(session_dir: &std::path::Path) {{\n{READ}}}\n", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_cfg_all_whose_own_argument_is_an_any_with_test_in_it_is_not_test_only() {
    let source = format!("#[cfg(all(any(unix, test, windows)))]\n{}", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(flagged(&found, "std::fs::read_to_string"), "{found:?}");
}

#[test]
fn a_cfg_all_with_test_among_its_own_arguments_is_test_only() {
    let source = format!("#[cfg(all(unix, test))]\n{}", function_with(READ));
    let found = scan(&[("lib.rs", &source)]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_file_a_shipped_module_declares_is_scanned_whatever_a_test_gated_path_says() {
    let found = scan(&[("lib.rs", "mod writer;\n#[cfg(test)]\n#[path = \"writer.rs\"]\nmod again;\n"), ("writer.rs", &function_with(READ))]);
    assert!(flagged(&found, "std::fs::read_to_string"), "{found:?}");
}

#[test]
fn a_file_only_a_test_gated_module_declares_is_skipped() {
    let found = scan(&[("lib.rs", "#[cfg(test)]\nmod fixtures;\n"), ("fixtures.rs", &function_with(READ))]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_file_that_gates_itself_with_an_inner_cfg_test_is_skipped_even_when_a_shipped_module_declares_it() {
    let source = format!("#![cfg(test)]\n{}", function_with(READ));
    let found = scan(&[("lib.rs", "mod gated;\n"), ("gated.rs", &source)]);
    assert!(found.is_empty(), "{found:?}");
}
