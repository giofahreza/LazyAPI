use ratatui::backend::Backend;
use ratatui::crossterm::ExecutableCommand;
use ratatui::prelude::Position;
use ratatui::{Frame, Terminal};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Block;

use std::io::stdout;

use edtui::EditorMode;

use crate::app::app::{App};
use crate::app::files::theme::THEME;
use crate::models::protocol::protocol::Protocol;
use crate::tui::app_states::AppState::*;
use crate::tui::utils::stateful::text_input::{cursor_shape_for_mode, TextInput};


impl App<'_> {
    /// The screen cell the terminal cursor should occupy after this frame,
    /// plus the editor mode that decides the cursor shape. Only one TextInput
    /// has display_cursor at a time, so we walk the known fields and
    /// pick the first one whose widget render just computed a screen cell.
    fn pending_terminal_cursor(&self) -> Option<((u16, u16), EditorMode)> {
        let env_table_input = &self.env_editor_table.selection_text_input;
        let script_inputs = (
            &self.script_console.pre_request_text_area,
            &self.script_console.post_request_text_area,
        );
        // The search popup's input field is also a TextInput; we grab it
        // through the public accessor on SearchState so the cursor shows up
        // there too.
        let search_input = self.search.input();
        let inputs: [&TextInput; 25] = [
            &self.env_name_input,
            &self.new_collection_input,
            &self.rename_collection_input,
            &self.new_request_popup.text_input,
            &self.new_request_popup.curl_text_area,
            &self.rename_request_input,
            &self.url_text_input,
            &self.auth_basic_username_text_input,
            &self.auth_basic_password_text_input,
            &self.auth_bearer_token_text_input,
            &self.auth_jwt_secret_text_input,
            &self.auth_jwt_payload_text_area,
            &self.auth_digest_username_text_input,
            &self.auth_digest_password_text_input,
            &self.auth_digest_domains_text_input,
            &self.auth_digest_realm_text_input,
            &self.auth_digest_nonce_text_input,
            &self.auth_digest_opaque_text_input,
            &self.body_file_text_input,
            &self.body_text_area,
            &self.message_text_area,
            script_inputs.0,
            script_inputs.1,
            env_table_input,
            search_input.unwrap_or(env_table_input),
        ];
        inputs.into_iter()
            .filter(|i| i.display_cursor)
            .find_map(|i| i.last_cursor_screen_cell.map(|pos| (pos, i.state.mode)))
    }

    fn ui(&mut self, frame: &mut Frame) {
        self.search.rendered.clear();

        frame.render_widget(Block::new().style(THEME.read().base()), frame.area());
        let main_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area());
        let sidebar_width = if frame.area().width >= 100 {
            (frame.area().width / 3).clamp(34, 56).min(frame.area().width.saturating_sub(60))
        } else if self.pane == crate::tui::workspace::Pane::Collections || self.collections_tree.selected.is_none() {
            frame.area().width
        } else { 0 };
        let inner_layout = Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(0)]).split(main_layout[0]);

        // LEFT LAYOUT

        let env_and_collections_layout = Layout::vertical([
            Constraint::Length(3), Constraint::Fill(1),
        ]).split(inner_layout[0]);
        if sidebar_width > 0 {
            self.render_environments(frame, env_and_collections_layout[0]);
            self.render_collections(frame, env_and_collections_layout[1]);
        }

        // REQUEST

        if !inner_layout[1].is_empty() { match self.collections_tree.selected {
            None => self.render_homepage(frame, inner_layout[1]),
            Some(selection) => {
                let selected_request = self.get_request_as_local_from_indexes(&selection).read().clone();

                match selected_request.protocol {
                    Protocol::HttpRequest(_) => self.render_http_request(frame, inner_layout[1], selected_request),
                    Protocol::WsRequest(_) => self.render_ws_request(frame, inner_layout[1], selected_request),
                }
            }
        } }

        self.render_footer(frame, main_layout[1]);

        // POPUPS

        match self.state {
            DisplayingEnvEditor | EditingEnvVariable | CreatingEnvironment | RenamingEnvironment | DeletingEnvironment => self.render_env_editor_popup(frame),
            DisplayingCookies | EditingCookies => self.render_cookies_popup(frame),
            DisplayingLogs => self.render_logs_popup(frame),
            ChoosingElementToCreate => self.render_creating_element_popup(frame),
            CreatingNewCollection => self.render_creating_new_collection_popup(frame),
            CreatingNewRequest => self.render_creating_new_request_popup(frame),
            DeletingCollection => self.render_deleting_collection_popup(frame),
            DeletingRequest => self.render_deleting_request_popup(frame),
            EditingRequestSettings => self.render_request_settings_popup(frame),
            RenamingCollection => self.render_renaming_collection_popup(frame),
            RenamingRequest => self.render_renaming_request_popup(frame),
            ChoosingRequestExportFormat => self.render_export_format_popup(frame),
            DisplayingRequestExport => {
                let scope = self.export_search_scope();
                self.search.register_lines(scope, &self.display_request_export.lines,
                    frame.area().width.min(100).saturating_sub(4), false);
                self.display_request_export.render(frame);
            },
            _ => {}
        }

        if self.should_display_help {
            self.render_help_popup(frame);
        }
        self.render_search_popup(frame);

        // Hand the cursor off to the real terminal in Insert / Search mode
        // (the painted cursor is suppressed there, so the terminal must draw
        // the block). In Normal / Visual mode the painted cell already shows
        // the cursor, so we leave the terminal cursor alone and let ratatui
        // hide it. We still emit the matching bar shape so the terminal
        // remembers it for the next time it needs to show a cursor.
        if let Some((cell, mode)) = self.pending_terminal_cursor() {
            // Insert / Search: the painted cursor is suppressed, so the real
            // terminal draws a block on top of the cell.
            // Normal / Visual: the painted white cell already shows the
            // cursor, so we leave the terminal cursor alone and just keep the
            // shape set to bar for the next time it needs to show.
            if matches!(mode, EditorMode::Insert | EditorMode::Search) {
                let (x, y) = cell;
                frame.set_cursor_position(Position::new(x, y));
            }
            let _ = stdout().execute(cursor_shape_for_mode(mode));
        }
    }

    pub fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), B::Error> {
        terminal.draw(|frame | self.ui(frame))?;
        Ok(())
    }
}
