mod agent_a11y;
mod agent_approval_card;
pub mod agent_approval_channel;
mod agent_busy_frames;
pub mod agent_cli_catalog;
#[cfg(test)]
mod agent_cli_export_fig_tests;
#[cfg(test)]
mod agent_cli_select_tests;
#[cfg(unix)]
pub mod agent_codex_cards;
#[cfg(not(unix))]
#[path = "agent_codex_cards_stub.rs"]
pub mod agent_codex_cards;
pub mod agent_composer;
mod agent_failure_redact;
pub mod agent_hook_outputs;
mod agent_mention;
mod agent_mention_popup;
pub mod agent_notify;
pub mod agent_open_flow;
pub mod agent_permissions;
pub mod agent_picker;
mod agent_presence;
mod agent_project_config;
mod agent_sample_prompts;
mod agent_selection_chip;
mod agent_session_failure;
pub mod agent_session_files;
pub mod agent_sessions;
mod agent_start_failure;
mod agent_stop;
pub mod agent_turn_card;
pub mod agent_turn_ledger;
pub mod agent_turn_marks;
pub mod agent_turn_status;
pub mod agent_usage;
mod app;
#[cfg(test)]
mod apply_node_spec_variable_tests;
mod approval_text;
mod board;
mod canvas;
mod check_designs;
mod cli;
mod cli_export_write;
mod clipboard;
mod cmdk;
#[cfg(unix)]
mod codex_remote;
mod comments;
mod comp_ops;
mod decks;
#[cfg(any(test, feature = "fixtures"))]
mod demo;
mod dev;
mod edit;
mod fig_export;
mod find;
mod font_refit;
mod fonts;
mod gemini_dialog;
mod gemini_screen;
mod gemini_status_hooks;
mod history;
mod home;
mod icons;
mod image_jobs;
mod io;
mod layout;
mod libraries;
#[cfg(test)]
mod lint;
mod live_canvas_scope;
#[cfg(test)]
mod live_doc_epoch_tests;
mod live_log_wake;
mod live_patch_apply;
mod live_patch_log;
mod live_spec_apply;
#[cfg(test)]
mod live_sync_interaction_tests;
#[cfg(test)]
mod live_sync_tests;
#[cfg(target_os = "macos")]
mod macos_menu;
mod node_field_diff;
mod ops;
mod panels;
mod paste_placement;
mod plugins;
mod present;
mod proto;
#[cfg(any(test, feature = "fixtures"))]
mod protovars;
mod richtext;
#[cfg(test)]
mod scratch_cli_home;
mod select_target;
mod shell;
#[cfg(any(test, feature = "fixtures"))]
mod showcase;
mod skills;
mod specs;
mod styles;
mod system_paste;
mod tabs;
mod terminal;
mod terminal_undo_chord;
#[cfg(test)]
mod test_agent_path;
#[cfg(test)]
mod test_alloc_count;
#[cfg(test)]
mod test_timing;
mod textedit;
mod textmeasure;
mod theme;
mod util;
mod variables;
#[cfg(any(test, feature = "fixtures"))]
mod vecfixture;
mod vecops;
mod video;
mod widgets;

use std::sync::atomic::Ordering;

/// The app icon (256 px PNG embedded at build time) for the taskbar, Dock and alt-tab. Every window
/// designos opens (including Present and "open in new window" processes) starts through `main`, so
/// they all get it.
fn window_icon() -> Option<egui::IconData> {
    const PNG: &[u8] = include_bytes!("../../../assets/icon/png/icon_256.png");
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width, height })
}

fn main() {
    designos_doc::text_measure::install(&designos_render::RenderTextMeasure);
    let opts = match cli::parse(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("designos: {e}\n{}", cli::USAGE);
            std::process::exit(2);
        }
    };
    if opts.help {
        println!("{}", cli::USAGE);
        return;
    }
    if opts.version {
        println!("{}", cli::version_line());
        return;
    }
    // Before any window or command: a name that is not a file (`designos doctor`) must not open an empty editor.
    if let Some(message) = cli::missing_file_message(&opts) {
        eprintln!("{message}");
        std::process::exit(2);
    }
    if let Some(dir) = &opts.seed_library {
        #[cfg(any(test, feature = "fixtures"))]
        match showcase::seed_library(dir) {
            Ok(()) => println!("designos: wrote the showcase sample files to {}", dir.display()),
            Err(e) => {
                eprintln!("designos: cannot seed {}: {e}", dir.display());
                std::process::exit(1);
            }
        }
        #[cfg(not(any(test, feature = "fixtures")))]
        {
            let _ = dir;
            eprintln!("designos: --seed-library requires a build with the 'fixtures' feature");
            std::process::exit(2);
        }
        #[cfg(any(test, feature = "fixtures"))]
        return;
    }
    if opts.is_agent_cli() {
        if let Err(e) = run_agent_cli(&opts) {
            eprintln!("designos: {e}");
            std::process::exit(2);
        }
        return;
    }
    agent_session_files::spawn_startup_sweep();
    let (w, h) = opts.size.unwrap_or((theme::DEFAULT_WINDOW.0 as u32, theme::DEFAULT_WINDOW.1 as u32));
    let screenshot_mode = opts.screenshot.is_some();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("design:os studio")
        .with_inner_size([w as f32, h as f32])
        .with_min_inner_size([theme::MIN_WINDOW.0, theme::MIN_WINDOW.1]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    if screenshot_mode {
        viewport = viewport.with_min_inner_size([200.0, 200.0]).with_resizable(true).with_active(false);
    } else {
        #[cfg(target_os = "macos")]
        {
            viewport = viewport.with_fullsize_content_view(true).with_titlebar_shown(false).with_title_shown(false);
        }
    }
    let native = eframe::NativeOptions { viewport, ..Default::default() };
    let result = eframe::run_native(
        "design:os studio",
        native,
        Box::new(move |cc| {
            // winit's default app menu quits through `terminate:`, which never raises a window close. Point its Quit
            // item (and the Cmd+Q key it owns) at the close instead, so the Save changes questions run first.
            #[cfg(target_os = "macos")]
            {
                if !macos_menu::install(&cc.egui_ctx) {
                    eprintln!("designos: the app menu has no Quit item to route through the window close");
                }
            }
            Ok(Box::new(shell::Shell::new(cc, opts)))
        }),
    );
    if let Err(e) = result {
        eprintln!("designos: {e}");
        std::process::exit(1);
    }
    if screenshot_mode {
        if let Some(err) = shell::SHOT_OPEN_ERROR.get() {
            eprintln!("{err}");
            std::process::exit(1);
        }
        let miss = shell::SELECT_MISS.get();
        let shot_ok = shell::SHOT_OK.load(Ordering::SeqCst);
        match miss {
            Some(message) => eprintln!("designos: {message}"),
            None if !shot_ok => eprintln!("designos: screenshot was not captured"),
            None => {}
        }
        let code = exit_code(miss.is_some(), shot_ok);
        if code != 0 {
            std::process::exit(code);
        }
    }
}

