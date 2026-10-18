//! Slots in Dev Mode code: every slot of a component is its own parameter, the slot's own layers
//! are that parameter's default (never rendered beside it), an instance routes each slot's
//! content to the slot's own parameter, and the generated identifiers never collide with a
//! reserved word or a required member of the target language.

use designos_doc::*;
use designos_export::{compose_for, react_for, react_with, swiftui_for, tailwind_for, ReactOptions};

fn text(d: &mut Document, parent: NodeId, content: &str) -> NodeId {
    let id = d.alloc_id();
    let style = TextStyle { font_family: "Inter".into(), font_size: 14.0, fills: vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))], ..Default::default() };
    let mut t = Node::text(id, Rect::new(0.0, 0.0, 120.0, 20.0), content, style);
    t.name = content.into();
    d.insert(parent, t).unwrap()
}

fn vertical(spacing: f32) -> AutoLayout {
    AutoLayout { direction: LayoutDirection::Vertical, item_spacing: spacing, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Fixed, ..Default::default() }
}

/// A `Dialog` component with one slot frame per `(name, default text)`, and an instance of it.
/// Every slot of the instance holds a copy of its default text.
fn dialog(slots: &[(&str, Option<&str>)]) -> (Document, NodeId, NodeId) {
    let mut d = Document::new("Slots");
    let page = d.add_page("Page 1");
    let comp_id = d.alloc_id();
    let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp.name = "Dialog".into();
    comp.size = Vec2::new(240.0, 160.0);
    if let NodeKind::Component(f) = &mut comp.kind {
        f.auto_layout = Some(AutoLayout { padding: [12.0; 4], primary_sizing: Sizing::Fixed, ..vertical(8.0) });
    }
    for (i, (name, _)) in slots.iter().enumerate() {
        comp.component.props.push(ComponentPropDef { name: format!("{name}#1:{i}"), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    }
    let comp_id = d.insert(page, comp).unwrap();
    for (i, (name, default)) in slots.iter().enumerate() {
        let id = d.alloc_id();
        let mut slot = Node::frame(id, Rect::new(0.0, 0.0, 216.0, 48.0));
        slot.name = (*name).into();
        slot.component.is_slot = true;
        slot.component.bindings.insert(OverrideField::SlotContent, format!("{name}#1:{i}"));
        if let NodeKind::Frame(f) = &mut slot.kind {
            f.auto_layout = Some(vertical(4.0));
        }
        let slot_id = d.insert(comp_id, slot).unwrap();
        if let Some(t) = default {
            text(&mut d, slot_id, t);
        }
    }
    create_instance(&mut d, comp_id, page, Vec2::new(300.0, 0.0)).apply(&mut d).unwrap();
    let inst = instances_of(&d, comp_id)[0];
    (d, comp_id, inst)
}

/// Replaces what the instance holds in slot number `index` with `texts` (possibly none).
fn fill(d: &mut Document, inst: NodeId, index: usize, prop: &str, texts: &[&str]) {
    let copy = d.get(inst).unwrap().children[index];
    for c in d.get(copy).unwrap().children.clone() {
        d.remove(c).unwrap();
    }
    let ids: Vec<NodeId> = texts
        .iter()
        .map(|t| {
            let id = text(d, copy, t);
            d.get_mut(id).unwrap().component.is_slot_content = true;
            id
        })
        .collect();
    d.get_mut(inst).unwrap().component.values.insert(prop.into(), PropValue::Slot(ids));
}

fn two_slot_dialog() -> (Document, NodeId, NodeId) {
    let (mut d, comp, inst) = dialog(&[("Header", Some("Title")), ("Body", None)]);
    fill(&mut d, inst, 0, "Header#1:0", &["Heading"]);
    fill(&mut d, inst, 1, "Body#1:1", &["Body copy"]);
    (d, comp, inst)
}

fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// Asserts `needles` occur in `hay` in the given order.
fn in_order(hay: &str, needles: &[&str]) {
    let mut at = 0;
    for n in needles {
        match hay[at..].find(n) {
            Some(i) => at += i + n.len(),
            None => panic!("`{n}` missing or out of order after byte {at} in:\n{hay}"),
        }
    }
}

#[test]
fn react_definition_has_one_prop_per_slot_and_defaults_only_as_fallback() {
    let (d, comp, _) = two_slot_dialog();
    for out in [react_for(&d, comp), react_with(&d, comp, &ReactOptions { tailwind: true })] {
        assert!(out.contains("export function Dialog({ header, body }: { header?: React.ReactNode; body?: React.ReactNode }) {"), "{out}");
        assert!(!out.contains("children"), "no shared children fallback:\n{out}");
        in_order(&out, &["{header ?? (", "Title", ")}", "{body}"]);
        assert_eq!(count(&out, "Title"), 1, "default content renders once, inside the fallback:\n{out}");
        assert_eq!(count(&out, "{header"), 1, "{out}");
        assert_eq!(count(&out, "{body"), 1, "{out}");
    }
}

#[test]
fn react_instance_routes_content_per_slot() {
    let (d, _, inst) = two_slot_dialog();
    for out in [react_for(&d, inst), react_with(&d, inst, &ReactOptions { tailwind: true })] {
        in_order(&out, &["<Dialog", "header={", "Heading", "}", "body={", "Body copy", "}", "/>"]);
        assert!(!out.contains("</Dialog>"), "named slots leave no children list:\n{out}");
        assert_eq!(count(&out, "Heading"), 1, "{out}");
        assert_eq!(count(&out, "Body copy"), 1, "{out}");
        assert!(!out.contains("Title"), "{out}");
    }
}

#[test]
fn react_instance_with_emptied_slot_suppresses_the_default() {
    let (mut d, _, inst) = dialog(&[("Header", Some("Title")), ("Body", None)]);
    fill(&mut d, inst, 0, "Header#1:0", &[]);
    fill(&mut d, inst, 1, "Body#1:1", &["Body copy"]);
    let out = react_for(&d, inst);
    in_order(&out, &["header={", "<></>", "}", "body={", "Body copy", "}"]);
}

#[test]
fn single_slot_uses_children_in_react() {
    let (mut d, comp, inst) = dialog(&[("Body", Some("Title"))]);
    fill(&mut d, inst, 0, "Body#1:0", &["Slot content"]);
    let def = react_for(&d, comp);
    assert!(def.contains("export function Dialog({ children }: { children?: React.ReactNode }) {"), "{def}");
    in_order(&def, &["{children ?? (", "Title", ")}"]);
    assert_eq!(count(&def, "Title"), 1, "{def}");
    let out = react_for(&d, inst);
    in_order(&out, &["<Dialog", ">", "Slot content", "</Dialog>"]);
}

#[test]
fn tailwind_html_keeps_default_content_once_and_routes_named_slots() {
    let (d, comp, inst) = two_slot_dialog();
    let def = tailwind_for(&d, comp);
    in_order(&def, &["<!-- slot: header -->", "Title", "<!-- slot: body -->"]);
    assert_eq!(count(&def, "Title"), 1, "{def}");
    let out = tailwind_for(&d, inst);
    in_order(&out, &["<Dialog", "<template slot=\"header\">", "Heading", "</template>", "<template slot=\"body\">", "Body copy", "</template>", "</Dialog>"]);
}

#[test]
fn swiftui_definition_is_a_generic_view_with_a_parameter_per_slot() {
    let (d, comp, _) = two_slot_dialog();
    let out = swiftui_for(&d, comp);
    assert!(out.contains("struct Dialog<HeaderContent: View, BodyContent: View>: View {"), "{out}");
    assert!(out.contains("@ViewBuilder header: () -> HeaderContent"), "{out}");
    assert!(out.contains("@ViewBuilder bodySlot: () -> BodyContent"), "{out}");
    assert!(!out.contains("body()"), "a slot never becomes a call of View.body:\n{out}");
    assert!(!out.contains("@ViewBuilder var"), "{out}");
    in_order(&out, &["if let header {", "header", "} else {", "Text(\"Title\")", "}", "if let bodySlot {", "bodySlot", "}"]);
    assert_eq!(count(&out, "Text(\"Title\")"), 1, "{out}");
    assert_eq!(count(&out, "var body: some View"), 1, "{out}");
    assert!(out.contains("extension Dialog where HeaderContent == EmptyView, BodyContent == EmptyView {"), "{out}");
}

#[test]
fn swiftui_instance_passes_a_closure_per_slot() {
    let (d, _, inst) = two_slot_dialog();
    let out = swiftui_for(&d, inst);
    in_order(&out, &["Dialog(", "header: {", "Text(\"Heading\")", "},", "bodySlot: {", "Text(\"Body copy\")", "}", ")"]);
    assert!(!out.contains("Title"), "{out}");
}

#[test]
fn swiftui_instance_leaves_an_omitted_slot_empty() {
    let (mut d, _, inst) = dialog(&[("Header", Some("Title")), ("Body", None)]);
    fill(&mut d, inst, 0, "Header#1:0", &[]);
    fill(&mut d, inst, 1, "Body#1:1", &["Body copy"]);
    let out = swiftui_for(&d, inst);
    in_order(&out, &["header: {", "EmptyView()", "},", "bodySlot: {", "Text(\"Body copy\")", "}"]);
}

#[test]
fn single_slot_uses_a_trailing_closure_in_swiftui() {
    let (mut d, comp, inst) = dialog(&[("Content", None)]);
    fill(&mut d, inst, 0, "Content#1:0", &["Slot content"]);
    let def = swiftui_for(&d, comp);
    assert!(def.contains("struct Dialog<ContentContent: View>: View {"), "{def}");
    assert!(def.contains("@ViewBuilder content: () -> ContentContent"), "{def}");
    let out = swiftui_for(&d, inst);
    in_order(&out, &["Dialog {", "Text(\"Slot content\")", "}"]);
}

#[test]
fn compose_definition_has_a_lambda_per_slot_with_the_default_content() {
    let (d, comp, _) = two_slot_dialog();
    let out = compose_for(&d, comp);
    in_order(
        &out,
        &[
            "@Composable",
            "fun Dialog(",
            "header: @Composable () -> Unit = {",
            "Text(",
            "\"Title\"",
            "},",
            "body: @Composable () -> Unit = {},",
            ") {",
            "header()",
            "body()",
        ],
    );
    assert_eq!(count(&out, "\"Title\""), 1, "default content renders once, as the parameter default:\n{out}");
    assert_eq!(count(&out, "header()"), 1, "{out}");
    assert_eq!(count(&out, "body()"), 1, "{out}");
}

#[test]
fn compose_instance_passes_a_named_lambda_per_slot() {
    let (d, _, inst) = two_slot_dialog();
    let out = compose_for(&d, inst);
    in_order(&out, &["Dialog(", "header = {", "\"Heading\"", "}", "body = {", "\"Body copy\"", "}", ")"]);
    assert!(!out.contains("Title"), "{out}");
}

#[test]
fn compose_instance_with_emptied_slot_suppresses_the_default() {
    let (mut d, _, inst) = dialog(&[("Header", Some("Title")), ("Body", None)]);
    fill(&mut d, inst, 0, "Header#1:0", &[]);
    fill(&mut d, inst, 1, "Body#1:1", &["Body copy"]);
    let out = compose_for(&d, inst);
    in_order(&out, &["header = {}", "body = {", "\"Body copy\"", "}"]);
}

#[test]
fn single_slot_uses_the_trailing_lambda_in_compose() {
    let (mut d, _, inst) = dialog(&[("Body", None)]);
    fill(&mut d, inst, 0, "Body#1:0", &["Slot content"]);
    let out = compose_for(&d, inst);
    in_order(&out, &["Dialog", ") {", "\"Slot content\"", "}"]);
}

#[test]
fn reserved_slot_names_are_renamed_in_every_language() {
    // `Body` is View.body in SwiftUI, `Class` and `Default` are JS keywords, `Modifier` is the
    // parameter every Composable call already takes.
    let (mut d, comp, inst) = dialog(&[("Body", Some("A")), ("Class", Some("B")), ("Modifier", Some("C"))]);
    fill(&mut d, inst, 0, "Body#1:0", &["x"]);
    fill(&mut d, inst, 1, "Class#1:1", &["y"]);
    fill(&mut d, inst, 2, "Modifier#1:2", &["z"]);

    let swift = swiftui_for(&d, comp);
    assert!(swift.contains("bodySlot"), "{swift}");
    assert!(swift.contains("var body: some View"), "{swift}");
    assert!(!swift.contains("body()"), "{swift}");
    assert!(swiftui_for(&d, inst).contains("bodySlot: {"));

    let react = react_for(&d, comp);
    assert!(react.contains("{ body, classSlot, modifier }"), "{react}");
    assert!(react_for(&d, inst).contains("classSlot={"));

    let kotlin = compose_for(&d, comp);
    assert!(kotlin.contains("modifierSlot: @Composable () -> Unit"), "{kotlin}");
    assert!(kotlin.contains("modifierSlot()"), "{kotlin}");
    assert!(compose_for(&d, inst).contains("modifierSlot = {"));
}

/// Adds a text, a boolean, a variant and an instance-swap property to `comp` and sets them on `inst`.
fn add_props(d: &mut Document, comp: NodeId, inst: NodeId) {
    let page = d.get(comp).unwrap().parent.unwrap();
    let mut icons = Vec::new();
    for name in ["Star", "Heart"] {
        let id = d.alloc_id();
        let mut c = Node::new(id, NodeKind::Component(FrameProps::default()));
        c.name = name.into();
        c.size = Vec2::new(16.0, 16.0);
        icons.push(d.insert(page, c).unwrap());
    }
    let defs = [
        ("Label#2:0", PropKind::Text, PropValue::Text("Save".into())),
        ("Disabled#2:1", PropKind::Boolean, PropValue::Bool(false)),
        ("Size#2:2", PropKind::Variant { options: vec!["S".into(), "L".into()] }, PropValue::Variant("S".into())),
        ("Icon#2:3", PropKind::InstanceSwap { preferred: vec![] }, PropValue::Instance(icons[0])),
    ];
    for (name, kind, default) in defs {
        d.get_mut(comp).unwrap().component.props.push(ComponentPropDef { name: name.into(), kind, default });
    }
    let values = &mut d.get_mut(inst).unwrap().component.values;
    values.insert("Label#2:0".into(), PropValue::Text("Go".into()));
    values.insert("Disabled#2:1".into(), PropValue::Bool(true));
    values.insert("Size#2:2".into(), PropValue::Variant("L".into()));
    values.insert("Icon#2:3".into(), PropValue::Instance(icons[1]));
}

/// Names of the named arguments the instance call passes at its own nesting level (`name = ...`).
fn kotlin_call_args(call: &str) -> Vec<String> {
    call.lines()
        .filter_map(|l| l.strip_prefix("    "))
        .filter(|l| !l.starts_with(' '))
        .filter_map(|l| l.split_once(" = ").map(|(n, _)| n.to_string()))
        .collect()
}

#[test]
fn compose_function_declares_every_argument_its_instance_passes() {
    let (mut d, comp, inst) = two_slot_dialog();
    add_props(&mut d, comp, inst);
    let def = compose_for(&d, comp);
    let call = compose_for(&d, inst);
    let args = kotlin_call_args(&call);
    for name in ["modifier", "label", "disabled", "size", "icon", "header", "body"] {
        assert!(args.contains(&name.to_string()), "instance passes {name}:\n{call}");
        assert!(def.contains(&format!("    {name}: ")), "definition declares {name}:\n{def}");
    }
    in_order(
        &def,
        &[
            "fun Dialog(",
            "modifier: Modifier = Modifier,",
            "label: String = \"Save\",",
            "disabled: Boolean = false,",
            "size: String = \"S\",",
            "icon: @Composable () -> Unit = { Star() },",
            "header: @Composable () -> Unit = {",
            "body: @Composable () -> Unit = {},",
            ") {",
        ],
    );
    // The passed modifier styles the root layout.
    assert_eq!(count(&def, "modifier = modifier"), 1, "{def}");
    let body = &def[def.find(") {").unwrap()..];
    assert!(body[body.find("modifier = ").unwrap()..].starts_with("modifier = modifier"), "the root's own modifier chains onto the parameter:\n{def}");
}

#[test]
fn compose_function_without_root_styling_still_applies_the_modifier() {
    let (d, comp, _) = dialog(&[("Body", None)]);
    let def = compose_for(&d, comp);
    assert!(def.contains("modifier: Modifier = Modifier,"), "{def}");
    assert!(def.contains("modifier = modifier"), "{def}");
}

#[test]
fn swiftui_init_declares_every_argument_its_instance_passes() {
    let (mut d, comp, inst) = two_slot_dialog();
    add_props(&mut d, comp, inst);
    let def = swiftui_for(&d, comp);
    let call = swiftui_for(&d, inst);
    in_order(&call, &["Dialog(", "label: \"Go\",", "disabled: true,", "size: \"L\",", "icon: AnyView(Heart()),", "header: {", "bodySlot: {", ")"]);
    in_order(
        &def,
        &[
            "init(label: String = \"Save\", disabled: Bool = false, size: String = \"S\", icon: AnyView = AnyView(Star()), @ViewBuilder header: () -> HeaderContent, @ViewBuilder bodySlot: () -> BodyContent) {",
            "extension Dialog where",
            "init(label: String = \"Save\", disabled: Bool = false, size: String = \"S\", icon: AnyView = AnyView(Star())) {",
        ],
    );
}

#[test]
fn react_slot_props_are_single_brace_jsx_expressions() {
    let (d, _, inst) = two_slot_dialog();
    for out in [react_for(&d, inst), react_with(&d, inst, &ReactOptions { tailwind: true })] {
        assert!(!out.contains("header={{") && !out.contains("body={{"), "{out}");
        assert!(out.contains("header={\n") && out.contains("body={\n"), "{out}");
    }
}

/// Parses the generated React (TSX) with esbuild. Needs `npx` and network on first use, so it is
/// opt-in: `cargo test -p designos-export -- --ignored`.
#[test]
#[ignore = "needs npx and esbuild"]
fn generated_react_parses() {
    use std::io::Write;
    use std::process::{Command as Proc, Stdio};
    let (mut d, comp, inst) = two_slot_dialog();
    add_props(&mut d, comp, inst);
    let mut failures = Vec::new();
    for (label, src) in [
        ("inline def", react_for(&d, comp)),
        ("inline inst", react_for(&d, inst)),
        ("tailwind def", react_with(&d, comp, &ReactOptions { tailwind: true })),
        ("tailwind inst", react_with(&d, inst, &ReactOptions { tailwind: true })),
    ] {
        let mut child = Proc::new("npx")
            .args(["--yes", "esbuild", "--loader=tsx"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("npx runs");
        child.stdin.take().unwrap().write_all(src.as_bytes()).unwrap();
        let out = child.wait_with_output().unwrap();
        if !out.status.success() {
            failures.push(format!("{label}:\n{src}\n{}", String::from_utf8_lossy(&out.stderr)));
        }
    }
    assert!(failures.is_empty(), "esbuild rejected:\n{}", failures.join("\n---\n"));
}

/// Type-checks the SwiftUI the generator writes for each slot shape: a component and an instance of it.
/// Needs `swiftc` and the macOS SDK, so it is opt-in: `cargo test -p designos-export -- --ignored`.
#[test]
#[ignore = "needs swiftc and the macOS SDK"]
fn generated_swiftui_type_checks() {
    use std::process::Command as Proc;
    let mut cases: Vec<(&str, (Document, NodeId, NodeId))> = vec![("two slots", two_slot_dialog())];
    let (mut d, comp, inst) = dialog(&[("Header", Some("Title")), ("Body", None)]);
    fill(&mut d, inst, 0, "Header#1:0", &[]);
    fill(&mut d, inst, 1, "Body#1:1", &["Body copy"]);
    cases.push(("emptied slot", (d, comp, inst)));
    let (mut d, comp, inst) = dialog(&[("Content", Some("Title"))]);
    fill(&mut d, inst, 0, "Content#1:0", &["Slot content"]);
    cases.push(("single slot", (d, comp, inst)));
    let (mut d, comp, inst) = dialog(&[("Body", Some("A")), ("Class", None), ("Self", None)]);
    fill(&mut d, inst, 0, "Body#1:0", &["x"]);
    cases.push(("reserved names", (d, comp, inst)));
    let (mut d, comp, inst) = two_slot_dialog();
    add_props(&mut d, comp, inst);
    cases.push(("props and slots", (d, comp, inst)));

    let dir = std::env::temp_dir().join(format!("designos-slot-swift-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    // The icon components the swap property refers to.
    const STUBS: &str = "struct Star: View { var body: some View { EmptyView() } }\nstruct Heart: View { var body: some View { EmptyView() } }";
    for (label, (d, comp, inst)) in &cases {
        let src = format!(
            "import SwiftUI\n\n{STUBS}\n\n{}\n\nstruct Screen: View {{\n    var body: some View {{\n{}\n    }}\n}}\n",
            swiftui_for(d, *comp),
            swiftui_for(d, *inst)
        );
        let file = dir.join("Slots.swift");
        std::fs::write(&file, &src).unwrap();
        let out = Proc::new("swiftc").arg("-typecheck").arg(&file).output().expect("swiftc runs");
        if !out.status.success() {
            failures.push(format!("{label}:\n{src}\n{}", String::from_utf8_lossy(&out.stderr)));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "swiftc rejected:\n{}", failures.join("\n---\n"));
}
