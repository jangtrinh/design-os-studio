//! `docs/docs-manifest.json`: page order per group, pinned frames and linked spec folders.
//!
//! Strict by design: every field is required, unknown fields are refused (a typo must not silently drop data), and
//! each error names the field's JSON path and the fix. Nothing is defaulted by omission.

use crate::error::{DocsError, ManifestError, Result};
use crate::group::Group;
use crate::paths::page_path_problem;
use serde::Serialize;

pub const MANIFEST_FILE: &str = "docs-manifest.json";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GroupEntry {
    pub id: Group,
    /// Page paths relative to the docs folder, in tree order.
    pub pages: Vec<String>,
}

/// A frame pinned on purpose (P11) under Screens or States.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Pin {
    pub id: String,
    pub file_id: String,
    pub node_id: String,
    pub name: String,
    /// `Screens` or `States`.
    pub group: Group,
    /// The pin id of the screen a state belongs to; required for states, forbidden for screens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_screen: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Manifest {
    pub schema_version: u32,
    /// Exactly the five groups, in the fixed order of `Group::ALL`.
    pub groups: [GroupEntry; 5],
    pub pins: Vec<Pin>,
    /// Spec v2 folders read in place (phase 07). A path may be absolute (picked with the system folder picker); the
    /// reader must confine every read to inside the linked folder (no `..`, no symlink out of it).
    pub linked_specs: Vec<String>,
}

impl Serialize for Group {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

impl Manifest {
    /// A manifest with the five groups and nothing in them.
    pub fn new() -> Self {
        Self { schema_version: SCHEMA_VERSION, groups: Group::ALL.map(|id| GroupEntry { id, pages: Vec::new() }), pins: Vec::new(), linked_specs: Vec::new() }
    }

    pub fn group(&self, id: Group) -> &GroupEntry {
        &self.groups[id.index()]
    }

    pub fn group_mut(&mut self, id: Group) -> &mut GroupEntry {
        &mut self.groups[id.index()]
    }

    /// Reads and validates a manifest.
    pub fn parse(text: &str) -> Result<Manifest> {
        let manifest = crate::manifest_parse::parse_json(text)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// The manifest as pretty JSON, after validating it, so an invalid manifest never reaches the disk.
    pub fn to_json_pretty(&self) -> Result<String> {
        self.validate()?;
        let mut text =
            serde_json::to_string_pretty(self).map_err(|e| ManifestError::error("(file)", format!("cannot be written ({e})"), "report this as a bug"))?;
        text.push('\n');
        Ok(text)
    }

    /// Checks everything that parsing alone cannot: fixed group order, page paths, unique ids, pin parents.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(version_error(self.schema_version));
        }
        let mut seen: Vec<(String, Group)> = Vec::new();
        for (i, (entry, expected)) in self.groups.iter().zip(Group::ALL).enumerate() {
            if entry.id != expected {
                return Err(ManifestError::error(
                    format!("groups[{i}].id"),
                    format!("is \"{}\" but position {i} must be \"{}\"", entry.id.as_str(), expected.as_str()),
                    group_fix(),
                ));
            }
            for (p, page) in entry.pages.iter().enumerate() {
                let field = format!("groups[{i}].pages[{p}]");
                if let Some(problem) = page_path_problem(page) {
                    return Err(ManifestError::error(
                        field,
                        format!("is {page:?}, which {problem}"),
                        "list pages as relative paths ending in .md, such as \"spec/checkout.md\"",
                    ));
                }
                // Compared without regard to case: `Spec.md` and `spec.md` are one file on macOS and Windows.
                if let Some((_, other)) = seen.iter().find(|(s, _)| *s == page.to_lowercase()) {
                    return Err(ManifestError::error(
                        field,
                        format!("lists {page:?}, which is already listed under {}", other.as_str()),
                        "list each page once, in the group it belongs to",
                    ));
                }
                seen.push((page.to_lowercase(), entry.id));
            }
        }
        self.validate_linked_specs()?;
        self.validate_pins()
    }

    /// Spec folders may be absolute (chosen with the system folder picker) or relative to the project, but must be
    /// a real path: not empty, no `..` segments, no control characters, each listed once.
    fn validate_linked_specs(&self) -> Result<()> {
        for (i, path) in self.linked_specs.iter().enumerate() {
            let field = format!("linked_specs[{i}]");
            let fix = "list each spec folder once, as a path without `..`, such as \"/Users/me/specs/checkout\"";
            if path.trim().is_empty() {
                return Err(ManifestError::error(field, "is empty", fix));
            }
            if path.chars().any(char::is_control) || path.split(['/', '\\']).any(|part| part == "..") {
                return Err(ManifestError::error(field, format!("is {path:?}, which has a `..` segment or a control character"), fix));
            }
            if self.linked_specs[..i].contains(path) {
                return Err(ManifestError::error(field, format!("lists {path:?} again"), fix));
            }
        }
        Ok(())
    }

    fn validate_pins(&self) -> Result<()> {
        for (i, pin) in self.pins.iter().enumerate() {
            let f = |name: &str| format!("pins[{i}].{name}");
            for (name, value) in [("id", &pin.id), ("file_id", &pin.file_id), ("node_id", &pin.node_id), ("name", &pin.name)] {
                if value.trim().is_empty() {
                    return Err(ManifestError::error(f(name), "is empty", "give the pin a value; copy file_id and node_id from the frame on the canvas"));
                }
            }
            for (name, value) in [("id", &pin.id), ("file_id", &pin.file_id), ("node_id", &pin.node_id)] {
                if value.contains(|c: char| c.is_whitespace() || c.is_control() || matches!(c, '#' | ')' | '(')) {
                    return Err(ManifestError::error(
                        f(name),
                        format!("is {value:?}, which has spaces or `#()` in it"),
                        "use the plain id with no spaces or `#`",
                    ));
                }
            }
            if !matches!(pin.group, Group::Screens | Group::States) {
                return Err(ManifestError::error(f("group"), format!("is \"{}\"", pin.group.as_str()), "pins live under \"screens\" or \"states\""));
            }
            if self.pins[..i].iter().any(|p| p.id == pin.id) {
                return Err(ManifestError::error(f("id"), format!("is {:?}, used by an earlier pin", pin.id), "give every pin its own id"));
            }
            match (&pin.group, &pin.parent_screen) {
                (Group::States, None) => {
                    return Err(ManifestError::error(
                        f("parent_screen"),
                        "is missing for a state",
                        "set parent_screen to the id of the screen pin this state belongs under",
                    ))
                }
                (Group::States, Some(parent)) => {
                    if !self.pins.iter().any(|p| p.id == *parent && p.group == Group::Screens) {
                        return Err(ManifestError::error(
                            f("parent_screen"),
                            format!("is {parent:?}, which is not a pinned screen"),
                            "set it to the id of a pin whose group is \"screens\"",
                        ));
                    }
                }
                (_, Some(_)) => return Err(ManifestError::error(f("parent_screen"), "is set on a screen", "remove parent_screen; only states have one")),
                _ => {}
            }
        }
        Ok(())
    }
}

pub(crate) fn group_fix() -> String {
    format!("list exactly five groups in this order: {}", Group::list())
}

pub(crate) fn version_error(found: u32) -> DocsError {
    ManifestError::error(
        "schema_version",
        format!("is {found}, but this Studio reads version {SCHEMA_VERSION}"),
        "Update Studio to open this project's docs, or restore a manifest written by this version",
    )
}
