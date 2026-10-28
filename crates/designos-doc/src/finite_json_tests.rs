//! Non-finite numbers in JSON: never written, and read back as a neutral value when an older
//! version wrote them as `null`.

use crate::finite_json::{from_str_lenient, to_string_finite};
use crate::{Color, Document, Node, Paint, Rect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One of every container shape a number can sit in.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Shapes {
    plain: f32,
    double: f64,
    opacity: f32,
    radii: [f32; 4],
    dashes: Vec<f32>,
    pair: (f32, f32),
    named: BTreeMap<String, f32>,
    maybe: Option<f32>,
    maybe_radii: Option<[f32; 2]>,
    tagged: Vec<Tagged>,
    nested: Option<Box<Shapes>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Tagged {
    Unit,
    New(f32),
    Tuple(f32, f64),
    Fields { w: f32, a: f32 },
}

fn all_bad(bad: f32) -> Shapes {
    Shapes {
        plain: bad,
        double: f64::from(bad),
        opacity: bad,
        radii: [1.0, bad, 3.0, bad],
        dashes: vec![bad, 2.0],
        pair: (bad, 5.0),
        named: BTreeMap::from([("k".to_string(), bad)]),
        maybe: Some(bad),
        maybe_radii: Some([bad, 7.0]),
        tagged: vec![Tagged::Unit, Tagged::New(bad), Tagged::Tuple(bad, f64::from(bad)), Tagged::Fields { w: bad, a: bad }],
        nested: Some(Box::new(Shapes { plain: bad, opacity: 0.5, ..Shapes::default() })),
    }
}

#[test]
fn finite_values_are_written_exactly_like_serde_json() {
    let value = Shapes { plain: 1.5, double: 2.25, opacity: 0.5, radii: [1.0; 4], maybe: Some(3.0), ..Shapes::default() };
    let (text, replaced) = to_string_finite(&value).unwrap();
    assert_eq!(text, serde_json::to_string(&value).unwrap());
    assert_eq!(replaced, 0);

    let mut doc = Document::new("finite");
    let page = doc.add_page("P");
    let id = doc.alloc_id();
    doc.insert(page, Node::rectangle(id, Rect::new(1.0, 2.0, 30.0, 40.0))).unwrap();
    let (text, replaced) = doc.to_json_counted().unwrap();
    assert_eq!(text, serde_json::to_string(&doc).unwrap());
    assert_eq!(replaced, 0);
}

#[test]
fn every_container_shape_is_checked_and_the_text_parses_back_strictly() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let (text, replaced) = to_string_finite(&all_bad(bad)).unwrap();
        // Three scalars, two radii, one dash, one pair entry, one map entry, one optional-array entry,
        // five enum fields and one nested number; the plain `Option<f32>` is left as null.
        assert_eq!(replaced, 15, "{bad}: {text}");
        let back: Shapes = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{bad}: {e}: {text}"));
        assert_eq!(back.plain, 0.0);
        assert_eq!(back.opacity, 1.0, "opacity is neutral at one");
        assert_eq!(back.radii, [1.0, 0.0, 3.0, 0.0]);
        assert_eq!(back.dashes, vec![0.0, 2.0]);
        assert_eq!(back.pair, (0.0, 5.0));
        assert_eq!(back.named["k"], 0.0);
        assert_eq!(back.maybe_radii, Some([0.0, 7.0]));
        assert_eq!(back.tagged[3], Tagged::Fields { w: 0.0, a: 1.0 }, "an alpha channel is neutral at one");
        assert_eq!(back.nested.unwrap().plain, 0.0);
        assert_eq!(back.maybe, None, "a non-finite Option<f32> is written as null and reads as None");
    }
}

#[test]
fn a_null_number_reads_as_the_neutral_value_and_is_counted() {
    let text = r#"{"plain":null,"double":null,"opacity":null,"radii":[1,null,3,null],"dashes":[null,2],"pair":[null,5],
        "named":{"k":null},"maybe":null,"maybe_radii":[null,7],
        "tagged":["Unit",{"New":null},{"Tuple":[null,null]},{"Fields":{"w":null,"a":null}}],
        "nested":{"plain":null,"double":0,"opacity":0.5,"radii":[0,0,0,0],"dashes":[],"pair":[0,0],"named":{},"maybe":null,"maybe_radii":null,"tagged":[],"nested":null}}"#;
    assert!(serde_json::from_str::<Shapes>(text).is_err(), "the strict parse must still refuse it");
    let (value, replaced): (Shapes, usize) = from_str_lenient(text).unwrap();
    assert_eq!(replaced, 15);
    let expected = all_bad(0.0);
    let expected = Shapes {
        opacity: 1.0,
        tagged: vec![Tagged::Unit, Tagged::New(0.0), Tagged::Tuple(0.0, 0.0), Tagged::Fields { w: 0.0, a: 1.0 }],
        maybe: None,
        ..expected
    };
    assert_eq!(value, expected);
}

