//! A `.fig` whose `derivedSymbolData` holds NaN sizes or transforms must import without putting a
//! non-finite number into the document. The fast import path reads these floats straight from the
//! kiwi message (no JSON intermediate, which would have dropped them), so a NaN there became the
//! size of a node and, saved as `null`, made the stored file impossible to reopen.

use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::import_fig;
use designos_fig::kiwi::KiwiValue;
use designos_fig::testkit::{figma_like_schema, fl, matrix, node, upgrade_message, vec2};

/// Replaces every `size` / `transform` inside a `derivedSymbolData` array with non-finite ones.
fn poison_derived_data(value: &mut KiwiValue, inside_derived: bool, bad: f32) -> usize {
    let mut poisoned = 0;
    match value {
        KiwiValue::Message(fields) | KiwiValue::Struct(fields) => {
            for (name, child) in fields.iter_mut() {
                match &**name {
                    "size" if inside_derived => {
                        *child = vec2(bad, bad);
                        poisoned += 1;
                    }
                    "transform" if inside_derived => {
                        let mut m = matrix(bad, 6.0);
                        if let KiwiValue::Struct(f) = &mut m {
                            f[0].1 = fl(bad);
                        }
                        *child = m;
                        poisoned += 1;
                    }
                    other => poisoned += poison_derived_data(child, inside_derived || other == "derivedSymbolData", bad),
                }
            }
        }
        KiwiValue::Array(items) => {
            for item in items {
                poisoned += poison_derived_data(item, inside_derived, bad);
            }
        }
        _ => {}
    }
    poisoned
}

/// The importer reads `derivedSymbolData` straight from the kiwi message only for a file with more
/// than 5000 node changes (the fast path for large files), which is what a complex design is and a
/// blank or small one is not. Pad the fixture past that line with plain rectangles.
fn pad_to_a_large_file(message: &mut KiwiValue) {
    let Some(KiwiValue::Array(changes)) = message.get_mut("nodeChanges") else { panic!("fixture has node changes") };
    for i in 0..5_100u32 {
        changes.push(node((9, i), (0, 1), &format!("z{i:05}"), "RECTANGLE", &format!("Pad {i}"), vec![("size", vec2(4.0, 4.0))]));
    }
}

/// A padded large `.fig`; with `Some(bad)` every derived size and transform in it is non-finite.
fn large_fig(poison: Option<f32>) -> Vec<u8> {
    let mut message = upgrade_message();
    pad_to_a_large_file(&mut message);
    if let Some(bad) = poison {
        assert!(poison_derived_data(&mut message, false, bad) >= 2, "the fixture must have derived sizes to poison");
    }
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 101)
}

#[test]
fn non_finite_derived_symbol_data_never_reaches_the_document() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let doc = import_fig(&large_fig(Some(bad))).unwrap();
        let (_, non_finite) = doc.to_json_counted().unwrap();
        assert_eq!(non_finite, 0, "{bad}: the imported document must hold only finite numbers");
        for n in doc.nodes.values() {
            assert!(n.size.x.is_finite() && n.size.y.is_finite(), "{bad}: {} has size {:?}", n.name, n.size);
            let t = n.transform;
            assert!([t.a, t.b, t.c, t.d, t.tx, t.ty].iter().all(|v| v.is_finite()), "{bad}: {} has transform {t:?}", n.name);
        }
    }
}

#[test]
fn a_non_finite_derived_entry_is_skipped_and_the_rest_of_the_file_imports_as_before() {
    let clean = import_fig(&large_fig(None)).unwrap();
    let poisoned = import_fig(&large_fig(Some(f32::NAN))).unwrap();
    assert_eq!(clean.nodes.len(), poisoned.nodes.len());
    let named = |d: &Document, name: &str| d.nodes.values().find(|n| n.name == name).map(|n| (n.size, n.transform)).unwrap();
    for name in ["Card", "Card B", "Pad 0", "Pad 5099"] {
        assert_eq!(named(&clean, name), named(&poisoned, name), "{name} has no derived data and must not change");
    }
}
