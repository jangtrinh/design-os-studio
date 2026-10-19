mod agent_a11y;
mod agent_approval_card;
mod agent_busy_frames;
pub mod agent_cli_catalog;
pub mod agent_composer;
mod agent_mention;
mod agent_mention_popup;
pub mod agent_notify;
pub mod agent_open_flow;
pub mod agent_permissions;
pub mod agent_picker;
mod agent_presence;
mod agent_sample_prompts;
mod agent_selection_chip;
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
mod board;
mod canvas;
mod check_designs;
mod cli;
mod clipboard;
mod cmdk;
mod comments;
mod comp_ops;
mod decks;
mod demo;
mod dev;
mod edit;
mod find;
mod fonts;
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
mod live_patch_apply;
mod live_patch_log;
#[cfg(test)]
mod live_sync_interaction_tests;
#[cfg(test)]
mod live_sync_tests;
mod ops;
mod panels;
mod plugins;
mod present;
mod proto;
mod protovars;
mod richtext;
mod shell;
mod showcase;
mod specs;
mod styles;
mod tabs;
mod terminal;
mod terminal_undo_chord;
#[cfg(test)]
mod test_timing;
mod textedit;
mod textmeasure;
mod theme;
mod util;
mod variables;
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
    if let Some(dir) = &opts.seed_library {
        match showcase::seed_library(dir) {
            Ok(()) => println!("designos: wrote the showcase sample files to {}", dir.display()),
            Err(e) => {
                eprintln!("designos: cannot seed {}: {e}", dir.display());
                std::process::exit(1);
            }
        }
        return;
    }
    if opts.is_agent_cli() {
        if let Err(e) = run_agent_cli(&opts) {
            eprintln!("designos: {e}");
            std::process::exit(2);
        }
        return;
    }
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
    let result = eframe::run_native("design:os studio", native, Box::new(move |cc| Ok(Box::new(shell::Shell::new(cc, opts)))));
    if let Err(e) = result {
        eprintln!("designos: {e}");
        std::process::exit(1);
    }
    if screenshot_mode && !shell::SHOT_OK.load(Ordering::SeqCst) {
        eprintln!("designos: screenshot was not captured");
        std::process::exit(1);
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

fn resolve_select_node(doc: &designos_doc::Document, select: Option<&str>) -> Option<designos_doc::NodeId> {
    let s = select?.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        return None;
    }
    if let Some(id) = s.parse::<u64>().ok().map(designos_doc::NodeId).filter(|id| doc.get(*id).is_some()) {
        return Some(id);
    }
    for &pid in &doc.pages {
        if let Some(p) = doc.get(pid) {
            if p.name == s {
                return Some(pid);
            }
            for &cid in &p.children {
                if doc.get(cid).is_some_and(|c| c.name == s) {
                    return Some(cid);
                }
            }
        }
    }
    let mut found = None;
    for &pid in &doc.pages {
        doc.walk(pid, |n, _| {
            if found.is_none() && (n.name == s || n.name.eq_ignore_ascii_case(s)) {
                found = Some(n.id);
            }
        });
        if found.is_some() {
            return found;
        }
    }
    None
}

fn run_agent_cli(opts: &cli::Options) -> Result<(), String> {
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
        let d = if !path.exists() && (opts.apply_spec.is_some() || opts.import_capture.is_some() || opts.import_variables.is_some()) {
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

    let selected_node = resolve_select_node(&doc, opts.select.as_deref());

    if let Some(spec_path) = &opts.apply_spec {
        let raw = std::fs::read_to_string(spec_path).map_err(|e| format!("Cannot read {}: {e}", spec_path.display()))?;
        let spec: designos_doc::NodeSpec = serde_json::from_str(&raw).map_err(|e| format!("Invalid NodeSpec JSON in {}: {e}", spec_path.display()))?;
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
        if let Some(parent) = out_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(out_path, &bytes).map_err(|e| format!("Cannot write {}: {e}", out_path.display()))?;
        let out = serde_json::json!({ "exported_fig": true, "path": out_path, "bytes": bytes.len(), "omitted_extensions": omitted });
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
        let bundle = designos_fig::export_variables(&doc, fmt).map_err(|e| format!("Cannot export variables: {e}"))?;
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
        let out = serde_json::json!({
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
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    Ok(())
}

#[cfg(test)]
mod icon_tests {
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
}
