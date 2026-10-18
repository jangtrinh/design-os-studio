//! Import checks against the owner's private design files. They are ignored by
//! default and skip when a file is absent (the files are local and never
//! committed). Run with `cargo test --release -p designos-fig --test real_files -- --ignored`;
//! set `DESIGNOS_REFERENCE_DIR` to point at the folder holding them (default:
//! `reference/` of the checkout or an ancestor, then `$HOME/Downloads`).

use designos_doc::*;
use designos_fig::import_fig;
use std::path::PathBuf;

fn reference(name: &str) -> Option<Vec<u8>> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("DESIGNOS_REFERENCE_DIR").map(PathBuf::from).into_iter().collect();
    // `reference/` of this checkout or of any ancestor (worktrees live inside the main checkout).
    let mut cur = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    while let Some(d) = cur {
        dirs.push(d.join("reference"));
        cur = d.parent().map(PathBuf::from);
    }
    dirs.extend(std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads")));
    dirs.into_iter().map(|d| d.join(name)).find_map(|p| std::fs::read(p).ok())
}

fn is_clone(n: &Node) -> bool {
    n.source_id.as_deref().is_some_and(|s| s.starts_with('I'))
}

fn subtree(doc: &Document, root: NodeId) -> Vec<&Node> {
    let mut out = Vec::new();
    doc.walk(root, |n, _| out.push(doc.get(n.id).unwrap()));
    out
}

#[test]
#[ignore = "needs the private reference/private-ui-kit.fig"]
fn private_ui_kit_imports_with_library_instances_resolved() {
    let Some(bytes) = reference("private-ui-kit.fig") else {
        eprintln!("private-ui-kit.fig not found: skipped");
        return;
    };
    let doc = import_fig(&bytes).expect("import");
    assert!(!doc.nodes.values().any(|n| matches!(n.kind, NodeKind::Unknown { .. })), "no Unknown node kinds");

    let kit = doc.pages.iter().copied().find(|p| doc.get(*p).unwrap().name.contains("UI Kit")).expect("UI Kit page");
    let own: Vec<&Node> = subtree(&doc, kit).into_iter().filter(|n| !is_clone(n)).collect();
    assert_eq!(own.len(), 224, "nodes authored on the UI Kit page");
    let instances: Vec<&&Node> = own.iter().filter(|n| matches!(n.kind, NodeKind::Instance { .. })).collect();
    assert_eq!(instances.len(), 66);
    for i in &instances {
        let comp = instance_component(i).unwrap_or_else(|| panic!("instance {:?} unresolved", i.name));
        assert!(is_library_node(&doc, comp), "points at a library component");
        assert!(!i.children.is_empty(), "instance {:?} has no children", i.name);
    }

    let lp = doc.library_page.expect("library page");
    assert!(!doc.pages.contains(&lp));
    assert!(doc.library.external.len() > 50, "remote components are external assets");
    assert!(doc.library.external.values().all(|e| e.library_id.starts_with(REMOTE_LIBRARY_PREFIX)));

    assert_eq!(doc.variables.len(), 112);
    assert_eq!(doc.variable_collections.len(), 8);
    assert!(
        doc.variables.values().any(|v| v.values.values().any(|x| matches!(x, VariableValue::Alias(t) if doc.variables.contains_key(t)))),
        "aliases resolve"
    );
    assert!(doc.nodes.values().any(|n| !n.variables.bindings.is_empty()), "nodes bind variables");
    assert_eq!(doc.images.len(), 34);
}

#[test]
#[ignore = "needs the private reference/shadcn-ui.fig"]
fn shadcn_ui_imports_with_local_components() {
    let Some(bytes) = reference("shadcn-ui.fig") else {
        eprintln!("shadcn-ui.fig not found: skipped");
        return;
    };
    let doc = import_fig(&bytes).expect("import");
    assert!(!doc.nodes.values().any(|n| matches!(n.kind, NodeKind::Unknown { .. })));
    assert_eq!(doc.pages.len(), 6);
    let (mut instances, mut resolved) = (0, 0);
    for n in doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Instance { .. })) {
        instances += 1;
        resolved += instance_component(n).is_some() as usize;
    }
    assert!(instances >= 400 && instances == resolved, "{resolved} of {instances} instances resolved");
    assert_eq!(doc.styles.len(), 35);
    assert!(doc.library.external.is_empty(), "everything is local");

    for (source, size, weight) in [("21:1325", 18.0, 600), ("13:2558", 16.0, 400)] {
        let n = doc.nodes.values().find(|n| n.source_id.as_deref() == Some(source)).expect("cover text");
        let NodeKind::Text(t) = &n.kind else { panic!("cover text node") };
        assert_eq!(t.style.font_size, size, "{} uses its linked style", n.name);
        assert_eq!(t.style.font_weight, weight);
    }
}

