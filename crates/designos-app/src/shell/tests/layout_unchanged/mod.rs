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
#[path = "../panel_presets.rs"]
mod panel_presets;

#[derive(Debug, Clone, Deserialize)]
pub struct CasesFile {
    #[serde(default)]
    pub bless_sha: Option<String>,
    pub cases: Vec<CaseEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TierTarget {
    #[serde(default)]
    pub select: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub prep: Option<String>,
    #[serde(default)]
    pub not_applicable: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
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
    use designos_doc::{
        Action, BinaryOp, Color, Comment, CommentThread, Document, Easing, Effect, Expression, Flow, FrameProps, LayoutGrid, Node, NodeId, NodeKind, Paint,
        Reaction, Rect as DRect, ShapeKind, Style, StyleMeta, StyleProperty, TextStyle, Transition, TransitionKind, Trigger, VariableKind, VariableStore,
        VariableValue, Vec2 as DV,
    };

    pub const PROTO_ADD_ID: NodeId = NodeId(636_200);
    pub const COMP_MAIN_ID: NodeId = NodeId(636_209);
    pub const COMP_INST_ID: NodeId = NodeId(636_210);

    pub fn comment_thread_doc(doc: &mut Document) {
        let page = doc.pages.first().copied().unwrap_or(NodeId(1));
        let anchor = crate::comments::model::make_anchor(doc, page, DV::new(180.0, 220.0), Some(NodeId(636_047)));
        doc.comments.push(CommentThread {
            id: "starter-c1".into(),
            anchor,
            messages: vec![Comment {
                id: "starter-c1-0".into(),
                author: "Kir".into(),
                author_id: None,
                author_color: None,
                text: "Align onboarding header spacing to the 8pt grid.".into(),
                created: "2026-10-27T00:00:00Z".into(),
                edited: None,
            }],
            resolved: false,
        });
    }

    pub fn comment_thread(doc: &mut Document, app: &mut App) {
        comment_thread_doc(doc);
        app.doc.comments = doc.comments.clone();
        app.comments.author = Some("Kir".into());
        app.tool = crate::canvas::tools::Tool::Comment;
        app.comments.open = Some("starter-c1".into());
    }

    pub fn prototype_link(doc: &mut Document) {
        let page = doc.pages.first().copied().unwrap_or(NodeId(1));
        let start_screen = NodeId(636_047);
        let dest_screen = NodeId(636_056);

        let mut store = VariableStore::default();
        let coll = store.add_collection("State");
        let light = store.collections[&coll].default_mode.clone();
        let dark = store.add_mode(&coll, "Dark").expect("add Dark mode");
        let count = store.add_variable(&coll, "count", VariableKind::Number).expect("add count var");
        let label = store.add_variable(&coll, "label", VariableKind::String).expect("add label var");
        store.set_value(&count, &light, VariableValue::Number(0.0));
        store.set_value(&count, &dark, VariableValue::Number(0.0));
        store.set_value(&label, &light, VariableValue::String("Step 1 of 2".into()));
        store.set_value(&label, &dark, VariableValue::String("Step 1 of 2".into()));
        let _ = store.command().apply(doc);

        let mut add = Node::frame(PROTO_ADD_ID, DRect::new(24.0, 720.0, 327.0, 48.0));
        add.name = "Add".into();
        add.corner_radii = [10.0; 4];
        add.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
        add.prototype.reactions.push(Reaction {
            trigger: Trigger::OnClick,
            actions: vec![
                Action::SetVariable { variable: count.clone(), value: Expression::binary(BinaryOp::Add, Expression::var(count), Expression::number(1.0)) },
                Action::Navigate { dest: dest_screen, preserve_scroll: false },
            ],
            transition: Some(Transition { kind: TransitionKind::Dissolve, easing: Easing::EaseOut, duration_ms: 250 }),
        });
        let parent = if doc.get(start_screen).is_some() { start_screen } else { page };
        let _ = doc.insert(parent, add);
        doc.flows.push(Flow { name: "Onboarding".into(), page, start: parent, description: String::new() });
    }

    pub fn grid_style(doc: &mut Document, target: NodeId) {
        let grid = LayoutGrid::columns(12, 20.0, 0.0, Color::rgba(255, 0, 0, 26));
        if let Some(f) = doc.get_mut(target).and_then(|n| n.kind.frame_props_mut()) {
            if f.grids.is_empty() {
                f.grids.push(grid.clone());
            }
        }
        let key = "starter-grid-12-col".to_string();
        doc.styles.insert(key.clone(), Style::Grid(vec![grid]));
        doc.style_meta.insert(key.clone(), StyleMeta { name: "Grid/12 column".into(), property: StyleProperty::Grid, ..StyleMeta::default() });
        if let Some(n) = doc.get_mut(target) {
            n.style_refs.insert("grid".into(), key);
        }
    }

    pub fn component_instance(doc: &mut Document) -> NodeId {
        let page = doc.pages.first().copied().unwrap_or(NodeId(1));
        if doc.get(COMP_INST_ID).is_some() {
            return COMP_INST_ID;
        }
        let mut main = Node::frame(COMP_MAIN_ID, DRect::new(-420.0, 0.0, 160.0, 44.0));
        main.name = "Button component".into();
        main.kind = NodeKind::Component(FrameProps::default());
        main.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
        main.corner_radii = [8.0; 4];
        let main_id = doc.insert(page, main).expect("insert main component");

        let mut inst = Node::frame(COMP_INST_ID, DRect::new(-420.0, 64.0, 160.0, 44.0));
        inst.name = "Button".into();
        inst.kind = NodeKind::Instance { frame: FrameProps::default(), component: Some(main_id), component_key: None };
        inst.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
        inst.corner_radii = [8.0; 4];
        doc.insert(page, inst).expect("insert component instance")
    }

    pub fn check_designs_findings(doc: &mut Document) {
        let page = doc.pages.first().copied().unwrap_or(NodeId(1));
        let shadow = Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 56),
            offset: DV::new(0.0, 6.0),
            radius: 16.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        };
        doc.styles.insert("audit-shadow".into(), Style::Effect(vec![shadow.clone()]));
        doc.style_meta.insert("audit-shadow".into(), StyleMeta { name: "Shadow/Card".into(), property: StyleProperty::Effect, ..StyleMeta::default() });
        doc.styles.insert("audit-surface".into(), Style::Fill(vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))]));
        doc.style_meta.insert("audit-surface".into(), StyleMeta { name: "Surface/Card".into(), property: StyleProperty::Fill, ..StyleMeta::default() });

        let card_id = doc.alloc_id();
        let mut card = Node::frame(card_id, DRect::new(-520.0, 200.0, 320.0, 180.0));
        card.name = "Audit drift card".into();
        card.fills = vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))];
        card.effects = vec![shadow];
        card.corner_radii = [16.0; 4];
        let card_id = doc.insert(page, card).expect("insert audit drift card");

        let txt_id = doc.alloc_id();
        let style = TextStyle { font_size: 16.0, font_weight: 400, fills: vec![Paint::solid(Color::rgb(0x8A, 0x8A, 0x8A))], ..TextStyle::default() };
        let mut note = Node::text(txt_id, DRect::new(24.0, 24.0, 240.0, 22.0), "Low contrast audit caption", style);
        note.name = "Audit caption".into();
        let _ = doc.insert(card_id, note);
    }

    pub fn published_library(doc: &mut Document, lib_root: Option<&Path>) {
        let mut store = VariableStore::default();
        let library = store.add_collection("Shared UI tokens");
        let mode = store.collections[&library].default_mode.clone();
        let id = store.add_variable(&library, "Brand/Primary", VariableKind::Color).expect("add library variable");
        store.set_value(&id, &mode, VariableValue::Color(Color::rgb(0x15, 0x70, 0xEF)));
        if let Some(c) = store.collections.get_mut(&library) {
            c.source_library_key = Some("lk-starter-shared".into());
        }
        let _ = store.command().apply(doc);
        for c in doc.variable_collections.values_mut() {
            if c.name == "Shared UI tokens" {
                c.source_library_key = Some("lk-starter-shared".into());
            }
        }
        if let Some(root) = lib_root {
            if let Ok(lib) = designos_library::Library::open(root) {
                let _ = lib.seed_starter(crate::home::load_starter_document);
                if let Some(entry) = lib.all_files().first() {
                    let _ = lib.publish_library(&entry.id, doc, "Shared starter tokens");
                }
            }
        }
    }

    pub fn blank_board(doc: &mut Document) {
        let mut board = crate::board::create::new_document("Untitled board");
        let page = board.pages.first().copied().unwrap_or(NodeId(1));
        let sticky_id = board.alloc_id();
        let mut sticky = Node::sticky(sticky_id, DRect::new(160.0, 140.0, 200.0, 200.0), "Sprint goals");
        sticky.name = "Sticky note".into();
        if let NodeKind::Sticky(s) = &mut sticky.kind {
            s.author = Some("Designer".into());
        }
        let _ = board.insert(page, sticky);

        let shape_id = board.alloc_id();
        let mut shape = Node::shape_with_text(shape_id, DRect::new(420.0, 160.0, 200.0, 120.0), ShapeKind::RoundedRect, "Architecture review");
        shape.name = "Shape".into();
        let _ = board.insert(page, shape);
        *doc = board;
    }

    pub fn blank_deck_doc(doc: &mut Document) {
        let mut deck = crate::decks::new_document("Untitled deck");
        let slide_id = deck.pages.first().and_then(|&p| deck.get(p)).and_then(|p| p.children.first().copied()).unwrap_or(NodeId(2));
        if let Some(s) = deck.get_mut(slide_id).and_then(|n| n.slide.as_mut()) {
            s.notes = "Opening slide notes for layout gate verification.".into();
        }
        let title_id = deck.alloc_id();
        let title_style = TextStyle { font_size: 44.0, font_weight: 700, fills: vec![Paint::solid(Color::WHITE)], ..TextStyle::default() };
        let mut title = Node::text(title_id, DRect::new(96.0, 120.0, 840.0, 64.0), "Design System Overview", title_style);
        title.name = "Slide title".into();
        let _ = deck.insert(slide_id, title);
        *doc = deck;
    }

    pub fn blank_deck(doc: &mut Document, app: &mut App) {
        blank_deck_doc(doc);
        app.replace_document(doc.clone(), None);
        if let Some(first_slide) = app.deck_slides().first().copied() {
            app.select_slide(first_slide);
        }
    }
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

    let spec_cases: Vec<&CaseEntry> =
        cases_file.cases.iter().filter(|c| c.spec == spec_key && c.tier_a.as_ref().is_none_or(|t| t.unreachable.is_none())).collect();
    assert!(!spec_cases.is_empty(), "no Tier A cases found in cases.json for spec {spec_key}");

    let stub_dir = crate::cli_resolve::stub_dir().to_path_buf();
    let pid = std::process::id();
    let mut run_matched = std::collections::BTreeSet::new();
    let mut run_cases = std::collections::BTreeSet::new();

    for (idx, case) in spec_cases.into_iter().enumerate() {
        let _ = case.spawns;
        let case_tmp = std::env::temp_dir().join(format!("designos-tier-a-{}-{}-{pid}-{idx}", spec_key.to_ascii_lowercase(), case.id));
        let _ = std::fs::remove_dir_all(&case_tmp);
        std::fs::create_dir_all(&case_tmp).expect("create case temp dir");

        let tier_a = case.tier_a.as_ref().unwrap_or_else(|| panic!("case {} missing tier_a target", case.id));
        let _ = (&tier_a.name, &tier_a.page);
        let prep_kind = tier_a.prep.as_deref();

        let mut expanded_args = Vec::with_capacity(case.args.len() + 4);
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
        if prep_kind == Some("published_library") && seeded_lib.is_none() {
            let d = case_tmp.join("seed-lib");
            crate::showcase::seed_library_at(&d, 1_700_000_000_000).expect("seed deterministic library");
            expanded_args.push("--library".into());
            expanded_args.push(d.to_string_lossy().into_owned());
            seeded_lib = Some(d);
        }
        if let Some(sel) = &tier_a.select {
            expanded_args.push("--select".into());
            expanded_args.push(sel.clone());
        }

        let mut opts = crate::cli::parse(expanded_args).unwrap_or_else(|e| panic!("case {} failed cli::parse: {e}", case.id));
        opts.size = Some((1440, 900));
        opts.scale = Some(1.0);
        opts.no_toast = true;
        opts.agent_cli_dir = Some(stub_dir.clone());
        opts.fixed_now = Some(1_700_000_060_000);

        {
            let _scoped = crate::cli_resolve::ScopedCliDirs::enter(stub_dir.clone());
            let lib_for_prep = seeded_lib.clone();
            let mut sim = if prep_kind.is_none() {
                Sim::with_starter(opts)
            } else {
                Sim::with_starter_prep(
                    opts,
                    |doc| match prep_kind {
                        Some("prototype_link") => prep::prototype_link(doc),
                        Some("grid_style") => prep::grid_style(doc, NodeId(262794)),
                        Some("component_instance") => {
                            let _ = prep::component_instance(doc);
                        }
                        Some("check_designs_findings") => prep::check_designs_findings(doc),
                        Some("published_library") => prep::published_library(doc, lib_for_prep.as_deref()),
                        Some("blank_board") => prep::blank_board(doc),
                        Some("blank_deck") => prep::blank_deck_doc(doc),
                        Some("comment_thread") => prep::comment_thread_doc(doc),
                        None => {}
                        Some(other) => panic!("case {}: unknown tier_a.prep '{other}'", case.id),
                    },
                    |app| match prep_kind {
                        Some("comment_thread") => {
                            app.comments.author = Some("Kir".into());
                        }
                        Some("published_library") => {
                            if app.vars.open {
                                app.vars.collection = app.doc.variable_collections.values().find(|c| c.is_library()).map(|c| c.id.clone());
                            }
                        }
                        Some("blank_deck") => {
                            if let Some(first_slide) = app.deck_slides().first().copied() {
                                app.select_slide(first_slide);
                            }
                        }
                        _ => {}
                    },
                )
            };
            assert!(sim.shell.select_miss.is_none(), "case {} had a --select miss: {:?}", case.id, sim.shell.select_miss);
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
            } else {
                match compare_non_bless_case(&cases_file, &baseline_path, &snap, &allow_entries, &case.virtualized) {
                    Ok(report) => {
                        if let Some(line) = format_new_names_report(&case.id, &report.new_names) {
                            println!("{line}");
                        }
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

fn require_blessed_cases_file(cases_file: &CasesFile) -> Result<&str, String> {
    match cases_file.bless_sha.as_deref().map(str::trim) {
        Some(sha) if !sha.is_empty() => Ok(sha),
        _ => Err("cases.json has no bless_sha: run the bless procedure (governing spec rule 11)".to_string()),
    }
}

fn compare_non_bless_case(
    cases_file: &CasesFile,
    baseline_path: &Path,
    snap: &crate::layout_snapshot::LayoutSnapshot,
    allow_entries: &crate::layout_snapshot::AllowList,
    virtualized: &[String],
) -> Result<crate::layout_snapshot::CompareReport, String> {
    require_blessed_cases_file(cases_file)?;
    crate::layout_snapshot::load_and_compare_baseline(baseline_path, snap, allow_entries, virtualized)
}

fn format_new_names_report(case_id: &str, new_names: &[String]) -> Option<String> {
    if new_names.is_empty() {
        None
    } else {
        Some(format!("layout gate: case {case_id}: new names (reported, not gated): {}", new_names.join(", ")))
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
        let _ = (&tier_a.not_applicable, &tier_a.note, &tier_b.not_applicable, &tier_b.note);
        assert!(tier_a.unreachable.is_none(), "case {} tier_a must not be unreachable", case.id);
        assert!(tier_b.unreachable.is_none(), "case {} tier_b must not be unreachable", case.id);
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

#[test]
fn unblessed_cases_file_fails_the_gate() {
    let raw = std::fs::read_to_string(baseline_dir().join("d-editor-default.layout.json")).expect("read d-editor-default baseline");
    let snap: crate::layout_snapshot::LayoutSnapshot = serde_json::from_str(&raw).expect("parse d-editor-default baseline");
    let allow = crate::layout_snapshot::AllowList::default();
    let existing_baseline = baseline_dir().join("d-editor-default.layout.json");

    let missing_sha = CasesFile { bless_sha: None, cases: Vec::new() };
    let err_none = compare_non_bless_case(&missing_sha, &existing_baseline, &snap, &allow, &[]).expect_err("missing bless_sha must fail");
    assert_eq!(err_none, "cases.json has no bless_sha: run the bless procedure (governing spec rule 11)");

    let empty_sha = CasesFile { bless_sha: Some(String::new()), cases: Vec::new() };
    let err_empty = compare_non_bless_case(&empty_sha, &existing_baseline, &snap, &allow, &[]).expect_err("empty bless_sha must fail");
    assert_eq!(err_empty, "cases.json has no bless_sha: run the bless procedure (governing spec rule 11)");

    let blessed = CasesFile { bless_sha: Some("8dfaee931c75bd36293bc03266716b6abf99d868".into()), cases: Vec::new() };
    let missing_file = baseline_dir().join("nonexistent-case.layout.json");
    let err_missing_file = compare_non_bless_case(&blessed, &missing_file, &snap, &allow, &[]).expect_err("missing baseline file must fail");
    assert!(err_missing_file.contains("missing layout baseline file:"), "missing baseline error must keep its bless hint, got: {err_missing_file}");
}

#[test]
fn new_named_widget_is_reported_not_failed() {
    let raw = std::fs::read_to_string(baseline_dir().join("d-editor-default.layout.json")).expect("read d-editor-default baseline");
    let base: crate::layout_snapshot::LayoutSnapshot = serde_json::from_str(&raw).expect("parse d-editor-default baseline");
    let mut cur = base.clone();
    cur.named.insert("new-tool-button".to_string(), crate::layout_snapshot::NamedWidget { region: "tab_bar".into(), rect: [100.0, 8.0, 24.0, 24.0], count: 1 });

    let report =
        crate::layout_snapshot::compare(&base, &cur, &crate::layout_snapshot::AllowList::default(), &[]).expect("extra named widget must report Ok, not fail");
    assert_eq!(report.new_names, vec!["new-tool-button".to_string()]);
    assert_eq!(
        format_new_names_report("d-editor-default", &report.new_names).as_deref(),
        Some("layout gate: case d-editor-default: new names (reported, not gated): new-tool-button")
    );
}
