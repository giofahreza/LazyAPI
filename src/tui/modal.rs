//! Modal editing is routed before application shortcuts, so inserted text stays literal.
use std::io::Stdout;
use crokey::{key, KeyCombination};
use edtui::EditorMode;
use ratatui::{backend::CrosstermBackend, Terminal};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::app::app::App;
use crate::app::files::key_bindings::{TextAreaMode, KEY_BINDINGS};
use crate::tui::app_states::AppState::*;
use crate::tui::utils::stateful::text_input::TextInput;

macro_rules! active_input {
    ($app:ident $(, $mutable:ident)?) => {
        match $app.state {
            EditingEnvVariable => Some(&$($mutable)? $app.env_editor_table.selection_text_input),
            CreatingEnvironment | RenamingEnvironment => Some(&$($mutable)? $app.env_name_input),
            CreatingNewCollection => Some(&$($mutable)? $app.new_collection_input),
            CreatingNewRequest if !$app.new_request_popup.is_curl() && $app.new_request_popup.selection == 2 => Some(&$($mutable)? $app.new_request_popup.text_input),
            CreatingNewRequest if $app.new_request_popup.is_curl() && $app.new_request_popup.selection == 1 => Some(&$($mutable)? $app.new_request_popup.text_input),
            CreatingNewRequest if $app.new_request_popup.is_curl() && $app.new_request_popup.selection == 2 => Some(&$($mutable)? $app.new_request_popup.curl_text_area),
            RenamingCollection => Some(&$($mutable)? $app.rename_collection_input),
            RenamingRequest => Some(&$($mutable)? $app.rename_request_input),
            EditingRequestUrl => Some(&$($mutable)? $app.url_text_input),
            EditingRequestParam => Some(&$($mutable)? $app.query_params_table.selection_text_input),
            EditingRequestAuthBasicUsername => Some(&$($mutable)? $app.auth_basic_username_text_input),
            EditingRequestAuthBasicPassword => Some(&$($mutable)? $app.auth_basic_password_text_input),
            EditingRequestAuthBearerToken => Some(&$($mutable)? $app.auth_bearer_token_text_input),
            EditingRequestAuthJwtSecret => Some(&$($mutable)? $app.auth_jwt_secret_text_input),
            EditingRequestAuthJwtPayload => Some(&$($mutable)? $app.auth_jwt_payload_text_area),
            EditingRequestAuthDigestUsername => Some(&$($mutable)? $app.auth_digest_username_text_input),
            EditingRequestAuthDigestPassword => Some(&$($mutable)? $app.auth_digest_password_text_input),
            EditingRequestAuthDigestDomains => Some(&$($mutable)? $app.auth_digest_domains_text_input),
            EditingRequestAuthDigestRealm => Some(&$($mutable)? $app.auth_digest_realm_text_input),
            EditingRequestAuthDigestNonce => Some(&$($mutable)? $app.auth_digest_nonce_text_input),
            EditingRequestAuthDigestOpaque => Some(&$($mutable)? $app.auth_digest_opaque_text_input),
            EditingRequestHeader => Some(&$($mutable)? $app.headers_table.selection_text_input),
            EditingRequestBodyTable => Some(&$($mutable)? $app.body_form_table.selection_text_input),
            EditingRequestBodyFile => Some(&$($mutable)? $app.body_file_text_input),
            EditingRequestBodyString => Some(&$($mutable)? $app.body_text_area),
            EditingRequestMessage => Some(&$($mutable)? $app.message_text_area),
            EditingPreRequestScript => Some(&$($mutable)? $app.script_console.pre_request_text_area),
            EditingPostRequestScript => Some(&$($mutable)? $app.script_console.post_request_text_area),
            _ => None,
        }
    };
}

