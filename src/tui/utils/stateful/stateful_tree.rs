use tui_tree_widget::{TreeItem, TreeState};

#[derive(Default)]
pub struct StatefulTree<'a> {
    pub state: TreeState<usize>,
    pub items: Vec<TreeItem<'a, usize>>,
    pub selected: Option<(usize, usize)>
}

impl StatefulTree<'_> {
    pub fn up(&mut self) {
        self.state.key_up();
    }

    pub fn down(&mut self) {
        self.state.key_down();
    }

}
