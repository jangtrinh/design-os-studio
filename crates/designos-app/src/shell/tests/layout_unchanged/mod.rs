use super::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

mod d;
mod e;
mod f;
mod g;
mod h;
mod i;
mod j;
mod k;

#[derive(Debug, Clone, Deserialize)]
pub struct CasesFile {
    #[serde(default)]
    pub bless_sha: Option<String>,
    pub cases: Vec<CaseEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepKind {
    CommentThread,
    PrototypeLink,
    GridStyle,
    ComponentInstance,
    CheckDesignsFindings,
    PublishedLibrary,
    BlankBoard,
    BlankDeck,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TierTarget {
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub select: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub prep: Vec<PrepKind>,
    #[serde(default)]
    pub unreachable: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaseEntry {
    pub id: String,
    pub spec: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub tier_a: Option<TierTarget>,
    #[serde(default)]
    pub tier_b: Option<TierTarget>,
    #[serde(default)]
    pub kit: bool,
    #[serde(default)]
    pub virtualized: Vec<String>,
    #[serde(default)]
    pub spawns: bool,
}

pub mod prep {
    use super::*;

    pub fn comment_thread(_doc: &mut designos_doc::Document, _app: &mut App) {}
    pub fn prototype_link(_doc: &mut designos_doc::Document) {}
    pub fn grid_style(_doc: &mut designos_doc::Document, _target: NodeId) {}
    pub fn component_instance(_doc: &mut designos_doc::Document) -> NodeId {
        NodeId(0)
    }
    pub fn check_designs_findings(_doc: &mut designos_doc::Document) {}
    pub fn published_library(_doc: &mut designos_doc::Document, _lib_root: Option<&Path>) {}
    pub fn blank_board(_doc: &mut designos_doc::Document) {}
    pub fn blank_deck(_doc: &mut designos_doc::Document, _app: &mut App) {}
}

pub fn baseline_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/layout-baseline")
}

pub fn load_cases_file() -> CasesFile {
    let path = baseline_dir().join("cases.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

pub fn load_allow_entries() -> crate::layout_snapshot::AllowList {
    let path = baseline_dir().join("allow.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    crate::layout_snapshot::parse_allow_list(&raw).unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

pub fn run_spec_cases(spec_key: &str) {
    if option_env!("CARGO_BIN_NAME") != Some("designos") {
        return;
    }
    const {
        assert!(cfg!(debug_assertions), "Tier A layout gate requires debug_assertions");
    };

    let bdir = baseline_dir();
    let cases_file = load_cases_file();
    let allow_entries = load_allow_entries();
    let bless = std::env::var_os("DESIGNOS_LAYOUT_BLESS").is_some();

    let spec_cases: Vec<&CaseEntry> = cases_file.cases.iter().filter(|c| !c.kit && c.spec == spec_key).collect();
    assert!(!spec_cases.is_empty(), "no headless cases found in cases.json for spec {spec_key}");

    let stub_dir = crate::cli_resolve::stub_dir().to_path_buf();
    let pid = std::process::id();
    let mut run_matched = std::collections::BTreeSet::new();
    let mut run_cases = std::collections::BTreeSet::new();

    for (idx, case) in spec_cases.into_iter().enumerate() {
        let _ = case.spawns;
        let case_tmp = std::env::temp_dir().join(format!("designos-tier-a-{}-{}-{pid}-{idx}", spec_key.to_ascii_lowercase(), case.id));
        let _ = std::fs::remove_dir_all(&case_tmp);
        std::fs::create_dir_all(&case_tmp).expect("create case temp dir");

        let mut expanded_args = Vec::with_capacity(case.args.len());
        let mut seeded_lib: Option<PathBuf> = None;
        for arg in &case.args {
            if arg.contains("{LIB}") {
                let lib_dir = seeded_lib.get_or_insert_with(|| {
                    let d = case_tmp.join("seed-lib");
                    crate::showcase::seed_library_at(&d, 1_700_000_000_000).expect("seed deterministic library");
                    d
                });
                expanded_args.push(arg.replace("{LIB}", &lib_dir.to_string_lossy()));
            } else {
                expanded_args.push(arg.clone());
            }
        }

        let mut opts = crate::cli::parse(expanded_args).unwrap_or_else(|e| panic!("case {} failed cli::parse: {e}", case.id));
        opts.size = Some((1440, 900));
        opts.scale = Some(1.0);
        opts.no_toast = true;
        opts.agent_cli_dir = Some(stub_dir.clone());
        opts.fixed_now = Some(1_700_000_060_000);

        {
            let _scoped = crate::cli_resolve::ScopedCliDirs::enter(stub_dir.clone());
            let mut sim = Sim::with(opts);
            sim.shell.apply_launch_defaults(case.id != "d-window-chrome");
            crate::theme::apply(&sim.ctx);
            sim.ctx.all_styles_mut(|s| s.debug.show_interactive_widgets = true);

            for _ in 0..8 {
                sim.frame(vec![]);
            }
            if case.id == "i-agents-start-failure" {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                let mut exited = false;
                while std::time::Instant::now() < deadline {
                    let done = sim.shell.app.terminal.tabs.first_mut().is_none_or(|t| !t.is_alive() && t.reader_eof.load(std::sync::atomic::Ordering::Relaxed));
                    if done {
                        exited = true;
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                assert!(exited, "case {}: stub child did not exit within 15s deadline", case.id);
                if let Some(t) = sim.shell.app.terminal.tabs.first_mut() {
                    t.spawned_at = std::time::Instant::now();
                }
                for _ in 0..2 {
                    sim.frame(vec![]);
                }
                assert!(
                    sim.shell.app.terminal.active_tab().is_some_and(|t| t.start_failure.is_some()),
                    "case {}: must capture the diagnosed start_failure state, never a half-started state",
                    case.id
                );
            }

            let mut snap = crate::layout_snapshot::capture(&sim.ctx, &sim.shell.app.ui.regions);
            snap.case = case.id.clone();
            assert!(!snap.named.is_empty() || !snap.unnamed.is_empty(), "case {} produced an empty widget snapshot", case.id);

            let baseline_path = bdir.join(format!("{}.layout.json", case.id));
            if bless {
                let json = serde_json::to_string_pretty(&snap).expect("serialize layout snapshot");
                std::fs::write(&baseline_path, format!("{json}\n")).expect("write blessed layout baseline");
            } else if cases_file.bless_sha.as_deref().is_some_and(|s| !s.is_empty()) || baseline_path.exists() {
                match crate::layout_snapshot::load_and_compare_baseline(&baseline_path, &snap, &allow_entries, &case.virtualized) {
                    Ok(report) => {
                        run_matched.extend(report.matched_allow_indices);
                        run_cases.insert(case.id.clone());
                    }
                    Err(err) => panic!("case {} failed layout gate:\n{err}", case.id),
                }
            }
        }

        let _ = std::fs::remove_dir_all(&case_tmp);
    }

    if !run_cases.is_empty() {
        if let Err(err) = crate::layout_snapshot::check_stale_allow_entries_for_run(&allow_entries, &run_matched, Some(&run_cases), Some(spec_key)) {
            panic!("spec {spec_key} failed stale allow check:\n{err}");
        }
    }
}

#[test]
fn cases_json_and_allow_json_match_spec_l_contract() {
    let cases_file = load_cases_file();
    let _allow = load_allow_entries();

    let headless: Vec<&CaseEntry> = cases_file.cases.iter().filter(|c| !c.kit).collect();
    let kit: Vec<&CaseEntry> = cases_file.cases.iter().filter(|c| c.kit).collect();
    assert_eq!(headless.len(), 74, "expected 74 headless cases in cases.json, got {}", headless.len());
    assert_eq!(kit.len(), 2, "expected 2 kit cases in cases.json, got {}", kit.len());

    for spec_key in ["D", "E", "F", "G", "H", "I", "J", "K"] {
        assert!(headless.iter().any(|c| c.spec == spec_key), "expected at least one headless case for spec {spec_key}");
    }

    for c in &headless {
        let args: Vec<String> = c.args.iter().map(|a| a.replace("{LIB}", "/tmp/designos-lib")).collect();
        crate::cli::parse(args).unwrap_or_else(|e| panic!("headless case {} failed cli::parse: {e}", c.id));
    }
}

#[test]
fn cases_json_has_no_old_fixture() {
    let path = baseline_dir().join("cases.json");
    let raw = std::fs::read_to_string(&path).expect("read cases.json");
    assert!(!raw.contains("--fixture"), "cases.json must not contain --fixture");
    assert!(!raw.contains("showcase"), "cases.json must not contain showcase");
    for banned in ["proto-vars", "variables-library", "board-empty"] {
        assert!(!raw.contains(banned), "cases.json must not reference old fixture {banned}");
    }

    let cases_file = load_cases_file();
    assert_eq!(cases_file.cases.len(), 76, "expected 76 cases in cases.json");
    for case in &cases_file.cases {
        for arg in &case.args {
            assert_ne!(arg, "--fixture", "case {} args must not contain --fixture", case.id);
            assert_ne!(arg, "showcase", "case {} args must not contain showcase", case.id);
        }
        let tier_a = case.tier_a.as_ref().unwrap_or_else(|| panic!("case {} missing tier_a target", case.id));
        let tier_b = case.tier_b.as_ref().unwrap_or_else(|| panic!("case {} missing tier_b target", case.id));
        if let Some(sel) = &tier_a.select {
            assert!(sel.starts_with("id:"), "case {} tier_a.select must use id:<n>, got {sel}", case.id);
        }
        if let Some(sel) = &tier_b.select {
            assert!(sel.starts_with("id:"), "case {} tier_b.select must use id:<n>, got {sel}", case.id);
        }
    }
}

#[test]
fn sim_loads_starter_and_seed_library_at_seeds_starter_not_showcase() {
    let tmp = std::env::temp_dir().join(format!("designos-r8-seed-starter-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::showcase::seed_library_at(&tmp, 1_700_000_000_000).expect("seed_library_at");

    let lib = designos_library::Library::open(&tmp).expect("open seeded library");
    let projects = lib.list_projects();
    assert!(
        projects.iter().any(|p| p.name == "Getting started"),
        "seed_library_at must seed the 'Getting started' starter project, got {:?}",
        projects.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
    assert!(!projects.iter().any(|p| p.name == "Marlow"), "seed_library_at must never seed the legacy 'Marlow' showcase project");
    let files: Vec<String> = lib.all_files().into_iter().map(|f| f.name).collect();
    assert!(files.iter().any(|n| n == "DESIGN-OS Studio Starter"), "seed_library_at must seed 'DESIGN-OS Studio Starter', got {files:?}");
    assert!(!files.iter().any(|n| n == "Mobile banking app"), "seed_library_at must never seed 'Mobile banking app'");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn prep_helpers_populate_missing_starter_states() {
    let starter = crate::home::load_starter_document().expect("load starter");
    assert_eq!(starter.nodes.len(), 646, "starter must have 646 nodes");

    // 1. comment_thread
    let mut doc_comments = starter.clone();
    let mut app_comments = App::new(doc_comments.clone());
    prep::comment_thread(&mut doc_comments, &mut app_comments);
    assert!(!doc_comments.comments.is_empty(), "prep::comment_thread must add a comment thread");
    assert_eq!(app_comments.tool, crate::canvas::tools::Tool::Comment, "prep::comment_thread must activate Tool::Comment");
    assert!(app_comments.comments.open.is_some(), "prep::comment_thread must open the thread popover");

    // 2. prototype_link
    let mut doc_proto = starter.clone();
    prep::prototype_link(&mut doc_proto);
    let add_node = doc_proto.nodes.values().find(|n| n.name == "Add").expect("prep::prototype_link must add node 'Add'");
    assert!(!add_node.prototype.reactions.is_empty(), "prep::prototype_link 'Add' node must have prototype reactions");
    assert!(!doc_proto.flows.is_empty(), "prep::prototype_link must add a prototype flow");

    // 3. grid_style
    let mut doc_grid = starter.clone();
    prep::grid_style(&mut doc_grid, NodeId(262794));
    assert!(doc_grid.styles.values().any(|s| matches!(s, designos_doc::Style::Grid(_))), "prep::grid_style must register a Style::Grid on the starter");
    assert!(
        doc_grid.get(NodeId(262794)).and_then(|n| n.kind.frame_props()).is_some_and(|f| !f.grids.is_empty()),
        "prep::grid_style must attach a LayoutGrid to the target frame"
    );

    // 4. component_instance
    let mut doc_comp = starter.clone();
    let inst_id = prep::component_instance(&mut doc_comp);
    assert!(doc_comp.nodes.values().any(|n| matches!(n.kind, designos_doc::NodeKind::Component(_))), "prep::component_instance must create a main Component");
    assert!(
        doc_comp.get(inst_id).is_some_and(|n| matches!(n.kind, designos_doc::NodeKind::Instance { .. })),
        "prep::component_instance must return a valid Instance NodeId"
    );

    // 5. check_designs_findings
    let mut doc_check = starter.clone();
    prep::check_designs_findings(&mut doc_check);
    let report = crate::check_designs::audit(&doc_check, &crate::check_designs::Scope::Page(NodeId(1)));
    assert!(!report.findings.is_empty(), "prep::check_designs_findings must produce Check designs findings on starter page 1");

    // 6. published_library
    let mut doc_lib = starter;
    prep::published_library(&mut doc_lib, None);
    assert!(doc_lib.variable_collections.values().any(|c| c.is_library()), "prep::published_library must add a subscribed library VariableCollection");

    // 7. blank_board
    let mut doc_board = designos_doc::Document::new("Placeholder");
    prep::blank_board(&mut doc_board);
    assert_eq!(doc_board.kind, designos_doc::DocumentKind::Board, "prep::blank_board must produce DocumentKind::Board");
    assert!(doc_board.nodes.values().any(|n| matches!(n.kind, designos_doc::NodeKind::Sticky(_))), "prep::blank_board must add a Sticky node");

    // 8. blank_deck
    let mut doc_deck = designos_doc::Document::new("Placeholder");
    let mut app_deck = App::new(doc_deck.clone());
    prep::blank_deck(&mut doc_deck, &mut app_deck);
    assert_eq!(doc_deck.kind, designos_doc::DocumentKind::Deck, "prep::blank_deck must produce DocumentKind::Deck");
    assert!(doc_deck.nodes.values().any(|n| n.slide.is_some()), "prep::blank_deck must have a slide frame");
}
