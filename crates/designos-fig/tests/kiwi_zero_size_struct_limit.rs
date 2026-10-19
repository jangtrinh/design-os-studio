//! A struct made only of empty structs encodes to zero bytes, so a tiny message can claim millions of them. The
//! decoder must refuse that long before it materialises gigabytes, while still decoding genuine messages.

use designos_fig::kiwi::{DefKind, FieldType, Native, Schema, SchemaBuilder};

fn varint(mut v: u32, out: &mut Vec<u8>) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}

/// `Message { a: E[] }` where `E` is an empty struct (`wide == 0`) or a struct of `wide` empty-struct fields.
fn schema_with_array_of_empty(wide: usize) -> (Schema, usize) {
    let mut b = SchemaBuilder::new();
    let empty = b.define("Empty", DefKind::Struct);
    let elem = if wide == 0 {
        empty
    } else {
        let w = b.define("Wide", DefKind::Struct);
        for i in 0..wide {
            b.field(w, &format!("f{i}"), FieldType::Def(empty), false, i as u32 + 1);
        }
        w
    };
    let m = b.define("Message", DefKind::Message);
    b.field(m, "a", FieldType::Def(elem), true, 1);
    (b.build(), m)
}

/// `repeats` occurrences of field `a`, each claiming `claimed` elements, backed by `claimed` bytes of padding
/// (the decoder only accepts a claimed count that fits the remaining input).
fn message_claiming(repeats: usize, claimed: usize) -> Vec<u8> {
    let mut msg = Vec::new();
    for _ in 0..repeats {
        varint(1, &mut msg);
        varint(claimed as u32, &mut msg);
    }
    msg.push(0);
    msg.extend(std::iter::repeat_n(0u8, claimed));
    msg
}

#[test]
fn rejects_array_of_empty_structs_beyond_the_zero_size_cap() {
    let (schema, m) = schema_with_array_of_empty(0);
    // Honest 200 KB input with 200k empty structs decodes under the input-scaled cap (16 * input_len),
    // while 20 arrays x 10k empty structs in ~10 KB (200k > 16 * 10k) is rejected.
    let honest = message_claiming(1, 200_000);
    assert!(schema.decode_message(m, &honest).is_ok());
    let msg = message_claiming(20, 10_000);
    let err = schema.decode_message(m, &msg).unwrap_err();
    assert!(err.to_string().contains("zero-size"), "{err}");
}

#[test]
fn rejects_array_of_wide_empty_structs_beyond_the_zero_size_cap() {
    // Each Wide element is 1 + 16 = 17 zero-size instances, exceeding 16 * input_len in a single array.
    let (schema, m) = schema_with_array_of_empty(16);
    let msg = message_claiming(1, 100_000);
    let err = schema.decode_message(m, &msg).unwrap_err();
    assert!(err.to_string().contains("zero-size"), "{err}");
}

#[test]
fn zero_size_cap_spans_the_whole_message_not_a_single_array() {
    // 40 occurrences of the field, each well under the cap on its own: the total is what counts.
    let (schema, m) = schema_with_array_of_empty(0);
    let msg = message_claiming(40, 5_000);
    assert!(schema.decode_message(m, &msg).is_err());
}

#[test]
fn zero_size_structs_within_the_cap_still_decode() {
    let (schema, m) = schema_with_array_of_empty(0);
    let msg = message_claiming(1, 10_000);
    let v = schema.decode_message(m, &msg).unwrap();
    assert_eq!(v.get("a").and_then(|a| a.as_array()).map(<[_]>::len), Some(10_000));
}

#[test]
fn structs_that_take_bytes_are_not_subject_to_the_zero_size_cap() {
    // `Pair { e: Empty, n: uint }` takes a byte per element, so it is not a zero-size struct itself; only its `Empty` field is (one instance per element).
    let mut b = SchemaBuilder::new();
    let empty = b.define("Empty", DefKind::Struct);
    let pair = b.define("Pair", DefKind::Struct);
    b.field(pair, "e", FieldType::Def(empty), false, 1);
    b.field(pair, "n", FieldType::Native(Native::Uint), false, 2);
    let m = b.define("Message", DefKind::Message);
    b.field(m, "a", FieldType::Def(pair), true, 1);
    let schema = b.build();

    let mut msg = Vec::new();
    varint(1, &mut msg);
    varint(30_000, &mut msg);
    msg.extend(std::iter::repeat_n(7u8, 30_000));
    msg.push(0);
    let v = schema.decode_message(m, &msg).unwrap();
    assert_eq!(v.get("a").and_then(|a| a.as_array()).map(<[_]>::len), Some(30_000));
}

#[test]
fn array_fields_inside_a_struct_make_it_take_bytes() {
    // `Holder { items: Empty[] }` is not zero-size (its array length is a byte), so nesting it must not be capped as one.
    let mut b = SchemaBuilder::new();
    let empty = b.define("Empty", DefKind::Struct);
    let holder = b.define("Holder", DefKind::Struct);
    b.field(holder, "items", FieldType::Def(empty), true, 1);
    let m = b.define("Message", DefKind::Message);
    b.field(m, "h", FieldType::Def(holder), true, 1);
    let schema = b.build();

    // 20k holders, each with an empty item list (one length byte): none of them is a zero-size instance.
    let mut msg = Vec::new();
    varint(1, &mut msg);
    varint(20_000, &mut msg);
    msg.extend(std::iter::repeat_n(0u8, 20_000));
    msg.push(0);
    let v = schema.decode_message(m, &msg).unwrap();
    assert_eq!(v.get("h").and_then(|a| a.as_array()).map(<[_]>::len), Some(20_000));
}
