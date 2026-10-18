//! Component and instance editing glue: override recording, main-component
//! sync at commit time, and the selection commands (create, detach, swap,
//! variants, properties). Pure logic lives in `designos_doc::components`.

use crate::app::App;
use crate::canvas::create::container_at;
use crate::edit::Tx;
use crate::layout;
use designos_doc::*;
use std::collections::BTreeMap;

fn is_noop(c: &Command) -> bool {
    matches!(c, Command::Batch(v) if v.is_empty())
}

fn ids_of(doc: &Document, cmd: &Command, out: &mut Vec<NodeId>) {
    match cmd {
        Command::Insert { parent, nodes, .. } => {
            out.push(*parent);
            out.extend(nodes.iter().map(|n| n.id));
        }
        Command::Remove { id } => {
            out.push(*id);
            out.extend(doc.get(*id).and_then(|n| n.parent));
        }
        Command::Reparent { id, parent, .. } => {
            out.push(*id);
            out.push(*parent);
            out.extend(doc.get(*id).and_then(|n| n.parent));
        }
        Command::Update { node } => {
            out.push(node.id);
            out.extend(doc.get(node.id).and_then(|n| n.parent));
        }
        Command::Batch(v) => v.iter().for_each(|c| ids_of(doc, c, out)),
        Command::InsertPage { .. }
        | Command::RemovePage { .. }
        | Command::SetFlows { .. }
        | Command::SetVariables { .. }
        | Command::SetGuides { .. }
        | Command::SetPrototypeDevice { .. }
        | Command::SetComments { .. }
        | Command::SetLibraryState { .. }
        | Command::SetImages { .. }
        | Command::SetStyles { .. } => {}
    }
}

/// The component (or set) whose instances must follow an edit to `id`.
fn owner_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    match doc.get(id)?.kind {
        NodeKind::ComponentSet(_) => Some(id),
        _ => enclosing_component(doc, id),
    }
}

/// Flags the fields a user edit changed on nodes that mirror a main component,
/// so later syncs keep them. `before` holds the pre-edit state of touched nodes.
pub fn record_overrides(doc: &Document, before: &BTreeMap<NodeId, Node>, cmds: &mut [Command]) {
    for c in cmds.iter_mut() {
        let Command::Update { node } = c else { continue };
        let Some(old) = before.get(&node.id) else { continue };
        if !is_linked(node) || old.component.source != node.component.source {
            continue;
        }
        let mut ov = record_override(old, node);
        if is_top_instance(node) {
            // Moving, hiding or locking an instance is not an override of its main.
            for f in [OverrideField::Transform, OverrideField::Visible, OverrideField::Locked] {
                ov.remove(&f);
            }
        } else {
            // Constraint relayout after resizing the instance is not a user override.
            if let Some(root) = instance_root_of(doc, node.id) {
                if before.get(&root).zip(doc.get(root)).is_some_and(|(a, b)| a.size != b.size) {
                    ov.remove(&OverrideField::Size);
                    ov.remove(&OverrideField::Transform);
                }
            }
            // Positions and fitted sizes inside auto layout come from the layout pass.
            let parent_al = node.parent.and_then(|p| doc.get(p)).and_then(|p| p.kind.frame_props()).is_some_and(|f| f.auto_layout.is_some());
            if parent_al {
                ov.remove(&OverrideField::Transform);
                let derived = node.layout_child.horizontal != Sizing::Fixed
                    || node.layout_child.vertical != Sizing::Fixed
                    || matches!(&node.kind, NodeKind::Text(t) if t.auto_resize != TextAutoResize::None);
                if derived {
                    ov.remove(&OverrideField::Size);
                }
            } else if matches!(
                &node.kind,
                NodeKind::Text(t)
                    if t.auto_resize == TextAutoResize::WidthAndHeight
                        || (t.auto_resize == TextAutoResize::Height
                            && old.size.x == node.size.x
                            && node.size == crate::layout::fit_text(t, node.size))
            ) {
                ov.remove(&OverrideField::Size);
            }
        }
        let align_ov = record_grid_align_overrides(old, node);
        node.component.grid_align_overrides[0] |= align_ov[0];
        node.component.grid_align_overrides[1] |= align_ov[1];
        node.component.overrides.extend(ov);
    }
}

