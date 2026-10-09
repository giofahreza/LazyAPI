use ratatui::layout::Direction::Vertical;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::Stylize;
use ratatui::widgets::{Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::app::App;
use crate::app::files::theme::THEME;
use crate::tui::utils::centered_rect::centered_rect;
use crate::tui::utils::stateful::text_input::SingleLineTextInput;

impl App<'_> {
    pub fn render_creating_new_request_popup(&mut self, frame: &mut Frame) {
        let popup_block = THEME.read().panel("", true)
            .borders(Borders::ALL)

            .bg(THEME.read().ui.main_background_color);


        let area = centered_rect(50, 9, frame.area());

        let new_request_layout = Layout::new(
            Vertical,
            vec![
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ]
        )
            .split(area);


        let selected_collection_name = self.collections[self.new_request_popup.selected_collection].name.clone();
        let selected_collection_paragraph = Paragraph::new(selected_collection_name)
            .fg(THEME.read().ui.font_color)
            .block(
                THEME.read().panel("Collection ← →", self.new_request_popup.selection == 0)
                    .borders(Borders::ALL)
            );

        let selected_protocol_name = self.new_request_popup.protocol.to_string();
        let selected_protocol_paragraph = Paragraph::new(selected_protocol_name)
            .fg(THEME.read().ui.font_color)
            .block(
                THEME.read().panel("Protocol ← →", self.new_request_popup.selection == 1)
                    .borders(Borders::ALL)
            );

        let highlight_and_display_cursor = self.new_request_popup.selection == 2;

        frame.render_widget(Clear, area);
        frame.render_widget(popup_block, area);
        frame.render_widget(selected_collection_paragraph, new_request_layout[0]);
        frame.render_widget(selected_protocol_paragraph, new_request_layout[1]);

        self.new_request_popup.text_input.highlight_text = highlight_and_display_cursor;
        self.new_request_popup.text_input.highlight_block = highlight_and_display_cursor;
        self.new_request_popup.text_input.display_cursor = highlight_and_display_cursor;

        frame.render_widget(SingleLineTextInput(&mut self.new_request_popup.text_input), new_request_layout[2]);
    }
}
