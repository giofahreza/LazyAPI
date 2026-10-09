use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::app::app::App;
use crate::app::files::theme::THEME;

impl<'a> App<'a> {
    pub(super) fn render_environments(&mut self, frame: &mut Frame, rect: Rect) {
        let current_environment = self.get_selected_env_as_local()
            .map(|env| env.read().name.clone()).unwrap_or_else(|| "None".into());

        let current_environment_paragraph = Paragraph::new(current_environment)
            .block(
                THEME.read().panel(ratatui::text::Line::from(vec![ratatui::text::Span::raw("Environment · "), ratatui::text::Span::styled("Ctrl+e", ratatui::style::Style::new().fg(ratatui::style::Color::White))]), false)
            );

        frame.render_widget(current_environment_paragraph, rect)
    }
}