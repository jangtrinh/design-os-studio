//! `designos_get_libraries` implementation (`T7.38`, Issue `#247`).

use super::library_source::resolve_library_root;
use designos_doc::{publishable_assets, Document, LibraryAssetKind};
use designos_library::LibraryIndex;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Debug)]
struct ListedLibrary {
    id: String,
    name: String,
    enabled: bool,
    payload: Value,
}

/// Executes `designos_get_libraries` against `doc` using the configured library root (`$DESIGNOS_LIBRARY` or platform default).
pub fn get_libraries(doc: &Document) -> Value {
    let root = resolve_library_root();
    get_libraries_at(doc, &root)
}

/// Executes `designos_get_libraries` against `doc` and an explicit `root` directory.
pub fn get_libraries_at(doc: &Document, root: &Path) -> Value {
    let assets = publishable_assets(doc);
    let mut local_components = 0usize;
    let mut local_styles = 0usize;
    let mut local_variables = 0usize;
    for (_, kind, _) in &assets {
        match kind {
            LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => local_components += 1,
            LibraryAssetKind::Style => local_styles += 1,
            LibraryAssetKind::Variable => local_variables += 1,
            LibraryAssetKind::VariableCollection => {}
        }
    }

    let local_entry = json!({
        "id": "local",
        "name": "This file",
        "origin": "local",
        "enabled": true,
        "components": local_components,
        "styles": local_styles,
        "variables": local_variables,
        "published": doc.library.published,
    });

    let self_published_id = doc.library.published.as_ref().map(|p| p.library_id.as_str()).filter(|s| !s.is_empty());

    let mut library_root_error: Option<String> = None;
    let mut disk_entries: Vec<ListedLibrary> = Vec::new();
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    if let Some(self_id) = self_published_id {
        seen_ids.insert(self_id.to_string());
    }

    if root.exists() && !root.is_dir() {
        library_root_error = Some(format!("Library root '{}' is not a directory", root.display()));
    } else if root.is_dir() {
        let libs_dir = root.join("libraries");
        if libs_dir.exists() && !libs_dir.is_dir() {
            library_root_error = Some(format!("Libraries path '{}' is not a directory", libs_dir.display()));
        } else if libs_dir.is_dir() {
            match std::fs::read_dir(&libs_dir) {
                Ok(read_dir) => {
                    for entry in read_dir.flatten() {
                        let path = entry.path();
                        if !path.is_dir() {
                            continue;
                        }
                        let dir_id = entry.file_name().to_string_lossy().into_owned();
                        if self_published_id == Some(dir_id.as_str()) {
                            continue;
                        }
                        let index_path = path.join("index.json");
                        match std::fs::read_to_string(&index_path).and_then(|s| serde_json::from_str::<LibraryIndex>(&s).map_err(Into::into)) {
                            Ok(idx) => {
                                if self_published_id == Some(idx.id.as_str()) {
                                    continue;
                                }
                                seen_ids.insert(idx.id.clone());
                                let enabled_ref = doc.library.enabled.iter().find(|r| r.id == idx.id);
                                let latest = idx.latest_version();
                                let payload = if let Some(en) = enabled_ref {
                                    let update_available = latest > en.version;
                                    let mut obj = json!({
                                        "id": idx.id,
                                        "name": idx.name,
                                        "origin": "library",
                                        "enabled": true,
                                        "enabled_version": en.version,
                                        "latest_version": latest,
                                        "update_available": update_available,
                                        "components": idx.components,
                                        "styles": idx.styles,
                                        "variables": idx.variables,
                                        "missing": false,
                                    });
                                    if en.version > latest {
                                        obj["version_mismatch"] = json!(true);
                                    }
                                    obj
                                } else {
                                    json!({
                                        "id": idx.id,
                                        "name": idx.name,
                                        "origin": "library",
                                        "enabled": false,
                                        "latest_version": latest,
                                        "components": idx.components,
                                        "styles": idx.styles,
                                        "variables": idx.variables,
                                        "missing": false,
                                    })
                                };
                                disk_entries.push(ListedLibrary { id: idx.id, name: idx.name, enabled: enabled_ref.is_some(), payload });
                            }
                            Err(err) => {
                                seen_ids.insert(dir_id.clone());
                                let enabled_ref = doc.library.enabled.iter().find(|r| r.id == dir_id);
                                let name = enabled_ref.map(|r| r.name.clone()).unwrap_or_else(|| dir_id.clone());
                                let mut obj = json!({
                                    "id": dir_id,
                                    "name": name,
                                    "origin": "library",
                                    "enabled": enabled_ref.is_some(),
                                    "missing": false,
                                    "error": err.to_string(),
                                });
                                if let Some(en) = enabled_ref {
                                    obj["enabled_version"] = json!(en.version);
                                }
                                disk_entries.push(ListedLibrary { id: dir_id, name, enabled: enabled_ref.is_some(), payload: obj });
                            }
                        }
                    }
                }
                Err(e) => {
                    library_root_error = Some(e.to_string());
                }
            }
        }
    }

    // Enabled libraries missing from the root (`T7.38` §3.2)
    for en in &doc.library.enabled {
        if seen_ids.contains(&en.id) {
            continue;
        }
        seen_ids.insert(en.id.clone());
        let mut components = 0usize;
        let mut styles = 0usize;
        let mut variables = 0usize;
        for ext in doc.library.external.values().filter(|e| e.library_id == en.id) {
            match ext.kind {
                LibraryAssetKind::Component | LibraryAssetKind::ComponentSet => components += 1,
                LibraryAssetKind::Style => styles += 1,
                LibraryAssetKind::Variable => variables += 1,
                LibraryAssetKind::VariableCollection => {}
            }
        }
        let payload = json!({
            "id": en.id,
            "name": en.name,
            "origin": "library",
            "enabled": true,
            "enabled_version": en.version,
            "latest_version": null,
            "update_available": false,
            "components": components,
            "styles": styles,
            "variables": variables,
            "missing": true,
        });
        disk_entries.push(ListedLibrary { id: en.id.clone(), name: en.name.clone(), enabled: true, payload });
    }

    // Order: local first, then enabled (by name, then id), then disabled (by name, then id).
    disk_entries.sort_by(|a, b| b.enabled.cmp(&a.enabled).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())).then_with(|| a.id.cmp(&b.id)));

    let mut libraries = Vec::with_capacity(1 + disk_entries.len());
    libraries.push(local_entry);
    for item in disk_entries {
        libraries.push(item.payload);
    }

    json!({
        "libraries": libraries,
        "library_root": root.to_string_lossy(),
        "library_root_error": library_root_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::tools::library_source::{clear_snapshot_cache, enabled_snapshots_from_root};
    use designos_doc::{
        Color, ExternalAsset, FrameProps, LibraryRef, Node, NodeKind, Paint, Style, Variable, VariableCollection, VariableKind, VariableMode, VariableValue,
    };
    use designos_library::Library;
    use std::collections::BTreeMap;

    fn make_temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("designos_t7_38_{tag}_{}", designos_library::new_id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_doc_with_assets(name: &str, comps: usize, styles: usize, vars: usize) -> Document {
        let mut doc = Document::new(name);
        let page = doc.add_page("Page 1");
        for i in 0..comps {
            let cid = doc.alloc_id();
            let mut c = Node::new(cid, NodeKind::Component(FrameProps::default()));
            c.name = format!("Comp/{i}");
            doc.insert(page, c).unwrap();
        }
        for i in 0..styles {
            doc.styles.insert(format!("Style/{i}"), Style::Fill(vec![Paint::solid(Color::rgb(10, 20, 30))]));
        }
        if vars > 0 {
            let mut var_ids = Vec::new();
            for i in 0..vars {
                let vid = format!("v{i}");
                var_ids.push(vid.clone());
                doc.variables.insert(
                    vid.clone(),
                    Variable {
                        id: vid,
                        name: format!("Token/{i}"),
                        collection: "c1".into(),
                        kind: VariableKind::Number,
                        values: BTreeMap::from([("m1".into(), VariableValue::Number(i as f32))]),
                        description: String::new(),
                        scopes: Vec::new(),
                        hidden_from_publishing: false,
                        key: None,
                        code_syntax: Default::default(),
                    },
                );
            }
            doc.variable_collections.insert(
                "c1".into(),
                VariableCollection {
                    id: "c1".into(),
                    name: "Tokens".into(),
                    modes: vec![VariableMode { id: "m1".into(), name: "Default".into() }],
                    default_mode: "m1".into(),
                    variables: var_ids,
                    key: None,
                    source_library_key: None,
                },
            );
        }
        doc
    }

    #[test]
    fn lists_every_library_with_counts() {
        let root = make_temp_dir("lists_every");
        let lib = Library::open(&root).unwrap();

        let brand_doc = build_doc_with_assets("Brand kit", 3, 2, 4);
        lib.publish_library("lib-brand", &brand_doc, "v1").unwrap();

        let icons_doc = build_doc_with_assets("Icons", 5, 0, 1);
        lib.publish_library("lib-icons", &icons_doc, "v1").unwrap();

        let mut active_doc = build_doc_with_assets("App Screen", 2, 1, 3);
        active_doc.library.enabled.push(LibraryRef { id: "lib-brand".into(), name: "Brand kit".into(), version: 1 });

        let out = get_libraries_at(&active_doc, &root);
        assert!(out["library_root_error"].is_null());
        let list = out["libraries"].as_array().unwrap();
        assert_eq!(list.len(), 3);

        assert_eq!(list[0]["id"], "local");
        assert_eq!(list[0]["origin"], "local");
        assert_eq!(list[0]["enabled"], true);
        assert_eq!(list[0]["components"], 2);
        assert_eq!(list[0]["styles"], 1);
        assert_eq!(list[0]["variables"], 3);

        assert_eq!(list[1]["id"], "lib-brand");
        assert_eq!(list[1]["origin"], "library");
        assert_eq!(list[1]["enabled"], true);
        assert_eq!(list[1]["enabled_version"], 1);
        assert_eq!(list[1]["latest_version"], 1);
        assert_eq!(list[1]["update_available"], false);
        assert_eq!(list[1]["components"], 3);
        assert_eq!(list[1]["styles"], 2);
        assert_eq!(list[1]["variables"], 4);
        assert_eq!(list[1]["missing"], false);

        assert_eq!(list[2]["id"], "lib-icons");
        assert_eq!(list[2]["origin"], "library");
        assert_eq!(list[2]["enabled"], false);
        assert_eq!(list[2]["latest_version"], 1);
        assert_eq!(list[2]["components"], 5);
        assert_eq!(list[2]["styles"], 0);
        assert_eq!(list[2]["variables"], 1);
        assert_eq!(list[2]["missing"], false);

        // Verify shared `enabled_snapshots_from_root`
        clear_snapshot_cache();
        let snaps = enabled_snapshots_from_root(&active_doc, &root);
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].library_id, "lib-brand");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn disabled_flagged_not_hidden() {
        let root = make_temp_dir("disabled_flagged");
        let lib = Library::open(&root).unwrap();
        let icons_doc = build_doc_with_assets("Icons", 4, 1, 0);
        lib.publish_library("lib-icons", &icons_doc, "v1").unwrap();

        let active_doc = Document::new("Host");
        let out = get_libraries_at(&active_doc, &root);
        let list = out["libraries"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        let disabled = &list[1];
        assert_eq!(disabled["id"], "lib-icons");
        assert_eq!(disabled["enabled"], false);
        assert_eq!(disabled["components"], 4);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn update_available_flag() {
        let root = make_temp_dir("update_avail");
        let lib = Library::open(&root).unwrap();
        let mut kit_doc = build_doc_with_assets("Brand kit", 1, 1, 1);
        let info = lib.publish_library("lib-brand", &kit_doc, "v1").unwrap();
        kit_doc.library.published = Some(info);
        lib.publish_library("lib-brand", &kit_doc, "v2").unwrap();

        let mut active_doc = Document::new("Host");
        active_doc.library.enabled.push(LibraryRef { id: "lib-brand".into(), name: "Brand kit".into(), version: 1 });

        let out = get_libraries_at(&active_doc, &root);
        let brand = &out["libraries"].as_array().unwrap()[1];
        assert_eq!(brand["enabled_version"], 1);
        assert_eq!(brand["latest_version"], 2);
        assert_eq!(brand["update_available"], true);

        // Edge case 5: rolled-back index where enabled_version > latest_version
        active_doc.library.enabled[0].version = 5;
        let out_mismatch = get_libraries_at(&active_doc, &root);
        let brand_mismatch = &out_mismatch["libraries"].as_array().unwrap()[1];
        assert_eq!(brand_mismatch["update_available"], false);
        assert_eq!(brand_mismatch["version_mismatch"], true);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_enabled_library() {
        let root = make_temp_dir("missing_enabled");
        let mut active_doc = Document::new("Host");
        active_doc.library.enabled.push(LibraryRef { id: "lib-gone".into(), name: "Deleted Kit".into(), version: 2 });
        active_doc
            .library
            .external
            .insert("c10".into(), ExternalAsset { library_id: "lib-gone".into(), version: 2, kind: LibraryAssetKind::Component, node: None, local_key: None });
        active_doc.library.external.insert(
            "style:s1".into(),
            ExternalAsset { library_id: "lib-gone".into(), version: 2, kind: LibraryAssetKind::Style, node: None, local_key: Some("s1".into()) },
        );

        let out = get_libraries_at(&active_doc, &root);
        let list = out["libraries"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        let missing = &list[1];
        assert_eq!(missing["id"], "lib-gone");
        assert_eq!(missing["enabled"], true);
        assert_eq!(missing["missing"], true);
        assert_eq!(missing["components"], 1);
        assert_eq!(missing["styles"], 1);
        assert_eq!(missing["variables"], 0);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unreadable_root() {
        let dir = make_temp_dir("unreadable");
        let file_as_root = dir.join("not_a_dir.json");
        std::fs::write(&file_as_root, b"{}").unwrap();

        let active_doc = build_doc_with_assets("Host", 1, 0, 0);
        let out = get_libraries_at(&active_doc, &file_as_root);
        let list = out["libraries"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["id"], "local");
        assert_eq!(list[0]["components"], 1);
        assert!(out["library_root_error"].as_str().is_some_and(|s| !s.is_empty()));

        // Also verify Engine dispatch rejects unknown arguments
        let mut eng = Engine::default();
        assert!(eng.call("designos_get_libraries", json!({"unexpected": 1})).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