const UNTITLED_UI_KIT: &str = "❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig";

/// Frames named in #525 where link and secondary buttons drew as filled primary pills.
const BUTTON_ISSUE_FRAMES: [&str; 7] = ["1685:415210", "1677:407177", "6277:291714", "1750:454032", "1719:453435", "1719:446490", "6277:315326"];

/// The variant name segment after `Hierarchy=` (`Link color`, `Secondary`, ...).
fn button_hierarchy(component_name: &str) -> Option<&str> {
    let rest = &component_name[component_name.find("Hierarchy=")? + "Hierarchy=".len()..];
    Some(rest.split(',').next().unwrap_or(rest).trim())
}

/// Result of comparing every button instance's fill, stroke and effects shell with its component.
#[derive(Default)]
struct ShellAudit {
    /// Instances checked, by `Hierarchy=` value.
    checked: std::collections::BTreeMap<String, usize>,
    checked_ids: std::collections::BTreeSet<NodeId>,
    /// Shells that kept another variant's look.
    stale: Vec<String>,
    /// Fields whose variable binding matches the component, so the value follows the variable mode.
    variable_resolved: usize,
    /// Fields the instance restyled itself (own binding or hard-coded value) without leaking a variant shell.
    host_restyled: usize,
}

impl ShellAudit {
    fn count(&self, family: &str) -> usize {
        self.checked.iter().filter(|(h, _)| h.starts_with(family)).map(|(_, n)| n).sum()
    }
}

/// Variable ids bound under `prefix` (`fills`, `strokes`, `effects`) by `n`.
fn bound_vars<'a>(n: &'a Node, prefix: &'a str) -> impl Iterator<Item = (&'a String, &'a String)> {
    n.variables.bindings.iter().filter(move |(k, _)| k.starts_with(prefix))
}

/// Checks the fill, stroke and effects shell of every `Hierarchy=` button instance.
///
/// The `.fig` importer applies host overrides straight onto the instance without recording them
/// in `component.overrides`, so an instance may legitimately differ from its component. A field is
/// stale when it was not overridden and:
/// - the component leaves it empty (link buttons have no fill, stroke or shadow) but the instance paints it;
/// - the component binds no variable and the value differs; or
/// - a non-Primary instance binds a variable that only the Primary variants bind, which is the old
///   shell the swap used to leave behind.
///
/// A field bound to the component's own variable follows the variable mode (dark frames), so its
/// value is not compared.
fn audit_button_shells(doc: &Document) -> ShellAudit {
    let primary_vars = |prefix: &str| -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        for n in doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))) {
            if button_hierarchy(&n.name) == Some("Primary") {
                out.extend(bound_vars(n, prefix).map(|(_, v)| v.clone()));
            }
        }
        out
    };
    let primary_fill_vars = primary_vars("fills");
    let primary_stroke_vars = primary_vars("strokes");
    let primary_effect_vars = primary_vars("effects");

    let mut audit = ShellAudit::default();
    for inst in doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Instance { .. })) {
        let Some(comp) = instance_component(inst).and_then(|c| doc.get(c)) else { continue };
        let Some(hierarchy) = button_hierarchy(&comp.name) else { continue };
        *audit.checked.entry(hierarchy.to_string()).or_default() += 1;
        audit.checked_ids.insert(inst.id);
        let fields = [
            ("fills", OverrideField::Fills, inst.fills != comp.fills, comp.fills.is_empty(), inst.fills.is_empty(), &primary_fill_vars),
            (
                "strokes",
                OverrideField::Stroke,
                inst.stroke.paints != comp.stroke.paints,
                comp.stroke.paints.is_empty(),
                inst.stroke.paints.is_empty(),
                &primary_stroke_vars,
            ),
            ("effects", OverrideField::Effects, inst.effects != comp.effects, comp.effects.is_empty(), inst.effects.is_empty(), &primary_effect_vars),
        ];
        for (prefix, override_field, differs, comp_empty, inst_empty, primary) in fields {
            if inst.component.overrides.contains(&override_field) {
                continue;
            }
            let same_bindings = bound_vars(inst, prefix).eq(bound_vars(comp, prefix));
            let leaked = hierarchy != "Primary" && bound_vars(inst, prefix).any(|(_, v)| primary.contains(v) && !bound_vars(comp, prefix).any(|(_, c)| c == v));
            let reason = if comp_empty && !inst_empty {
                Some("paints a field the component leaves empty")
            } else if leaked {
                Some("binds a variable of the Primary variant")
            } else if same_bindings && comp.variables.bindings.keys().any(|k| k.starts_with(prefix)) {
                audit.variable_resolved += differs as usize;
                None
            } else if same_bindings {
                differs.then_some("differs from the component")
            } else {
                audit.host_restyled += 1;
                None
            };
            if let Some(reason) = reason {
                audit.stale.push(format!("{:?} ({:?}) of {:?}: {prefix} {reason}", inst.name, inst.source_id, comp.name));
            }
        }
    }
    audit
}