/// Wraps `cmd` so that editing a main component also re-syncs its instances in
/// the same undo step. Cascades through components nested in other components.
pub fn with_sync(doc: &mut Document, cmd: Command) -> Command {
    let mut ids = Vec::new();
    ids_of(doc, &cmd, &mut ids);
    let mut queue: Vec<NodeId> = ids.iter().filter_map(|i| owner_of(doc, *i)).collect();
    if queue.is_empty() {
        return cmd;
    }
    let Ok(inv) = cmd.clone().apply(doc) else { return cmd };
    queue.extend(ids.iter().filter_map(|i| owner_of(doc, *i)));
    queue.sort();
    queue.dedup();
    let mut syncs = Vec::new();
    let mut invs = Vec::new();
    let mut rounds = 0;
    while let Some(o) = queue.pop() {
        rounds += 1;
        if rounds > 16 {
            break;
        }
        let s = sync_instances(doc, o);
        if is_noop(&s) {
            continue;
        }
        let mut touched = Vec::new();
        ids_of(doc, &s, &mut touched);
        let Ok(i) = s.clone().apply(doc) else { continue };
        invs.push(i);
        syncs.push(s);
        for t in touched {
            if let Some(o2) = owner_of(doc, t) {
                if o2 != o && !queue.contains(&o2) {
                    queue.push(o2);
                }
            }
        }
    }
    for i in invs.into_iter().rev() {
        let _ = i.apply(doc);
    }
    let _ = inv.apply(doc);
    if syncs.is_empty() {
        cmd
    } else {
        Command::Batch(std::iter::once(cmd).chain(syncs).collect())
    }
}

/// One row of the Assets panel.
#[derive(Clone, Debug)]
pub struct AssetEntry {
    pub page: String,
    /// Frame or section the component sits in (empty when directly on the page).
    pub container: String,
    pub name: String,
    pub id: NodeId,
    /// Number of variants when the entry stands for a component set.
    pub variants: usize,
}

/// True if page point `p` lies inside `id`'s box (honouring rotation and parent transforms).
fn contains_page_point(doc: &Document, id: NodeId, p: Vec2) -> bool {
    let (Some(n), Some(inv)) = (doc.get(id), doc.world_transform(id).invert()) else { return false };
    let l = inv.apply(p);
    l.x >= 0.0 && l.y >= 0.0 && l.x <= n.size.x && l.y <= n.size.y
}

impl App {
    fn push_cmd(&mut self, tx: &mut Tx, cmd: Command) -> bool {
        if is_noop(&cmd) {
            return false;
        }
        match tx.cmd(&mut self.doc, cmd) {
            Ok(()) => true,
            Err(e) => {
                self.show_toast(format!("Edit failed: {e}"), true);
                false
            }
        }
    }

    /// Re-fits self-sizing texts below `root` (after a property changed their characters).
    fn refit_texts(&mut self, tx: &mut Tx, root: NodeId) {
        let mut ids = Vec::new();
        self.doc.walk(root, |n, _| {
            if matches!(&n.kind, NodeKind::Text(t) if t.auto_resize != TextAutoResize::None) {
                ids.push(n.id);
            }
        });
        for id in ids {
            let Some(n) = self.doc.get(id) else { continue };
            let NodeKind::Text(t) = &n.kind else { continue };
            let size = layout::fit_text(t, n.size);
            if size != n.size {
                if let Some(n) = tx.ed.node(&mut self.doc, id) {
                    n.size = size;
                }
                layout::relayout_around(&mut self.doc, &mut tx.ed, id);
            }
        }
    }

