use std::sync::Arc;
use parking_lot::RwLock;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use regex::Regex;
use crokey::{key, KeyCombination};
use crate::app::files::key_bindings::KEY_BINDINGS;
use crate::tui::app_states::AppState::*;
use crate::tui::utils::stateful::text_input::TextInput;

use crate::app::app::App;
use crate::app::files::environment::OS_ENV_VARS;
use crate::app::files::theme::THEME;
use crate::models::environment::Environment;
use crate::models::request::KeyValue;

impl App<'_> {
    pub fn tui_next_environment(&mut self) {
        if self.selected_environment + 1 < self.environments.len() {
            self.selected_environment += 1;
        }
        else {
            self.selected_environment = 0;
        }
    }

    pub fn tui_update_env_variable_table(&mut self) {
        self.env_editor_index = self.env_editor_index.min(self.environments.len().saturating_sub(1));
        self.env_editor_table.rows = self.get_env_as_local_from_index(self.env_editor_index)
            .map(|local| local.read().values.iter().map(|(key, value)| KeyValue {
                enabled: true, data: (key.clone(), value.clone()),
            }).collect()).unwrap_or_default();
        self.env_editor_table.clamp_selection();
    }

    pub fn tui_modify_env_variable(&mut self) {
        let Some((row, column)) = self.env_editor_table.selection else { return; };
        let input_text = self.env_editor_table.selection_text_input.to_string();
        let result = if column == 0 {
            self.rename_env_key_by_index(self.env_editor_index, row, input_text)
        } else {
            self.set_env_value_by_index(self.env_editor_index, row, input_text)
        };
        match result {
            Ok(()) => self.display_env_editor_state(),
            Err(error) => self.editor_notice = Some(format!("Could not save: {error:#}")),
        }
    }

    pub fn tui_create_env_variable(&mut self) {
        match self.create_env_value(self.env_editor_index, None, String::new()) {
            Ok(()) => {
                self.tui_update_env_variable_table();
                self.env_editor_table.update_selection(Some((self.env_editor_table.rows.len() - 1, 0)));
            }
            Err(error) => self.editor_notice = Some(format!("Could not add variable: {error:#}")),
        }
    }

    pub fn tui_delete_env_variable(&mut self) {
        let Some((row, _)) = self.env_editor_table.selection else { return; };
        match self.delete_env_index(self.env_editor_index, row) {
            Ok(()) => self.tui_update_env_variable_table(),
            Err(error) => self.editor_notice = Some(format!("Could not delete variable: {error:#}")),
        }
    }

    fn tui_environment_name_dialog(&mut self, rename: bool) {
        if rename && self.environments.is_empty() { return; }
        self.env_name_input = TextInput::new(None);
        self.env_name_input.is_single_line = true;
        self.env_name_input.default_mode = self.env_editor_table.selection_text_input.default_mode;
        if rename {
            let name = self.environments[self.env_editor_index].read().name.clone();
            self.env_name_input.push_str(&name);
            self.env_name_input.move_cursor_line_end();
        }
        self.state = if rename { RenamingEnvironment } else { CreatingEnvironment };
        self.begin_text_edit(key!(i));
    }

    pub fn tui_save_environment_name(&mut self) {
        let name = self.env_name_input.to_string();
        let result = if self.state == CreatingEnvironment {
            self.create_environment(&name).map(|index| {
                self.env_editor_index = index;
                self.selected_environment = index;
                self.env_editor_list_focused = false;
                self.env_editor_table.update_selection(None);
            })
        } else { self.rename_environment(self.env_editor_index, &name) };
        match result {
            Ok(()) => self.display_env_editor_state(),
            Err(error) => self.editor_notice = Some(format!("Could not save: {error:#}")),
        }
    }

    pub fn move_environment_cursor(&mut self, down: bool) {
        self.env_editor_index = if down {
            (self.env_editor_index + 1).min(self.environments.len().saturating_sub(1))
        } else { self.env_editor_index.saturating_sub(1) };
        self.env_editor_table.update_selection(None);
        self.tui_update_env_variable_table();
    }

    pub fn handle_environment_key(&mut self, pressed: KeyCombination) -> bool {
        let b = *KEY_BINDINGS.read();
        let n = b.generic.navigation;
        let a = b.generic.list_and_table_actions;
        let w = b.workspace;
        if (matches!(self.state, Normal | SelectedRequest) || (self.request_editor_active() && self.editor_shortcuts_allowed()))
            && pressed == b.main_menu.display_env_editor {
            if self.commit_text_edit() { self.display_env_editor_state(); }
            return true;
        }
        if self.state == DeletingEnvironment {
            if pressed == key!(esc) || pressed == key!(ctrl-c) || pressed == key!(q) {
                self.display_env_editor_state();
            } else if [n.move_cursor_left, key!(left), key!(n)].contains(&pressed) {
                self.env_delete_confirmed = false;
            } else if [n.move_cursor_right, key!(right), key!(y)].contains(&pressed) {
                self.env_delete_confirmed = true;
            } else if pressed == key!(tab) {
                self.env_delete_confirmed = !self.env_delete_confirmed;
            } else if pressed == n.select {
                if self.env_delete_confirmed {
                    match self.delete_environment(self.env_editor_index) {
                        Ok(()) => { self.env_editor_list_focused = true; self.display_env_editor_state(); }
                        Err(error) => self.editor_notice = Some(format!("Could not delete: {error:#}")),
                    }
                } else { self.display_env_editor_state(); }
            }
            return true;
        }
        if !matches!(self.state, DisplayingEnvEditor | EditingEnvVariable) { return false; }
        let left = pressed == w.focus_left || pressed == w.focus_up;
        let right = pressed == w.focus_right || pressed == w.focus_down;
        let tab = (pressed == key!(tab) || pressed == key!(shift-backtab)) && self.editor_shortcuts_allowed();
        if left || right || tab {
            if self.commit_text_edit() {
                self.env_editor_list_focused = if tab { !self.env_editor_list_focused } else { left };
                if self.environments.is_empty() { self.env_editor_list_focused = true; }
            }
            return true;
        }
        if self.state == EditingEnvVariable { return false; }
        if [n.go_back, key!(ctrl-c), key!(q), b.main_menu.display_env_editor].contains(&pressed) {
            self.editor_notice = None;
            self.normal_state();
            return true;
        }
        if pressed == key!(shift-N) || (self.env_editor_list_focused && pressed == a.create_element) {
            self.tui_environment_name_dialog(false);
            return true;
        }
        if pressed == key!(shift-R) || (self.env_editor_list_focused && pressed == a.rename_element) {
            self.tui_environment_name_dialog(true);
            return true;
        }
        if pressed == key!(shift-D) || (self.env_editor_list_focused && pressed == a.delete_element) {
            if !self.environments.is_empty() {
                self.editor_notice = None;
                self.env_delete_confirmed = false;
                self.state = DeletingEnvironment;
            }
            return true;
        }
        if self.env_editor_list_focused {
            if pressed == n.move_cursor_down || pressed == key!(down) { self.move_environment_cursor(true); }
            else if pressed == n.move_cursor_up || pressed == key!(up) { self.move_environment_cursor(false); }
            else if pressed == n.select || pressed == key!(space) {
                if !self.environments.is_empty() {
                    self.selected_environment = self.env_editor_index;
                    if pressed == n.select { self.env_editor_list_focused = false; }
                    self.editor_notice = None;
                }
            } else if pressed == n.move_cursor_right || pressed == key!(right) {
                if !self.environments.is_empty() { self.env_editor_list_focused = false; }
            } else { return false; }
            return true;
        }
        if pressed == a.rename_element && self.env_editor_table.is_selected() {
            self.env_editor_table.select_column(0);
            self.edit_env_variable_state();
            self.begin_text_edit(key!(enter));
            return true;
        }
        false
    }

}