/// The exit status of a finished `--screenshot` run: 2 when `--select` matched nothing (no PNG is written), which
/// outranks 1 for a PNG that was not captured, and 0 otherwise.
fn exit_code(select_miss: bool, shot_ok: bool) -> i32 {
    match (select_miss, shot_ok) {
        (true, _) => 2,
        (false, false) => 1,
        (false, true) => 0,
    }
}

enum SaveTarget {
    Library(designos_library::Library, String),
    Path(std::path::PathBuf),
}

impl SaveTarget {
    fn save(&self, doc: &designos_doc::Document) -> Result<(), String> {
        match self {
            Self::Library(lib, id) => lib.save_file(id, doc).map_err(|e| format!("Cannot save library file {id}: {e}")),
            Self::Path(path) => io::save_path(doc, path).map(|_| ()),
        }
    }
}

fn run_agent_cli(opts: &cli::Options) -> Result<(), String> {
    // Validate `--export-variables` option combinations (`#1140` §3 rule 10) before loading or creating anything.
    if opts.export_variables.is_none() {
        if opts.mode.is_some() {
            return Err("--mode requires --export-variables".into());
        }
        if !opts.collections.is_empty() {
            return Err("--collection requires --export-variables and --mode".into());
        }
        if opts.resolve_aliases {
            return Err("--resolve-aliases requires --export-variables and --mode".into());
        }
        if opts.color_format.is_some() {
            return Err("--color-format requires --export-variables --format dtcg".into());
        }
    } else if let Some(out_target) = &opts.export_variables {
        let fmt = opts.format.as_deref().and_then(designos_fig::VariablesFormat::parse_label).unwrap_or(designos_fig::VariablesFormat::FigmaRest);
        let ext = out_target.extension().and_then(|e| e.to_str()).unwrap_or("");
        if opts.mode.is_some() {
            if fmt != designos_fig::VariablesFormat::Dtcg {
                return Err("--mode requires --format dtcg".into());
            }
            if !ext.eq_ignore_ascii_case("json") {
                return Err("--mode requires a .json output file path".into());
            }
        } else {
            if !opts.collections.is_empty() {
                return Err("--collection requires --mode <name>".into());
            }
            if opts.resolve_aliases {
                return Err("--resolve-aliases requires --mode <name>".into());
            }
            if fmt == designos_fig::VariablesFormat::Dtcg && ext.eq_ignore_ascii_case("json") {
                return Err("DTCG export to a .json path requires --mode <name>; pass a directory, a .zip file, or --mode <name>".into());
            }
        }
        if let Some(cf) = &opts.color_format {
            if fmt != designos_fig::VariablesFormat::Dtcg {
                return Err("--color-format requires --format dtcg".into());
            }
            if designos_fig::ColorFormat::parse_label(cf).is_none() {
                return Err(format!("unknown --color-format {cf:?} (expected hex or object)"));
            }
        }
    }

    // An unusable --export-fig target stops the command before it loads, applies or saves anything.
    if let Some(out_path) = &opts.export_fig {
        cli_export_write::check_target(out_path, "fig", opts.overwrite)?;
    }
    let (mut doc, save_target) = if let Some(id) = &opts.open_id {
        let root = opts.library.clone().unwrap_or_else(designos_library::Library::default_root);
        let lib = designos_library::Library::open(&root).map_err(|e| format!("Cannot open library at {}: {e}", root.display()))?;
        let mut d = lib.load_file(id).map_err(|e| format!("Cannot load library file {id}: {e}"))?;
        if let Ok(entry) = lib.file(id) {
            d.name = entry.name;
        }
        io::ensure_page(&mut d);
        (d, SaveTarget::Library(lib, id.clone()))
    } else if let Some(path) = &opts.file {
        let d = if !path.exists() && opts.creates_missing_file() {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled");
            let mut d = designos_doc::Document::new(stem);
            io::ensure_page(&mut d);
            d
        } else {
            io::load_path(path)?
        };
        let target_path = opts.out.clone().unwrap_or_else(|| path.clone());
        (d, SaveTarget::Path(target_path))
    } else if let Some(out_path) = &opts.out {
        let stem = out_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled");
        let mut d = designos_doc::Document::new(stem);
        io::ensure_page(&mut d);
        (d, SaveTarget::Path(out_path.clone()))
    } else {
        return Err("Agent CLI requires a file path, --out <path>, or --open-id <id>".into());
    };

    // Resolve --select before anything is imported, applied or saved: a miss must leave the file as it was.
    let selected_target = select_target::resolve_flag(&mut doc, opts.select.as_deref())?;
    let selected_node = selected_target.and_then(|t| t.node);
    if let (Some(target), Some(raw)) = (selected_target, opts.select.as_deref()) {
        if let Some(note) = target.note(raw.trim()) {
            eprintln!("designos: {note}");
        }
    }

    if let Some(cap_path) = &opts.import_capture {
        let raw = std::fs::read_to_string(cap_path).map_err(|e| format!("Cannot read {}: {e}", cap_path.display()))?;
        let page = doc.pages.first().copied().ok_or("Document has no pages")?;
        let (cmd, report) = designos_webcapture::import_capture_json(&doc, page, &raw, &designos_webcapture::ImportOptions::default())
            .map_err(|e| format!("Web capture import failed in {}: {e}", cap_path.display()))?;
        let mut hist = designos_doc::History::default();
        hist.exec(&mut doc, cmd).map_err(|e| format!("Web capture import failed: {e}"))?;
        doc.relayout_subtree(page);
        doc.flush_pending_relayout();
        if doc.name == "Untitled" || opts.file.is_none() {
            doc.name = report.root_name.clone();
        }
        save_target.save(&doc)?;
        let out = serde_json::json!({
            "root_node_id": report.root_node_id.0,
            "root_name": report.root_name,
            "nodes_created": report.nodes_created,
            "summary_toast": report.summary_toast(),
            "report": report,
            "saved": true,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if let Some(spec_path) = &opts.apply_spec {
        let raw = std::fs::read_to_string(spec_path).map_err(|e| format!("Cannot read {}: {e}", spec_path.display()))?;
        let spec = cli::parse_spec_json(&raw).map_err(|e| format!("Invalid NodeSpec JSON in {}: {e}", spec_path.display()))?;
        let parent = selected_node.or_else(|| doc.pages.first().copied()).ok_or("Document has no pages")?;
        let root_id = doc.apply_spec(parent, &spec)?;
        // no window drains the deferred layout here, and the queue is not part of the file
        doc.flush_pending_relayout();
        save_target.save(&doc)?;
        let out = serde_json::json!({ "root_node_id": root_id.0, "saved": true });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if let Some(text) = &opts.add_comment {
        let page_id = selected_node.and_then(|nid| doc.page_of(nid)).or_else(|| doc.pages.first().copied()).ok_or("Document has no pages")?;
        let anchor_node = selected_node.filter(|nid| !doc.pages.contains(nid));
        let offset = match anchor_node.and_then(|nid| doc.get(nid)) {
            Some(n) => designos_doc::Vec2::new((n.size.x * 0.5).min(24.0), (n.size.y * 0.5).min(24.0)),
            None => designos_doc::Vec2::new(40.0, 40.0),
        };
        let mut n = doc.comments.len() + 1;
        while doc.comments.iter().any(|c| c.id == format!("t{n}")) {
            n += 1;
        }
        let thread_id = format!("t{n}");
        let msg = comments::model::comment(format!("{thread_id}-1"), "Agent", text);
        doc.comments.push(designos_doc::CommentThread {
            id: thread_id.clone(),
            anchor: designos_doc::CommentAnchor { page: page_id, node: anchor_node, offset },
            messages: vec![msg],
            resolved: false,
        });
        save_target.save(&doc)?;
        let resolved = doc.resolve_comment_targets().into_iter().find(|c| c.id == thread_id).ok_or("Failed to resolve created comment")?;
        println!("{}", serde_json::to_string_pretty(&resolved).unwrap());
    }

    if let Some(thread_id) = &opts.reply_comment {
        let t = doc.comments.iter_mut().find(|t| t.id == *thread_id).ok_or_else(|| format!("Comment thread '{thread_id}' not found"))?;
        let msg_text = opts.message.clone().unwrap_or_else(|| "Updated per feedback.".into());
        t.messages.push(comments::model::comment(designos_library::new_id(), "Agent", &msg_text));
        if opts.resolve {
            t.resolved = true;
        }
        let resolved_now = t.resolved;
        save_target.save(&doc)?;
        let out = serde_json::json!({
            "thread_id": thread_id,
            "replied": true,
            "resolved": resolved_now,
            "saved": true,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if let Some(thread_id) = &opts.resolve_comment {
        let t = doc.comments.iter_mut().find(|t| t.id == *thread_id).ok_or_else(|| format!("Comment thread '{thread_id}' not found"))?;
        t.resolved = true;
        save_target.save(&doc)?;
        let out = serde_json::json!({
            "thread_id": thread_id,
            "resolved": true,
            "saved": true,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if let Some(out_path) = &opts.export_spec {
        let target_id = selected_node
            .or_else(|| doc.pages.first().and_then(|p| doc.get(*p)).and_then(|p| p.children.first()).copied())
            .or_else(|| doc.pages.first().copied())
            .ok_or("No node found to export")?;
        let spec = doc.export_spec(target_id)?;
        let json = serde_json::to_string_pretty(&spec).map_err(|e| format!("Cannot serialize spec: {e}"))?;
        std::fs::write(out_path, json).map_err(|e| format!("Cannot write {}: {e}", out_path.display()))?;
        let out = serde_json::json!({ "exported_node_id": target_id.0, "path": out_path });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if let Some(out_path) = &opts.export_fig {
        let (bytes, omitted) = designos_fig::export_fig_with_report(&doc).map_err(|e| format!("Cannot export .fig: {e}"))?;
        cli_export_write::write_export(out_path, "fig", &bytes, opts.overwrite)?;
        // A `.fig` has no place for comments: say how many threads this file left out (the native format keeps them).
        let out = serde_json::json!({
            "exported_fig": true,
            "path": out_path,
            "bytes": bytes.len(),
            "omitted_extensions": omitted,
            "omitted_comment_threads": doc.comments.len(),
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if let Some(var_path) = &opts.import_variables {
        let fmt = opts.format.as_deref().and_then(designos_fig::VariablesFormat::parse_label).unwrap_or(designos_fig::VariablesFormat::Auto);
        let bytes = std::fs::read(var_path).map_err(|e| format!("Cannot read {}: {e}", var_path.display()))?;
        let mut import_opts = designos_fig::ImportVariablesOptions::default();
        // If importing a manifest or sibling .tokens.json files from a directory, collect siblings
        if let Some(parent_dir) = var_path.parent() {
            if var_path.file_name().and_then(|s| s.to_str()) == Some("tokens.manifest.json") {
                if let Ok(entries) = std::fs::read_dir(parent_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p != *var_path && p.extension().and_then(|e| e.to_str()) == Some("json") {
                            if let (Some(fname), Ok(fbytes)) = (p.file_name().and_then(|s| s.to_str()), std::fs::read(&p)) {
                                import_opts.additional_files.push((fname.to_string(), fbytes));
                            }
                        }
                    }
                }
            }
        }
        let before_store = designos_doc::VariableStore::from_doc(&doc);
        let mut working = designos_doc::Document::new("variables_work");
        working.variable_collections = doc.variable_collections.clone();
        working.variables = doc.variables.clone();
        let report = designos_fig::import_variables_bytes(&mut working, &bytes, var_path.file_name().and_then(|s| s.to_str()), fmt, import_opts)
            .map_err(|e| format!("Cannot import variables from {}: {e}", var_path.display()))?;
        let mut after_store = designos_doc::VariableStore { collections: working.variable_collections, variables: working.variables };
        after_store.enforce_library_read_only(&before_store);
        doc.variable_collections = after_store.collections;
        doc.variables = after_store.variables;
        save_target.save(&doc)?;
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }

    if let Some(out_target) = &opts.export_variables {
        let fmt = opts.format.as_deref().and_then(designos_fig::VariablesFormat::parse_label).unwrap_or(designos_fig::VariablesFormat::FigmaRest);
        let color_format = opts.color_format.as_deref().and_then(designos_fig::ColorFormat::parse_label).unwrap_or(designos_fig::ColorFormat::Object);

        if let Some(mode_name) = &opts.mode {
            let mode_opts = designos_fig::DtcgModeOptions {
                mode: mode_name.clone(),
                collections: opts.collections.clone(),
                resolve_aliases: opts.resolve_aliases,
                color_format,
            };
            let (bytes, report) = designos_fig::export_dtcg_mode(&doc, &mode_opts).map_err(|e| format!("Cannot export variables: {e}"))?;
            if let Some(parent) = out_target.parent().filter(|p| !p.as_os_str().is_empty()) {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(out_target, &bytes).map_err(|e| format!("Cannot write {}: {e}", out_target.display()))?;
            let out = serde_json::json!({
                "format": "dtcg",
                "path": out_target,
                "mode": report.mode,
                "collections": report.collections,
                "skipped_collections": report.skipped_collections,
                "inlined_for_aliases": report.inlined_for_aliases,
                "tokens": report.tokens,
                "aliases_resolved": report.aliases_resolved,
                "aliases_resolved_via_default_mode": report.aliases_resolved_via_default_mode,
                "values_from_default_mode": report.values_from_default_mode,
                "color_format": report.color_format.label(),
                "omitted_extensions": report.omitted_extensions,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        } else {
            let bundle = designos_fig::export_variables_with_color_format(&doc, fmt, color_format).map_err(|e| format!("Cannot export variables: {e}"))?;
            let ext = out_target.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("zip") {
                if let Some(parent) = out_target.parent().filter(|p| !p.as_os_str().is_empty()) {
                    let _ = std::fs::create_dir_all(parent);
                }
                let zip_bytes = bundle.to_zip_bytes().map_err(|e| format!("Cannot build zip: {e}"))?;
                std::fs::write(out_target, &zip_bytes).map_err(|e| format!("Cannot write {}: {e}", out_target.display()))?;
            } else if ext.eq_ignore_ascii_case("json") && bundle.files.len() == 1 {
                if let Some(parent) = out_target.parent().filter(|p| !p.as_os_str().is_empty()) {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(out_target, &bundle.files[0].1).map_err(|e| format!("Cannot write {}: {e}", out_target.display()))?;
            } else {
                let mut seen_names = std::collections::BTreeSet::new();
                let mut dup_names = Vec::new();
                for (name, _) in &bundle.files {
                    if !seen_names.insert(name.as_str()) && !dup_names.contains(&name.as_str()) {
                        dup_names.push(name.as_str());
                    }
                }
                if !dup_names.is_empty() {
                    return Err(format!("Refusing to overwrite duplicate export file(s): {}", dup_names.join(", ")));
                }
                std::fs::create_dir_all(out_target).map_err(|e| format!("Cannot create dir {}: {e}", out_target.display()))?;
                for (name, bytes) in &bundle.files {
                    let safe_name = std::path::Path::new(name)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .filter(|s| !s.is_empty() && *s != "." && *s != ".." && !name.contains("..") && !name.contains('/') && !name.contains('\\'))
                        .ok_or_else(|| format!("Refusing to write unsafe export filename '{name}'"))?;
                    let dest = out_target.join(safe_name);
                    std::fs::write(&dest, bytes).map_err(|e| format!("Cannot write {}: {e}", dest.display()))?;
                }
            }
            let files_written: Vec<&str> = bundle.files.iter().map(|(n, _)| n.as_str()).collect();
            let out = serde_json::json!({
                "format": bundle.format.label(),
                "path": out_target,
                "files": files_written,
                "omitted_extensions": bundle.omitted_extensions,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
    }

    if let Some(lang_id) = &opts.codegen {
        let lang = designos_export::CodeLanguage::from_id(lang_id).ok_or_else(|| format!("Unknown codegen language '{lang_id}'"))?;
        let target_id = selected_node
            .or_else(|| doc.pages.first().and_then(|p| doc.get(*p)).and_then(|p| p.children.first()).copied())
            .or_else(|| doc.pages.first().copied())
            .ok_or("No node found to generate code for")?;
        let (resolved, _) = designos_export::apply_bound_variables_subtree(&doc, target_id);
        println!("{}", lang.generate(&resolved, target_id, false));
    }

    if opts.comments_json {
        let targets = doc.resolve_comment_targets();
        let unresolved_count = targets.iter().filter(|c| !c.resolved).count();
        let out = serde_json::json!({
            "comments": targets,
            "unresolved_count": unresolved_count,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if opts.inspect_json {
        println!("{}", serde_json::to_string_pretty(&inspect_json(&doc, selected_node)).unwrap());
    }

    Ok(())
}

/// What `--inspect-json` prints: the pages and screens of the document (with their Figma ids), plus the node that
/// `--select` addressed, in page coordinates, so an agent can read where it is without a second command.
fn inspect_json(doc: &designos_doc::Document, selected: Option<designos_doc::NodeId>) -> serde_json::Value {
    let mut out = serde_json::json!({
        "name": doc.name,
        "kind": doc.kind,
        "node_count": doc.nodes.len(),
        "unresolved_comments": doc.comments.iter().filter(|c| !c.resolved).count(),
        "pages": doc.pages.iter().filter_map(|pid| doc.get(*pid)).map(|p| serde_json::json!({
            "id": p.id.0,
            "name": p.name,
            "child_count": p.children.len(),
            "screens": p.children.iter().filter_map(|cid| doc.get(*cid)).map(|c| serde_json::json!({
                "id": c.id.0,
                "name": c.name,
                "kind": c.kind.default_name(),
                "width": c.size.x,
                "height": c.size.y,
                "dev_status": c.dev.status.map(|s| s.label()),
                "source_id": c.source_id,
            })).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    });
    if let Some(n) = selected.and_then(|id| doc.get(id)) {
        let b = designos_doc::node_world_bounds(doc, n.id);
        out["selection"] = serde_json::json!({
            "id": n.id.0,
            "name": n.name,
            "kind": n.kind.default_name(),
            "page_id": doc.page_of(n.id).map(|p| p.0),
            "source_id": n.source_id,
            "x": b.x,
            "y": b.y,
            "width": b.w,
            "height": b.h,
        });
    }
    out
}

#[cfg(test)]
mod icon_tests {
    #[test]
    fn select_resolves_native_fig_and_name_selectors_without_fallback() {
        let mut doc = designos_doc::Document::new("selector test");
        let page = doc.add_page("Page");
        let mut node = designos_doc::Node::new(designos_doc::NodeId(2), designos_doc::NodeKind::Rectangle);
        node.name = "Card".into();
        node.source_id = Some("12:34".into());
        doc.insert(page, node).unwrap();

        let resolve = |doc: &mut designos_doc::Document, select: Option<&str>| super::select_target::resolve_flag(doc, select).map(|t| t.and_then(|t| t.node));
        assert_eq!(resolve(&mut doc, Some("id:2")).unwrap(), Some(designos_doc::NodeId(2)));
        assert_eq!(resolve(&mut doc, Some("fig:12:34")).unwrap(), Some(designos_doc::NodeId(2)));
        assert_eq!(resolve(&mut doc, Some("name:Card")).unwrap(), Some(designos_doc::NodeId(2)));
        assert_eq!(resolve(&mut doc, Some("Card")).unwrap(), Some(designos_doc::NodeId(2)));
        assert!(resolve(&mut doc, Some("id:999")).is_err());
        assert!(resolve(&mut doc, Some("Missing")).is_err());
        assert_eq!(resolve(&mut doc, Some("none")).unwrap(), None);
        assert_eq!(resolve(&mut doc, None).unwrap(), None);
    }

    #[test]
    fn embedded_window_icon_decodes() {
        let icon = super::window_icon().expect("icon decodes");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn agent_cli_apply_comment_and_export_roundtrip() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("designos_agent_cli_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let doc_path = dir.join("test.designos");
        let spec_in = dir.join("spec_in.json");
        let spec_out = dir.join("spec_out.json");

        let mut doc = designos_doc::Document::new("CLI Test");
        doc.add_page("Page 1");
        super::io::save_path(&doc, &doc_path).unwrap();

        std::fs::write(
            &spec_in,
            r#"{"preset":"easeui-card","name":"PricingCard","children":[{"preset":"easeui-button-primary","name":"BuyBtn","text":"Upgrade"}]}"#,
        )
        .unwrap();

        let opts = super::cli::parse([doc_path.to_string_lossy().into_owned(), "--apply-spec".into(), spec_in.to_string_lossy().into_owned()]).unwrap();
        super::run_agent_cli(&opts).unwrap();

        let opts_comment = super::cli::parse([
            doc_path.to_string_lossy().into_owned(),
            "--select".into(),
            "BuyBtn".into(),
            "--add-comment".into(),
            "Make button orange".into(),
        ])
        .unwrap();
        super::run_agent_cli(&opts_comment).unwrap();

        let reloaded = super::io::load_path(&doc_path).unwrap();
        assert_eq!(reloaded.comments.len(), 1);
        let tid = reloaded.comments[0].id.clone();

        let opts_reply = super::cli::parse([
            doc_path.to_string_lossy().into_owned(),
            "--reply-comment".into(),
            tid,
            "--message".into(),
            "Done".into(),
            "--resolve".into(),
            "--export-spec".into(),
            spec_out.to_string_lossy().into_owned(),
            "--inspect-json".into(),
            "--comments-json".into(),
        ])
        .unwrap();
        super::run_agent_cli(&opts_reply).unwrap();

        let final_doc = super::io::load_path(&doc_path).unwrap();
        assert!(final_doc.comments[0].resolved);
        assert_eq!(final_doc.comments[0].messages.len(), 2);
        assert!(spec_out.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn web_capture_cli_import_and_out_roundtrip() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("designos_web_capture_cli_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let cap_in = dir.join("card.capture.json");
        let doc_out = dir.join("card.designos");

        std::fs::write(
            &cap_in,
            r##"{
                "schema_version": 1,
                "url": "https://example.com/card",
                "title": "CLI Captured Card",
                "viewport": { "width": 800, "height": 600, "dpr": 2.0 },
                "root": {
                    "tag": "div",
                    "bounds": { "x": 0, "y": 0, "w": 300, "h": 120 },
                    "style": { "display": "flex", "flex_direction": "column", "row_gap": 10, "background_color": "#ffffff" },
                    "children": [
                        {
                            "tag": "p",
                            "bounds": { "x": 12, "y": 12, "w": 276, "h": 24 },
                            "style": { "color": "#111827", "font_family": "Inter", "font_size": 16, "font_weight": 500, "line_height": 24 },
                            "text_runs": [{ "text": "Captured via CLI", "bounds": { "x": 12, "y": 12, "w": 180, "h": 24 } }]
                        }
                    ]
                }
            }"##,
        )
        .unwrap();

        let opts =
            super::cli::parse(["--import-capture".into(), cap_in.to_string_lossy().into_owned(), "--out".into(), doc_out.to_string_lossy().into_owned()])
                .unwrap();
        super::run_agent_cli(&opts).unwrap();

        let loaded = super::io::load_path(&doc_out).unwrap();
        assert_eq!(loaded.name, "CLI Captured Card — example.com");
        assert!(loaded.nodes.values().any(|n| matches!(&n.kind, designos_doc::NodeKind::Text(t) if t.content == "Captured via CLI")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_apply_spec_reports_field_and_expected_type_in_plain_words() {
        let dir = std::env::temp_dir().join(format!("designos_rt07_cli_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("r7.designos");
        let spec_path = dir.join("aa.json");
        let mut doc = designos_doc::Document::new("RT07");
        doc.add_page("Page 1");
        super::io::save_path(&doc, &doc_path).unwrap();

        let cases: &[(&str, &str)] = &[
            (r#"{"name":"Card","children":[[{"kind":"rectangle","name":"Box"}]]}"#, "children must be an array of NodeSpec objects"),
            (r#"{"name":"Card","children":[[]]}"#, "children must be an array of NodeSpec objects"),
            (r#"{"name":"Card","children":"nope"}"#, "children must be an array of NodeSpec objects"),
            (r#"[{"kind":"frame","name":"Card"}]"#, "spec must be a NodeSpec object"),
            (r#"{"name":"Card","layout":"row"}"#, "layout must be a LayoutSpec object"),
            (r#"{"name":"Card","layout":[]}"#, "layout must be a LayoutSpec object"),
            (r#"{"name":"Card","overrides":[[{"path":"Title"}]]}"#, "overrides must be an array of OverrideSpec objects"),
            (r#"{"name":"Card","overrides":[[]]}"#, "overrides must be an array of OverrideSpec objects"),
            (r#"{"name":"Card","in_progress":"yes"}"#, "in_progress must be a boolean or an in_progress object"),
            (r#"{"name":"Card","in_progress":[]}"#, "in_progress must be a boolean or an in_progress object"),
            (r#"{"name":"Card","width":true}"#, "width must be a number or a \"var:...\" string"),
            (r#"{"name":"Card","opacity":[]}"#, "opacity must be a number between 0 and 1 or a \"var:...\" string"),
            (r#"{"name":"Card","corner_radii":[8,8]}"#, "corner_radii must be an array of 4 numbers or \"var:...\" strings"),
            (r#"{"name":"Card","layout":{"padding":16}}"#, "padding must be an array of 4 numbers or \"var:...\" strings"),
            (r#"{"name":"Card","visible":123}"#, "visible must be a boolean or a \"var:...\" string"),
            (r#"{"name":"Card","id":"abc"}"#, "id must be a non-negative integer"),
            (r#"{"name":"Card","font_weight":"bold"}"#, "font_weight must be a number between 100 and 900"),
            (r#"{"name":"Card","col_span":"two"}"#, "col_span must be a positive integer"),
            (r#"{"name":"Card","layout":{"grid_columns":"three"}}"#, "grid_columns must be a positive integer"),
            (r#"{"name":"Card","x":"left"}"#, "x must be a number"),
            (r#"{"name":"Card","min_width":true}"#, "min_width must be a number"),
            (r#"{"name":"Card","clips_content":"yes"}"#, "clips_content must be a boolean"),
            (r#"{"name":"Card","variant":["md"]}"#, "variant must be an object"),
            (r#"{"name":"Card","mode":["dark"]}"#, "mode must be an object"),
            (r#"{"name":"Card","props":[]}"#, "props must be an object"),
            (r#"{"name":"Card","annotations":"one"}"#, "annotations must be an array of strings"),
            (r#"{"name":"Card","comments":[[]]}"#, "comments must be an array of comment thread objects"),
        ];

        let banned_rust_types = [
            "expected u64",
            "expected u32",
            "expected u16",
            "expected f32",
            "NumberOrVar",
            "BoolOrVar",
            "InProgressSpec",
            "struct LayoutSpec",
            "struct NodeSpec",
            "struct OverrideSpec",
            "tuple of size 4",
        ];

        for (raw_json, expected_phrase) in cases {
            std::fs::write(&spec_path, raw_json).unwrap();
            let opts = super::cli::parse([doc_path.to_string_lossy().into_owned(), "--apply-spec".into(), spec_path.to_string_lossy().into_owned()]).unwrap();
            let err = super::run_agent_cli(&opts).expect_err(&format!("spec {raw_json} must be refused"));
            assert!(err.contains(expected_phrase), "for {raw_json}, expected {expected_phrase:?} in {err:?}");
            for banned in banned_rust_types {
                assert!(!err.contains(banned), "for {raw_json}, error must not name Rust type {banned:?}: {err:?}");
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round3_cli_apply_spec_reports_nested_path() {
        let dir = std::env::temp_dir().join(format!("designos_rt07_r3_nested_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("target.designos");
        let spec_path = dir.join("spec.json");
        std::fs::write(&spec_path, r#"{"kind":"frame","name":"Root","children":[{"kind":"frame","name":"Child","layout":{"padding":16}}]}"#).unwrap();
        let opts = super::cli::parse([doc_path.to_string_lossy().into_owned(), "--apply-spec".into(), spec_path.to_string_lossy().into_owned()]).unwrap();
        let err_nested = super::run_agent_cli(&opts).expect_err("nested padding must fail");
        assert!(
            err_nested.contains("children/0/layout/padding must be an array of 4 numbers or \"var:...\" strings"),
            "expected nested path in {err_nested:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round3_cli_apply_spec_refuses_duplicate_keys() {
        let dir = std::env::temp_dir().join(format!("designos_rt07_r3_dup_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("target.designos");
        let spec_path = dir.join("spec.json");
        std::fs::write(&spec_path, r#"{"kind":"frame","name":"A","name":"B"}"#).unwrap();
        let opts = super::cli::parse([doc_path.to_string_lossy().into_owned(), "--apply-spec".into(), spec_path.to_string_lossy().into_owned()]).unwrap();
        let err_dup = super::run_agent_cli(&opts).expect_err("duplicate JSON keys must be refused");
        assert!(err_dup.contains("duplicate field `name`"), "expected duplicate field error in {err_dup:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round3_cli_apply_spec_unknown_field_omits_code_prefix() {
        let dir = std::env::temp_dir().join(format!("designos_rt07_r3_unk_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("target.designos");
        let spec_path = dir.join("spec.json");
        std::fs::write(&spec_path, r##"{"kind":"frame","name":"Card","colour":"#fff"}"##).unwrap();
        let opts = super::cli::parse([doc_path.to_string_lossy().into_owned(), "--apply-spec".into(), spec_path.to_string_lossy().into_owned()]).unwrap();
        let err_unk = super::run_agent_cli(&opts).expect_err("unknown field must be refused");
        assert!(!err_unk.contains("UNKNOWN_FIELD:"), "user-facing CLI error must not include internal code prefix: {err_unk:?}");
        assert!(err_unk.contains("Unknown field `colour`"), "expected Unknown field `colour` in {err_unk:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round3_cli_apply_spec_rejects_f32_overflow_with_clear_message() {
        let dir = std::env::temp_dir().join(format!("designos_rt07_r3_ovf_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("target.designos");
        let spec_path = dir.join("spec.json");
        std::fs::write(&spec_path, r#"{"kind":"frame","name":"Big","x":1e40}"#).unwrap();
        let opts = super::cli::parse([doc_path.to_string_lossy().into_owned(), "--apply-spec".into(), spec_path.to_string_lossy().into_owned()]).unwrap();
        let err_overflow = super::run_agent_cli(&opts).expect_err("f32 overflow must be refused");
        assert!(err_overflow.contains("x must be a finite number"), "expected finite-number message in {err_overflow:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cli_screenshot_loads_good_file_once() {
        let dir = std::env::temp_dir().join(format!("designos_rt04_load_once_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let good_path = dir.join("good.designos");
        let png_path = dir.join("good.png");
        let mut good_doc = designos_doc::Document::new("Good");
        good_doc.add_page("Page 1");
        super::io::save_path(&good_doc, &good_path).unwrap();

        let opts = super::cli::parse([good_path.to_string_lossy().into_owned(), "--screenshot".into(), png_path.to_string_lossy().into_owned()]).unwrap();

        let ctx = egui::Context::default();
        let _shell = super::shell::Shell::build(&ctx, opts);

        assert_eq!(super::io::load_call_count(&good_path), 1, "--screenshot must load {} exactly once", good_path.display());
        assert!(super::shell::SHOT_OPEN_ERROR.get().is_none());

        let broken_path = dir.join("garbage.fig");
        let broken_png = dir.join("broken.png");
        std::fs::write(&broken_path, b"not a fig file at all").unwrap();
        let broken_opts =
            super::cli::parse([broken_path.to_string_lossy().into_owned(), "--screenshot".into(), broken_png.to_string_lossy().into_owned()]).unwrap();
        let broken_ctx = egui::Context::default();
        let mut broken_shell = super::shell::Shell::build(&broken_ctx, broken_opts);
        assert_eq!(super::io::load_call_count(&broken_path), 1, "--screenshot must attempt to load broken file {} only once", broken_path.display());
        let recorded = super::shell::SHOT_OPEN_ERROR.get().expect("broken file sets SHOT_OPEN_ERROR");
        assert!(recorded.starts_with(&format!("{} could not be opened: ", broken_path.display())), "got: {recorded}");
        assert!(recorded.ends_with(". Check the file, then try again."), "got: {recorded}");
        let out = broken_ctx.run_ui(egui::RawInput::default(), |ui| broken_shell.draw(ui));
        assert!(out.shapes.is_empty(), "broken screenshot must not render blank Untitled document");
        let cmds = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.commands.as_slice()).unwrap_or(&[]);
        assert!(cmds.contains(&egui::ViewportCommand::Close), "broken screenshot must close viewport");
        assert!(!broken_png.exists(), "broken screenshot must not write PNG");

        let mut app = super::app::App::new(designos_doc::Document::new("Test"));
        assert!(app.try_open_path(good_path.clone()).is_ok());
        assert!(app.try_open_path(broken_path).is_err());
        assert!(app.try_open_path(dir.join("missing.webcapture.json")).is_err());
        let bad_wc = dir.join("bad.webcapture.json");
        std::fs::write(&bad_wc, "{not valid json").unwrap();
        assert!(app.try_open_path(bad_wc).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_capture_with_unknown_select_writes_nothing() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("designos_web_capture_select_miss_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cap_in = dir.join("card.capture.json");
        let doc_out = dir.join("card.designos");
        let original = b"existing document must survive selector failure";
        std::fs::write(&doc_out, original).unwrap();
        std::fs::write(
            &cap_in,
            r#"{"schema_version":1,"url":"https://example.com","title":"Card","viewport":{"width":800,"height":600,"dpr":1},"root":{"tag":"div","bounds":{"x":0,"y":0,"w":10,"h":10}}}"#,
        )
        .unwrap();

        let opts = super::cli::parse([
            "--import-capture".into(),
            cap_in.to_string_lossy().into_owned(),
            "--out".into(),
            doc_out.to_string_lossy().into_owned(),
            "--select".into(),
            "Definitely-Missing-1127".into(),
        ])
        .unwrap();
        let err = super::run_agent_cli(&opts).unwrap_err();
        assert!(err.contains("matched no node"), "{err}");
        assert_eq!(std::fs::read(&doc_out).unwrap(), original, "a selector miss must leave the output untouched");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod variables_cli_tests {
    #[test]
    fn cli_directory_export_never_overwrites_within_one_run() {
        let dir = std::env::temp_dir().join(format!("designos_1140_dir_export_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("kit.designos");
        let out_dir = dir.join("dtcg_out");

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
        doc.variable_collections.insert(
            "VariableCollectionId:20:2".into(),
            designos_doc::VariableCollection {
                id: "VariableCollectionId:20:2".into(),
                name: "_Primitives".into(),
                modes: vec![designos_doc::VariableMode { id: "20:m1".into(), name: "Style".into() }],
                default_mode: "20:m1".into(),
                variables: vec!["VariableID:20:v1".into()],
                key: Some("k-lib".into()),
                source_library_key: Some("lib-1".into()),
                extensions: std::collections::BTreeMap::new(),
            },
        );
        doc.variables.insert(
            "VariableID:20:v1".into(),
            designos_doc::Variable {
                id: "VariableID:20:v1".into(),
                name: "Colors/Brand/500".into(),
                collection: "VariableCollectionId:20:2".into(),
                kind: designos_doc::VariableKind::Color,
                values: std::collections::BTreeMap::from([("20:m1".into(), designos_doc::VariableValue::Color(designos_doc::Color::rgb(20, 20, 20)))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: std::collections::BTreeMap::new(),
                extensions: std::collections::BTreeMap::new(),
                mode_extensions: std::collections::BTreeMap::new(),
            },
        );
        super::io::save_path(&doc, &doc_path).unwrap();

        let opts = super::cli::parse([
            doc_path.to_string_lossy().into_owned(),
            "--export-variables".into(),
            out_dir.to_string_lossy().into_owned(),
            "--format".into(),
            "dtcg".into(),
        ])
        .unwrap();
        super::run_agent_cli(&opts).unwrap();

        let bundle = designos_fig::export_variables(&doc, designos_fig::VariablesFormat::Dtcg).unwrap();
        let on_disk_count = std::fs::read_dir(&out_dir).unwrap().filter_map(Result::ok).count();
        assert_eq!(on_disk_count, bundle.files.len(), "files count in bundle must equal files on disk");
        assert!(out_dir.join("_Primitives.Style.tokens.json").exists());
        assert!(out_dir.join("_Primitives (2).Style.tokens.json").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dtcg_to_a_json_path_without_mode_is_an_error() {
        let dir = std::env::temp_dir().join(format!("designos_1140_json_no_mode_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("kit.designos");
        let json_out = dir.join("x.json");

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
        super::io::save_path(&doc, &doc_path).unwrap();

        let opts = super::cli::parse([
            doc_path.to_string_lossy().into_owned(),
            "--export-variables".into(),
            json_out.to_string_lossy().into_owned(),
            "--format".into(),
            "dtcg".into(),
        ])
        .unwrap();
        let err = super::run_agent_cli(&opts).expect_err("dtcg export to .json without --mode must fail");
        assert!(err.contains("--mode"), "error should mention --mode, got: {err:?}");
        assert!(!json_out.exists(), "no directory or file named x.json may be created");

        // Also check Rule 10 option combinations fail cleanly without writing output.
        for bad_args in [
            vec![doc_path.to_string_lossy().into_owned(), "--mode".into(), "Style".into()],
            vec![
                doc_path.to_string_lossy().into_owned(),
                "--export-variables".into(),
                json_out.to_string_lossy().into_owned(),
                "--format".into(),
                "figma-rest".into(),
                "--mode".into(),
                "Style".into(),
            ],
            vec![
                doc_path.to_string_lossy().into_owned(),
                "--export-variables".into(),
                dir.join("out.zip").to_string_lossy().into_owned(),
                "--format".into(),
                "dtcg".into(),
                "--mode".into(),
                "Style".into(),
            ],
            vec![
                doc_path.to_string_lossy().into_owned(),
                "--export-variables".into(),
                dir.join("out_dir").to_string_lossy().into_owned(),
                "--format".into(),
                "dtcg".into(),
                "--resolve-aliases".into(),
            ],
            vec![
                doc_path.to_string_lossy().into_owned(),
                "--export-variables".into(),
                dir.join("out_dir").to_string_lossy().into_owned(),
                "--format".into(),
                "dtcg".into(),
                "--collection".into(),
                "_Primitives".into(),
            ],
            vec![
                doc_path.to_string_lossy().into_owned(),
                "--export-variables".into(),
                json_out.to_string_lossy().into_owned(),
                "--format".into(),
                "figma-rest".into(),
                "--color-format".into(),
                "hex".into(),
            ],
        ] {
            let parsed = super::cli::parse(bad_args.clone()).unwrap();
            assert!(super::run_agent_cli(&parsed).is_err(), "expected error for args {bad_args:?}");
        }
        assert!(!json_out.exists(), "no output file created after bad args");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
