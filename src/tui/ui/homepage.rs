use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::widgets::Paragraph;
use crate::app::app::App;
use crate::app::files::theme::THEME;

impl App<'_> {
    pub(super) fn render_homepage(&mut self, frame: &mut Frame, rect: Rect) {
        let theme = THEME.read();
        let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(2), Constraint::Fill(1)]).split(rect);
        frame.render_widget(Paragraph::new("No open requests").centered().fg(theme.ui.font_color).bold(), layout[1]);
        frame.render_widget(Paragraph::new("Select a request in Collections and press Enter").centered().fg(ratatui::style::Color::White), layout[3]);
    }
}