    /// Selected instance roots (a selected layer inside an instance counts as its instance).
    pub fn selected_instances(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        for id in self.top_selection() {
            let inst = match self.doc.get(id).map(|n| &n.kind) {
                Some(NodeKind::Instance { .. }) if self.doc.get(id).is_some_and(is_top_instance) => Some(id),
                _ => instance_root_of(&self.doc, id),
            };
            if let Some(i) = inst {
                if !out.contains(&i) {
                    out.push(i);
                }
            }
        }
        out
    }

    /// Component an instance was made from.
    pub fn main_of(&self, instance: NodeId) -> Option<NodeId> {
        self.doc.get(instance).and_then(instance_component).filter(|c| self.doc.get(*c).is_some())
    }

    /// Ctrl+Alt+K: turns the selection into main components.
    pub fn create_component(&mut self) {
        let tops = self.top_selection();
        let mut tx = Tx::new();
        let mut made = Vec::new();
        for id in tops {
            let cmd = create_component(&mut self.doc, id);
            if self.push_cmd(&mut tx, cmd) {
                made.push(id);
            }
        }
        if made.is_empty() {
            return;
        }
        self.run_tx(tx);
        self.selection = made.iter().filter_map(|id| self.component_for(*id)).collect();
        self.show_toast(if made.len() == 1 { "Created component" } else { "Created components" }, false);
    }

    /// The component a just-converted node became (itself, or its wrapper).
    fn component_for(&self, id: NodeId) -> Option<NodeId> {
        let n = self.doc.get(id)?;
        if matches!(n.kind, NodeKind::Component(_)) {
            return Some(id);
        }
        n.parent.filter(|p| matches!(self.doc.get(*p).map(|n| &n.kind), Some(NodeKind::Component(_))))
    }

    /// Ctrl+Alt+B: detaches the selected instances.
    pub fn detach_selection(&mut self) {
        let insts = self.selected_instances();
        let mut tx = Tx::new();
        let mut done = Vec::new();
        for i in insts {
            let cmd = detach_instance(&mut self.doc, i);
            if self.push_cmd(&mut tx, cmd) {
                done.push(i);
            }
        }
        if done.is_empty() {
            return;
        }
        self.run_tx(tx);
        self.selection = done;
    }

    /// Context menu "Combine as variants".
    pub fn combine_selection_as_variants(&mut self) {
        let tops = self.top_selection();
        if tops.is_empty() {
            return;
        }
        let mut tx = Tx::new();
        let mut comps = Vec::new();
        for id in tops {
            let is_comp = matches!(self.doc.get(id).map(|n| &n.kind), Some(NodeKind::Component(_)));
            if !is_comp {
                let cmd = create_component(&mut self.doc, id);
                self.push_cmd(&mut tx, cmd);
            }
            if let Some(c) = self.component_for(id) {
                comps.push(c);
            }
        }
        if comps.is_empty() {
            tx.cancel(&mut self.doc);
            return;
        }
        let cmd = combine_as_variants(&mut self.doc, &comps);
        if !self.push_cmd(&mut tx, cmd) {
            tx.cancel(&mut self.doc);
            return;
        }
        let set = comps.first().and_then(|c| self.doc.get(*c)).and_then(|n| n.parent);
        self.run_tx(tx);
        if let Some(set) = set {
            self.selection = vec![set];
        }
        self.show_toast("Combined as variants", false);
    }

    /// Selects the main component of an instance and brings it into view.
    pub fn go_to_main(&mut self, id: NodeId) {
        let inst = if matches!(self.doc.get(id).map(|n| &n.kind), Some(NodeKind::Instance { .. })) { Some(id) } else { instance_root_of(&self.doc, id) };
        match inst.and_then(|i| self.main_of(i)) {
            Some(comp) => self.go_to_main_component(comp),
            None => self.show_toast("Main component not found", true),
        }
    }

    /// Selects a main component, switching page if needed, and zooms to it.
    pub fn go_to_main_component(&mut self, comp: NodeId) {
        // Copies of library components live on the hidden library page.
        if is_library_node(&self.doc, comp) {
            self.go_to_library_main(comp);
            return;
        }
        if let Some(page) = self.doc.page_of(comp) {
            if page != self.page {
                self.switch_page(page);
            }
        }
        self.selection = vec![comp];
        self.fit_selection();
    }

