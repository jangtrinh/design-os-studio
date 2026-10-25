//! Per-document and per-node settings that are not part of the scene itself:
//! ruler guides, export presets and the prototype device.

use crate::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// WCAG contrast threshold level for Check designs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContrastLevel {
    #[default]
    AA,
    AAA,
}

impl ContrastLevel {
    pub fn is_aa(&self) -> bool {
        matches!(self, ContrastLevel::AA)
    }
}

/// Persistent key for an ignored Check designs finding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IgnoreKey {
    pub node: NodeId,
    pub rule: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub property: String,
}

impl IgnoreKey {
    pub fn new(rule: impl Into<String>, node: NodeId) -> Self {
        Self { node, rule: rule.into(), property: String::new() }
    }
}

/// Per-document Check designs configuration: disabled rules, contrast level, and ignored findings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckDesignsSettings {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub disabled_rules: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "ContrastLevel::is_aa")]
    pub contrast_level: ContrastLevel,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub ignored: BTreeSet<IgnoreKey>,
}

impl CheckDesignsSettings {
    pub fn is_empty(&self) -> bool {
        self.disabled_rules.is_empty() && self.contrast_level.is_aa() && self.ignored.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuideAxis {
    /// A vertical line at `offset` on the x axis.
    X,
    /// A horizontal line at `offset` on the y axis.
    Y,
}

/// A ruler guide. Page guides have `frame: None` and page-space offsets;
/// frame guides move with their frame and use frame-local offsets.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub axis: GuideAxis,
    pub offset: f32,
    pub frame: Option<NodeId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFileFormat {
    #[default]
    Png,
    Jpg,
    Svg,
    Pdf,
}

/// How an export size is constrained, as in Figma's export rows.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExportConstraint {
    Scale(f32),
    Width(f32),
    Height(f32),
}

impl Default for ExportConstraint {
    fn default() -> Self {
        ExportConstraint::Scale(1.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExportPreset {
    pub format: ExportFileFormat,
    pub constraint: ExportConstraint,
    /// File name suffix, e.g. "@2x".
    pub suffix: String,
}

/// Device the prototype is presented on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PrototypeDevice {
    /// Preset name ("iPhone 15", "Desktop", ...); `None` = no device frame.
    pub preset: Option<String>,
    /// Viewport size in px when a preset is chosen.
    pub size: Option<crate::Vec2>,
    pub background: Option<crate::Color>,
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn guides_and_device_undo() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let mut h = History::default();
        let g = Guide { axis: GuideAxis::X, offset: 40.0, frame: None };
        h.exec(&mut d, Command::SetGuides { page, guides: vec![g] }).unwrap();
        assert_eq!(d.guides[&page], vec![g]);
        h.exec(&mut d, Command::SetPrototypeDevice { device: PrototypeDevice { preset: Some("Desktop".into()), ..Default::default() } }).unwrap();
        h.undo(&mut d).unwrap();
        assert_eq!(d.prototype_device, PrototypeDevice::default());
        h.undo(&mut d).unwrap();
        assert!(!d.guides.contains_key(&page));
        h.redo(&mut d).unwrap();
        assert_eq!(d.guides[&page].len(), 1);
    }

    #[test]
    fn export_settings_roundtrip_and_omitted_when_empty() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 1.0, 1.0));
        n.export_settings.push(ExportPreset { format: ExportFileFormat::Svg, constraint: ExportConstraint::Width(512.0), suffix: "@512".into() });
        d.insert(page, n).unwrap();
        let back = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back.get(id).unwrap().export_settings, d.get(id).unwrap().export_settings);
        assert!(!serde_json::to_string(back.get(page).unwrap()).unwrap().contains("export_settings"));
    }

    #[test]
    fn settings_round_trip_and_old_files_load() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let default_json = d.to_json().unwrap();
        assert!(!default_json.contains("check_designs"), "empty settings write no key: {default_json}");
        let loaded_old = Document::from_json(&default_json).unwrap();
        assert!(loaded_old.check_designs.is_empty());

        let mut h = History::default();
        let mut next = CheckDesignsSettings { contrast_level: ContrastLevel::AAA, ..Default::default() };
        next.disabled_rules.insert("REQUIRE_TEXT_STYLES".into());
        next.ignored.insert(IgnoreKey { node: id, rule: "RECOMMEND_VARIABLES".into(), property: "fills/0".into() });
        h.exec(&mut d, Command::SetCheckDesignsSettings(next.clone())).unwrap();
        assert_eq!(d.check_designs, next);

        let with_settings_json = d.to_json().unwrap();
        assert!(with_settings_json.contains("\"check_designs\""));
        let round = Document::from_json(&with_settings_json).unwrap();
        assert_eq!(round.check_designs, next);

        h.undo(&mut d).unwrap();
        assert!(d.check_designs.is_empty());
        h.redo(&mut d).unwrap();
        assert_eq!(d.check_designs, next);

        // Deleting a node via History and undoing the delete must preserve its ignored keys.
        h.exec(&mut d, Command::Remove { id }).unwrap();
        h.undo(&mut d).unwrap();
        assert_eq!(d.check_designs, next, "undoing node delete must preserve its ignored check-designs keys");

        // Deleting the node drops its keys when the saved file is read back (garbage collection).
        d.remove(id).unwrap();
        let after_delete_json = d.to_json().unwrap();
        let after_delete = Document::from_json(&after_delete_json).unwrap();
        assert!(after_delete.check_designs.ignored.is_empty());
    }
}
