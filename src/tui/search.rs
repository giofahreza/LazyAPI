//! Contextual search never dispatches query text as application shortcuts.
use std::sync::Arc;

use crokey::{key, KeyCombination};
use edtui::EditorMode;
use ratatui::{Frame, crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Margin, Rect}, style::{Color, Stylize},
    text::{Line, Span}, widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap}};

use crate::app::{app::App, files::theme::THEME};
use crate::tui::{app_states::AppState::*, ui::param_tabs::param_tabs::RequestParamsTabs,
    utils::stateful::{stateful_custom_table::StatefulCustomTable,
    stateful_scrollbar::StatefulScrollbar, text_input::{SingleLineTextInput, TextInput}}, workspace::Pane};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScope {
    Collections, Environments, Variables(usize), Help, Params(usize), Headers(usize),
    Form(usize), Cookies, Response(usize, u8), Logs, Export(usize),
}

impl SearchScope {
    fn title(self) -> &'static str {
        match self {
            Self::Collections => "Collections", Self::Environments => "Environments",
            Self::Variables(_) => "Environment variables", Self::Help => "Keyboard shortcuts",
            Self::Params(_) => "Query parameters", Self::Headers(_) => "Request headers",
            Self::Form(_) => "Form body", Self::Cookies => "Cookies", Self::Logs => "Logs",
            Self::Export(_) => "Request export",
            Self::Response(_, tab) => match tab {
                0 => "Response body", 1 => "Messages", 2 => "Response cookies",
                3 => "Response headers", _ => "Console",
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Location { Tree(Vec<usize>), Row(usize), Text(usize) }

#[derive(Clone)]
pub struct SearchEntry {
    location: Location,
    label: String,
    fields: Vec<String>,
    scroll: usize,
    width: u16,
    wrap: bool,
}

impl SearchEntry {
    fn new(location: Location, label: String, fields: Vec<String>) -> Self {
        Self { location, label, fields, scroll: 0, width: 0, wrap: false }
    }

    fn matches(&self, query: &str) -> bool {
        let text = self.label.to_lowercase();
        query.split_whitespace().all(|word| text.contains(&word.to_lowercase()))
    }

    fn column(&self, query: &str) -> usize {
        self.fields.iter().position(|value| {
            let value = value.to_lowercase();
            query.split_whitespace().any(|word| value.contains(&word.to_lowercase()))
        }).unwrap_or(0)
    }

    fn text_scroll(&self, query: &str) -> (usize, usize) {
        let text = self.fields.first().map(String::as_str).unwrap_or("");
        let word = query.split_whitespace().next().unwrap_or("").to_lowercase();
        let lower_offset = text.to_lowercase().find(&word).unwrap_or(0);
        // Case folding may change byte lengths (e.g. İ), so map back by character.
        let mut prefix = String::new();
        let mut lower_bytes = 0;
        for ch in text.chars() {
            if lower_bytes >= lower_offset { break; }
            prefix.push(ch);
            lower_bytes += ch.to_lowercase().map(char::len_utf8).sum::<usize>();
        }
        if self.wrap {
            prefix.push('x');
            let row = Paragraph::new(prefix).wrap(Wrap::default()).line_count(self.width.max(1)).saturating_sub(1);
            (self.scroll + row.saturating_sub(1), 0)
        } else {
            (self.scroll, Line::from(prefix).width().saturating_sub(self.width as usize / 3))
        }
    }
}

struct SearchPopup {
    scope: SearchScope,
    input: TextInput,
    results: Vec<SearchEntry>,
    selected: usize,
    list: ListState,
}

struct SearchHistory {
    scope: SearchScope,
    query: String,
    location: Location,
    index: usize,
    count: usize,
}

#[derive(Default)]
pub struct SearchState {
    popup: Option<SearchPopup>,
    history: Option<SearchHistory>,
    pub rendered: Vec<(SearchScope, Vec<SearchEntry>)>,
}

impl SearchState {
    pub fn is_open(&self) -> bool { self.popup.is_some() }

    /// Borrow the search input when the popup is open, so the main render
    /// can include it in the cursor scan without exposing the private
    /// `popup` field.
    pub fn input(&self) -> Option<&crate::tui::utils::stateful::text_input::TextInput> {
        self.popup.as_ref().map(|p| &p.input)
    }

    pub fn highlight_lines(&self, scope: SearchScope, lines: &mut [Line<'_>]) {
        let query = if let Some(popup) = &self.popup && popup.scope == scope { popup.input.to_string() }
            else if let Some(history) = &self.history && history.scope == scope { history.query.clone() }
            else { return; };
        highlight_matches(lines, &query);
    }

    pub fn register_lines(&mut self, scope: SearchScope, lines: &[Line<'_>], width: u16, wrap: bool) -> usize {
        let mut scroll = 0;
        let entries = lines.iter().enumerate().map(|(index, line)| {
            let text = line.to_string();
            let mut entry = SearchEntry::new(Location::Text(index), format!("{:>5}  {}", index + 1, text), vec![text]);
            entry.scroll = scroll;
            entry.width = width;
            entry.wrap = wrap;
            scroll += if wrap { Paragraph::new(line.clone()).wrap(Wrap::default()).line_count(width.max(1)).max(1) } else { 1 };
            entry
        }).collect();
        self.rendered.push((scope, entries));
        scroll
    }

    pub fn register_help(&mut self, rows: &[(String, String)], width: u16, key_width: usize) {
        let mut scroll = 0;
        let entries = rows.iter().enumerate().map(|(index, (keys, description))| {
            let text = if keys.is_empty() { description.clone() } else { format!("{keys:<key_width$} {description}") };
            let mut entry = SearchEntry::new(Location::Text(index), text.clone(), vec![text.clone()]);
            entry.scroll = scroll;
            scroll += textwrap::wrap(&text, width.max(1) as usize).len();
            entry
        }).collect();
        self.rendered.push((SearchScope::Help, entries));
    }
}

fn scroll_to(scrollbar: &mut StatefulScrollbar, value: usize) {
    let value = value.min(u16::MAX as usize) as u16;
    scrollbar.set_max_scroll(scrollbar.max_scroll.max(value));
    scrollbar.scroll = value;
    scrollbar.state = scrollbar.state.position(value as usize);
}

fn highlight_matches(lines: &mut [Line<'_>], query: &str) {
    let pattern = query.split_whitespace().map(regex::escape).collect::<Vec<_>>().join("|");
    if pattern.is_empty() { return; }
    let Ok(pattern) = regex::RegexBuilder::new(&pattern).case_insensitive(true).build() else { return; };
    let theme = THEME.read();
    for line in lines {
        let mut spans = vec![];
        for span in &line.spans {
            let mut end = 0;
            for found in pattern.find_iter(&span.content) {
                if found.start() > end { spans.push(Span::styled(span.content[end..found.start()].to_string(), span.style)); }
                spans.push(Span::styled(found.as_str().to_string(), span.style.fg(theme.palette.accent).bold().underlined()));
                end = found.end();
            }
            if end < span.content.len() { spans.push(Span::styled(span.content[end..].to_string(), span.style)); }
        }
        line.spans = spans;
    }
}

impl App<'_> {
    fn search_request_id(&self) -> usize {
        self.collections_tree.selected.map(|position| Arc::as_ptr(&self.get_request_as_local_from_indexes(&position)) as usize).unwrap_or(0)
    }

    pub fn response_search_scope(&self) -> SearchScope {
        SearchScope::Response(self.search_request_id(), self.request_result_tab as u8)
    }

    pub fn export_search_scope(&self) -> SearchScope { SearchScope::Export(self.search_request_id()) }

    pub fn search_scope(&self) -> Option<SearchScope> {
        if self.should_display_help { return Some(SearchScope::Help); }
        if self.active_input().is_some() || self.editor_command.is_some() || self.pending_window { return None; }
        let request = self.search_request_id();
        match self.state {
            Normal => Some(SearchScope::Collections),
            DisplayingEnvEditor if self.env_editor_list_focused => Some(SearchScope::Environments),
            DisplayingEnvEditor => Some(SearchScope::Variables(self.environments.get(self.env_editor_index)
                .map(|env| Arc::as_ptr(env) as usize).unwrap_or(0))),
            DisplayingCookies => Some(SearchScope::Cookies),
            DisplayingLogs => Some(SearchScope::Logs),
            DisplayingRequestExport => Some(self.export_search_scope()),
            SelectedRequest if self.pane == Pane::Response => Some(self.response_search_scope()),
            SelectedRequest if self.pane == Pane::Parameters => match self.request_param_tab {
                RequestParamsTabs::QueryParams => Some(SearchScope::Params(request)),
                RequestParamsTabs::Headers => Some(SearchScope::Headers(request)),
                RequestParamsTabs::Body if self.get_selected_request_as_local().read().get_http_request()
                    .is_ok_and(|request| request.body.get_form().is_ok()) => Some(SearchScope::Form(request)),
                _ => None,
            },
            _ => None,
        }
    }

    fn search_table(&self, scope: SearchScope) -> Option<&StatefulCustomTable<'_>> {
        match scope {
            SearchScope::Variables(_) => Some(&self.env_editor_table),
            SearchScope::Params(_) => Some(&self.query_params_table),
            SearchScope::Headers(_) => Some(&self.headers_table),
            SearchScope::Form(_) => Some(&self.body_form_table),
            _ => None,
        }
    }

    fn search_entries(&self, scope: SearchScope) -> Vec<SearchEntry> {
        if let Some(table) = self.search_table(scope) {
            return table.rows.iter().enumerate().map(|(index, row)| SearchEntry::new(Location::Row(index),
                format!("{} = {}", row.data.0, row.data.1), vec![row.data.0.clone(), row.data.1.clone()])).collect();
        }
        match scope {
            SearchScope::Collections => {
                let mut entries = vec![];
                for (collection_index, collection) in self.collections.iter().enumerate() {
                    entries.push(SearchEntry::new(Location::Tree(vec![collection_index]),
                        format!("{}  ({} requests)", collection.name, collection.requests.len()), vec![]));
                    for (request_index, request) in collection.requests.iter().enumerate() {
                        let request = request.read();
                        let method = request.get_http_request().map(|request| request.method.to_string()).unwrap_or_else(|_| "WS".into());
                        entries.push(SearchEntry::new(Location::Tree(vec![collection_index, request_index]),
                            format!("{} / {}  · {} {}", collection.name, request.name, method, request.url), vec![]));
                    }
                }
                entries
            },
            SearchScope::Environments => self.environments.iter().enumerate().map(|(index, env)| {
                SearchEntry::new(Location::Row(index), env.read().name.clone(), vec![])
            }).collect(),
            SearchScope::Cookies => self.cookies_popup.cookies_table.rows.iter().enumerate().map(|(index, row)| {
                SearchEntry::new(Location::Row(index), row.join(" · "), row.to_vec())
            }).collect(),
            _ => self.search.rendered.iter().find(|(target, _)| *target == scope)
                .map(|(_, entries)| entries.clone()).unwrap_or_default(),
        }
    }

    fn refresh_search_results(&mut self) {
        let Some(popup) = &self.search.popup else { return; };
        let query = popup.input.to_string();
        let results = self.search_entries(popup.scope).into_iter().filter(|entry| entry.matches(&query)).collect::<Vec<_>>();
        let popup = self.search.popup.as_mut().unwrap();
        popup.selected = popup.selected.min(results.len().saturating_sub(1));
        popup.results = results;
    }

    fn jump_to_search_result(&mut self, scope: SearchScope, entry: &SearchEntry, query: &str) {
        match (&scope, &entry.location) {
            (SearchScope::Collections, Location::Tree(path)) => {
                if path.len() == 2 { self.collections_tree.state.open(vec![path[0]]); }
                self.collections_tree.state.select(path.clone());
            },
            (SearchScope::Environments, Location::Row(index)) => {
                self.env_editor_index = *index;
                self.env_editor_table.update_selection(None);
                self.tui_update_env_variable_table();
            },
            (_, Location::Row(index)) => {
                let column = entry.column(query);
                let table = match scope {
                    SearchScope::Variables(_) => Some(&mut self.env_editor_table),
                    SearchScope::Params(_) => Some(&mut self.query_params_table),
                    SearchScope::Headers(_) => Some(&mut self.headers_table),
                    SearchScope::Form(_) => Some(&mut self.body_form_table),
                    _ => None,
                };
                if let Some(table) = table { table.update_selection(Some((*index, column.min(1)))); }
                else if scope == SearchScope::Cookies {
                    self.cookies_popup.cookies_table.selection = Some((*index, column));
                    for state in &mut self.cookies_popup.cookies_table.lists_states { state.select(Some(*index)); }
                }
            },
            (_, Location::Text(_)) => {
                let (row, column) = entry.text_scroll(query);
                match scope {
                    SearchScope::Help => self.help_popup.scroll = row.min(self.help_popup.max_scroll as usize) as u16,
                    SearchScope::Response(_, _) => {
                        scroll_to(&mut self.result_vertical_scrollbar, row);
                        scroll_to(&mut self.result_horizontal_scrollbar, column);
                    },
                    SearchScope::Logs => {
                        scroll_to(&mut self.logs_vertical_scrollbar, row);
                        scroll_to(&mut self.logs_horizontal_scrollbar, column);
                    },
                    SearchScope::Export(_) => {
                        scroll_to(&mut self.display_request_export.vertical_scrollbar, row);
                        scroll_to(&mut self.display_request_export.horizontal_scrollbar, column);
                    },
                    _ => {},
                }
            },
            _ => {},
        }
    }

    pub fn handle_search_key(&mut self, pressed: KeyCombination) -> bool {
        if self.search.popup.is_some() {
            if pressed == key!(esc) || pressed == key!(ctrl-c) {
                self.search.popup = None;
                return true;
            }
            if pressed == key!(enter) {
                self.refresh_search_results();
                let popup = self.search.popup.as_ref().unwrap();
                if let Some(entry) = popup.results.get(popup.selected).cloned() {
                    let query = popup.input.to_string();
                    let history = SearchHistory { scope: popup.scope, query: query.clone(), location: entry.location.clone(),
                        index: popup.selected, count: popup.results.len() };
                    self.jump_to_search_result(history.scope, &entry, &query);
                    self.search.history = if query.trim().is_empty() { None } else { Some(history) };
                    self.search.popup = None;
                }
                return true;
            }
            let event: KeyEvent = pressed.into();
            let popup = self.search.popup.as_mut().unwrap();
            if pressed == key!(down) || pressed == key!(ctrl-n) || pressed == key!(ctrl-j) || pressed == key!(tab) {
                if !popup.results.is_empty() { popup.selected = (popup.selected + 1) % popup.results.len(); }
            } else if pressed == key!(up) || pressed == key!(ctrl-p) || pressed == key!(ctrl-k) || pressed == key!(shift-backtab) {
                if !popup.results.is_empty() { popup.selected = (popup.selected + popup.results.len() - 1) % popup.results.len(); }
            } else if pressed == key!(pagedown) || pressed == key!(pageup) {
                popup.selected = if pressed == key!(pagedown) { (popup.selected + 10).min(popup.results.len().saturating_sub(1)) }
                    else { popup.selected.saturating_sub(10) };
            } else {
                let before = popup.input.to_string();
                match event.code {
                    KeyCode::Char('u') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                        popup.input.clear(); popup.input.state.mode = EditorMode::Insert;
                    },
                    KeyCode::Char('w') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                        let chars: Vec<_> = before.chars().collect();
                        let end = popup.input.state.cursor.col.min(chars.len());
                        let mut start = end;
                        while start > 0 && chars[start - 1].is_whitespace() { start -= 1; }
                        while start > 0 && !chars[start - 1].is_whitespace() { start -= 1; }
                        popup.input.state.execute(edtui::actions::DeleteChar(end - start));
                    },
                    KeyCode::Char(ch) if !event.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) => popup.input.push_char(ch),
                    KeyCode::Backspace => popup.input.delete_char_backward(),
                    KeyCode::Delete => popup.input.delete_char_forward(),
                    KeyCode::Left => popup.input.move_cursor_left(),
                    KeyCode::Right => popup.input.move_cursor_right(),
                    KeyCode::Home => popup.input.move_cursor_line_start(),
                    KeyCode::End => popup.input.move_cursor_line_end(),
                    _ => {},
                }
                if before != popup.input.to_string() { popup.selected = 0; }
            }
            self.refresh_search_results();
            return true;
        }
        let Some(scope) = self.search_scope() else { return false; };
        if pressed == key!('/') {
            let mut input = TextInput::new(None);
            input.insert_mode_only = true;
            input.display_cursor = true;
            input.update_handler();
            input.state.mode = EditorMode::Insert;
            self.search.popup = Some(SearchPopup { scope, input, results: vec![], selected: 0, list: ListState::default() });
            self.pending_g = None;
            self.editor_notice = None;
            self.refresh_search_results();
            return true;
        }
        let Some(history) = &self.search.history else { return false; };
        if history.scope != scope { return false; }
        if pressed == key!(esc) {
            self.search.history = None;
            self.editor_notice = None;
            return true;
        }
        if pressed == key!(n) || pressed == key!(shift-N) {
            let query = history.query.clone();
            let results: Vec<_> = self.search_entries(scope).into_iter().filter(|entry| entry.matches(&query)).collect();
            if results.is_empty() {
                self.editor_notice = Some("No matches · / search · Esc clear search".into());
                return true;
            }
            let current = results.iter().position(|entry| entry.location == history.location);
            let index = match (pressed == key!(n), current) {
                (true, Some(index)) => (index + 1) % results.len(),
                (false, Some(index)) => (index + results.len() - 1) % results.len(),
                (true, None) => 0, (false, None) => results.len() - 1,
            };
            self.jump_to_search_result(scope, &results[index], &query);
            self.search.history = Some(SearchHistory { scope, query, location: results[index].location.clone(), index, count: results.len() });
            return true;
        }
        false
    }

    pub fn paste_search(&mut self, text: String) {
        if let Some(popup) = &mut self.search.popup {
            // Paste is data, including control characters and shortcut letters.
            let text = text.replace("\r\n", " ").chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect();
            popup.input.paste(text);
            popup.selected = 0;
            self.refresh_search_results();
        }
    }

    pub fn search_hint(&self) -> Option<String> {
        let scope = self.search_scope()?;
        match &self.search.history {
            Some(history) if history.scope == scope => Some(format!("/{}  {}/{} · n/N match · Esc clear search", history.query, history.index + 1, history.count)),
            _ => Some("/ search".into()),
        }
    }

    pub fn render_search_popup(&mut self, frame: &mut Frame) {
        if !self.search.is_open() { return; }
        self.refresh_search_results();
        let popup = self.search.popup.as_mut().unwrap();
        let screen = frame.area();
        let width = screen.width.saturating_sub(2).min(110);
        let height = screen.height.saturating_sub(2).min(24);
        let area = Rect::new(screen.x + (screen.width - width) / 2, screen.y + (screen.height - height) / 2, width, height);
        let theme = THEME.read();
        let block = theme.panel(format!(" Search {} · {} {} ", popup.scope.title(), popup.results.len(),
            if popup.results.len() == 1 { "match" } else { "matches" }), true)
            .title_bottom(Line::from(" ↑/↓ or Ctrl+n/p select · Enter jump · Esc cancel · Ctrl+u clear ").fg(Color::White))
            .bg(theme.ui.main_background_color);
        let inner = block.inner(area).inner(Margin::new(1, 0));
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let parts = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(0)]).split(inner);
        let input = Layout::horizontal([Constraint::Length(2), Constraint::Min(0)]).split(parts[0]);
        frame.render_widget(Paragraph::new("/ ").fg(Color::White), input[0]);
        frame.render_widget(SingleLineTextInput(&mut popup.input), input[1]);
        if popup.results.is_empty() {
            frame.render_widget(Paragraph::new("No matches").fg(theme.ui.secondary_foreground_color), parts[2]);
        } else {
            let mut lines: Vec<_> = popup.results.iter().map(|entry| Line::from(entry.label.chars()
                .map(|ch| if ch.is_control() { ' ' } else { ch }).collect::<String>())).collect();
            highlight_matches(&mut lines, &popup.input.to_string());
            let items = lines.into_iter().map(ListItem::new);
            popup.list.select(Some(popup.selected));
            frame.render_stateful_widget(List::new(items).style(theme.base()).highlight_symbol("› ")
                .highlight_style(theme.selected()), parts[2], &mut popup.list);
        }
    }
}
