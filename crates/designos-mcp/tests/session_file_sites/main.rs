//! Completeness of the session-file helper, made mechanical: every filesystem call in the non-test source of designos-mcp goes
//! through `session_file_io` or is allowlisted with a reason, and every mention of a session-file name is allowlisted. A raw
//! call, an unlisted call or name, and a stale entry fail here, so the next review does not have to find them by hand.

mod allowlist;
mod scan;

use scan::{problems, read_sources, Allowed, Source};

fn source(path: &str, text: &str) -> Source {
    Source { path: path.into(), text: text.into() }
}

#[test]
fn every_filesystem_call_and_session_file_name_is_accounted_for() {
    let found = problems(&read_sources(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))), allowlist::FS_CALLS, allowlist::NAMES);
    assert!(found.is_empty(), "\n{}\n", found.join("\n"));
}

const WORKSPACE: Allowed = Allowed { file: "src/a.rs", func: Some("save"), why: "a --workspace path" };

#[test]
fn a_call_through_the_helper_needs_no_entry() {
    let sources = [source("src/a.rs", "fn save() {\n    session_file_io::append(path, b\"x\", false);\n    self.file_problems.read_to_string(&p);\n}\n")];
    assert_eq!(problems(&sources, &[], &[]), Vec::<String>::new());
}

#[test]
fn a_raw_call_that_is_not_listed_fails() {
    let sources = [source("src/a.rs", "fn save() {\n    std::fs::write(path, bytes).unwrap();\n}\n")];
    let found = problems(&sources, &[], &[]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("not through session_file_io and not allowlisted: src/a.rs:2 (fn save)"), "{found:?}");
    assert_eq!(problems(&sources, &[WORKSPACE], &[]), Vec::<String>::new(), "the same call is fine once it is listed with a reason");
}

#[test]
fn a_raw_call_on_a_session_file_fails_even_in_a_listed_function() {
    for line in [
        "std::fs::read_to_string(session_dir.join(\"x\")).ok();",
        "std::fs::read_to_string(self.bridge.live_doc_patch_path.clone().unwrap()).ok();",
        "std::fs::read_to_string(\"context.json\").ok();",
    ] {
        let sources = [source("src/a.rs", &format!("fn save() {{\n    {line}\n}}\n"))];
        let found = problems(&sources, &[WORKSPACE], &[Allowed { file: "src/a.rs", func: Some("save"), why: "builds a path" }]);
        assert!(found.iter().any(|p| p.contains("raw filesystem call on a session file")), "{line}: {found:?}");
    }
}

#[test]
fn an_unlisted_session_file_name_fails_and_a_listed_one_does_not() {
    let sources = [source("src/a.rs", "fn path_of(dir: &Path) -> PathBuf {\n    dir.join(\"doc-patch.jsonl\")\n}\n")];
    let found = problems(&sources, &[], &[]);
    assert!(found.iter().any(|p| p.contains("session-file name or env var not allowlisted: src/a.rs:2 (fn path_of)")), "{found:?}");
    let listed = [Allowed { file: "src/a.rs", func: Some("path_of"), why: "builds the path; every use goes through the helper" }];
    assert_eq!(problems(&sources, &[], &listed), Vec::<String>::new());
}

#[test]
fn a_stale_entry_and_an_entry_without_a_reason_fail() {
    let sources = [source("src/a.rs", "fn other() {}\n")];
    let found = problems(&sources, &[WORKSPACE], &[]);
    assert!(found.iter().any(|p| p.contains("stale fs-call allowlist entry: src/a.rs")), "{found:?}");
    let no_reason = Allowed { file: "src/a.rs", func: Some("other"), why: " " };
    assert!(problems(&sources, &[no_reason], &[]).iter().any(|p| p.contains("needs a reason")));
    let whole_file = Allowed { file: "src/a.rs", func: None, why: "it is fine" };
    assert!(problems(&sources, &[whole_file], &[]).iter().any(|p| p.contains("needs a reason")), "a whole-file entry must say where its paths come from");
}

#[test]
fn the_helper_files_and_test_code_are_not_flagged() {
    let helper = source("src/session_file_io/no_follow.rs", "fn open() {\n    std::fs::OpenOptions::new().open(session_dir);\n}\n");
    let gated = source(
        "src/a.rs",
        "#[cfg(test)]\nmod tests {\n    fn t() {\n        std::fs::write(\"context.json\", b\"x\");\n    }\n}\n#[cfg(test)]\nmod a_tests;\n",
    );
    let test_file = source("src/a_tests.rs", "fn t() {\n    std::fs::write(session_dir, b\"x\");\n}\n");
    assert_eq!(problems(&[helper, gated, test_file], &[], &[]), Vec::<String>::new());
}

#[test]
fn a_file_named_like_a_test_but_not_gated_is_scanned() {
    let sources = [source("src/a.rs", "fn main() {}\n"), source("src/b_tests.rs", "fn t() {\n    std::fs::write(session_dir, b\"x\");\n}\n")];
    assert!(problems(&sources, &[], &[]).iter().any(|p| p.contains("src/b_tests.rs:2")));
}

#[test]
fn the_scan_finds_the_sites_it_is_there_for() {
    let sources = read_sources(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    let paths: Vec<_> = sources.iter().map(|s| s.path.as_str()).collect();
    for expected in ["src/server.rs", "src/tools/ledger.rs", "src/session_file_io/no_follow.rs", "src/tools/session_options.rs"] {
        assert!(paths.contains(&expected), "{expected} is not scanned");
    }
}
