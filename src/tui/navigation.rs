use std::time::{Duration, Instant};
use crokey::{key, KeyCombination};
use crate::app::app::App;
use crate::app::files::key_bindings::KEY_BINDINGS;
use crate::tui::app_states::AppState::*;
use crate::tui::ui::param_tabs::param_tabs::RequestParamsTabs;
use crate::tui::utils::stateful::stateful_custom_table::StatefulCustomTable;
use crate::tui::workspace::Pane;

impl<'a> App<'a> {
    /// Neovim's window prefix is a one-shot command in Normal mode. Insert
    /// Ctrl+w remains the editor's delete-previous-word operation.
    pub fn handle_window_key(&mut self, pressed: KeyCombination) -> bool {
        if self.should_display_help || self.editor_command.is_some() { return false; }
        let workspace = matches!(self.state, Normal | SelectedRequest) || self.request_editor_active();
        if !workspace { self.pending_window = false; return false; }
        if self.pending_window {
            self.pending_window = false;
            let direction = if pressed == key!(h) || pressed == key!(ctrl-h) || pressed == key!(left) { Some('h') }
                else if pressed == key!(j) || pressed == key!(ctrl-j) || pressed == key!(down) { Some('j') }
                else if pressed == key!(k) || pressed == key!(ctrl-k) || pressed == key!(up) { Some('k') }
                else if pressed == key!(l) || pressed == key!(ctrl-l) || pressed == key!(right) { Some('l') }
                else { None };
            if let Some(direction) = direction {
                let destination = self.pane_in_direction(direction);
                if destination != self.pane && self.commit_text_edit() { self.focus_pane(destination); }
            } else if pressed == key!(w) || pressed == key!(ctrl-w) {
                let destination = match self.pane {
                    Pane::Collections => Pane::Url,
                    Pane::Url => Pane::Parameters,
                    Pane::Parameters => Pane::Response,
                    Pane::Response => Pane::Collections,
                };
                if self.commit_text_edit() { self.focus_pane(destination); }
            }
            return true;
        }
        if pressed == key!(ctrl-w) && self.editor_shortcuts_allowed() {
            self.pending_window = true;
            self.pending_g = None;
            self.editor_notice = None;
            return true;
        }
        false
    }

