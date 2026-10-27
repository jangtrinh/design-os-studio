//! Which serialized fields of a node an edit changed, found without turning the node into JSON.
//!
//! A palette operation commits tens of thousands of nodes in one step, and the record of what the user
//! edited looks at every one of them on the UI thread. Two `serde_json::Value`s per node cost about a
//! hundred microseconds each for a page of vectors and instances, which made one click take seconds.
//!
//! The answer is the same as comparing the two nodes' JSON object by object, key by key (`id`, `parent` and
//! `children` left out), and is found in two steps:
//!
//! 1. Compare the nodes field by field with `!=`. Equal fields serialize alike, so only the fields that
//!    differ can be in the answer, and a node that did not change at all stops here.
//! 2. Encode just those fields from both nodes (see [`encode`]) and compare the bytes. A field can differ
//!    as a value and still serialize alike: NaN and infinity are both `null`, and a value that
//!    `skip_serializing_if` treats as the default is left out whatever it is. Only the encoding says which.
//!
//! The field list below is checked against `Node` at compile time: a field added to `Node` does not
//! compile until it is listed here or named as one the diff leaves out.

mod encode;

use designos_doc::Node;
use encode::Tape;
use std::cell::RefCell;

/// More fields than `Node` has, so the field sets below never allocate.
const MAX_FIELDS: usize = 48;

/// Past this many bytes a reused buffer is released after use instead of kept.
const KEPT_BUFFER_BYTES: usize = 1 << 20;

/// A short list of field names on the stack.
struct FieldSet {
    names: [&'static str; MAX_FIELDS],
    len: usize,
}

impl FieldSet {
    fn new() -> Self {
        Self { names: [""; MAX_FIELDS], len: 0 }
    }

    fn push(&mut self, name: &'static str) {
        self.names[self.len] = name;
        self.len += 1;
    }

    fn as_slice(&self) -> &[&'static str] {
        &self.names[..self.len]
    }
}

thread_local! {
    /// Encodings of the node before and after. Reused for every node of a step.
    static TAPES: RefCell<(Tape, Tape)> = RefCell::new((Tape::default(), Tape::default()));
}

#[cfg(test)]
thread_local! {
    static FIELD_PASSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ENCODED_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many nodes this thread has compared field by field so far (a node that did not change is not): work a
/// test can read before and after instead of timing it.
#[cfg(test)]
pub(crate) fn field_passes_on_this_thread() -> usize {
    FIELD_PASSES.with(std::cell::Cell::get)
}

/// Bytes this thread has encoded for comparison so far: the work the second step did, counted for the same reason.
#[cfg(test)]
pub(crate) fn encoded_bytes_on_this_thread() -> usize {
    ENCODED_BYTES.with(std::cell::Cell::get)
}

/// Lists, in `Node`'s own order, the names of the fields whose values differ between `before` and `after`.
/// `id`, `parent` and `children` are never listed: they say who the node is and where it sits.
macro_rules! differing_fields {
    ($before:expr, $after:expr; $($field:ident),+ $(,)?) => {{
        let (before, after): (&Node, &Node) = ($before, $after);
        // Every field of `Node` is named in this pattern, so `Node` gaining one stops the build here.
        let Node { id: _, parent: _, children: _, $($field: _),+ } = before;
        const _: () = assert!([$(stringify!($field)),+].len() <= MAX_FIELDS);
        let mut set = FieldSet::new();
        $(if before.$field != after.$field {
            set.push(stringify!($field));
        })+
        set
    }};
}

/// Calls `report` once for each serialized field of `Node` that `serde_json::to_value` would give a
/// different value for in `before` and `after`, in no particular order.
pub(crate) fn for_each_changed_field(before: &Node, after: &Node, mut report: impl FnMut(&'static str)) {
    if before == after {
        return;
    }
    #[cfg(test)]
    FIELD_PASSES.with(|n| n.set(n.get() + 1));
    let candidates = differing_fields!(before, after;
        source_id, name, kind, visible, locked, opacity, blend, transform, size, fills, stroke, effects, corner_radii,
        corner_smoothing, constraints, layout_child, is_mask, style_refs, fill_geometry, stroke_geometry, component,
        prototype, variables, export_settings, dev, slide, lock_aspect, hyperlink, plugin_data, detached_symbol_id,
        fig_metadata);
    if candidates.len == 0 {
        return;
    }
    let mut changed = FieldSet::new();
    TAPES.with(|tapes| {
        let (before_tape, after_tape) = &mut *tapes.borrow_mut();
        if before_tape.capture(before, candidates.as_slice()) && after_tape.capture(after, candidates.as_slice()) {
            #[cfg(test)]
            ENCODED_BYTES.with(|n| n.set(n.get() + before_tape.len() + after_tape.len()));
            before_tape.for_each_difference(after_tape, |name| changed.push(name));
        }
        before_tape.release_if_larger_than(KEPT_BUFFER_BYTES);
        after_tape.release_if_larger_than(KEPT_BUFFER_BYTES);
    });
    // reported after the tapes are put back, so a `report` that diffs another node does not find them taken
    for &name in changed.as_slice() {
        report(name);
    }
}