    /// Adds an instance of `comp` centred on page point `at`; returns it.
    pub fn insert_instance(&mut self, comp: NodeId, at: Vec2) -> Option<NodeId> {
        let size = self.doc.get(comp)?.size;
        let target = Vec2::new(at.x - size.x / 2.0, at.y - size.y / 2.0);
        // A selected slot takes the instance only when it is dropped on that slot.
        let selected_slot = match self.selection.as_slice() {
            [only] if is_slot_container(&self.doc, *only) && contains_page_point(&self.doc, *only, at) => Some(*only),
            _ => None,
        };
        let mut parent = selected_slot.unwrap_or_else(|| container_at(&self.doc, self.page, at, &[]));
        let mut tx = Tx::new();
        for attempt in 0..2 {
            let inv = self.doc.world_transform(parent).invert()?;
            let local = inv.apply(target);
            let mut cmd = create_instance(&mut self.doc, comp, parent, local);
            if is_noop(&cmd) {
                // Inside the component itself (or its descendants): fall back to the page.
                if attempt == 0 {
                    parent = self.page;
                    continue;
                }
                return None;
            }
            if is_slot_container(&self.doc, parent) {
                if let Command::Insert { nodes, .. } = &mut cmd {
                    if let Some(root) = nodes.first_mut() {
                        if let Err(err) = check_slot_insert(&self.doc, parent, &[root]) {
                            self.show_toast(err.to_string(), true);
                            return None;
                        }
                        apply_slot_stretch_on_insert(&self.doc, parent, root);
                    }
                }
            }
            let id = match &cmd {
                Command::Insert { nodes, .. } => nodes.first().map(|n| n.id),
                _ => None,
            };
            if !self.push_cmd(&mut tx, cmd) {
                return None;
            }
            let is_inst_slot =
                is_slot_container(&self.doc, parent) && (self.doc.get(parent).is_some_and(is_linked) || instance_root_of(&self.doc, parent).is_some());
            if is_inst_slot {
                if let Some(sn) = tx.ed.node(&mut self.doc, parent) {
                    sn.component.overrides.insert(OverrideField::SlotContent);
                }
            }
            layout::relayout_around(&mut self.doc, &mut tx.ed, parent);
            self.run_tx(tx);
            if let Some(id) = id {
                self.selection = vec![id];
            }
            return id;
        }
        None
    }

    /// Inserts an instance of `comp` at the end of `slot`, enforcing [`SlotPropConfig`]
    /// constraints, stretching on insert when configured, and keeping the enclosing instance linked.
    pub fn insert_instance_into_slot(&mut self, slot: NodeId, comp: NodeId) -> Option<NodeId> {
        if !is_slot_container(&self.doc, slot) {
            return None;
        }
        let cmd = create_instance(&mut self.doc, comp, slot, Vec2::ZERO);
        let Command::Insert { index, nodes, .. } = cmd else {
            return None;
        };
        let id = nodes.first().map(|n| n.id);
        let slot_cmd = match insert_into_slot(&mut self.doc, slot, index, nodes) {
            Ok(c) => c,
            Err(err) => {
                self.show_toast(err.to_string(), true);
                return None;
            }
        };
        let mut tx = Tx::new();
        if !self.push_cmd(&mut tx, slot_cmd) {
            return None;
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, slot);
        self.run_tx(tx);
        self.selection = vec![slot];
        id
    }

    /// Sets (or with `None` resets) a component property on an instance.
    pub fn set_instance_prop(&mut self, inst: NodeId, name: &str, value: Option<PropValue>) {
        let mut tx = Tx::new();
        let cmd = set_instance_prop(&mut self.doc, inst, name, value);
        if self.push_cmd(&mut tx, cmd) {
            self.refit_texts(&mut tx, inst);
            self.run_tx(tx);
        }
    }

