use super::*;
use crate::theme;

#[test]
fn left_panel_actions_are_32_and_keep_their_right_edge() {
    let s = Sim::new();
    let pages = s.rect_of("pages-list");
    let sidebar = s.rect_of("sidebar");
    let add = s.rect_of("page-add");
    let search = s.rect_of("page-search");
    let layers_search = s.rect_of("layers-search");
    let collapse = s.rect_of("layers-collapse-all");
    let file = s.rect_of("file-header");
    for (name, r) in [("sidebar", sidebar), ("page-add", add), ("page-search", search), ("layers-search", layers_search), ("layers-collapse-all", collapse)] {
        assert!((r.width() - theme::CONTROL_XS).abs() < 0.1, "{name} width {}", r.width());
        assert!((r.height() - theme::CONTROL_XS).abs() < 0.1, "{name} height {}", r.height());
    }
    let pitch = theme::CONTROL_XS + theme::SPACE_FACT;
    assert!((add.left() - search.left() - pitch).abs() < 0.1, "page button pitch {}", add.left() - search.left());
    assert!((add.left() - search.right() - theme::SPACE_FACT).abs() < 0.1, "page button gap {}", add.left() - search.right());
    assert!((layers_search.left() - collapse.left() - pitch).abs() < 0.1, "layers button pitch");
    assert!((layers_search.left() - collapse.right() - theme::SPACE_FACT).abs() < 0.1, "layers button gap");
    assert!((pages.right() - add.right() - theme::PANEL_ACTION_RIGHT_PAD).abs() < 0.1, "page add right pad");
    assert!((pages.right() - layers_search.right() - theme::PANEL_ACTION_RIGHT_PAD).abs() < 0.1, "layers search right pad");
    assert!((pages.right() - sidebar.right() - theme::LEFT_SIDEBAR_BTN_RIGHT_PAD).abs() < 0.1, "sidebar right pad");
    assert!(file.right() <= sidebar.left() - theme::SPACE_FACT + 0.1, "file name runs under the sidebar button: file {:?} sidebar {:?}", file, sidebar);
    let file_left = theme::RAIL_WIDTH + theme::BORDER_WIDTH + theme::INSET_FRAME;
    assert!((file.left() - file_left).abs() < 0.1, "file name left edge {}", file.left());
}
