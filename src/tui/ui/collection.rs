use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::prelude::Style;
use crate::app::files::theme::THEME;
use tui_tree_widget::{Tree, TreeItem};
use rayon::prelude::*;

use crate::app::app::App;

impl<'a> App<'a> {
    pub(super) fn render_collections(&mut self, frame: &mut Frame, rect: Rect) {
        let items: Vec<TreeItem<'a, usize>> = self.collections
            .par_iter()
            .enumerate()
            .map(|(collection_index, request)| {
                request.to_tree_item(collection_index)
            })
            .collect();

        
        self.collections_tree.items = items;
        let tree_items = self.collections_tree.items.clone();
        
        let collections_tree = Tree::new(&tree_items)
            .unwrap()
            .style(Style::new().fg(THEME.read().ui.font_color).bg(THEME.read().ui.main_background_color))
            .highlight_style(THEME.read().selected())
            .highlight_symbol("› ")
            .node_closed_symbol("")
            .node_no_children_symbol("")
            .block(
                THEME.read().panel(self.pane_title(crate::tui::workspace::Pane::Collections), self.pane == crate::tui::workspace::Pane::Collections)
            );

        frame.render_stateful_widget(
            collections_tree,
            rect,
            &mut self.collections_tree.state
        );
    }
}