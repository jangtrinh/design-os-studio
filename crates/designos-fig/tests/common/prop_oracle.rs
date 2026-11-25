//! A check of the component property records of a saved `.fig` that reads only the saved records, so it does not depend on any
//! table the exporter keeps. Shared by the fixture tests and the kit checks through `#[path]`.
//!
//! Figma resolves a property id inside one owner: a component set with its variants and everything under them, or a main
//! component that is no variant. The owner of a reference is found from the saved tree: `parentIndex` for ancestry, `symbolID` for
//! the main of an instance.
#![allow(dead_code)]

use designos_fig::kiwi::KiwiValue as V;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub type G = (u32, u32);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Violation {
    /// The id is defined in the file, but not by the owner of the layer that names it.
    CrossOwner { record: G, key: &'static str, id: G },
    /// The id is defined by no layer of the saved file, and the original file either defined it or never named it.
    Dangling { record: G, key: &'static str, id: G },
    /// The owner defines the id under a name the original file never gave it.
    WrongName { record: G, key: &'static str, id: G, name: String },
}

impl Violation {
    fn same_as(&self, other: &Violation) -> bool {
        self == other
    }
}

fn pair(g: &V) -> Option<G> {
    let n = |k: &str| match g.get(k) {
        Some(V::Uint(u)) => Some(*u),
        _ => None,
    };
    Some((n("sessionID")?, n("localID")?))
}

struct Tree<'a> {
    rec: HashMap<G, &'a V>,
    parent: HashMap<G, G>,
}

fn ty(r: &V) -> &str {
    match r.get("type") {
        Some(V::Enum(t)) => t,
        _ => "",
    }
}

fn is_state_group(r: &V) -> bool {
    ty(r) == "FRAME" && matches!(r.get("isStateGroup"), Some(V::Bool(true)))
}

fn is_component(r: &V) -> bool {
    ty(r) == "SYMBOL" || is_state_group(r)
}

impl<'a> Tree<'a> {
    fn new(all: &'a [V]) -> Self {
        let mut rec = HashMap::new();
        let mut parent = HashMap::new();
        for r in all {
            let Some(g) = r.get("guid").and_then(pair) else { continue };
            rec.insert(g, r);
            if let Some(p) = r.get("parentIndex").and_then(|p| p.get("guid")).and_then(pair) {
                parent.insert(g, p);
            }
        }
        Tree { rec, parent }
    }

    /// The root of the owner group of the component `g`: its nearest state group above (not through an instance), else itself.
    fn root(&self, g: G) -> G {
        let mut cur = self.parent.get(&g).copied();
        for _ in 0..self.rec.len() {
            let Some(c) = cur else { break };
            let Some(r) = self.rec.get(&c) else { break };
            if is_state_group(r) {
                return c;
            }
            if ty(r) == "INSTANCE" {
                break;
            }
            cur = self.parent.get(&c).copied().filter(|p| *p != c);
        }
        g
    }

    fn main_of(&self, instance: &V) -> Option<G> {
        instance.get("symbolData").and_then(|s| s.get("symbolID")).and_then(pair).filter(|m| self.rec.contains_key(m))
    }

    /// The owner of a binding on `g`: the group of the component it sits in, or, inside an instance, of that instance's main.
    fn binding_owner(&self, g: G) -> Option<G> {
        let r = self.rec.get(&g)?;
        if is_component(r) {
            return Some(self.root(g));
        }
        let mut cur = self.parent.get(&g).copied();
        for _ in 0..self.rec.len() {
            let c = cur?;
            let r = self.rec.get(&c)?;
            if is_component(r) {
                return Some(self.root(c));
            }
            if ty(r) == "INSTANCE" {
                return self.main_of(r).map(|m| self.root(m));
            }
            cur = self.parent.get(&c).copied().filter(|p| *p != c);
        }
        None
    }

    fn assignment_owner(&self, g: G) -> Option<G> {
        let r = self.rec.get(&g)?;
        if ty(r) == "INSTANCE" {
            return self.main_of(r).map(|m| self.root(m));
        }
        self.binding_owner(g)
    }
}

fn pcm_ids(v: &V, out: &mut Vec<G>) {
    match v {
        V::Message(f) | V::Struct(f) => {
            if let Some(g) = v.get("propRefValue").and_then(|p| p.get("defId")).and_then(pair) {
                out.push(g);
            }
            f.iter().for_each(|(_, x)| pcm_ids(x, out));
        }
        V::Array(a) => a.iter().for_each(|x| pcm_ids(x, out)),
        _ => {}
    }
}

/// Every reference to a property id in a record: (key, id, whether it is an assignment).
fn references(r: &V) -> Vec<(&'static str, G, bool)> {
    let mut out = Vec::new();
    for (list, key, id_key, assignment) in [
        ("componentPropAssignments", "componentPropAssignments", "defID", true),
        ("componentPropRefs", "componentPropRefs", "defID", false),
        ("variantPropSpecs", "variantPropSpecs", "propDefId", false),
        ("componentPropDefs", "parentPropDefId", "parentPropDefId", false),
    ] {
        for e in r.get(list).and_then(V::as_array).unwrap_or(&[]) {
            if let Some(g) = e.get(id_key).and_then(pair).filter(|g| *g != (0, 0)) {
                out.push((key, g, assignment));
            }
        }
    }
    if let Some(p) = r.get("parameterConsumptionMap") {
        let mut ids = Vec::new();
        pcm_ids(p, &mut ids);
        out.extend(ids.into_iter().map(|g| ("parameterConsumptionMap", g, false)));
    }
    out
}

/// What the original file said about its properties.
pub struct Original {
    defined: HashSet<G>,
    referenced: HashSet<G>,
    names: HashMap<G, BTreeSet<String>>,
}

pub fn original(all: &[V]) -> Original {
    let mut o = Original { defined: HashSet::new(), referenced: HashSet::new(), names: HashMap::new() };
    for r in all {
        for d in r.get("componentPropDefs").and_then(V::as_array).unwrap_or(&[]) {
            if let Some(id) = d.get("id").and_then(pair) {
                o.defined.insert(id);
                if let Some(V::String(n)) = d.get("name") {
                    o.names.entry(id).or_default().insert(n.clone());
                }
            }
        }
        o.referenced.extend(references(r).into_iter().map(|(_, g, _)| g));
    }
    o
}

/// The violations in the saved records.
pub fn check(saved: &[V], was: &Original) -> Vec<Violation> {
    let tree = Tree::new(saved);
    // who defines what, and under which name
    let mut defined_by: HashMap<G, HashSet<G>> = HashMap::new();
    let mut name_in: HashMap<(G, G), String> = HashMap::new();
    for r in saved {
        let Some(at) = r.get("guid").and_then(pair) else { continue };
        let Some(owner) = tree.binding_owner(at) else {
            continue;
        };
        for d in r.get("componentPropDefs").and_then(V::as_array).unwrap_or(&[]) {
            let Some(id) = d.get("id").and_then(pair) else { continue };
            defined_by.entry(id).or_default().insert(owner);
            if let Some(V::String(n)) = d.get("name") {
                name_in.insert((owner, id), n.clone());
            }
        }
    }
    let mut defined_anywhere: HashSet<G> = HashSet::new();
    for r in saved {
        for d in r.get("componentPropDefs").and_then(V::as_array).unwrap_or(&[]) {
            if let Some(id) = d.get("id").and_then(pair) {
                defined_anywhere.insert(id);
            }
        }
    }
    let mut out = Vec::new();
    for r in saved {
        let Some(at) = r.get("guid").and_then(pair) else { continue };
        for (key, id, assignment) in references(r) {
            // the picks of an instance, like its assignments, name properties of the set of its main
            let is_instance = tree.rec.get(&at).is_some_and(|r| ty(r) == "INSTANCE");
            let owner = if assignment || (key == "variantPropSpecs" && is_instance) { tree.assignment_owner(at) } else { tree.binding_owner(at) };
            let Some(owner) = owner else { continue };
            match defined_by.get(&id) {
                Some(owners) if owners.contains(&owner) => {
                    if let (Some(n), Some(orig)) = (name_in.get(&(owner, id)), was.names.get(&id)) {
                        if !orig.contains(n) {
                            out.push(Violation::WrongName { record: at, key, id, name: n.clone() });
                        }
                    }
                }
                Some(_) => out.push(Violation::CrossOwner { record: at, key, id }),
                None if defined_anywhere.contains(&id) => out.push(Violation::CrossOwner { record: at, key, id }),
                None if was.defined.contains(&id) || !was.referenced.contains(&id) => out.push(Violation::Dangling { record: at, key, id }),
                None => {}
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The violations of `saved` the original file did not already have: the original file has dangling assignments of its own (a
/// library's properties).
pub fn new_violations(saved: &[V], original_records: &[V]) -> Vec<Violation> {
    let was = original(original_records);
    let before = check(original_records, &was);
    check(saved, &was).into_iter().filter(|v| !before.iter().any(|b| b.same_as(v))).collect()
}

pub fn counts(v: &[Violation]) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for x in v {
        *m.entry(match x {
            Violation::CrossOwner { .. } => "CrossOwner",
            Violation::Dangling { .. } => "Dangling",
            Violation::WrongName { .. } => "WrongName",
        })
        .or_insert(0) += 1;
    }
    m
}

/// How many references to a property id the records hold.
pub fn reference_count(records: &[V]) -> usize {
    records.iter().map(|r| references(r).len()).sum()
}

/// What the original file said, and what the oracle found in it: the baseline a saved file is compared with.
pub struct Baseline {
    pub was: Original,
    pub before: Vec<Violation>,
    pub references: usize,
}

pub fn baseline(original_records: &[V]) -> Baseline {
    let was = original(original_records);
    let before = check(original_records, &was);
    Baseline { references: reference_count(original_records), was, before }
}

pub fn new_against(saved: &[V], base: &Baseline) -> Vec<Violation> {
    check(saved, &base.was).into_iter().filter(|v| !base.before.iter().any(|b| b.same_as(v))).collect()
}