    /// Swaps an instance to another component.
    pub fn swap_instance(&mut self, inst: NodeId, comp: NodeId) {
        let mut tx = Tx::new();
        let cmd = swap_instance(&mut self.doc, inst, comp);
        if self.push_cmd(&mut tx, cmd) {
            self.run_tx(tx);
        }
    }

    /// "Reset all changes" on the selected instances.
    pub fn reset_selection(&mut self) {
        let insts = self.selected_instances();
        let mut tx = Tx::new();
        let mut any = false;
        for i in insts {
            let cmd = reset_instance(&mut self.doc, i);
            any |= self.push_cmd(&mut tx, cmd);
        }
        if any {
            let roots = self.selected_instances();
            for r in roots {
                self.refit_texts(&mut tx, r);
            }
            self.run_tx(tx);
        }
    }

    /// Resets specific overrides of one layer inside an instance.
    pub fn reset_node_overrides(&mut self, node: NodeId, fields: &[OverrideField]) {
        let mut tx = Tx::new();
        let cmd = reset_overrides(&mut self.doc, node, fields);
        if self.push_cmd(&mut tx, cmd) {
            self.run_tx(tx);
        }
    }

    /// Owner of the property definitions shown for `comp`: its set, else itself.
    pub fn prop_owner(&self, comp: NodeId) -> NodeId {
        self.doc.get(comp).and_then(|n| n.parent).filter(|p| matches!(self.doc.get(*p).map(|n| &n.kind), Some(NodeKind::ComponentSet(_)))).unwrap_or(comp)
    }

    fn comp_edit(&mut self, f: impl FnOnce(&mut Document) -> Command) {
        let mut tx = Tx::new();
        let cmd = f(&mut self.doc);
        if self.push_cmd(&mut tx, cmd) {
            self.run_tx(tx);
        }
    }

