use crate::app::app::App;
use crate::app::files::theme::THEME;
use crate::tui::app_states::AppState::*;
use crate::tui::tui_logic::utils::key_value_vec_to_items_list;
use crate::tui::utils::stateful::text_input::SingleLineTextInput;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::widgets::{Borders, Clear, List, ListItem, Paragraph, Wrap};

fn centered(screen: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(screen.width.saturating_sub(2));
    let height = height.min(screen.height.saturating_sub(4));
    Rect::new(screen.x + (screen.width - width) / 2, screen.y + (screen.height - height) / 2, width, height)
}

impl App<'_> {
    pub fn render_env_editor_popup(&mut self, frame: &mut Frame) {
        let area = centered(frame.area(), 136, 30);
        let theme = THEME.read();
        let block = THEME.read().panel("", true).title(" Environments ").borders(Borders::ALL)
            .bg(theme.ui.main_background_color);
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let layout = Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).split(inner);
        frame.render_widget(Paragraph::new("Use {{VARIABLE}} in requests; env.VARIABLE in scripts. Enter on an environment activates it.")
            .fg(ratatui::style::Color::White).wrap(Wrap { trim: true }), layout[0]);
        let panes = if inner.width < 65 {
            Layout::vertical([Constraint::Length(6), Constraint::Min(0)]).split(layout[1])
        } else {
            Layout::horizontal([Constraint::Percentage(28), Constraint::Percentage(72)]).split(layout[1])
        };
        let list_title = if self.env_editor_list_focused { "▶ Environments" } else { "Environments" };
        let list_block = theme.panel(list_title, self.env_editor_list_focused);
        if self.environments.is_empty() {
            let create = crate::app::files::key_bindings::KEY_BINDINGS.read().generic.list_and_table_actions.create_element;
            frame.render_widget(Paragraph::new(format!("No environments yet.\nPress {create} to create one.")).fg(ratatui::style::Color::White).wrap(Wrap { trim: true }).block(list_block), panes[0]);
        } else {
            let items: Vec<ListItem> = self.environments.iter().enumerate().map(|(i, env)| {
                let env = env.read();
                ListItem::new(format!("{}{}", env.name, if i == self.selected_environment { " [active]" } else { "" }))
            }).collect();
            self.env_editor_list_state.select(Some(self.env_editor_index));
            frame.render_stateful_widget(List::new(items).block(list_block).highlight_symbol("› ")
                .highlight_style(Style::new().bg(theme.palette.selection).bold()), panes[0], &mut self.env_editor_list_state);
        }
        let local_env = self.get_env_as_local_from_index(self.env_editor_index);
        let title = local_env.as_ref().map(|env| format!("{} {} · Variables", if self.env_editor_list_focused { "" } else { "▶" }, env.read().name))
            .unwrap_or_else(|| "Variables".into());
        let variable_block = theme.panel(title, !self.env_editor_list_focused);
        let variables_area = variable_block.inner(panes[1]);
        frame.render_widget(variable_block, panes[1]);
        if local_env.is_none() {
            frame.render_widget(Paragraph::new("Create an environment, then add variables such as BASE_URL or API_TOKEN.")
                .wrap(Wrap { trim: true }), variables_area);
        } else {
            self.env_editor_table.is_editing = self.state == EditingEnvVariable;
            let mut rows = key_value_vec_to_items_list(&local_env, &self.env_editor_table.rows);
            frame.render_stateful_widget(&mut self.env_editor_table, variables_area, &mut rows);
        }
        if matches!(self.state, CreatingEnvironment | RenamingEnvironment) {
            let area = centered(frame.area(), 64, 3);
            let block = THEME.read().panel("", true).borders(Borders::ALL)
                .title(if self.state == CreatingEnvironment { "New environment name" } else { "Rename environment" })
                .bg(theme.ui.main_background_color);
            let input_area = block.inner(area);
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);
            self.env_name_input.display_cursor = true;
            frame.render_widget(SingleLineTextInput(&mut self.env_name_input), input_area);
        } else if self.state == DeletingEnvironment {
            let area = centered(frame.area(), 68, 7);
            let block = THEME.read().panel("", true).borders(Borders::ALL).title("Delete environment?").bg(theme.ui.main_background_color);
            let inner = block.inner(area);
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);
            let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);
            let env = self.environments[self.env_editor_index].read();
            frame.render_widget(Paragraph::new(format!("Delete \"{}\" and its {} variables?\nThis removes its environment file.", env.name, env.values.len())).wrap(Wrap { trim: true }), layout[0]);
            let choices = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(layout[1]);
            for (index, label) in ["Cancel", "Delete"].iter().enumerate() {
                let focused = self.env_delete_confirmed == (index == 1);
                let mut paragraph = Paragraph::new(format!("{}[ {label} ]", if focused { "> " } else { "  " })).centered();
                if focused { paragraph = paragraph.fg(theme.palette.background).bg(if index == 1 { theme.palette.error } else { theme.palette.accent }).bold(); }
                frame.render_widget(paragraph, choices[index]);
            }
        }
    }
}
