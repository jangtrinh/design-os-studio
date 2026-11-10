//! Small in-memory documents for the design-system scan tests: nodes that carry Figma ids (`source_id`) the way a
//! `.fig` import leaves them, so a test can tell a Figma id from Studio's own numeric node id.
#![allow(dead_code)]

use designos_doc::*;
use serde_json::Value;

pub struct World {
    pub doc: Document,
}

impl World {
    pub fn new() -> Self {
        Self { doc: Document::new("scan-world") }
    }

    pub fn page(&mut self, name: &str, figma_id: &str) -> NodeId {
        let page = self.doc.add_page(name);
        self.doc.get_mut(page).unwrap().source_id = Some(figma_id.into());
        page
    }

    /// Adds a node of `kind` under `parent`, 100 x 50, with `figma_id` as its Figma id.
    pub fn add(&mut self, parent: NodeId, kind: NodeKind, name: &str, figma_id: &str) -> NodeId {
        let id = self.doc.alloc_id();
        let mut node = Node::new(id, kind);
        node.name = name.into();
        node.size = Vec2::new(100.0, 50.0);
        node.source_id = Some(figma_id.into());
        self.doc.insert(parent, node).unwrap()
    }

    pub fn frame(&mut self, parent: NodeId, name: &str, figma_id: &str) -> NodeId {
        self.add(parent, NodeKind::Frame(FrameProps::default()), name, figma_id)
    }

    pub fn section(&mut self, parent: NodeId, name: &str, figma_id: &str) -> NodeId {
        self.add(parent, NodeKind::Section, name, figma_id)
    }

    pub fn component(&mut self, parent: NodeId, name: &str, figma_id: &str) -> NodeId {
        self.add(parent, NodeKind::Component(FrameProps::default()), name, figma_id)
    }

    pub fn component_set(&mut self, parent: NodeId, name: &str, figma_id: &str) -> NodeId {
        self.add(parent, NodeKind::ComponentSet(FrameProps::default()), name, figma_id)
    }

    /// An instance of `main` (or of nothing), with the Figma id `figma_id`.
    pub fn instance(&mut self, parent: NodeId, name: &str, figma_id: &str, main: Option<NodeId>) -> NodeId {
        self.add(parent, NodeKind::Instance { frame: FrameProps::default(), component: main, component_key: None }, name, figma_id)
    }

    pub fn bind(&mut self, node: NodeId, key: &str, variable: &str) {
        self.doc.get_mut(node).unwrap().variables.bindings.insert(key.into(), variable.into());
    }

    /// A collection with `modes` (id, name) and the variables `(id, name, kind, value per mode in order)`.
    pub fn collection(&mut self, id: &str, name: &str, library: Option<&str>, modes: &[(&str, &str)], vars: &[(&str, &str, VariableValue)]) {
        let mut listed = Vec::new();
        for (vid, vname, value) in vars {
            let kind = match value {
                VariableValue::Color(_) => VariableKind::Color,
                VariableValue::Number(_) => VariableKind::Number,
                VariableValue::String(_) => VariableKind::String,
                VariableValue::Boolean(_) => VariableKind::Boolean,
                VariableValue::Alias(_) => VariableKind::Color,
            };
            self.doc.variables.insert(
                (*vid).into(),
                Variable {
                    id: (*vid).into(),
                    name: (*vname).into(),
                    collection: id.into(),
                    kind,
                    values: modes.iter().map(|(mid, _)| ((*mid).to_string(), value.clone())).collect(),
                    description: String::new(),
                    scopes: Vec::new(),
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            listed.push((*vid).to_string());
        }
        self.doc.variable_collections.insert(
            id.into(),
            VariableCollection {
                id: id.into(),
                name: name.into(),
                modes: modes.iter().map(|(mid, mname)| VariableMode { id: (*mid).into(), name: (*mname).into() }).collect(),
                default_mode: modes[0].0.into(),
                variables: listed,
                key: None,
                source_library_key: library.map(String::from),
                extensions: Default::default(),
            },
        );
    }
}

/// The scan of `doc` as JSON, as the output file holds it.
pub fn scan_json(doc: &Document) -> Value {
    serde_json::from_str(&designos_fig::scan_design_system(doc).expect("the scan succeeds").to_json_text()).unwrap()
}

/// The entry of `components` named `name`.
pub fn component<'a>(scan: &'a Value, name: &str) -> &'a Value {
    scan["components"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("no component {name}: {scan:#}"))
}

/// The entry of `frames` named `name`.
pub fn frame<'a>(scan: &'a Value, name: &str) -> &'a Value {
    scan["frames"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap_or_else(|| panic!("no frame {name}: {scan:#}"))
}
