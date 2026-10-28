//! Original, fully editable three-slide sample for `--fixture deck`.
#![cfg(any(test, feature = "fixtures"))]
use designos_doc::*;

fn color(hex: u32) -> Color {
    Color::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn text(doc: &mut Document, parent: NodeId, at: (f32, f32), content: &str, size: f32, weight: u16, ink: u32) {
    let id = doc.alloc_id();
    let style = TextStyle { font_size: size, font_weight: weight, fills: vec![Paint::solid(color(ink))], ..Default::default() };
    let mut node = Node::text(id, Rect::new(at.0, at.1, 10.0, 10.0), content, style);
    if let NodeKind::Text(props) = &node.kind {
        node.size = crate::layout::estimate_text_size(props);
    }
    node.name = content.into();
    doc.insert(parent, node).unwrap();
}

fn rect(doc: &mut Document, parent: NodeId, bounds: Rect, fill: u32, radius: f32) {
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, bounds);
    node.fills = vec![Paint::solid(color(fill))];
    node.corner_radii = [radius; 4];
    doc.insert(parent, node).unwrap();
}

fn circle(doc: &mut Document, parent: NodeId, bounds: Rect, fill: u32) {
    let id = doc.alloc_id();
    let mut node = Node::ellipse(id, bounds);
    node.fills = vec![Paint::solid(color(fill))];
    doc.insert(parent, node).unwrap();
}

fn slide(doc: &mut Document, number: usize, name: &str, fill: u32, notes: &str) -> NodeId {
    let page = doc.pages[0];
    let id = doc.alloc_id();
    let mut node = super::slide_node(id, number);
    node.name = name.into();
    node.transform.tx = (number - 1) as f32 * (super::SLIDE_WIDTH + 120.0);
    node.fills = vec![Paint::solid(color(fill))];
    node.slide.as_mut().unwrap().notes = notes.into();
    doc.insert(page, node).unwrap()
}

pub fn document() -> Document {
    let mut doc = Document::new("A little more possibility");
    doc.kind = DocumentKind::Deck;
    doc.add_page("Slides");
    let first = slide(
        &mut doc,
        1,
        "Make room for possibility",
        0x171D31,
        "Welcome everyone. This deck is built from editable frames, shapes, and text. Start with the idea: good tools give people room to think.",
    );
    text(&mut doc, first, (80.0, 60.0), "DESIGNOS DECKS  /  FIELD NOTES", 17.0, 600, 0xB4BBCE);
    text(&mut doc, first, (80.0, 210.0), "Make room", 83.0, 700, 0xFFFFFF);
    text(&mut doc, first, (80.0, 310.0), "for possibility.", 83.0, 700, 0xFFFFFF);
    rect(&mut doc, first, Rect::new(82.0, 460.0, 76.0, 5.0), 0xC7EF7D, 2.0);
    text(&mut doc, first, (80.0, 500.0), "A canvas for ideas. A story worth sharing.", 25.0, 400, 0xBEC6DA);
    text(&mut doc, first, (80.0, 642.0), "DESIGN EXPLORATIONS     /     OCTOBER 2026", 15.0, 500, 0x8E9AB7);
    // Geometric sculpture: an original arrangement of editable native shapes.
    circle(&mut doc, first, Rect::new(955.0, 155.0, 230.0, 230.0), 0xC7EF7D);
    rect(&mut doc, first, Rect::new(895.0, 305.0, 170.0, 250.0), 0x9890F5, 85.0);
    rect(&mut doc, first, Rect::new(1065.0, 405.0, 160.0, 150.0), 0xF2AFA1, 20.0);
    circle(&mut doc, first, Rect::new(1110.0, 451.0, 67.0, 67.0), 0x171D31);

    let second = slide(
        &mut doc,
        2,
        "Three moves that matter",
        0xF5F3EC,
        "Walk through the three stages. Gather without judging; shape the strongest thread; share early enough for feedback to matter.",
    );
    text(&mut doc, second, (80.0, 60.0), "THE APPROACH", 17.0, 600, 0x686F80);
    text(&mut doc, second, (80.0, 125.0), "Three moves that matter.", 64.0, 700, 0x171D31);
    for (index, title, subtitle, fill) in [
        (0, "Gather", "Make space for different ideas.", 0xE2EDCC),
        (1, "Shape", "Find the thread worth following.", 0xE0DDF7),
        (2, "Share", "Let the story start a conversation.", 0xF3D8CF),
    ] {
        let x = 80.0 + index as f32 * 382.0;
        rect(&mut doc, second, Rect::new(x, 280.0, 354.0, 314.0), fill, 22.0);
        text(&mut doc, second, (x + 28.0, 309.0), &format!("0{}", index + 1), 23.0, 500, 0x686F80);
        text(&mut doc, second, (x + 28.0, 430.0), title, 43.0, 700, 0x171D31);
        text(&mut doc, second, (x + 28.0, 513.0), subtitle, 16.0, 400, 0x41495B);
    }
    text(&mut doc, second, (80.0, 652.0), "DESIGNOS DECKS", 15.0, 600, 0x686F80);
    text(&mut doc, second, (1150.0, 652.0), "02 / 03", 15.0, 500, 0x686F80);

    let third = slide(
        &mut doc,
        3,
        "Start with one good question",
        0xC7EF7D,
        "Pause and invite the audience in. Ask which idea they would explore first. Leave the question on screen while people respond.",
    );
    text(&mut doc, third, (80.0, 60.0), "YOUR NEXT CHAPTER", 17.0, 600, 0x3B5130);
    text(&mut doc, third, (80.0, 215.0), "What will you", 89.0, 700, 0x171D31);
    text(&mut doc, third, (80.0, 323.0), "make possible?", 89.0, 700, 0x171D31);
    rect(&mut doc, third, Rect::new(80.0, 511.0, 325.0, 62.0), 0x171D31, 31.0);
    text(&mut doc, third, (109.0, 527.0), "Start with one good question.", 18.0, 500, 0xFFFFFF);
    circle(&mut doc, third, Rect::new(1030.0, 480.0, 300.0, 300.0), 0x171D31);
    circle(&mut doc, third, Rect::new(1080.0, 530.0, 200.0, 200.0), 0x9890F5);
    text(&mut doc, third, (80.0, 652.0), "DESIGNOS DECKS     /     THANK YOU", 15.0, 600, 0x3B5130);
    doc
}
