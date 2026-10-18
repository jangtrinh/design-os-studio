//! Slot parameters shared by the React, Tailwind, SwiftUI and Compose generators.
//!
//! A slot is a container whose content a caller supplies. On a component definition every slot
//! becomes its own parameter, and the slot's own children are the default used when the caller
//! passes nothing; they are never rendered beside the parameter. An instance routes the content
//! of each slot to that slot's parameter.

use crate::codegen::*;
use designos_doc::*;

/// Target language, for identifiers that must not collide with its reserved words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lang {
    React,
    Swift,
    Kotlin,
}

const JS_RESERVED: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "key",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "ref",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// `body` is the required member of `View`; the rest are Swift keywords.
const SWIFT_RESERVED: &[&str] = &[
    "Any",
    "Protocol",
    "Self",
    "Type",
    "as",
    "associatedtype",
    "await",
    "body",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "let",
    "nil",
    "open",
    "operator",
    "private",
    "protocol",
    "public",
    "repeat",
    "rethrows",
    "return",
    "self",
    "static",
    "struct",
    "subscript",
    "super",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "var",
    "where",
    "while",
];

/// `modifier` is the parameter every generated Composable call already uses; the rest are Kotlin hard keywords.
const KOTLIN_RESERVED: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "modifier",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
];

/// `name` as an identifier of `lang`: reserved words and required members get a `Slot` suffix.
pub(crate) fn slot_ident(name: &str, lang: Lang) -> String {
    let reserved = match lang {
        Lang::React => JS_RESERVED,
        Lang::Swift => SWIFT_RESERVED,
        Lang::Kotlin => KOTLIN_RESERVED,
    };
    if reserved.contains(&name) {
        format!("{name}Slot")
    } else {
        name.to_string()
    }
}

/// Whether `id` is a slot container node (`is_slot` or `is_slot_container`).
fn is_slot_node(doc: &Document, id: NodeId) -> bool {
    doc.get(id).is_some_and(|n| n.component.is_slot || is_slot_container(doc, id))
}

fn clean_prop_name(raw: &str) -> &str {
    raw.split('#').next().unwrap_or(raw).trim()
}

fn bound_slot_name(n: &Node) -> Option<String> {
    let c = camel(clean_prop_name(n.component.bindings.get(&OverrideField::SlotContent)?));
    (!c.is_empty()).then_some(c)
}

/// The camelCase name of the slot that container `id` fills.
fn slot_param_name(doc: &Document, id: NodeId) -> Option<String> {
    if !is_slot_node(doc, id) {
        return None;
    }
    let n = doc.get(id)?;
    if let Some(c) = bound_slot_name(n) {
        return Some(c);
    }
    if let Some(c) = n.component.source.and_then(|s| doc.get(s)).and_then(bound_slot_name) {
        return Some(c);
    }
    let comp = enclosing_component(doc, id).or_else(|| n.component.source.and_then(|s| enclosing_component(doc, s)));
    if let Some(comp) = comp {
        let defs: Vec<ComponentPropDef> = prop_defs(doc, comp).into_iter().filter(|d| d.kind == PropKind::Slot).collect();
        let by_name = defs.iter().find(|d| clean_prop_name(&d.name).eq_ignore_ascii_case(&n.name));
        if let Some(d) = by_name.or_else(|| (defs.len() == 1).then(|| &defs[0])) {
            let c = camel(clean_prop_name(&d.name));
            if !c.is_empty() {
                return Some(c);
            }
        }
    }
    let c = camel(&n.name);
    Some(if c.is_empty() { "children".into() } else { c })
}

/// One slot of a component: its camelCase name and the first container that renders it.
pub(crate) struct SlotInfo {
    pub name: String,
    pub container: Option<NodeId>,
}

