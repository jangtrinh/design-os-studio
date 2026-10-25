//! Carrier entries: node data that Figma's schema has no field for, or holds in another shape, rides in the node's
//! `pluginData` under the reserved `designos-studio` id as `field:<name>` entries. Each holds the JSON of the legacy
//! value of NodeChange field `<name>`; the importer puts it back in place of the real field's value before it reads
//! the node, so the legacy readers take it as they take a file written before the real shapes. Figma keeps plugin
//! data of a plugin it does not know and ignores it.

use super::STUDIO_PLUGIN_ID;
use crate::kiwi::KiwiValue as V;
use crate::testkit::msg;
use serde_json::Value;

/// Key prefix of a carrier entry; the rest is the legacy field name.
pub(crate) const CARRIER_KEY_PREFIX: &str = "field:";
/// Legacy fields a carrier entry may restore. The importer ignores any other name.
pub(crate) const CARRIED_FIELDS: [&str; 6] = ["gridAutoTracks", "gridColumns", "gridRows", "gridColumnAnchorIndex", "gridRowAnchorIndex", "variantPropSpecs"];

/// Fields to carry in `pluginData`: (legacy field name, its legacy JSON value).
pub(super) type Carried = Vec<(&'static str, Value)>;

/// `pluginData` entries for the carried fields.
pub(super) fn carrier_entries(carried: &[(&'static str, Value)]) -> Vec<V> {
    carried
        .iter()
        .map(|(name, value)| {
            msg(vec![
                ("pluginID", V::String(STUDIO_PLUGIN_ID.into())),
                ("key", V::String(format!("{CARRIER_KEY_PREFIX}{name}"))),
                ("value", V::String(value.to_string())),
            ])
        })
        .collect()
}