#[test]
#[ignore = "needs the private Untitled UI PRO VARIABLES v8.0 kit"]
fn untitled_ui_swapped_link_and_secondary_buttons_match_variant_shell() {
    let Some(bytes) = reference("untitled-ui.fig").or_else(|| reference(UNTITLED_UI_KIT)) else {
        eprintln!("Untitled UI kit not found: skipped");
        return;
    };
    let mut doc = import_fig(&bytes).expect("import the kit");
    // Pages stay deferred after import; every page must be expanded before its buttons exist.
    assert!(doc.hydrate_all_pages(), "kit pages were deferred");
    let audit = audit_button_shells(&doc);
    eprintln!("checked={:?} variable_resolved={} host_restyled={} stale={}", audit.checked, audit.variable_resolved, audit.host_restyled, audit.stale.len());

    let mut by_kind = std::collections::BTreeMap::<String, usize>::new();
    for line in &audit.stale {
        let (component, reason) = line.rsplit_once(": ").expect("stale line");
        let hierarchy = component.split("Hierarchy=").nth(1).and_then(|r| r.split(',').next()).unwrap_or("?");
        *by_kind.entry(format!("Hierarchy={hierarchy}: {reason}")).or_default() += 1;
    }
    eprintln!("stale by kind: {by_kind:#?}");
    assert!(audit.stale.is_empty(), "{} stale button shells, first: {:#?}", audit.stale.len(), &audit.stale[..audit.stale.len().min(5)]);
    assert_eq!((audit.variable_resolved, audit.host_restyled), (99, 53), "fields following a variable mode / restyled by the instance");
    assert_eq!(audit.count("Link"), 17_381, "Link, Link color and Link gray button instances");
    assert_eq!(audit.count("Secondary"), 4_810, "Secondary button instances");
    assert_eq!(audit.checked.values().sum::<usize>(), 29_540, "all Hierarchy= button instances");

    // Each frame named in the issue holds checked Link and Secondary buttons, on every page.
    for frame in BUTTON_ISSUE_FRAMES {
        let root = doc.nodes.values().find(|n| n.source_id.as_deref() == Some(frame)).unwrap_or_else(|| panic!("frame {frame} missing")).id;
        let (mut link, mut secondary) = (0usize, 0usize);
        for n in subtree(&doc, root).into_iter().filter(|n| audit.checked_ids.contains(&n.id)) {
            let hierarchy = button_hierarchy(&doc.get(instance_component(n).unwrap()).unwrap().name).unwrap();
            link += hierarchy.starts_with("Link") as usize;
            secondary += hierarchy.starts_with("Secondary") as usize;
        }
        eprintln!("frame {frame}: link={link} secondary={secondary}");
        assert!(link + secondary > 0, "frame {frame} has no checked link or secondary button");
    }

    // Settings pages 01 Mobile: every "View integration" button is link text with no fill, stroke or shadow.
    let mobile = doc.nodes.values().find(|n| n.source_id.as_deref() == Some("1685:415210")).expect("Settings pages 01 Mobile").id;
    let mut view_integration = 0usize;
    for n in subtree(&doc, mobile) {
        let NodeKind::Text(text) = &n.kind else { continue };
        if text.content != "View integration" {
            continue;
        }
        let mut cur = n.parent;
        while let Some(pid) = cur {
            let p = doc.get(pid).unwrap();
            if matches!(p.kind, NodeKind::Instance { .. }) && p.name.contains("Button") {
                view_integration += 1;
                let comp = doc.get(instance_component(p).expect("Button component")).unwrap();
                assert_eq!(button_hierarchy(&comp.name), Some("Link color"), "component {:?}", comp.name);
                assert!(p.fills.is_empty() && p.stroke.paints.is_empty() && p.effects.is_empty(), "View integration button kept a filled shell");
                break;
            }
            cur = p.parent;
        }
    }
    assert_eq!(view_integration, 9, "View integration link buttons in 1685:415210");
}

