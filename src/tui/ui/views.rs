use crate::app::app::App;

#[derive(Copy, Clone, PartialEq)]
pub enum RequestView {
    Normal,
    OnlyResult,
    OnlyParams,
}

impl App<'_> {
    /// Match LazyDB's focused-pane layout when a split would be too narrow.
    pub(super) fn visible_request_view(&self, terminal_width: u16) -> RequestView {
        if terminal_width >= 100 { return self.request_view; }
        if self.pane == crate::tui::workspace::Pane::Response {
            RequestView::OnlyResult
        } else {
            RequestView::OnlyParams
        }
    }

    pub fn next_request_view(&mut self) {
        self.request_view = match self.request_view {
            RequestView::Normal => RequestView::OnlyResult,
            RequestView::OnlyResult => RequestView::OnlyParams,
            RequestView::OnlyParams => RequestView::Normal
        };
        match self.request_view {
            RequestView::OnlyResult => self.focus_pane(crate::tui::workspace::Pane::Response),
            RequestView::OnlyParams => self.focus_pane(crate::tui::workspace::Pane::Parameters),
            RequestView::Normal => {},
        }
    }
}
