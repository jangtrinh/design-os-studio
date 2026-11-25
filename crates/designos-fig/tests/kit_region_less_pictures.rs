//! Kit check (ignored by default): a picture hash of every imported vector whose network is marked for the closed-loop fill
//! and has no region, drawn as the file has it (fills, strokes, stored outlines) at 2x. Reads `KIT_CLONE`, writes one
//! `key hash` line per vector to `PICTURES_OUT`; two builds are compared by diffing their files. Keys and hashes only.

use designos_doc::*;

#[test]
#[ignore = "reads KIT_CLONE, writes PICTURES_OUT"]
fn picture_hashes_of_the_region_less_kit_vectors() {
    let kit = std::env::var_os("KIT_CLONE").or_else(|| std::env::var_os("DESIGNOS_KIT_PATH")).expect("KIT_CLONE");
    let out = std::env::var("PICTURES_OUT").expect("PICTURES_OUT");
    let bytes = std::fs::read(kit).unwrap();
    let mut doc = designos_fig::import_fig(&bytes).unwrap();
    doc.hydrate_all_pages();
    let mut lines: Vec<String> = Vec::new();
    for (id, n) in doc.nodes.iter() {
        let NodeKind::Vector { network: Some(net), .. } = &n.kind else { continue };
        if !net.fills_closed_loops_without_regions || !net.regions.is_empty() {
            continue;
        }
        let key = n.source_id.clone().unwrap_or_else(|| format!("{id:?}"));
        let Some(pm) = designos_render::render_node(&doc, *id, 2.0) else {
            lines.push(format!("{key} none"));
            continue;
        };
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in pm.data() {
            h = (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
        }
        let visible_fill = n.fills.iter().any(|f| f.visible && f.opacity > 0.0);
        let stroked = n.stroke.weight > 0.0 && n.stroke.paints.iter().any(|p| p.visible);
        lines.push(format!("{key} {h:016x} {}x{} fill={visible_fill} stroke={stroked} align={:?}", pm.width(), pm.height(), n.stroke.align));
    }
    lines.sort();
    std::fs::write(out, lines.join("\n") + "\n").unwrap();
    eprintln!("PICTURES {} vectors", lines.len());
}