#[test]
#[ignore = "needs the private Untitled UI .fig"]
fn untitled_ui_instance_text_overrides_resolve() {
    let bytes = reference("untitled-ui.fig")
        .or_else(|| std::fs::read("/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig").ok());
    let Some(bytes) = bytes else {
        eprintln!("untitled-ui.fig not found: skipped");
        return;
    };
    let mut doc = import_fig(&bytes).expect("import");
    for page in doc.pages.clone() {
        doc.ensure_page_hydrated(page);
    }

    for (inst_src, expected_snip, expect_runs) in [
        ("3936:417924", "For apps and websites with a lot of user-generated content", false),
        ("3936:417929", "Things go wrong. When they do, it can be a frustrating experience", false),
        ("11185:8546", "Your brand color and base white and black styles.", false),
        ("9352:550138", "This component has been moved to Base components -> Progress indicators.", true),
    ] {
        let inst = doc.nodes.values().find(|n| n.source_id.as_deref() == Some(inst_src)).unwrap_or_else(|| panic!("missing instance {inst_src}"));
        let texts: Vec<&TextProps> = subtree(&doc, inst.id)
            .into_iter()
            .filter_map(|n| match &n.kind {
                NodeKind::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().all(|t| !t.content.starts_with("Lorem ipsum")),
            "instance {inst_src} still has un-overridden Lorem ipsum: {:?}",
            texts.iter().map(|t| &t.content).collect::<Vec<_>>()
        );
        let body = texts.iter().find(|t| t.content.contains(expected_snip)).unwrap_or_else(|| {
            panic!("instance {inst_src} missing expected text {expected_snip:?}; found: {:?}", texts.iter().map(|t| &t.content).collect::<Vec<_>>())
        });
        if expect_runs {
            assert!(!body.runs.is_empty(), "instance {inst_src} should preserve mixed-style runs");
            assert!(
                body.runs.iter().any(|r| r.style.font_weight >= 600),
                "instance {inst_src} should have bold/semibold run weights: {:?}",
                body.runs.iter().map(|r| r.style.font_weight).collect::<Vec<_>>()
            );
        }
    }

    // Across the entire file, instances of Design note (2680:401975) and Figma Variables note (5263:372534)
    // that override their supporting text resolve to the overridden copy instead of falling back to Lorem ipsum.
    let note_comp_ids: Vec<NodeId> =
        doc.nodes.values().filter(|n| matches!(n.source_id.as_deref(), Some("2680:401975" | "5263:372534"))).map(|n| n.id).collect();
    assert_eq!(note_comp_ids.len(), 2);
    let mut overridden_notes = 0usize;
    for n in doc.nodes.values() {
        if let NodeKind::Instance { component: Some(c), .. } = n.kind {
            if note_comp_ids.contains(&c) {
                let has_overridden_body = subtree(&doc, n.id).into_iter().any(|sub| {
                    matches!(&sub.kind, NodeKind::Text(t) if sub.component.overrides.contains(&OverrideField::Characters) && !t.content.starts_with("Lorem ipsum"))
                });
                if has_overridden_body {
                    overridden_notes += 1;
                }
            }
        }
    }
    assert!(overridden_notes >= 40, "expected >= 40 overridden note instances, found {overridden_notes}");
}

#[test]
#[ignore = "needs the private reference/untitled-ui.fig"]
fn untitled_ui_pie_chart_resized_instances_match() {
    let Some(bytes) = reference("untitled-ui.fig").or_else(|| reference("❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig")) else {
        eprintln!("untitled-ui.fig not found: skipped");
        return;
    };
    let mut doc = import_fig(&bytes).expect("import untitled-ui.fig");
    for page in doc.pages.clone() {
        doc.ensure_page_hydrated(page);
    }
    let pie_set = doc.nodes.values().find(|n| n.source_id.as_deref() == Some("1084:114")).expect("Pie chart component set (1084:114)").id;
    let mut count = 0usize;
    for n in subtree(&doc, pie_set) {
        if matches!(n.kind, NodeKind::Instance { .. }) && n.name == "_Pie chart" {
            count += 1;
            let inst_sz = n.size;
            assert_eq!(n.children.len(), 7, "each _Pie chart instance has 7 arc children");
            for &cid in &n.children {
                let child = doc.get(cid).expect("arc child");
                assert!(
                    (child.size.x - inst_sz.x).abs() < 1e-2 && (child.size.y - inst_sz.y).abs() < 1e-2,
                    "child {:?} of {:?}-sized _Pie chart has size {:?}",
                    child.name,
                    inst_sz,
                    child.size
                );
                let center = child.transform.apply(Vec2::new(child.size.x * 0.5, child.size.y * 0.5));
                assert!(
                    (center.x - inst_sz.x * 0.5).abs() < 1e-2 && (center.y - inst_sz.y * 0.5).abs() < 1e-2,
                    "child {:?} of {:?}-sized _Pie chart is off-centre: center={:?}",
                    child.name,
                    inst_sz,
                    center
                );
            }
        }
    }
    assert_eq!(count, 10, "expected all 10 _Pie chart instances across the 10 variants of 1084:114");
}
