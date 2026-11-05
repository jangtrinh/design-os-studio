//! JSON to `Manifest`, field by field, so every error names the field and the fix.

use crate::error::{DocsError, ManifestError, Result};
use crate::group::Group;
use crate::manifest::{group_fix, version_error, GroupEntry, Manifest, Pin, SCHEMA_VERSION};
use serde_json::{Map, Value};

pub(crate) fn parse_json(text: &str) -> Result<Manifest> {
    let value: Value = serde_json::from_str(text).map_err(|e| {
        ManifestError::error(
            "(file)",
            format!("is not valid JSON ({e}; line {} column {})", e.line(), e.column()),
            "fix the JSON syntax, or restore the file from a backup",
        )
    })?;
    let Value::Object(root) = value else {
        return Err(ManifestError::error("(file)", "is not a JSON object", "start the file with `{` and list schema_version, groups, pins and linked_specs"));
    };
    from_object(&root)
}

fn from_object(root: &Map<String, Value>) -> Result<Manifest> {
    const FIELDS: [(&str, &str); 4] = [
        ("schema_version", "add \"schema_version\": 1"),
        (
            "groups",
            "add \"groups\": a list of the five groups in this order: overview | spec | screens | states | discovery, each {\"id\": ..., \"pages\": [...]}",
        ),
        ("pins", "add \"pins\": [] (a list, empty when nothing is pinned)"),
        ("linked_specs", "add \"linked_specs\": [] (a list of folder paths, empty when none are linked)"),
    ];
    for (name, fix) in FIELDS {
        if !root.contains_key(name) {
            return Err(ManifestError::error(name, "is missing", fix));
        }
    }
    if let Some(extra) = root.keys().find(|k| !FIELDS.iter().any(|(n, _)| n == k)) {
        return Err(unknown(extra.clone()));
    }
    let version =
        root["schema_version"].as_u64().ok_or_else(|| ManifestError::error("schema_version", "is not a whole number", "set \"schema_version\": 1"))?;
    if version != u64::from(SCHEMA_VERSION) {
        return Err(version_error(u32::try_from(version).unwrap_or(u32::MAX)));
    }
    let groups =
        array("groups", &root["groups"], "a list of the five groups")?.iter().enumerate().map(|(i, v)| group_entry(i, v)).collect::<Result<Vec<_>>>()?;
    let count = groups.len();
    let groups: [GroupEntry; 5] = groups.try_into().map_err(|_| ManifestError::error("groups", format!("has {count} groups"), group_fix()))?;
    Ok(Manifest {
        schema_version: SCHEMA_VERSION,
        groups,
        pins: array("pins", &root["pins"], "a list of pins")?.iter().enumerate().map(|(i, v)| pin(i, v)).collect::<Result<_>>()?,
        linked_specs: array("linked_specs", &root["linked_specs"], "a list of folder paths")?
            .iter()
            .enumerate()
            .map(|(i, v)| string(&format!("linked_specs[{i}]"), v))
            .collect::<Result<_>>()?,
    })
}

fn unknown(field: String) -> DocsError {
    ManifestError::error(
        field,
        "is not a field this version knows",
        "remove it (or fix the spelling); unknown fields are refused so no data is silently dropped",
    )
}

fn array<'a>(field: &str, v: &'a Value, what: &str) -> Result<&'a Vec<Value>> {
    v.as_array().ok_or_else(|| ManifestError::error(field, "is not a list", format!("make it {what}, written with [ ]")))
}

fn string(field: &str, v: &Value) -> Result<String> {
    v.as_str().map(str::to_string).ok_or_else(|| ManifestError::error(field, "is not a text value", "write it as a JSON string in double quotes"))
}

fn object<'a>(field: &str, v: &'a Value) -> Result<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| ManifestError::error(field, "is not an object", "write it as { ... } with its fields"))
}

fn require<'a>(obj: &'a Map<String, Value>, base: &str, name: &str, fix: &str) -> Result<&'a Value> {
    obj.get(name).ok_or_else(|| ManifestError::error(format!("{base}.{name}"), "is missing", fix))
}

fn reject_unknown(obj: &Map<String, Value>, base: &str, known: &[&str]) -> Result<()> {
    match obj.keys().find(|k| !known.contains(&k.as_str())) {
        Some(extra) => Err(unknown(format!("{base}.{extra}"))),
        None => Ok(()),
    }
}

fn group_entry(i: usize, v: &Value) -> Result<GroupEntry> {
    let base = format!("groups[{i}]");
    let obj = object(&base, v)?;
    let id_value = require(obj, &base, "id", &group_fix())?;
    let id_text = string(&format!("{base}.id"), id_value)?;
    let pages_value = require(obj, &base, "pages", "add \"pages\": [] (the page paths of this group, in order)")?;
    reject_unknown(obj, &base, &["id", "pages"])?;
    let id =
        Group::parse(&id_text).ok_or_else(|| ManifestError::error(format!("{base}.id"), format!("is {id_text:?}, which is not a doc group"), group_fix()))?;
    let pages = array(&format!("{base}.pages"), pages_value, "a list of page paths")?
        .iter()
        .enumerate()
        .map(|(p, page)| string(&format!("{base}.pages[{p}]"), page))
        .collect::<Result<_>>()?;
    Ok(GroupEntry { id, pages })
}

fn pin(i: usize, v: &Value) -> Result<Pin> {
    let base = format!("pins[{i}]");
    let obj = object(&base, v)?;
    let copy = "copy file_id and node_id from the frame on the canvas";
    let text = |name: &str| -> Result<String> { string(&format!("{base}.{name}"), require(obj, &base, name, &format!("add \"{name}\"; {copy}"))?) };
    let (id, file_id, node_id, name) = (text("id")?, text("file_id")?, text("node_id")?, text("name")?);
    let group_value = require(obj, &base, "group", "add \"group\": \"screens\" or \"states\"")?;
    reject_unknown(obj, &base, &["id", "file_id", "node_id", "name", "group", "parent_screen"])?;
    let group = group_value
        .as_str()
        .and_then(Group::parse)
        .filter(|g| matches!(g, Group::Screens | Group::States))
        .ok_or_else(|| ManifestError::error(format!("{base}.group"), format!("is {group_value}, not a pin group"), "use \"screens\" or \"states\""))?;
    let parent_screen = match obj.get("parent_screen") {
        None | Some(Value::Null) => None,
        Some(v) => Some(string(&format!("{base}.parent_screen"), v)?),
    };
    Ok(Pin { id, file_id, node_id, name, group, parent_screen })
}