fn note_slot(out: &mut Vec<SlotInfo>, name: String, container: Option<NodeId>) {
    if name.is_empty() {
        return;
    }
    match out.iter_mut().find(|s| s.name == name) {
        Some(s) => {
            if s.container.is_none() {
                s.container = container;
            }
        }
        None => out.push(SlotInfo { name, container }),
    }
}

/// The slots of the component definition `root`, in declaration order. Empty for an instance.
pub(crate) fn slot_infos(doc: &Document, root: NodeId) -> Vec<SlotInfo> {
    let Some(n) = doc.get(root) else { return Vec::new() };
    if matches!(n.kind, NodeKind::Instance { .. }) {
        return Vec::new();
    }
    let mut out: Vec<SlotInfo> = Vec::new();
    if matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
        for d in prop_defs(doc, root) {
            if d.kind == PropKind::Slot {
                note_slot(&mut out, camel(clean_prop_name(&d.name)), None);
            }
        }
    }
    fn walk(doc: &Document, root: NodeId, id: NodeId, out: &mut Vec<SlotInfo>) {
        let Some(n) = doc.get(id) else { return };
        if !n.visible || (id != root && matches!(n.kind, NodeKind::Instance { .. })) {
            return;
        }
        if let Some(name) = slot_param_name(doc, id) {
            note_slot(out, name, Some(id));
        }
        for c in &n.children {
            walk(doc, root, *c, out);
        }
    }
    walk(doc, root, root, &mut out);
    out
}

fn has_default_content(doc: &Document, info: &SlotInfo) -> bool {
    info.container.and_then(|c| doc.get(c)).is_some_and(|c| !visible_children(doc, c).is_empty())
}

/// A slot container on a component definition.
pub(crate) struct SlotSite {
    /// camelCase slot name.
    pub name: String,
    /// The component has exactly one slot.
    pub single: bool,
    /// The container's own visible children: the content used when the caller passes none.
    pub defaults: Vec<NodeId>,
}

/// The slot that container `id` renders, or `None` when `id` is not a slot container or is a
/// copy inside an instance (those render as ordinary layers).
pub(crate) fn slot_site(doc: &Document, id: NodeId) -> Option<SlotSite> {
    if instance_root_of(doc, id).is_some() {
        return None;
    }
    let name = slot_param_name(doc, id)?;
    let n = doc.get(id)?;
    let count = enclosing_component(doc, id).map(|c| slot_infos(doc, c).len()).unwrap_or(1);
    Some(SlotSite { name, single: count == 1, defaults: visible_children(doc, n).iter().map(|c| c.id).collect() })
}

/// A non-slot component property, as a parameter of a generated slot component.
pub(crate) struct PropParam {
    /// camelCase name, as `instance_args` passes it.
    pub name: String,
    pub default: PropDefault,
}

pub(crate) enum PropDefault {
    Bool(bool),
    /// Text, or the selected option of a variant property.
    Text(String),
    /// Swapped-in component: its view name, if the default names one.
    Instance(Option<String>),
}

/// The non-slot properties of component definition `root` with their defaults, in declaration
/// order. A generated component with slots declares these so every argument an instance passes
/// resolves. Empty for anything but a component or component set.
pub(crate) fn prop_params(doc: &Document, root: NodeId) -> Vec<PropParam> {
    let Some(n) = doc.get(root) else { return Vec::new() };
    if !matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) {
        return Vec::new();
    }
    let mut out: Vec<PropParam> = Vec::new();
    for def in prop_defs(doc, root) {
        let default = match (&def.kind, &def.default) {
            (PropKind::Slot, _) => continue,
            (_, PropValue::Bool(b)) => PropDefault::Bool(*b),
            (_, PropValue::Text(t) | PropValue::Variant(t)) => PropDefault::Text(t.clone()),
            (_, PropValue::Instance(c)) => PropDefault::Instance(doc.get(*c).map(|c| pascal(&c.name))),
            (PropKind::Boolean, _) => PropDefault::Bool(false),
            (PropKind::Variant { options }, _) => PropDefault::Text(options.first().cloned().unwrap_or_default()),
            (PropKind::InstanceSwap { .. }, _) => PropDefault::Instance(None),
            _ => PropDefault::Text(String::new()),
        };
        let name = camel(clean_prop_name(&def.name));
        if !name.is_empty() && !out.iter().any(|p| p.name == name) {
            out.push(PropParam { name, default });
        }
    }
    out
}

