//! Stored-data use on open: per node class, how many nodes carry the data Figma stored for them and how many Studio
//! draws or lays out from that data when the file opens. Each decision is the app's own, observed after every page is
//! hydrated and before any change:
//!
//! - instances: a top-level instance with `derivedSymbolData` keeps it when hydration recorded an imported layout for
//!   it (the set the layout leg calls refused is the rest);
//! - texts: `designos_render::stored_glyph_use` says whether the renderer draws the saved glyphs (the gate that
//!   `text_runs_for_node` and `layout_node` share); the drops are named by the first rule that explains them;
//! - fills and strokes: the renderer draws a non-empty `fill_geometry`, and a stored stroke outline only while
//!   `Node::live_stroke_outline` still holds (its stamp matches the layer's size and stroke).
//!
//! An item that is not drawn from its stored data is a miss in the total bucket and in the bucket of its reason.

use super::board::{broken_comparator, Part};
use super::geometry::GeoTruth;
use super::instance_hydration;
use super::instances::Truth;
use super::keys::NodeKeys;
use super::raw::Raw;
use designos_doc::*;

/// Box layers (their own geometry is a rounded rectangle) against path layers, so a drop reads by group.
fn type_group(ty: &str) -> &'static str {
    match ty {
        "FRAME" | "INSTANCE" | "SYMBOL" | "RECTANGLE" | "ROUNDED_RECTANGLE" | "SECTION" | "COMPONENT_SET" => {
            "box layers (frame, instance, component, rectangle)"
        }
        _ => "path layers (vector, boolean, ellipse, star, polygon, line)",
    }
}

/// Instance records whose `derivedSymbolData` key is present and empty: nothing stored to keep.
pub fn empty_derived(raw: &Raw) -> usize {
    raw.live()
        .filter(|r| {
            r.get("derivedSymbolData")
                .or_else(|| r.get("symbolData").and_then(|s| s.get("derivedSymbolData")))
                .and_then(|v| v.as_array())
                .is_some_and(<[_]>::is_empty)
        })
        .count()
}

/// Why a text node does not draw its saved glyphs, named by the renderer's own gate (`stored_glyph_use`): the text no
/// longer matches what Figma stored, or it is shaped for the named reason. For a node the importer kept no glyphs for,
/// the cause is named too. `None` when it draws them.
pub fn text_drop(node: &Node) -> Option<(&'static str, Option<&'static str>)> {
    let NodeKind::Text(props) = &node.kind else { return Some(("not a text layer", None)) };
    match designos_render::stored_glyph_use(node) {
        designos_render::StoredGlyphUse::Drawn => None,
        designos_render::StoredGlyphUse::Edited => Some(("saved glyphs no longer match the text", None)),
        designos_render::StoredGlyphUse::Fallback(reason) => Some((reason, None)),
        designos_render::StoredGlyphUse::NoStoredData => Some((
            "importer kept no glyphs",
            Some(if props.content.is_empty() {
                "no characters"
            } else if node.fig_metadata.contains_key("derivedTextData") {
                "derivedTextData left as JSON on the layer"
            } else {
                "no data kept for the layer (not in the file, over the import allowance, or not writable in Studio's schema)"
            }),
        )),
    }
}

pub fn part(inst: &Truth, geo: &[GeoTruth], text_guids: &[String], empty_derived: usize, doc: &Document, keys: &NodeKeys) -> Part {
    let mut p = Part::new(
        "stored_use",
        "Stored-data use on open",
        "a node that carries the data Figma stored for it draws or lays itself out from that data on open, by the decision the app makes",
    );
    p.print_detail("texts: importer kept no glyphs, by cause");
    p.note("texts: the drop reasons are the renderer's own, in the order its gate looks for them (see `designos_render::stored_glyph_use`); the fonts of the machine that runs this do not decide it");
    let broken = broken_comparator();
    let refused = instance_hydration::refused(inst, doc, keys);
    for i in &inst.instances {
        let Some(&id) = keys.by_source.get(&i.guid) else {
            p.unscored("instance record with no layer", "the record did not become a layer on a page");
            continue;
        };
        if instance_root_of(doc, id) != Some(id) {
            p.unscored("nested instance", "hydration records an imported layout only for a top-level instance; the holder's stored layout carries it");
            continue;
        }
        let kept = !refused.contains(&i.guid);
        let mut b = vec!["instances: total".to_string()];
        if !kept {
            b.push(format!("instances: not drawn from stored layout, hydration refused it, {} override", if i.overrides.is_empty() { "no" } else { "with" }));
        }
        p.record(format!("{}#instance", i.guid), keys.is_hidden(id), broken || kept, &b);
    }
    p.unscored_n("instance with an empty derivedSymbolData", "nothing stored to keep", empty_derived as u64);
    for g in text_guids {
        let Some(&id) = keys.by_source.get(g) else {
            p.unscored("text record with no layer", "the record did not become a layer on a page");
            continue;
        };
        let Some(n) = doc.get(id) else { continue };
        let resolved = resolve_node(doc, n);
        if matches!(&resolved.kind, NodeKind::Text(t) if t.content.is_empty()) {
            p.unscored("text with no characters", "nothing to draw, so the stored data has nothing to say");
            continue;
        }
        let drop = text_drop(&resolved);
        let mut b = vec!["texts: total".to_string()];
        if let Some((reason, cause)) = drop {
            b.push(format!("texts: not drawn, {reason}"));
            if let (Some(cause), false) = (cause, keys.is_hidden(id)) {
                p.detail("texts: importer kept no glyphs, by cause", cause);
            }
        }
        p.record(format!("{g}#text"), keys.is_hidden(id), broken || drop.is_none(), &b);
    }
    for t in geo {
        let Some(&id) = keys.by_source.get(&t.guid) else {
            p.unscored("record with no layer", "the record did not become a layer on a page");
            continue;
        };
        let Some(n) = doc.get(id) else { continue };
        let node = resolve_node(doc, n);
        let hidden = keys.is_hidden(id);
        if !t.fill.is_empty() {
            let drawn = node.fill_geometry.as_deref().is_some_and(|g| g.iter().any(|path| !path.cmds.is_empty()));
            let mut b = vec!["fills: total".to_string(), format!("fills: type {}", t.ty), format!("fills: {}", type_group(&t.ty))];
            if !drawn {
                b.push("fills: not drawn from stored outline, no fill outline left on the layer".to_string());
            }
            p.record(format!("{}#fill", t.guid), hidden, broken || drawn, &b);
        }
        if !t.stroke.is_empty() {
            let drawn = node.live_stroke_outline().is_some();
            let mut b = vec!["strokes: total".to_string(), format!("strokes: type {}", t.ty), format!("strokes: {}", type_group(&t.ty))];
            if !drawn {
                b.push(if node.stroke_geometry.as_deref().is_some_and(|g| !g.is_empty()) {
                    "strokes: not drawn, stale outline".to_string()
                } else {
                    "strokes: not drawn, no stroke outline left on the layer".to_string()
                });
            }
            p.record(format!("{}#stroke", t.guid), hidden, broken || drawn, &b);
        }
    }
    p
}
