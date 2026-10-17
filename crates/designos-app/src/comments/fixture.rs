//! `--fixture comments`: the demo document with Figma comments imported from
//! `fixtures/figma_comments.json` plus a few native threads, an open thread and the
//! comment tool active. For screenshots.

use super::{import, model, time};
use crate::app::App;
use crate::canvas::tools::Tool;
use designos_doc::{Comment, CommentThread, NodeId, Vec2};

/// Layers the fixture's Figma node ids refer to.
const SOURCE_IDS: [(&str, &str); 3] = [("Welcome", "10:1"), ("Hero", "10:2"), ("Card", "10:3")];

fn msg(id: &str, author: &str, text: &str, minutes_ago: u64) -> Comment {
    let at = time::now_ms().saturating_sub(minutes_ago * 60_000);
    Comment { id: id.into(), author: author.into(), author_id: None, author_color: None, text: text.into(), created: time::format_rfc3339(at), edited: None }
}

pub fn apply(app: &mut App) {
    for (name, sid) in SOURCE_IDS {
        if let Some(n) = app.doc.nodes.values_mut().find(|n| n.name == name) {
            n.source_id = Some(sid.into());
        }
    }
    app.comments.author = Some("Kir".into());
    let named = |app: &App, name: &str| app.doc.nodes.values().find(|n| n.name == name).map(|n| n.id);
    let page = app.page;
    let mut threads: Vec<CommentThread> = Vec::new();
    if let Ok((mut figma, _)) = import::parse_threads(&app.doc, import::FIXTURE) {
        // The export is years old; make the first conversation look recent.
        if let Some(t) = figma.first_mut() {
            let ago = [48, 12];
            for (m, a) in t.messages.iter_mut().zip(ago) {
                m.created = time::format_rfc3339(time::now_ms().saturating_sub(a * 60_000));
            }
            t.messages.push(msg("fx-1", "Kir", "I bumped it to 40px, can you check the contrast on the dark theme as well?", 5));
        }
        threads.extend(figma);
    }
    let mut native = |node: Option<NodeId>, point: (f32, f32), who: &str, text: &str, ago: u64, id: &str| {
        let anchor = model::make_anchor(&app.doc, page, Vec2::new(point.0, point.1), node);
        threads.push(CommentThread { id: id.into(), anchor, messages: vec![msg(&format!("{id}-0"), who, text, ago)], resolved: false });
    };
    let card = named(app, "Card");
    let card_pos = card.map(|c| crate::util::world_aabb(&app.doc, c)).map(|b| (b.x + b.w - 30.0, b.y + 24.0)).unwrap_or((400.0, 100.0));
    native(card, card_pos, "Kir", "Use the shared card shadow here.", 90, "fx-card");
    native(None, (-120.0, 180.0), "Marcus", "Idea: a second avatar row on the side.", 1500, "fx-free");
    app.doc.comments = threads;
    app.tool = Tool::Comment;
    app.comments.open = app.doc.comments.first().map(|t| t.id.clone());
}
