use crate::app::app::App;
use crate::app::files::theme::THEME;
use crate::models::request::Request;
use crate::tui::app_states::AppState;
use crate::tui::ui::views::RequestView;
use ratatui::layout::Direction::{Horizontal, Vertical};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::widgets::{Padding, Paragraph};
use ratatui::Frame;
use crate::tui::utils::stateful::text_input::SingleLineTextInput;

impl App<'_> {
    pub fn render_http_request(&mut self, frame: &mut Frame, rect: Rect, request: Request) {
        let request_layout = Layout::new(
            Vertical,
            [
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Fill(1)
            ],
        )
            .split(rect);

        // REQUEST NAME

        self.render_request_tabs(frame, request_layout[0]);

        // REQUEST HEADER LAYOUT

        let request_header_layout = Layout::new(
            Horizontal,
            [
                Constraint::Length(10),
                Constraint::Min(0)
            ],
        )
            .split(request_layout[1]);

        // REQUEST METHOD

        let http_request = request.get_http_request().unwrap();
        let method = http_request.method.clone();

        let method_block = THEME.read().panel("Method", false).title_alignment(Alignment::Center)
            .padding(Padding::horizontal(1))
            ;

        let method_area = method_block.inner(request_header_layout[0]);

        let method_paragraph = Paragraph::new(method.to_string())
            .fg(method.get_color()).bold()
            .centered();

        frame.render_widget(method_block, request_header_layout[0]);
        frame.render_widget(method_paragraph, method_area);

        // REQUEST URL

        self.url_text_input.highlight_block = self.pane == crate::tui::workspace::Pane::Url;
        self.url_text_input.display_cursor = matches!(self.state, AppState::EditingRequestUrl);
        frame.render_widget(SingleLineTextInput(&mut self.url_text_input), request_header_layout[1]);

        // REQUEST MAIN LAYOUT

        let visible_view = self.visible_request_view(frame.area().width);
        let request_main_layout_constraints = match visible_view {
            RequestView::Normal => [
                Constraint::Percentage(50),
                Constraint::Percentage(50)
            ],
            RequestView::OnlyResult => [
                Constraint::Percentage(0),
                Constraint::Percentage(100)
            ],
            RequestView::OnlyParams => [
                Constraint::Percentage(100),
                Constraint::Percentage(0)
            ]
        };

        let request_main_layout = Layout::new(
            Horizontal,
            request_main_layout_constraints,
        )
            .split(request_layout[2]);


        let (should_render_params, should_render_result) = match visible_view {
            RequestView::Normal => (true, true),
            RequestView::OnlyResult => (false, true),
            RequestView::OnlyParams => (true, false)
        };

        // REQUEST PARAMS

        if should_render_params {
            let params_block = THEME.read().panel(self.pane_title(crate::tui::workspace::Pane::Parameters), self.pane == crate::tui::workspace::Pane::Parameters);

            let request_params_area = params_block.inner(request_main_layout[0]);

            frame.render_widget(params_block, request_main_layout[0]);
            self.render_request_params(frame, request_params_area, &request);
        }

        // REQUEST RESULT LAYOUT

        if should_render_result {
            let result_block = THEME.read().panel(self.pane_title(crate::tui::workspace::Pane::Response), self.pane == crate::tui::workspace::Pane::Response);
            let result_block_area = result_block.inner(request_main_layout[1]);

            frame.render_widget(result_block, request_main_layout[1]);
            self.render_request_result(frame, result_block_area, &request);
        }
    }
}
