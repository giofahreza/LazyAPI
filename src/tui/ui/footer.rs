use crate::app::app::App;
use crate::app::files::{key_bindings::KEY_BINDINGS, theme::THEME};
use crate::tui::app_states::AppState::*;
use ratatui::{Frame, layout::Rect, style::{Color, Style}, text::{Line, Span}, widgets::Paragraph};
use super::tabs::{clip_text, text_width};

impl App<'_> {
    fn footer_hint(&self) -> String {
        let bindings = *KEY_BINDINGS.read();
        if self.search.is_open() {
            "Type to search · ↑/↓ select · Enter jump · Esc cancel · Ctrl+u clear".into()
        } else if self.should_display_help {
            format!("{} · j/k scroll · Esc/?/q close", self.search_hint().unwrap_or_else(|| "/ search".into()))
        } else if self.pending_window {
            "Ctrl+w · h/j/k/l focus pane · w next pane · Esc cancel".into()
        } else if let Some(command) = &self.editor_command {
            format!(":{command}█")
        } else if let Some(notice) = &self.editor_notice {
            notice.clone()
        } else if let Some(input) = self.active_input() {
            if bindings.generic.text_input.mode != crate::app::files::key_bindings::TextAreaMode::Vim {
                format!("{} save/finish · {} discard · F1 help", bindings.generic.text_input.save_and_quit_area, bindings.generic.text_input.quit_without_saving)
            } else {
                match input.state.mode {
                    edtui::EditorMode::Normal => "i/a/I/A insert · :w save · :wq finish · Ctrl+c discard · ? help".into(),
                    edtui::EditorMode::Insert => format!("Esc / Ctrl+[ Normal · {}Ctrl+s save/finish · Ctrl+c discard · F1 help", if input.is_single_line { "Enter / " } else { "" }),
                    edtui::EditorMode::Visual => "h/j/k/l select · y yank · d delete · c change · Esc Normal · F1 help".into(),
                    edtui::EditorMode::Search => format!("/{}█ · Enter find · Esc cancel search · F1 help", input.state.search_pattern()),
                }
            }
        } else {
            let context = match self.state {
                Normal => format!("Enter open · {} new · {} rename", bindings.generic.list_and_table_actions.create_element, bindings.generic.list_and_table_actions.rename_element),
                SelectedRequest if self.pane == crate::tui::workspace::Pane::Response => format!("h/j/k/l scroll · {} copy", bindings.request_selected.result_tabs.yank_response_part),
                DisplayingEnvEditor => if self.env_editor_list_focused {
                    format!("{} new · {} rename · {} delete · Enter activate · Ctrl+l variables", bindings.generic.list_and_table_actions.create_element, bindings.generic.list_and_table_actions.rename_element, bindings.generic.list_and_table_actions.delete_element)
                } else {
                    format!("{} add variable · Enter inspect / i edit · {} delete · Ctrl+h environments", bindings.generic.list_and_table_actions.create_element, bindings.generic.list_and_table_actions.delete_element)
                },
                DeletingEnvironment => "h/l choose · Enter confirm · Esc cancel".into(),
                SelectedRequest => format!("{} insert / Enter inspect", bindings.generic.list_and_table_actions.edit_element),
                CreatingNewRequest => "Tab next field · Enter confirm · Esc cancel".into(),
                _ => "Enter confirm · Esc back".into(),
            };
            let workspace = if matches!(self.state, Normal | SelectedRequest) {
                format!(" · {}/{} tabs · Ctrl+h/j/k/l panes{}", bindings.workspace.previous_request,
                    bindings.workspace.next_request, if self.state == SelectedRequest { format!(" · {} send", bindings.request_selected.send_request) } else { String::new() })
            } else { String::new() };
            let search = self.search_hint().map(|hint| format!(" · {hint}")).unwrap_or_default();
            format!("{} help{search} · {context}{workspace}", bindings.generic.display_help)
        }
    }

    pub(super) fn render_footer(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() { return; }
        let theme = THEME.read();
        let mut spans = vec![Span::styled(" LazyAPI ", theme.active_cell())];
        if area.width >= 140 {
            spans.push(Span::styled(format!(" v{}", env!("CARGO_PKG_VERSION")), Style::new().fg(theme.ui.secondary_foreground_color)));
        }
        spans.push(Span::raw("  "));
        let modified = self.active_input().is_some_and(|input| input.is_modified());
        let mut status = format!("{} · {}{}", self.mode_label(), self.editing_context(), if modified { " [+]" } else { "" });
        if area.width >= 140 && let Some(tab) = self.active_request_tab {
            status.push_str(&format!(" · Requests {}/{}", tab + 1, self.request_tabs.len()));
        }
        if area.width >= 140 && let Some(input) = self.active_input() {
            status.push_str(&format!(" {} {}:{}", input.vim_pending, input.state.cursor.row + 1, input.state.cursor.col + 1));
        }
        let used = spans.iter().map(Span::width).sum::<usize>();
        let status = clip_text(&status, (area.width as usize).saturating_sub(used + 12));
        spans.push(Span::styled(status, Style::new().fg(theme.others.selection_highlight_color).bold()));
        spans.push(Span::raw("  "));
        let used = spans.iter().map(Span::width).sum::<usize>();
        let available = (area.width as usize).saturating_sub(used);
        let hint = self.footer_hint();
        let text = if self.editor_command.is_some() || self.editor_notice.is_some() || self.pending_window
            || self.active_input().is_some_and(|input| input.state.mode == edtui::EditorMode::Search) {
            clip_text(&hint, available)
        } else {
            let mut packed = String::new();
            for part in hint.split(" · ") {
                let next = if packed.is_empty() { part.into() } else { format!("{packed} · {part}") };
                if text_width(&next) > available { break; }
                packed = next;
            }
            if packed.is_empty() { clip_text("? help", available) } else { packed }
        };
        spans.push(Span::styled(text, Style::new().fg(Color::White)));
        frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().fg(theme.ui.font_color).bg(theme.ui.main_background_color)), area);
    }
}
