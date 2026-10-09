use crokey::{key, KeyCombination};
use ratatui::layout::{Margin, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Borders, Clear, Paragraph};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use crate::app::app::App;
use crate::app::files::key_bindings::KEY_BINDINGS;
use crate::app::files::theme::THEME;
use crate::tui::app_states::AVAILABLE_EVENTS;

impl App<'_> {
    pub fn handle_help_key(&mut self, pressed: KeyCombination) -> bool {
        let bindings = *KEY_BINDINGS.read();
        let event: KeyEvent = pressed.into();
        let help_key = event.code == KeyCode::F(1);
        if self.should_display_help {
            if pressed == key!(esc) || pressed == key!(q) || pressed == key!(ctrl-c) || pressed == bindings.generic.display_help || help_key {
                self.should_display_help = false;
            } else if pressed == bindings.generic.navigation.move_cursor_down || pressed == key!(down) {
                self.help_popup.scroll = self.help_popup.scroll.saturating_add(1).min(self.help_popup.max_scroll);
            } else if pressed == bindings.generic.navigation.move_cursor_up || pressed == key!(up) {
                self.help_popup.scroll = self.help_popup.scroll.saturating_sub(1);
            } else if pressed == bindings.workspace.page_down || pressed == key!(pagedown) {
                self.help_popup.scroll = self.help_popup.scroll.saturating_add(10).min(self.help_popup.max_scroll);
            } else if pressed == bindings.workspace.page_up || pressed == key!(pageup) {
                self.help_popup.scroll = self.help_popup.scroll.saturating_sub(10);
            } else if pressed == key!(shift-G) { self.help_popup.scroll = self.help_popup.max_scroll; }
            else if pressed == key!(g) { self.help_popup.scroll = 0; }
            return true;
        }
        if help_key || (pressed == bindings.generic.display_help && self.editor_shortcuts_allowed()) {
            self.should_display_help = true;
            self.help_popup.scroll = 0;
            return true;
        }
        false
    }

    pub fn shortcut_rows(&self) -> Vec<(String, String)> {
        let b = *KEY_BINDINGS.read();
        let w = b.workspace;
        let n = b.generic.navigation;
        let a = b.generic.list_and_table_actions;
        let r = b.request_selected;
        let mut rows: Vec<(String, String)> = vec![];
        if matches!(self.state, crate::tui::app_states::AppState::DisplayingEnvEditor
            | crate::tui::app_states::AppState::DeletingEnvironment) {
            rows.extend([
                (String::new(), "ENVIRONMENT MANAGER".into()),
                (b.main_menu.display_env_editor.to_string(), "Open environment manager, including when no environments exist".into()),
                (format!("{}/{} · {}/{}", w.focus_left, w.focus_right, w.focus_up, w.focus_down), "Focus environments / variables; saves an edited cell first".into()),
                ("Tab / Shift+Tab".into(), "Switch between environments and variables in Normal mode".into()),
                ("j/k · gg/G · Ctrl+u/d".into(), "Browse environments or variable rows".into()),
                ("Enter / Space on environment".into(), "Activate environment; Enter also focuses its variables".into()),
                (format!("{} / {} / {} (list)", a.create_element, a.rename_element, a.delete_element), "Create / rename / delete environment (delete asks first)".into()),
                ("N / R / D (Normal)".into(), "Create / rename / delete environment from either pane".into()),
                (format!("{} / {} (variables)", a.create_element, a.delete_element), "Add / delete variable in the displayed environment".into()),
                (format!("Enter / {} / {}", a.edit_element, a.rename_element), "Inspect cell / insert in cell / edit variable name".into()),
                ("Esc / q".into(), "Close manager; Esc inside an editor keeps its draft in Normal".into()),
                (String::new(), "Browsing or editing does not activate an environment. [active] marks the one used by requests.".into()),
            ]);
        }
        let vim = b.generic.text_input.mode == crate::app::files::key_bindings::TextAreaMode::Vim;
        if vim {
            rows.extend([
                (String::new(), "MODES AND EDITING".into()),
                ("Enter on a value".into(), "Open its editor in Normal mode; i starts inserting".into()),
                ("i / a / I / A (editor)".into(), "Insert before / after cursor / first text / line end".into()),
                ("Esc / Ctrl+[".into(), "Return to Normal; repeated Esc keeps the editor and draft".into()),
                ("Ctrl+s / Enter".into(), "Save and finish / finish a single-line field (except search)".into()),
                ("Ctrl+c / :q!".into(), "Discard edits since the last save and leave the editor".into()),
                (":w / :wq / :x".into(), "Save and keep editing / save and finish".into()),
                (":q".into(), "Leave an unchanged editor; warn if there are unsaved edits".into()),
                (":help / F1".into(), "Help; F1 also works in Insert/Visual/Search mode".into()),
                ("h j k l / w b e / 0 ^ $".into(), "Cursor, word, and line motions in editor Normal mode".into()),
                ("o / O".into(), "Insert a line below / above in multiline editors".into()),
                ("v / V / y / p".into(), "Visual selection / whole line / yank selection / paste".into()),
                ("u / Ctrl+r".into(), "Undo / redo (history survives :w)".into()),
                ("x / X / s".into(), "Delete at cursor / before cursor / substitute character".into()),
                ("dd / D / d$".into(), "Delete line / from cursor to end of line".into()),
                ("C / c$ / cc / S".into(), "Change to end of line / change whole line".into()),
                ("ciw / diw / yiw".into(), "Change / delete / yank inner word".into()),
                ("ci\" / ci( / ci{ / ci[".into(), "Change inside quotes/brackets; d and y also work".into()),
                ("/ · Enter · n / N".into(), "Search field, accept search, next / previous match".into()),
                (":noh".into(), "Clear search highlighting".into()),
                ("Ctrl+w / Ctrl+u (Insert)".into(), "Delete previous word / back to first text of line".into()),
                (String::new(), "[+] means unsaved edits; the footer identifies the field and cursor.".into()),
                (String::new(), "[ ] ? : and application letters remain literal text in Insert mode.".into()),
                (String::new(), "Ctrl+c discards an edit in LazyAPI; Esc keeps it. Dialogs start in Insert.".into()),
            ]);
        }
        rows.extend([
            (String::new(), "WORKSPACE".into()),
            ("/ · Enter · n / N".into(), "Search focused list, shortcuts, or text view; jump; next / previous match".into()),
            ("Esc after search".into(), "Clear pane search; press Esc again for the usual back/close action".into()),
            (format!("{}/{}/{}/{}", n.move_cursor_left, n.move_cursor_down, n.move_cursor_up, n.move_cursor_right), "Navigate the focused pane (arrows also work)".into()),
            (format!("{} / {}", w.focus_left, w.focus_right), "Focus pane to the left / right".into()),
            (format!("{} / {}", w.focus_up, w.focus_down), "Focus URL above / parameters below".into()),
            ("Ctrl+w h/j/k/l · Ctrl+w w".into(), "Normal mode: focus neighboring / next pane; Esc cancels prefix".into()),
            (format!("{} / {}", w.previous_request, w.next_request), "Previous / next open request".into()),
            (w.close_request.to_string(), "Close tab from workspace; X deletes backward in editor Normal".into()),
            (format!("{} / {}", r.param_next_tab, w.previous_panel), "Next / previous panel in the focused request pane".into()),
            ("gg / G".into(), "First / last row in lists and responses".into()),
            (format!("{} / {}", w.page_up, w.page_down), "Move up / down 10 rows".into()),
            (b.generic.display_help.to_string(), "Show shortcuts in Normal mode".into()),
            (b.main_menu.exit.to_string(), "Quit from the workspace".into()),
            (String::new(), "Pane/tab navigation saves the current field before leaving.".into()),
            (String::new(), "Tab/Shift+Tab switches panels from editor Normal mode; Insert Tab stays text.".into()),
            (String::new(), "COLLECTIONS AND TABLES".into()),
            (n.select.to_string(), "Expand collection / open request in a tab".into()),
            (format!("{} / {}", b.main_menu.expand_collection, b.main_menu.unselect_request), "Expand / collapse collection".into()),
            (a.create_element.to_string(), "Create collection, request, or table row".into()),
            (a.rename_element.to_string(), "Rename selected collection or request".into()),
            (a.delete_element.to_string(), "Delete selected item (collections/requests ask first)".into()),
            (a.duplicate_element.to_string(), "Duplicate selected item".into()),
            (a.toggle_element.to_string(), "Enable / disable selected parameter or header".into()),
            (format!("{} / {}", b.main_menu.move_request_up, b.main_menu.move_request_down), "Reorder collection or request".into()),
            ("Tab / Shift+Tab".into(), "Next / previous field in the new-request dialog".into()),
            (String::new(), "REQUESTS".into()),
            (r.change_url.to_string(), "Edit URL".into()),
            (r.change_method.to_string(), "Cycle HTTP method in URL/parameters pane".into()),
            (r.send_request.to_string(), "Send/cancel HTTP request; connect/disconnect WebSocket".into()),
            ("Ctrl+Enter".into(), "Send WebSocket message when its editor/panel is focused".into()),
            (r.request_settings.to_string(), "Request settings".into()),
            (r.next_view.to_string(), "Cycle split / response / parameters view".into()),
            (r.export_request.to_string(), "Export request".into()),
            (r.param_tabs.change_auth_method.to_string(), "Cycle auth method in parameters pane".into()),
            (r.param_tabs.change_body_content_type.to_string(), "Cycle body type in HTTP parameters pane".into()),
            (r.param_tabs.change_message_type.to_string(), "Cycle WebSocket message type".into()),
            (r.result_tabs.yank_response_part.to_string(), "Yank the focused response panel".into()),
            (b.main_menu.next_environment.to_string(), "Next environment".into()),
            (b.main_menu.display_env_editor.to_string(), "Environment editor".into()),
            (b.main_menu.display_cookies.to_string(), "Cookies".into()),
            (b.main_menu.display_logs.to_string(), "Logs".into()),
            (String::new(), format!("CURRENT CONTEXT: {}", self.state)),
        ]);
        // Legacy input events describe the old save/cancel dispatch. The modal
        // editor above owns those keys, so do not show contradictory actions.
        for event in AVAILABLE_EVENTS.read().iter().filter(|_| !self.in_input()) {
            let binding = event.get_event_key_bindings();
            if !binding.keys.is_empty() && !matches!(event, crate::tui::events::AppEvent::Documentation(_)) {
                rows.push((binding.keys.iter().map(ToString::to_string).collect::<Vec<_>>().join(" / "), binding.event_name.clone()));
            }
        }
        rows
    }

    pub fn render_help_popup(&mut self, frame: &mut Frame) {
        let screen = frame.area();
        let width = screen.width.saturating_sub(2).min(106);
        let height = screen.height.saturating_sub(2).min(36);
        let area = Rect::new(screen.x + (screen.width - width) / 2, screen.y + (screen.height - height) / 2, width, height);
        let theme = THEME.read();
        let block = THEME.read().panel("", true).borders(Borders::ALL)
            .title(" Keyboard shortcuts ")
            .title_bottom(Line::from(" / search · j/k scroll · Ctrl+u/d page · Esc/?/q close ").fg(ratatui::style::Color::White))
            .bg(theme.ui.secondary_background_color);
        let inner = block.inner(area).inner(Margin::new(1, 0));
        let key_width = if inner.width < 70 { 20 } else { 31 };
        let mut lines = Vec::new();
        let rows = self.shortcut_rows();
        self.search.register_help(&rows, inner.width, key_width);
        for (keys, description) in rows {
            if keys.is_empty() {
                for line in textwrap::wrap(&description, inner.width.max(1) as usize) {
                    lines.push(Line::from(line.into_owned()).bold());
                }
            } else {
                // Wrap long rows so shortcuts remain readable in small terminals.
                let text = format!("{keys:<key_width$} {description}");
                for line in textwrap::wrap(&text, inner.width.max(1) as usize) {
                    lines.push(Line::from(Span::raw(line.into_owned()).fg(ratatui::style::Color::White)));
                }
            }
        }
        self.help_popup.max_scroll = lines.len().saturating_sub(inner.height as usize).min(u16::MAX as usize) as u16;
        self.help_popup.scroll = self.help_popup.scroll.min(self.help_popup.max_scroll);
        self.search.highlight_lines(crate::tui::search::SearchScope::Help, &mut lines);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(lines).fg(ratatui::style::Color::White).scroll((self.help_popup.scroll, 0)), inner);
    }
}