impl App<'_> {
    pub fn active_input(&self) -> Option<&TextInput> { active_input!(self) }
    pub fn active_input_mut(&mut self) -> Option<&mut TextInput> { active_input!(self, mut) }

    pub fn editor_normal_mode(&self) -> bool {
        self.editor_command.is_none() && self.active_input().is_none_or(|input| input.state.mode == EditorMode::Normal)
    }

    pub fn editor_shortcuts_allowed(&self) -> bool {
        self.editor_normal_mode() && self.active_input().is_none_or(|input| input.vim_pending.is_empty())
    }

    pub fn mode_label(&self) -> &'static str {
        if self.search.is_open() { return "SEARCH"; }
        if self.editor_command.is_some() { return "COMMAND"; }
        match self.active_input().map(|input| input.state.mode) {
            Some(EditorMode::Insert) => "INSERT",
            Some(EditorMode::Visual) => "VISUAL",
            Some(EditorMode::Search) => "SEARCH",
            _ => "NORMAL",
        }
    }

    pub fn request_editor_active(&self) -> bool {
        self.active_input().is_some() && !matches!(self.state,
            EditingEnvVariable | CreatingEnvironment | RenamingEnvironment | CreatingNewCollection | CreatingNewRequest | RenamingCollection | RenamingRequest)
    }

    pub fn start_insert(&mut self) {
        if let Some(input) = self.active_input_mut() {
            input.state.execute(edtui::actions::SwitchMode(EditorMode::Insert));
        }
    }

    /// A body/message panel is already a text editor: focusing it should expose
    /// its Normal-mode cursor immediately, without an extra Enter first.
    pub fn open_focused_body_editor(&mut self) {
        use crate::tui::ui::param_tabs::param_tabs::RequestParamsTabs;
        use crate::tui::workspace::Pane;
        if self.state != SelectedRequest || self.pane != Pane::Parameters
            || KEY_BINDINGS.read().generic.text_input.mode != TextAreaMode::Vim {
            return;
        }
        match self.request_param_tab {
            RequestParamsTabs::Body => self.edit_request_body_file_or_string_state(),
            RequestParamsTabs::Message => self.edit_request_message_state(),
            _ => return,
        }
        if self.active_input().is_some() { self.begin_text_edit(key!(enter)); }
    }

    /// Called once when a field opens. Enter inspects it in Normal mode; an
    /// insert/append command performs the same action as it would inside Vim.
    pub fn begin_text_edit(&mut self, entry: KeyCombination) {
        self.editor_command = None;
        self.editor_notice = None;
        self.pending_g = None;
        let vim = KEY_BINDINGS.read().generic.text_input.mode == TextAreaMode::Vim;
        let dialog = matches!(self.state, CreatingEnvironment | RenamingEnvironment | CreatingNewCollection | CreatingNewRequest | RenamingCollection | RenamingRequest);
        if let Some(input) = self.active_input_mut() {
            input.saved_text = input.to_string();
            input.update_handler();
            if vim && !dialog {
                input.state.mode = EditorMode::Normal;
                input.move_cursor_start();
                input.move_cursor_line_start();
                if entry != key!(enter) {
                    let command = if [key!(a), key!(shift-I), key!(shift-A)].contains(&entry) { entry } else { key!(i) };
                    input.key_event(command, None);
                }
            } else {
                input.state.execute(edtui::actions::SwitchMode(EditorMode::Insert));
            }
        }
    }

    pub fn editing_context(&self) -> String {
        let column = |selection: Option<(usize, usize)>| if selection.is_some_and(|(_, col)| col == 1) { "value" } else { "key" };
        match self.state {
            EditingRequestUrl => "Edit URL".into(),
            EditingRequestParam => format!("Edit query {}", column(self.query_params_table.selection)),
            EditingRequestHeader => format!("Edit header {}", column(self.headers_table.selection)),
            EditingRequestBodyTable => format!("Edit form {}", column(self.body_form_table.selection)),
            EditingEnvVariable => format!("Edit environment {}", column(self.env_editor_table.selection)),
            EditingRequestBodyString => "Edit body".into(),
            EditingRequestBodyFile => "Edit body file".into(),
            EditingRequestMessage => "Edit message".into(),
            EditingPreRequestScript => "Edit pre-request script".into(),
            EditingPostRequestScript => "Edit post-request script".into(),
            CreatingNewCollection => "New collection".into(),
            CreatingNewRequest => "New request".into(),
            RenamingCollection => "Rename collection".into(),
            RenamingRequest => "Rename request".into(),
            DisplayingEnvEditor => "Environments".into(),
            CreatingEnvironment => "New environment".into(),
            RenamingEnvironment => "Rename environment".into(),
            DeletingEnvironment => "Delete environment".into(),
            _ if self.active_input().is_some() => format!("Edit auth {}", self.active_input().unwrap().block_title.as_deref().unwrap_or("value")),
            _ => self.pane.label().into(),
        }
    }

    /// Returns false when validation kept the editor open. Navigation must then stop.
    pub fn commit_text_edit(&mut self) -> bool {
        match self.state {
            EditingEnvVariable => self.tui_modify_env_variable(),
            CreatingEnvironment | RenamingEnvironment => self.tui_save_environment_name(),
            CreatingNewCollection => self.tui_new_collection(),
            CreatingNewRequest => self.tui_new_request(),
            RenamingCollection => self.tui_rename_collection(),
            RenamingRequest => self.tui_rename_request(),
            EditingRequestUrl => self.tui_modify_request_url(),
            EditingRequestParam => self.tui_modify_request_query_param(),
            EditingRequestAuthBasicUsername => self.tui_modify_request_auth_basic_username(),
            EditingRequestAuthBasicPassword => self.tui_modify_request_auth_basic_password(),
            EditingRequestAuthBearerToken => self.tui_modify_request_auth_bearer_token(),
            EditingRequestAuthJwtSecret => self.tui_modify_request_auth_jwt_secret(),
            EditingRequestAuthJwtPayload => self.tui_modify_request_auth_jwt_payload(),
            EditingRequestAuthDigestUsername => self.tui_modify_request_auth_digest_username(),
            EditingRequestAuthDigestPassword => self.tui_modify_request_auth_digest_password(),
            EditingRequestAuthDigestDomains => self.tui_modify_request_auth_digest_domains(),
            EditingRequestAuthDigestRealm => self.tui_modify_request_auth_digest_realm(),
            EditingRequestAuthDigestNonce => self.tui_modify_request_auth_digest_nonce(),
            EditingRequestAuthDigestOpaque => self.tui_modify_request_auth_digest_opaque(),
            EditingRequestHeader => self.tui_modify_request_header(),
            EditingRequestBodyTable => self.tui_modify_request_form_data(),
            EditingRequestBodyFile | EditingRequestBodyString => self.tui_modify_request_body(),
            EditingRequestMessage => self.tui_store_request_message(),
            EditingPreRequestScript => self.tui_modify_pre_request_script(),
            EditingPostRequestScript => self.tui_modify_post_request_script(),
            _ => return true,
        }
        let committed = self.active_input().is_none();
        if committed {
            self.editor_command = None;
            self.editor_notice = None;
        } else if self.editor_notice.is_none() {
            self.editor_notice = Some("Could not save; check the value. Ctrl+c discards this edit.".into());
        }
        committed
    }

    fn write_text_edit(&mut self) {
        // Saving a field must retain its cursor, selection, search, and undo
        // history. The existing save path refreshes all inputs on success.
        let keep_open = self.request_editor_active() || self.state == EditingEnvVariable;
        let state = self.state;
        let env_selection = self.env_editor_table.selection;
        let snapshot = self.active_input().cloned();
        if self.commit_text_edit() && keep_open && let Some(mut input) = snapshot {
            self.state = state;
            if state == EditingEnvVariable { self.env_editor_table.update_selection(env_selection); }
            input.saved_text = input.to_string();
            *self.active_input_mut().unwrap() = input;
            self.editor_notice = Some("Saved · :wq finish · :q close · i insert".into());
        }
    }

    fn cancel_text_edit(&mut self) {
        self.editor_command = None;
        self.editor_notice = None;
        match self.state {
            EditingEnvVariable | CreatingEnvironment | RenamingEnvironment => self.display_env_editor_state(),
            CreatingNewCollection | CreatingNewRequest | RenamingCollection | RenamingRequest => self.normal_state(),
            _ => self.select_request_state(),
        }
    }

    /// A small, explicit Ex command set; commands never execute shell or HTTP requests.
    pub fn handle_editor_command(&mut self, pressed: KeyCombination) -> bool {
        if self.should_display_help { return false; }
        let vim = KEY_BINDINGS.read().generic.text_input.mode == TextAreaMode::Vim;
        if self.editor_command.is_none() {
            if vim && self.active_input().is_some() && self.editor_shortcuts_allowed() && pressed == key!(':') {
                self.editor_command = Some(String::new());
                self.editor_notice = None;
                return true;
            }
            return false;
        }
        let event: KeyEvent = pressed.into();
        match event.code {
            KeyCode::Esc => { self.editor_command = None; }
            KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => { self.editor_command = None; }
            KeyCode::Backspace => {
                if self.editor_command.as_mut().unwrap().pop().is_none() { self.editor_command = None; }
            }
            KeyCode::Char('u') if event.modifiers.contains(KeyModifiers::CONTROL) => { self.editor_command.as_mut().unwrap().clear(); }
            KeyCode::Enter => {
                let command = self.editor_command.take().unwrap();
                match command.trim() {
                    "" => {}
                    "w" | "write" => self.write_text_edit(),
                    "wq" | "x" | "exit" => { self.commit_text_edit(); }
                    "q" | "quit" => {
                        if self.active_input().is_some_and(TextInput::is_modified) {
                            self.editor_notice = Some("Unsaved changes · :w save · :wq finish · :q! discard".into());
                        } else { self.cancel_text_edit(); }
                    }
                    "q!" | "quit!" => self.cancel_text_edit(),
                    "h" | "help" => { self.should_display_help = true; self.help_popup.scroll = 0; }
                    "noh" | "nohlsearch" => { self.active_input_mut().unwrap().state.execute(edtui::actions::StopSearch); }
                    _ => { self.editor_notice = Some("Unknown command · :w · :wq · :q · :q! · :noh · :help".into()); }
                }
                self.sync_request_tabs();
            }
            KeyCode::Char(ch) if !event.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => {
                // Commands are short; bound this buffer so a paste cannot grow it indefinitely.
                if self.editor_command.as_ref().unwrap().len() < 80 { self.editor_command.as_mut().unwrap().push(ch); }
            }
            _ => {}
        }
        true
    }

    pub async fn handle_editor_key(&mut self, pressed: KeyCombination, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> bool {
        let is_tab = pressed == key!(tab) || pressed == key!(shift-backtab);
        let editor_is_normal = self.active_input().is_some_and(|input| input.state.mode == EditorMode::Normal);

        // Tab changes fields in the creation dialog from Normal mode. In
        // Insert mode it must reach edtui, which treats an unmodified Tab as
        // a literal tab character.
        if self.state == CreatingNewRequest && is_tab && editor_is_normal {
            if pressed == key!(tab) { self.new_request_popup.next_input(); }
            else { self.new_request_popup.previous_input(); }
            if self.active_input().is_some() { self.start_insert(); }
            return true;
        }
        let Some(input) = self.active_input() else { return false; };
        // Request-panel navigation owns Tab only after the editor has returned
        // to Normal mode. Returning false lets handle_workspace_key commit and
        // switch panels; Insert mode continues through key_event below.
        if is_tab && input.state.mode == EditorMode::Normal { return false; }
        let bindings = KEY_BINDINGS.read().generic.text_input;
        let single_line = input.is_single_line;
        let searching = input.state.mode == EditorMode::Search;
        let vim = bindings.mode == TextAreaMode::Vim;
        self.editor_notice = None;

        // Rename popups are insert-only; Esc cancels instead of toggling to Normal mode.
        let rename_popup = matches!(self.state, RenamingCollection | RenamingRequest);
        if pressed == key!(esc) && rename_popup {
            self.cancel_text_edit();
        } else if pressed == key!(esc) && vim {
            self.active_input_mut().unwrap().enter_normal_mode();
        } else if pressed == key!(ctrl-c) || pressed == bindings.quit_without_saving {
            self.cancel_text_edit();
        } else if pressed == key!(ctrl-s) || pressed == bindings.save_and_quit_area
            || (single_line && !searching && pressed == bindings.save_and_quit_single_line) {
            self.commit_text_edit();
        } else if pressed == key!(ctrl-enter) && self.state == EditingRequestMessage {
            if self.commit_text_edit() { self.tui_send_request_message().await; }
        } else {
            self.active_input_mut().unwrap().key_event(pressed, Some(terminal));
        }
        true
    }
}