/// Content an instance passes to one slot. An empty `ids` suppresses the slot's default content.
pub(crate) struct SlotFill {
    pub name: String,
    pub ids: Vec<NodeId>,
}

/// What an instance passes to its component's slots.
pub(crate) struct InstanceSlots {
    /// Every slot of the component, in declaration order.
    pub component: Vec<SlotInfo>,
    /// The slots that need an argument, in declaration order.
    pub fills: Vec<SlotFill>,
}

impl InstanceSlots {
    /// The component has exactly one slot.
    pub fn single(&self) -> bool {
        self.component.len() == 1
    }
}

/// Groups the visible content of instance `n` by slot. A slot with no content still gets an
/// (empty) fill when its component gives it default content, so the instance stays empty there.
pub(crate) fn instance_slots(doc: &Document, n: &Node) -> InstanceSlots {
    let Some(comp) = instance_component(n) else { return InstanceSlots { component: Vec::new(), fills: Vec::new() } };
    let mut component = slot_infos(doc, comp);
    let mut pairs: Vec<(String, NodeId)> = Vec::new();
    for def in prop_defs(doc, comp) {
        if def.kind == PropKind::Slot {
            if let PropValue::Slot(ids) = prop_value(doc, n.id, &def) {
                let name = camel(clean_prop_name(&def.name));
                pairs.extend(ids.into_iter().map(|id| (name.clone(), id)));
            }
        }
    }
    fn walk(doc: &Document, root: NodeId, id: NodeId, pairs: &mut Vec<(String, NodeId)>) {
        let Some(node) = doc.get(id) else { return };
        if !node.visible || (id != root && matches!(node.kind, NodeKind::Instance { .. })) {
            return;
        }
        if id != root {
            if let Some(name) = slot_param_name(doc, id) {
                pairs.extend(node.children.iter().map(|c| (name.clone(), *c)));
                return;
            }
        }
        for &c in &node.children {
            walk(doc, root, c, pairs);
        }
    }
    walk(doc, n.id, n.id, &mut pairs);

    let mut seen: Vec<NodeId> = Vec::new();
    let mut groups: Vec<(String, Vec<NodeId>)> = Vec::new();
    for (name, id) in pairs {
        if !doc.get(id).is_some_and(|c| c.visible) || seen.contains(&id) {
            continue;
        }
        seen.push(id);
        match groups.iter_mut().find(|g| g.0 == name) {
            Some(g) => g.1.push(id),
            None => groups.push((name, vec![id])),
        }
    }

    let mut fills: Vec<SlotFill> = Vec::new();
    for info in &component {
        let ids = groups.iter().position(|g| g.0 == info.name).map(|i| groups.remove(i).1).unwrap_or_default();
        if !ids.is_empty() || has_default_content(doc, info) {
            fills.push(SlotFill { name: info.name.clone(), ids });
        }
    }
    for (name, ids) in groups {
        component.push(SlotInfo { name: name.clone(), container: None });
        fills.push(SlotFill { name, ids });
    }
    InstanceSlots { component, fills }
}

/// React prop that carries slot `name`: `children` for the only slot or a slot named children.
pub(crate) fn react_prop(name: &str, single: bool) -> String {
    if single || name == "children" {
        "children".into()
    } else {
        slot_ident(name, Lang::React)
    }
}