#[test]
fn lenient_parsing_does_not_hide_other_errors() {
    let wrong_type = from_str_lenient::<Shapes>(r#"{"plain":"text"}"#).unwrap_err();
    assert!(wrong_type.to_string().contains("invalid type: string \"text\", expected f32"), "{wrong_type}");

    #[derive(Debug, Deserialize)]
    struct Named {
        name: String,
    }
    assert_eq!(from_str_lenient::<Named>(r#"{"name":"x"}"#).unwrap().0.name, "x");
    let null_string = from_str_lenient::<Named>(r#"{"name":null}"#).unwrap_err();
    assert!(null_string.to_string().contains("invalid type: null, expected a string"), "{null_string}");
    assert!(from_str_lenient::<Shapes>("{").unwrap_err().is_eof());
    assert!(from_str_lenient::<f32>("1 2").unwrap_err().to_string().contains("trailing characters"));
}

#[test]
fn borrowed_strings_still_borrow() {
    #[derive(Debug, Deserialize)]
    struct Borrowed<'a> {
        #[serde(borrow)]
        name: &'a str,
        value: f32,
    }
    let (b, replaced): (Borrowed, usize) = from_str_lenient(r#"{"name":"plain","value":null}"#).unwrap();
    assert_eq!((b.name, b.value, replaced), ("plain", 0.0, 1));
}

fn document_with_bad_numbers() -> Document {
    let mut doc = Document::new("bad");
    let page = doc.add_page("P");
    let first = doc.alloc_id();
    let first = doc.insert(page, Node::rectangle(first, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    let second = doc.alloc_id();
    let second = doc.insert(page, Node::rectangle(second, Rect::new(5.0, 5.0, 10.0, 10.0))).unwrap();
    let n = doc.get_mut(first).unwrap();
    n.size.x = f32::NAN;
    n.opacity = f32::NAN;
    n.corner_radii[2] = f32::INFINITY;
    n.fills = vec![Paint::solid(Color { r: 1.0, g: f32::NAN, b: 0.0, a: f32::NAN })];
    let n = doc.get_mut(second).unwrap();
    n.transform.tx = f32::NEG_INFINITY;
    n.transform.a = f32::NAN;
    doc
}

#[test]
fn a_document_with_non_finite_numbers_saves_and_reopens_cleanly() {
    let doc = document_with_bad_numbers();
    let (json, replaced) = doc.to_json_counted().unwrap();
    assert_eq!(replaced, 7);
    assert_eq!(doc.to_json().unwrap(), json);

    let (back, report) = Document::from_json_with_report(&json).unwrap();
    assert!(report.is_clean(), "{report:?}: what we write must need no repair");
    let mut ids: Vec<_> = back.nodes.iter().filter(|(_, n)| !matches!(n.kind, crate::NodeKind::Page { .. })).map(|(id, _)| *id).collect();
    ids.sort();
    let first = back.get(ids[0]).unwrap();
    assert_eq!((first.size.x, first.opacity, first.corner_radii[2]), (0.0, 1.0, 0.0));
    let crate::PaintKind::Solid(c) = &first.fills[0].kind else { panic!("solid fill") };
    assert_eq!((c.r, c.g, c.b, c.a), (1.0, 0.0, 0.0, 1.0));
    let second = back.get(ids[1]).unwrap();
    assert_eq!((second.transform.tx, second.transform.a), (0.0, 1.0));
}

#[test]
fn a_file_with_null_numbers_opens_and_reports_the_repair() {
    // Exactly what versions before the fix wrote: serde_json's own `null` for every non-finite number.
    let legacy = serde_json::to_string(&document_with_bad_numbers()).unwrap();
    assert!(legacy.contains("null"));
    let strict = serde_json::from_str::<Document>(&legacy).unwrap_err();
    assert!(strict.to_string().contains("invalid type: null, expected f32"), "{strict}");

    let (doc, report) = Document::from_json_with_report(&legacy).unwrap();
    assert_eq!(report.non_finite_numbers, 7);
    assert!(!report.is_clean());
    assert_eq!(report.structural_summary(), None, "not a node-graph defect, so callers that refuse repaired graphs still open it");
    let first = doc.nodes.values().find(|n| n.size.x == 0.0 && n.size.y == 10.0).expect("repaired rectangle");
    assert_eq!(first.opacity, 1.0);
    assert_eq!(Document::from_json(&legacy).unwrap().nodes.len(), doc.nodes.len());

    // Saving it again leaves no null number behind.
    let (resaved, replaced) = doc.to_json_counted().unwrap();
    assert_eq!(replaced, 0);
    assert!(Document::from_json_with_report(&resaved).unwrap().1.is_clean());
}

#[test]
fn a_null_in_a_non_number_slot_is_still_an_error() {
    let mut value: serde_json::Value = serde_json::from_str(&Document::new("x").to_json().unwrap()).unwrap();
    value["name"] = serde_json::Value::Null;
    let err = Document::from_json_with_report(&value.to_string()).unwrap_err();
    assert!(err.to_string().contains("expected a string"), "{err}");
}

#[test]
fn the_neutral_values_match_the_model_defaults() {
    use crate::finite_json::neutral_for;
    use crate::{Affine, NodeId};
    let rect = Node::rectangle(NodeId(1), Rect::new(0.0, 0.0, 1.0, 1.0));
    assert_eq!(f64::from(rect.opacity), neutral_for("opacity"));
    assert_eq!(f64::from(Paint::solid(Color::BLACK).opacity), neutral_for("opacity"));
    assert_eq!(f64::from(Color::BLACK.a), neutral_for("a"));
    assert_eq!((f64::from(Affine::IDENTITY.a), f64::from(Affine::IDENTITY.d)), (neutral_for("a"), neutral_for("d")));
    assert_eq!(neutral_for("width"), 0.0, "every other number is neutral at zero");
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Styled {
    opacity: f32,
    width: f32,
}

#[test]
fn only_struct_fields_get_a_neutral_of_one_never_map_values() {
    // A struct's `opacity` reads one; its other numbers read zero.
    let (styled, replaced): (Styled, usize) = from_str_lenient(r#"{"opacity":null,"width":null}"#).unwrap();
    assert_eq!((styled, replaced), (Styled { opacity: 1.0, width: 0.0 }, 2));

    // A map value reads zero whatever its key says, and writing a NaN there gives the same zero.
    let (map, replaced): (BTreeMap<String, f32>, usize) = from_str_lenient(r#"{"a":null,"d":null,"opacity":null,"x":null}"#).unwrap();
    assert_eq!(replaced, 4);
    assert!(map.values().all(|v| *v == 0.0), "{map:?}");
    let written: BTreeMap<String, f32> = ["a", "d", "opacity", "x"].into_iter().map(|k| (k.to_string(), f32::NAN)).collect();
    let (text, replaced) = to_string_finite(&written).unwrap();
    assert_eq!(replaced, 4);
    assert_eq!(serde_json::from_str::<BTreeMap<String, f32>>(&text).unwrap(), map, "read and write agree for a map");

    // Read and write agree for a struct as well.
    let (text, _) = to_string_finite(&Styled { opacity: f32::NAN, width: f32::NAN }).unwrap();
    assert_eq!(serde_json::from_str::<Styled>(&text).unwrap(), Styled { opacity: 1.0, width: 0.0 });

    // A map key does not leak into the struct that is its value.
    let (nested, _): (BTreeMap<String, Styled>, usize) = from_str_lenient(r#"{"opacity":{"opacity":null,"width":null}}"#).unwrap();
    assert_eq!(nested["opacity"], Styled { opacity: 1.0, width: 0.0 });
}

#[test]
fn only_a_null_number_error_triggers_the_second_parse() {
    use crate::finite_json::{from_str_repairing, lenient_parses_on_this_thread as lenient_parses};

    #[derive(Debug, Deserialize)]
    struct Slots {
        name: String,
        count: u32,
        value: f32,
        wide: f64,
    }
    let unrelated = [
        r#"{"name":"n","count":1,"value":"text","wide":2.5}"#, // a string where a number belongs
        r#"{"name":null,"count":1,"value":1.5,"wide":2.5}"#,   // null for a string
        r#"{"name":"n","count":null,"value":1.5,"wide":2.5}"#, // null for an integer
        r#"{"name":"n","count":1,"value":1.5}"#,               // a missing field
        r#"{"name":"n","count":1,"value":1.5,"wide":[}"#,      // a syntax error
        r#"{"name":"n","count":1,"value":1.5,"wide":2.5"#,     // truncated text
    ];
    for text in unrelated {
        let before = lenient_parses();
        assert!(from_str_repairing::<Slots>(text).is_err(), "{text}");
        assert_eq!(lenient_parses(), before, "{text}: an unrelated error must fail after the strict parse alone");
    }

    let repairable = [(r#"{"name":"n","count":1,"value":null,"wide":2.5}"#, 0.0, 2.5), (r#"{"name":"n","count":1,"value":1.5,"wide":null}"#, 1.5, 0.0)];
    for (text, value, wide) in repairable {
        let before = lenient_parses();
        let (slots, replaced) = from_str_repairing::<Slots>(text).unwrap();
        assert_eq!((replaced, lenient_parses() - before), (1, 1), "{text}: a null number takes exactly one lenient parse");
        assert_eq!((slots.name.as_str(), slots.count, slots.value, slots.wide), ("n", 1, value, wide), "{text}");
    }

    // The same through a document: a wrong type fails fast.
    let before = lenient_parses();
    let mut value: serde_json::Value = serde_json::from_str(&Document::new("x").to_json().unwrap()).unwrap();
    value["name"] = serde_json::json!(5);
    assert!(Document::from_json_with_report(&value.to_string()).is_err());
    assert_eq!(lenient_parses(), before);
}

#[test]
fn null_number_detection_does_not_rely_on_error_message_text() {
    use crate::finite_json::{from_str_repairing, lenient_parses_on_this_thread as lenient_parses};
    use serde::de::{Deserializer, Visitor};
    use std::fmt;

    #[derive(Debug, PartialEq)]
    struct RewordedFloat32(f32);

    impl<'de> Deserialize<'de> for RewordedFloat32 {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct V;
            impl Visitor<'_> for V {
                type Value = RewordedFloat32;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a 32-bit floating-point number")
                }
                fn visit_f32<E: serde::de::Error>(self, v: f32) -> Result<Self::Value, E> {
                    Ok(RewordedFloat32(v))
                }
                fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                    Ok(RewordedFloat32(v as f32))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                    Ok(RewordedFloat32(v as f32))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                    Ok(RewordedFloat32(v as f32))
                }
            }
            deserializer.deserialize_f32(V)
        }
    }

    #[derive(Debug, PartialEq)]
    struct RewordedFloat64(f64);

    impl<'de> Deserialize<'de> for RewordedFloat64 {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct V;
            impl Visitor<'_> for V {
                type Value = RewordedFloat64;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a 64-bit floating-point number")
                }
                fn visit_f32<E: serde::de::Error>(self, v: f32) -> Result<Self::Value, E> {
                    Ok(RewordedFloat64(f64::from(v)))
                }
                fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                    Ok(RewordedFloat64(v))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                    Ok(RewordedFloat64(v as f64))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                    Ok(RewordedFloat64(v as f64))
                }
            }
            deserializer.deserialize_f64(V)
        }
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct RewordedRecord {
        opacity: RewordedFloat32,
        width: RewordedFloat64,
    }

    // Even when the error text does not end with "expected f32" / "expected f64", a null in an
    // f32/f64 slot is classified structurally and repaired on the lenient second pass.
    let before = lenient_parses();
    let (rec, replaced) = from_str_repairing::<RewordedRecord>(r#"{"opacity":null,"width":null}"#).unwrap();
    assert_eq!((rec, replaced), (RewordedRecord { opacity: RewordedFloat32(1.0), width: RewordedFloat64(0.0) }, 2));
    assert_eq!(lenient_parses() - before, 1);

    // A non-null wrong type (including the string "null") in the same float slot still fails on the
    // strict pass without starting a second parse.
    for bad in [r#"{"opacity":"null","width":1.0}"#, r#"{"opacity":"text","width":1.0}"#, r#"{"opacity":true,"width":1.0}"#] {
        let before = lenient_parses();
        assert!(from_str_repairing::<RewordedRecord>(bad).is_err(), "{bad}");
        assert_eq!(lenient_parses(), before, "{bad}: non-null wrong type in float slot must not trigger lenient parse");
    }
}

#[test]
fn strict_pass_skips_key_wrapping_on_large_document_and_reports_overhead() {
    use crate::finite_json::{from_str_repairing, lenient_parses_on_this_thread as lenient_parses, wrapped_str_visits_on_this_thread as wrapped_str_visits};
    use crate::{AutoLayout, Effect, LayoutDirection, NodeKind, Stroke, Vec2};
    use std::time::Instant;

    let mut doc = Document::new("large-strict-bench");
    let page = doc.add_page("Page 1");
    for i in 0..2_500u32 {
        let id = doc.alloc_id();
        let mut node = Node::frame(id, Rect::new((i % 50) as f32 * 24.0, (i / 50) as f32 * 24.0, 180.0, 96.0));
        node.name = format!("Frame {i}");
        node.fills = vec![Paint::solid(Color { r: 0.15, g: 0.45, b: 0.85, a: 0.95 })];
        node.stroke = Stroke { paints: vec![Paint::solid(Color::BLACK)], weight: 1.5, ..Stroke::default() };
        node.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 },
            offset: Vec2::new(0.0, 4.0),
            radius: 12.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
        node.corner_radii = [8.0, 8.0, 8.0, 8.0];
        if let NodeKind::Frame(container) = &mut node.kind {
            container.auto_layout =
                Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 8.0, padding: [12.0, 16.0, 12.0, 16.0], ..AutoLayout::default() });
        }
        doc.insert(page, node).unwrap();
    }

    let (json, replaced_out) = doc.to_json_counted().unwrap();
    assert_eq!(replaced_out, 0);
    assert!(json.len() > 1_000_000, "expected >1 MB fixture, got {} bytes", json.len());

    // Warm up both paths once.
    let plain_warm: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(plain_warm.nodes.len(), doc.nodes.len());

    let lenient_before = lenient_parses();
    let str_visits_before = wrapped_str_visits();
    let (strict_warm, repaired_warm): (Document, usize) = from_str_repairing(&json).unwrap();
    let strict_str_visits = wrapped_str_visits() - str_visits_before;
    assert_eq!(repaired_warm, 0);
    assert_eq!(strict_warm.nodes.len(), doc.nodes.len());
    assert_eq!(lenient_parses(), lenient_before, "clean document must not run lenient fallback");
    assert_eq!(strict_str_visits, 0, "strict pass must not wrap struct/map keys or string values (saw {strict_str_visits} wrapped string visits)");

    let runs = 5;
    let mut plain_best = std::time::Duration::MAX;
    let mut strict_best = std::time::Duration::MAX;
    for _ in 0..runs {
        let t0 = Instant::now();
        let d_plain: Document = serde_json::from_str(&json).unwrap();
        let dt_plain = t0.elapsed();
        assert_eq!(d_plain.nodes.len(), doc.nodes.len());
        plain_best = plain_best.min(dt_plain);

        let t1 = Instant::now();
        let (d_strict, rep): (Document, usize) = from_str_repairing(&json).unwrap();
        let dt_strict = t1.elapsed();
        assert_eq!((d_strict.nodes.len(), rep), (doc.nodes.len(), 0));
        strict_best = strict_best.min(dt_strict);
    }

    let plain_ms = plain_best.as_secs_f64() * 1000.0;
    let strict_ms = strict_best.as_secs_f64() * 1000.0;
    let ratio = strict_ms / plain_ms.max(0.001);
    eprintln!(
        "[#99 finite_json bench] {} bytes ({} nodes): serde_json={plain_ms:.2}ms vs from_str_strict={strict_ms:.2}ms (ratio={ratio:.2}x)",
        json.len(),
        doc.nodes.len()
    );
    // The overhead is reported, not asserted: a wall-clock ratio of two ~30 ms parses flakes on shared runners
    // (1.18x measured idle, 1.52x on macos-14 CI, 2.3x and more on a loaded laptop). What this test guards is
    // structural: the strict pass visits no wrapped strings and never falls back to the lenient parse (asserted
    // above). The `json_roundtrip_bench` example measures the ratio on demand.
}

#[test]
fn counting_non_finite_numbers_matches_a_save_and_builds_no_text() {
    let doc = document_with_bad_numbers();
    let before = crate::finite_json::serialize_calls_on_this_thread();
    assert_eq!(doc.count_non_finite().unwrap(), 7);
    assert_eq!(crate::finite_json::count_non_finite(&all_bad(f32::NAN)).unwrap(), 15, "same rules as the save, `Option` slots included");
    assert_eq!(crate::finite_json::serialize_calls_on_this_thread(), before, "counting must not build a JSON text");
    assert_eq!(Document::new("clean").count_non_finite().unwrap(), 0);

    let _ = doc.to_json_counted().unwrap();
    assert_eq!(crate::finite_json::serialize_calls_on_this_thread(), before + 1, "a save builds exactly one text");
}
