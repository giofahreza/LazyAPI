use ratatui::Frame;
use ratatui::layout::Direction::Horizontal;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::Style;
use ratatui::style::Stylize;
use ratatui::widgets::{Borders, Clear, Paragraph};

use crate::app::app::App;
use crate::app::files::theme::THEME;
use crate::tui::utils::centered_rect::centered_rect;

impl App<'_> {
    pub fn render_deleting_request_popup(&mut self, frame: &mut Frame) {
        let popup_block = THEME.read().panel("", true)
            .title("Confirm delete request")
            .borders(Borders::ALL)
            .style(Style::default().bg(THEME.read().ui.main_background_color));

        let area = centered_rect(30, 3, frame.area());

        let deleting_request_layout = Layout::new(
            Horizontal,
            vec![
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
        )
            .vertical_margin(1)
            .horizontal_margin(1)
            .split(area);

        let mut no_paragraph = Paragraph::new("[ Cancel ]").centered();
        let mut yes_paragraph = Paragraph::new("[ Delete ]").centered();

        match self.delete_request_popup.state {
            false => no_paragraph = no_paragraph.style(THEME.read().active_cell()),
            true => yes_paragraph = yes_paragraph.fg(THEME.read().palette.background).bg(THEME.read().palette.error).bold(),
        }

        frame.render_widget(Clear, area);
        frame.render_widget(popup_block, area);
        frame.render_widget(no_paragraph, deleting_request_layout[0]);
        frame.render_widget(yes_paragraph, deleting_request_layout[1]);
    }
}