/// React function header for `n`, with one `React.ReactNode` prop per slot.
pub(crate) fn react_slot_signature(doc: &Document, n: &Node) -> String {
    let name = view_name(doc, n);
    let infos = slot_infos(doc, n.id);
    if infos.is_empty() {
        return format!("export function {name}() {{");
    }
    let single = enclosing_component(doc, n.id).map(|c| slot_infos(doc, c).len()).unwrap_or(infos.len()) == 1;
    let mut props: Vec<String> = Vec::new();
    for i in &infos {
        let p = react_prop(&i.name, single);
        if !props.contains(&p) {
            props.push(p);
        }
    }
    let types: Vec<String> = props.iter().map(|p| format!("{p}?: React.ReactNode")).collect();
    format!("export function {name}({{ {} }}: {{ {} }}) {{", props.join(", "), types.join("; "))
}

/// Renders layer `id` at `depth`.
pub(crate) type RenderNode<'a> = &'a dyn Fn(&mut Out, NodeId, usize);

/// JSX for a slot on a definition: `{prop}`, or `{prop ?? (<default content/>)}` when the slot has default content.
pub(crate) fn emit_react_slot(o: &mut Out, depth: usize, site: &SlotSite, render: RenderNode) {
    let prop = react_prop(&site.name, site.single);
    if site.defaults.is_empty() {
        o.line(depth, format!("{{{prop}}}"));
        return;
    }
    o.line(depth, format!("{{{prop} ?? ("));
    emit_fragment(o, depth + 1, &site.defaults, render);
    o.line(depth, ")}");
}

/// One element, or several wrapped in a fragment (an empty list is an empty fragment).
fn emit_fragment(o: &mut Out, depth: usize, ids: &[NodeId], render: RenderNode) {
    match ids {
        [] => o.line(depth, "<></>"),
        [one] => render(o, *one, depth),
        many => {
            o.line(depth, "<>");
            for id in many {
                render(o, *id, depth + 1);
            }
            o.line(depth, "</>");
        }
    }
}

/// What an instance passes to a React component: the `children` content and named props.
pub(crate) struct ReactFills {
    pub children: Option<Vec<NodeId>>,
    pub props: Vec<(String, Vec<NodeId>)>,
}

impl ReactFills {
    pub fn is_empty(&self) -> bool {
        self.children.is_none() && self.props.is_empty()
    }
}

pub(crate) fn react_fills(doc: &Document, n: &Node) -> ReactFills {
    let slots = instance_slots(doc, n);
    let single = slots.single();
    let mut out = ReactFills { children: None, props: Vec::new() };
    for f in slots.fills {
        let prop = react_prop(&f.name, single);
        if prop == "children" {
            out.children = Some(f.ids);
        } else {
            out.props.push((prop, f.ids));
        }
    }
    out
}

/// Writes an instance element: `head` (indent offset, text) lines of the opening tag without its
/// closing `>`/`/>`, then one `prop={...}` attribute per named slot, then the `children` content.
pub(crate) fn emit_jsx_instance(o: &mut Out, depth: usize, tag: &str, head: &[(usize, String)], fills: &ReactFills, render: RenderNode) {
    let close = if fills.children.is_some() { ">" } else { "/>" };
    if head.len() == 1 && fills.props.is_empty() {
        o.line(depth, format!("{}{close}", head[0].1));
    } else {
        for (off, text) in head {
            o.line(depth + off, text);
        }
        for (prop, ids) in &fills.props {
            o.line(depth + 1, format!("{prop}={{"));
            emit_fragment(o, depth + 2, ids, render);
            o.line(depth + 1, "}");
        }
        o.line(depth, close);
    }
    if let Some(ids) = &fills.children {
        emit_fragment_children(o, depth + 1, ids, render);
        o.line(depth, format!("</{tag}>"));
    }
}

/// Children of an element: each layer in turn, or an empty fragment when the content must stay empty.
fn emit_fragment_children(o: &mut Out, depth: usize, ids: &[NodeId], render: RenderNode) {
    if ids.is_empty() {
        o.line(depth, "<></>");
    }
    for id in ids {
        render(o, *id, depth);
    }
}
