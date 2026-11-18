//! Figma's saved glyph outlines are on only a few text layers, so the field that holds them must cost every other
//! node no more than one pointer.

use designos_doc::Node;

fn size_of_field<T>(_: fn(&Node) -> &T) -> usize {
    std::mem::size_of::<T>()
}

#[test]
fn saved_glyphs_add_one_pointer_to_a_node() {
    let field = size_of_field(|n| &n.fig_text_geometry);
    assert!(field <= 8, "`Node::fig_text_geometry` takes {field} bytes inline in every node (the whole node is {})", std::mem::size_of::<Node>());
}
