use ratatui::layout::Direction::Vertical;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::Stylize;
use ratatui::widgets::{Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::app::App;
use crate::app::files::theme::THEME;
use crate::tui::utils::centered_rect::centered_rect;
use crate::tui::utils::stateful::text_input::{MultiLineTextInput, SingleLineTextInput};

impl App<'_> {
    pub fn render_creating_new_request_popup(&mut self, frame: &mut Frame) {
        let popup_block = THEME.read().panel("", true)
            .borders(Borders::ALL)

            .bg(THEME.read().ui.main_background_color);


        let (area_width, area_height, constraints) = if self.new_request_popup.is_curl() {
            (80, 18, vec![Constraint::Length(3), Constraint::Length(3), Constraint::Length(10)])
        } else {
            (50, 9, vec![Constraint::Length(3), Constraint::Length(3), Constraint::Length(3)])
        };
        let area = centered_rect(area_width, area_height, frame.area());
        let new_request_layout = Layout::new(Vertical, constraints).split(area);


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

        let name_is_active = if self.new_request_popup.is_curl() {
            self.new_request_popup.selection == 1
        } else {
            self.new_request_popup.selection == 2
        };

        frame.render_widget(Clear, area);
        frame.render_widget(popup_block, area);
        frame.render_widget(selected_collection_paragraph, new_request_layout[0]);

        if self.new_request_popup.is_curl() {
            self.new_request_popup.text_input.highlight_text = name_is_active;
            self.new_request_popup.text_input.highlight_block = name_is_active;
            self.new_request_popup.text_input.display_cursor = name_is_active;
            frame.render_widget(SingleLineTextInput(&mut self.new_request_popup.text_input), new_request_layout[1]);

            let curl_is_active = self.new_request_popup.selection == 2;
            self.new_request_popup.curl_text_area.highlight_text = curl_is_active;
            self.new_request_popup.curl_text_area.highlight_block = curl_is_active;
            self.new_request_popup.curl_text_area.display_cursor = curl_is_active;
            frame.render_widget(MultiLineTextInput(&mut self.new_request_popup.curl_text_area), new_request_layout[2]);
        } else {
            frame.render_widget(selected_protocol_paragraph, new_request_layout[1]);

            self.new_request_popup.text_input.highlight_text = name_is_active;
            self.new_request_popup.text_input.highlight_block = name_is_active;
            self.new_request_popup.text_input.display_cursor = name_is_active;

            frame.render_widget(SingleLineTextInput(&mut self.new_request_popup.text_input), new_request_layout[2]);
        }
    }
}
