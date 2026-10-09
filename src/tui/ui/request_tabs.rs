use ratatui::{Frame, layout::Rect};
use crate::app::app::App;
use crate::tui::workspace::Pane;

impl App<'_> {
    pub fn pane_title(&self, pane: Pane) -> String {
        if self.pane == pane { format!("▶ {}", pane.label()) }
        else { pane.label().to_string() }
    }

    pub fn render_request_tabs(&self, frame: &mut Frame, area: Rect) {
        let labels = self.request_tabs.iter().map(|tab| {
            let request = tab.request.read();
            let kind = match &request.protocol {
                crate::models::protocol::protocol::Protocol::HttpRequest(http) => http.method.to_string(),
                crate::models::protocol::protocol::Protocol::WsRequest(_) => "WS".into(),
            };
            format!("{} {}{} ×", kind, if request.is_pending { "… " } else { "" }, super::tabs::clip_text(&request.name, 48))
        }).collect::<Vec<_>>();
        super::tabs::render_tabs(frame, area, &labels, self.active_request_tab.unwrap_or(0), true);
    }
}