    /// Adds a property definition with a unique name.
    pub fn add_prop_def(&mut self, owner: NodeId, kind: PropKind) {
        let base = match &kind {
            PropKind::Boolean => "Boolean",
            PropKind::Text => "Text",
            PropKind::InstanceSwap { .. } => "Instance",
            PropKind::Variant { .. } => "Variant",
            PropKind::Slot => "Slot",
        };
        let taken: Vec<String> = self.doc.get(owner).map(|n| n.component.props.iter().map(|p| p.name.clone()).collect()).unwrap_or_default();
        let mut name = base.to_string();
        let mut i = 1;
        while taken.contains(&name) {
            i += 1;
            name = format!("{base} {i}");
        }
        let default = match &kind {
            PropKind::Boolean => PropValue::Bool(true),
            PropKind::Text => PropValue::Text("Text".into()),
            PropKind::InstanceSwap { .. } => PropValue::Instance(
                self.doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Component(_)) && n.id != owner).map(|n| n.id).unwrap_or(NodeId(0)),
            ),
            PropKind::Variant { options } => PropValue::Variant(options.first().cloned().unwrap_or_default()),
            PropKind::Slot => PropValue::Slot(Vec::new()),
        };
        self.comp_edit(|d| add_prop(d, owner, ComponentPropDef { name, kind, default }));
    }

    pub fn rename_prop_def(&mut self, owner: NodeId, old: &str, new: &str) {
        let (old, new) = (old.to_string(), new.trim().to_string());
        self.comp_edit(|d| rename_prop(d, owner, &old, &new));
    }

    pub fn delete_prop_def(&mut self, owner: NodeId, name: &str) {
        let name = name.to_string();
        self.comp_edit(|d| delete_prop(d, owner, &name));
    }

    pub fn set_prop_default(&mut self, owner: NodeId, name: &str, value: PropValue) {
        let Some(mut n) = self.doc.get(owner).cloned() else { return };
        if let Some(d) = n.component.props.iter_mut().find(|d| d.name == name) {
            d.default = value;
        }
        self.comp_edit(|_| Command::Update { node: Box::new(n) });
    }

    /// Binds a layer's visibility / characters / instance to a property (or unbinds with `None`).
    pub fn bind_prop(&mut self, node: NodeId, field: OverrideField, prop: Option<&str>) {
        let prop = prop.map(String::from);
        self.comp_edit(|d| bind_prop(d, node, field, prop.as_deref()));
    }

    /// Converts a frame inside a main component into a component slot in one undo step.
    pub fn convert_to_slot(&mut self, frame: NodeId) {
        let mut tx = Tx::new();
        let cmd = convert_to_slot(&mut self.doc, frame);
        if self.push_cmd(&mut tx, cmd) {
            self.run_tx(tx);
            self.show_toast("Converted to slot", false);
        }
    }

    /// Converts any selected frames inside a main component into slots in one undo step.
    pub fn convert_selection_to_slot(&mut self) {
        let ids = self.selection.clone();
        let mut tx = Tx::new();
        let mut count = 0usize;
        for id in ids {
            let cmd = convert_to_slot(&mut self.doc, id);
            if self.push_cmd(&mut tx, cmd) {
                count += 1;
            }
        }
        if count > 0 {
            self.run_tx(tx);
            self.show_toast(if count == 1 { "Converted to slot" } else { "Converted to slots" }, false);
        }
    }

    /// Updates the [`SlotPropConfig`] for a slot property on a component or component set.
    pub fn set_slot_config(&mut self, owner: NodeId, prop_name: &str, config: SlotPropConfig) {
        let prop_name = prop_name.to_string();
        self.comp_edit(|d| set_slot_config(d, owner, &prop_name, config));
    }

    /// Migrates `node` (an instance-swap or boolean container region inside a main component)
    /// to a component slot in one undo step, or shows an honest toast and returns `Err` on unsupported shapes.
    pub fn migrate_node_to_slot(&mut self, node: NodeId) -> Result<(), &'static str> {
        let cmd = match migrate_region_to_slot(&mut self.doc, node) {
            Ok(cmd) => cmd,
            Err(msg) => {
                self.show_toast(msg, true);
                return Err(msg);
            }
        };
        let mut tx = Tx::new();
        if self.push_cmd(&mut tx, cmd) {
            self.run_tx(tx);
            self.show_toast("Migrated to slot", false);
        }
        Ok(())
    }

    /// Migrates the currently selected layer to a component slot in one undo step.
    #[cfg(test)]
    pub fn migrate_selection_to_slot(&mut self) -> Result<(), &'static str> {
        let Some(&node) = self.selection.first() else {
            let msg = "Select a layer inside a main component to migrate to a slot";
            self.show_toast(msg, true);
            return Err(msg);
        };
        if self.selection.len() > 1 {
            let msg = "Select a single instance-swap or boolean region to migrate to a slot";
            self.show_toast(msg, true);
            return Err(msg);
        }
        self.migrate_node_to_slot(node)
    }

    /// Components for the Assets panel, in page order.
    pub fn local_components(&self) -> Vec<AssetEntry> {
        let mut out = Vec::new();
        for pid in &self.doc.pages {
            let Some(page) = self.doc.get(*pid) else { continue };
            let mut found = Vec::new();
            self.doc.walk(*pid, |n, _| {
                if matches!(n.kind, NodeKind::Component(_)) {
                    found.push(n.id);
                }
            });
            for c in found {
                let Some(n) = self.doc.get(c) else { continue };
                let set = n.parent.and_then(|p| self.doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_)));
                // A set is listed once, through its first variant.
                if let Some(set) = set {
                    if set.children.iter().copied().find(|k| matches!(self.doc.get(*k).map(|x| &x.kind), Some(NodeKind::Component(_)))) != Some(c) {
                        continue;
                    }
                }
                let container = set
                    .map(|s| s.parent)
                    .unwrap_or(n.parent)
                    .and_then(|p| self.doc.get(p))
                    .filter(|p| !matches!(p.kind, NodeKind::Page { .. }))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let label = set.map(|s| s.name.clone()).unwrap_or_else(|| n.name.clone());
                out.push(AssetEntry { page: page.name.clone(), container, name: label, id: c, variants: set.map(|s| s.children.len()).unwrap_or(0) });
            }
        }
        out
    }
}