pub fn tui_add_color_to_env_keys<'a>(local_env: &Option<Arc<RwLock<Environment>>>, input: String) -> Line<'a> {
    if !input.contains('{') {
        return Line::raw(input);
    }

    let mut spans: Vec<Span> = vec![];

    let regex = Regex::new(r"\{\{(\w+)}}").unwrap();
    let mut tmp_index: usize = 0;

    if let Some(local_env) = local_env {
        let env = local_env.read();

        let mut keys: Vec<&str> = env.values.keys().map(|key| key.as_str()).collect();
        keys.extend(OS_ENV_VARS.keys().map(|k| k.as_str()));
        keys.extend(vec![
            "NOW",
            "TIMESTAMP",
            "UUIDv4",
            "UUIDv7"
        ]);
        // A name shared with the process environment or a dynamic variable must
        // only emit one span for each placeholder occurrence.
        keys.sort_unstable();
        keys.dedup();

        for match_ in regex.captures_iter(&input) {
            for sub_match in match_.iter() {
                if let Some(sub_match) = sub_match {
                    for key in &keys {
                        if sub_match.as_str() == &format!("{{{{{}}}}}", key) {
                            let range = sub_match.range();

                            spans.push(Span::raw(input[tmp_index..range.start].to_string()));
                            spans.push(
                                Span::raw(sub_match.as_str().to_owned())
                                    .fg(THEME.read().others.environment_variable_highlight_color)
                            );

                            tmp_index = range.end;
                        }
                    }
                }
            }
        }

        spans.push(Span::raw(String::from(&input[tmp_index..input.len()])));
    }
    else {
        spans.push(Span::raw(input.to_string()));
    }

    return Line::from(spans);
}
