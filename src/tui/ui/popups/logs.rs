use crate::app::app::App;
use crate::app::files::theme::THEME;
use crate::app::log::LOGS;
use crate::tui::utils::centered_rect::centered_rect;
use ratatui::layout::Direction::Vertical;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::{Line, Span, Style};
use ratatui::style::Stylize;
use ratatui::widgets::{Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation};
use ratatui::Frame;
use tracing::Level;

impl App<'_> {
    pub fn render_logs_popup(&mut self, frame: &mut Frame) {
        let popup_block = THEME.read().panel("", true)
            .title("Logs")
            .borders(Borders::ALL)

            .bg(THEME.read().ui.secondary_background_color);

        let area = centered_rect(120, 25, frame.area());

        frame.render_widget(Clear, area);
        frame.render_widget(popup_block, area);

        let logger_layout = Layout::new(
            Vertical,
            vec![
                Constraint::Fill(1)
            ]
        )
            .vertical_margin(1)
            .horizontal_margin(2)
            .split(area);

        let logs = LOGS.lock();
        let mut lines = vec![];

        let muted_color = THEME.read().ui.secondary_foreground_color;
        let font_color = THEME.read().ui.font_color;

        for log in logs.iter() {
            let level_color = match log.1 {
                Level::ERROR => THEME.read().palette.error,
                Level::WARN => THEME.read().palette.warning,
                Level::INFO => THEME.read().palette.success,
                Level::DEBUG => THEME.read().palette.action,
                Level::TRACE => THEME.read().palette.muted,
            };

            lines.push(
                Line::from(vec![
                    Span::raw(&log.0).fg(muted_color),
                    Span::raw(format!(" {:>5} ", log.1)).fg(level_color),
                    Span::raw(format!("{}: ", log.2)).fg(muted_color),
                    Span::raw(&log.3).fg(font_color),
                ])
            );
        }

        lines.reverse();
        self.search.register_lines(crate::tui::search::SearchScope::Logs, &lines, logger_layout[0].width, false);
        self.search.highlight_lines(crate::tui::search::SearchScope::Logs, &mut lines);

        let logs_paragraph = Paragraph::new(lines)
            .scroll((
                self.logs_vertical_scrollbar.scroll,
                self.logs_horizontal_scrollbar.scroll,
            ));

        frame.render_widget(logs_paragraph, logger_layout[0]);

        // For mysterious reasons, they won't show up...
        let logger_vertical_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::new().fg(font_color));
        let logger_horizontal_scrollbar = Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .style(Style::new().fg(font_color))
            .thumb_symbol("■");

        frame.render_stateful_widget(
            logger_vertical_scrollbar,
            area,
            &mut self.logs_vertical_scrollbar.state,
        );

        frame.render_stateful_widget(
            logger_horizontal_scrollbar,
            area,
            &mut self.logs_horizontal_scrollbar.state
        );
    }
}