    pub fn handle_workspace_key(&mut self, pressed: KeyCombination) -> bool {
        let bindings = *KEY_BINDINGS.read();
        let keys = bindings.workspace;
        let workspace = matches!(self.state, Normal | SelectedRequest) || self.request_editor_active();
        if workspace {
            let direction = if pressed == keys.focus_left { Some('h') }
                else if pressed == keys.focus_down { Some('j') }
                else if pressed == keys.focus_up { Some('k') }
                else if pressed == keys.focus_right { Some('l') }
                else { None };
            if let Some(direction) = direction {
                self.pending_g = None;
                let destination = self.pane_in_direction(direction);
                if destination != self.pane && self.commit_text_edit() { self.focus_pane(destination); }
                return true;
            }
            if self.editor_shortcuts_allowed() && (pressed == keys.next_request || pressed == keys.previous_request
                || (pressed == keys.close_request && self.active_input().is_none())) {
                self.pending_g = None;
                if self.commit_text_edit() {
                    if pressed == keys.close_request { self.close_request_tab(); }
                    else { self.switch_request_tab(pressed == keys.next_request); }
                }
                return true;
            }
            if self.request_editor_active() && self.editor_shortcuts_allowed()
                && (pressed == keys.previous_panel || pressed == bindings.request_selected.param_next_tab)
                && self.pane == Pane::Parameters {
                if self.commit_text_edit() {
                    if pressed == keys.previous_panel { self.previous_request_panel(); }
                    else { self.tui_next_request_param_tab(); }
                    self.open_focused_body_editor();
                }
                return true;
            }
            if matches!(self.state, EditingRequestBodyString | EditingRequestBodyFile)
                && self.editor_shortcuts_allowed() && pressed == bindings.request_selected.param_tabs.change_body_content_type {
                if self.commit_text_edit() {
                    self.tui_next_request_content_type();
                    self.open_focused_body_editor();
                }
                return true;
            }
        }
        // These commands belong to lists/panes, not the Vim editor.
        if self.active_input().is_some() { return false; }
        if self.state == SelectedRequest {
            if pressed == keys.previous_panel {
                self.previous_request_panel();
                self.open_focused_body_editor();
                return true;
            }
            if pressed == bindings.request_selected.param_next_tab {
                match self.pane {
                    Pane::Parameters => self.tui_next_request_param_tab(),
                    Pane::Response => self.tui_next_request_result_tab(),
                    _ => {}
                }
                self.open_focused_body_editor();
                return true;
            }
            if self.pane == Pane::Url && (pressed == bindings.generic.list_and_table_actions.edit_element
                || [key!(enter), key!(a), key!(shift-I), key!(shift-A)].contains(&pressed)) {
                self.edit_request_url_state();
                self.begin_text_edit(pressed);
                return true;
            }
            if pressed == bindings.main_menu.exit {
                self.should_quit = true;
                return true;
            }
        }
        let navigable = matches!(self.state, Normal | SelectedRequest | DisplayingEnvEditor | DisplayingLogs);
        if navigable {
            if pressed == keys.page_up || pressed == keys.page_down {
                for _ in 0..10 { self.move_focused_row(pressed == keys.page_down); }
                self.pending_g = None;
                return true;
            }
            if pressed == key!(g) {
                if self.pending_g.take().is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
                    self.jump_focused_edge(false);
                } else { self.pending_g = Some(Instant::now()); }
                return true;
            }
            self.pending_g = None;
            if pressed == key!(shift-G) { self.jump_focused_edge(true); return true; }
        }
        if self.state == CreatingNewRequest && self.new_request_popup.selection != 2 {
            if pressed == bindings.generic.navigation.move_cursor_down { self.new_request_popup.next_input(); return true; }
            if pressed == bindings.generic.navigation.move_cursor_up { self.new_request_popup.previous_input(); return true; }
            if pressed == key!(esc) || pressed == key!(ctrl-c) { self.normal_state(); return true; }
        }
        false
    }

    fn focused_table(&mut self) -> Option<&mut StatefulCustomTable<'a>> {
        if self.state == DisplayingEnvEditor && !self.env_editor_list_focused { return Some(&mut self.env_editor_table); }
        if self.state != SelectedRequest || self.pane != Pane::Parameters { return None; }
        match self.request_param_tab {
            RequestParamsTabs::QueryParams => Some(&mut self.query_params_table),
            RequestParamsTabs::Headers => Some(&mut self.headers_table),
            RequestParamsTabs::Body => Some(&mut self.body_form_table),
            _ => None,
        }
    }

    fn move_focused_row(&mut self, down: bool) {
        if self.state == DisplayingEnvEditor && self.env_editor_list_focused {
            self.move_environment_cursor(down);
        } else if self.state == Normal {
            if down { self.collections_tree.down(); } else { self.collections_tree.up(); }
        } else if let Some(table) = self.focused_table() {
            if let Some((row, column)) = table.selection {
                let row = if down { (row + 1).min(table.rows.len().saturating_sub(1)) } else { row.saturating_sub(1) };
                table.update_selection(Some((row, column)));
            }
        } else if self.state == DisplayingLogs {
            if down { self.logs_vertical_scrollbar.page_down(); } else { self.logs_vertical_scrollbar.page_up(); }
        } else if self.pane == Pane::Response {
            if down { self.result_vertical_scrollbar.page_down(); } else { self.result_vertical_scrollbar.page_up(); }
        }
    }

    fn jump_focused_edge(&mut self, last: bool) {
        if self.state == DisplayingEnvEditor && self.env_editor_list_focused {
            self.env_editor_index = if last { self.environments.len().saturating_sub(1) } else { 0 };
            self.env_editor_table.update_selection(None);
            self.tui_update_env_variable_table();
        } else if self.state == Normal {
            if last { self.collections_tree.state.select_last(); } else { self.collections_tree.state.select_first(); }
        } else if let Some(table) = self.focused_table() {
            if let Some((_, column)) = table.selection {
                table.update_selection(Some((if last { table.rows.len().saturating_sub(1) } else { 0 }, column)));
            }
        } else if self.state == DisplayingLogs {
            if last { self.logs_vertical_scrollbar.bottom(); } else { self.logs_vertical_scrollbar.top(); }
        } else if self.pane == Pane::Response {
            if last { self.result_vertical_scrollbar.bottom(); } else { self.result_vertical_scrollbar.top(); }
        }
    }

    /// Arrow and Enter aliases are only applied outside text editors.
    pub fn navigation_alias(&self, pressed: KeyCombination) -> KeyCombination {
        let bindings = *KEY_BINDINGS.read();
        let nav = bindings.generic.navigation;
        if self.state == CreatingNewRequest { return pressed; }
        // A configured add-row shortcut takes priority over Vim entry aliases.
        // Active text editors handle their own `a` before reaching this routing.
        let adding_row = pressed == bindings.generic.list_and_table_actions.create_element
            && (self.state == DisplayingEnvEditor
                || (self.state == SelectedRequest && self.pane == Pane::Parameters
                    && matches!(self.request_param_tab, RequestParamsTabs::QueryParams | RequestParamsTabs::Headers | RequestParamsTabs::Body)));
        if adding_row { return pressed; }
        if matches!(self.state, SelectedRequest | DisplayingEnvEditor)
            && (self.state == DisplayingEnvEditor || self.pane == Pane::Parameters)
            && [key!(a), key!(shift-I), key!(shift-A)].contains(&pressed)
            && bindings.generic.text_input.mode == crate::app::files::key_bindings::TextAreaMode::Vim {
            return bindings.generic.list_and_table_actions.edit_element;
        }
        if pressed == key!(up) { nav.move_cursor_up }
        else if pressed == key!(down) { nav.move_cursor_down }
        else if pressed == key!(left) {
            if self.state == Normal { bindings.main_menu.unselect_request } else { nav.move_cursor_left }
        } else if pressed == key!(right) {
            if self.state == Normal { bindings.main_menu.expand_collection } else { nav.move_cursor_right }
        } else if pressed == key!(enter) && matches!(self.state, SelectedRequest | DisplayingEnvEditor) {
            bindings.generic.list_and_table_actions.edit_element
        } else { pressed }
    }
}
