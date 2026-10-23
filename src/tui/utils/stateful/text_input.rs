use crate::app::files::key_bindings::{CustomTextArea, TextAreaMode, KEY_BINDINGS};
use crate::app::files::theme::THEME;
use crokey::{key, KeyCombination};
use edtui::actions::insert::PushLine;
use edtui::actions::motion::MoveToFirstRow;
use edtui::actions::search::StartSearch;
use edtui::actions::{Composed, CopySelection, DeleteChar, FindNext, FindPrevious, InsertChar, LineBreak, MoveBackward, MoveDown, MoveForward, MoveToEndOfLine, MoveToStartOfLine, MoveUp, MoveWordBackward, MoveWordForward, OpenSystemEditor, Paste, Redo, RemoveChar, RemoveCharFromSearch, SelectCurrentSearch, StopSearch, SwitchMode, Undo};
use edtui::events::{KeyEventHandler, KeyEventRegister, KeyInput};
use edtui::{system_editor, EditorEventHandler, EditorMode, EditorState, EditorStatusLine, EditorTheme, EditorView, LineNumbers};
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::{Color, Modifier, Position, Rect, Style, Widget};
use ratatui::widgets::Padding;
use ratatui::Terminal;
use std::collections::HashMap;
use std::io::Stdout;

#[derive(Clone)]
pub struct TextInput {
    pub state: EditorState,
    pub event_handler: Option<EditorEventHandler>,
    pub default_mode: EditorMode,
    pub block_title: Option<String>,
    pub is_single_line: bool,
    pub insert_mode_only: bool,
    pub highlight_text: bool,
    pub highlight_block: bool,
    pub display_cursor: bool,
    pub saved_text: String,
    pub vim_pending: String,
    /// Filled in by the widget's `render` with the on-screen cell of the
    /// editor's cursor so the main render can hand it to the terminal.
    pub last_cursor_screen_cell: Option<(u16, u16)>,
}

pub struct SingleLineTextInput<'a>(pub &'a mut TextInput);
pub struct MultiLineTextInput<'a>(pub &'a mut TextInput);

/// Map an editor mode to the terminal cursor shape we want the OS to draw.
/// The terminal remembers the shape across frames, so the caller can emit
/// the escape once per mode change.
pub fn cursor_shape_for_mode(mode: EditorMode) -> SetCursorStyle {
    match mode {
        EditorMode::Insert | EditorMode::Search => SetCursorStyle::BlinkingBar,
        EditorMode::Normal | EditorMode::Visual => SetCursorStyle::BlinkingBar,
    }
}

fn input_theme(input: &TextInput) -> EditorTheme<'static> {
    let theme = THEME.read();
    // Insert mode: no painted cursor. The real terminal draws a caret on
    // top of the insertion point (see `cursor_shape_for_mode`) and the user can still
    // read the character underneath.
    // Normal / Visual mode: keep the original "painted character" cursor
    // (white background, foreground = background, BOLD) so the cursor still
    // looks like a recoloured cell instead of a terminal-drawn bar.
    // Search mode and unfocused fields: no painted cursor.
    let cursor = if !input.display_cursor
        || matches!(input.state.mode, EditorMode::Insert | EditorMode::Search)
    {
        Style::default()
    } else {
        Style::default()
            .fg(theme.palette.background)
            .bg(theme.ui.font_color)
            .add_modifier(Modifier::BOLD)
    };
    EditorTheme::default()
        .base(Style::default().fg(theme.ui.font_color).bg(theme.ui.main_background_color))
        .line_numbers_style(Style::default().fg(theme.ui.secondary_foreground_color))
        .cursor_style(cursor)
        .selection_style(if input.display_cursor {
            Style::default().fg(theme.ui.font_color).bg(theme.palette.selection)
        } else { Style::default() })
}


/// Insert/Search mode uses the terminal cursor because edtui's painted cursor
/// is intentionally hidden. Use a sentinel style for one render pass so the
/// terminal position comes from edtui's own viewport and coordinate mapping,
/// then render again with the real theme so the sentinel is never visible.
fn terminal_cursor_needed(input: &TextInput) -> bool {
    input.display_cursor && matches!(input.state.mode, EditorMode::Insert | EditorMode::Search)
}

fn cursor_marker_style() -> Style {
    Style::default()
        .fg(Color::Rgb(1, 2, 3))
        .bg(Color::Rgb(4, 5, 6))
}

fn find_cursor_marker(buffer: &Buffer, area: Rect) -> Option<(u16, u16)> {
    let marker_foreground = Color::Rgb(1, 2, 3);
    let marker_background = Color::Rgb(4, 5, 6);

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let position = Position::new(x, y);
            if let Some(cell) = buffer.cell(position)
                && cell.fg == marker_foreground
                && cell.bg == marker_background
            {
                return Some((x, y));
            }
        }
    }

    None
}

fn render_single_line_editor(input: &mut TextInput, area: Rect, buffer: &mut Buffer) -> Option<(u16, u16)> {
    if terminal_cursor_needed(input) {
        let marker_theme = input_theme(input).hide_status_line().cursor_style(cursor_marker_style());
        let actual_theme = input_theme(input).hide_status_line();
        EditorView::new(&mut input.state)
            .theme(marker_theme)
            .wrap(false)
            .render(area, buffer);
        let cursor = find_cursor_marker(buffer, area);
        EditorView::new(&mut input.state)
            .theme(actual_theme)
            .wrap(false)
            .render(area, buffer);
        cursor
    } else {
        let actual_theme = input_theme(input).hide_status_line();
        EditorView::new(&mut input.state)
            .theme(actual_theme)
            .wrap(false)
            .render(area, buffer);
        None
    }
}

fn multiline_editor_theme(input: &TextInput) -> EditorTheme<'static> {
    let mut theme = input_theme(input).hide_status_line();
    if input.display_cursor && input.state.mode == EditorMode::Search {
        theme = theme.status_line(EditorStatusLine::default()
            .style_line(THEME.read().base())
            .style_mode(Style::new().fg(THEME.read().palette.accent))
            .style_search(Style::new().fg(THEME.read().ui.font_color)));
    }
    if let Some(block_title) = &input.block_title {
        theme = theme.block(THEME.read().panel(block_title.clone(), input.highlight_block)
            .padding(Padding::horizontal(1)));
    }
    theme
}

fn render_multiline_editor(input: &mut TextInput, area: Rect, buffer: &mut Buffer) -> Option<(u16, u16)> {
    if terminal_cursor_needed(input) {
        let marker_theme = multiline_editor_theme(input).cursor_style(cursor_marker_style());
        let actual_theme = multiline_editor_theme(input);
        EditorView::new(&mut input.state)
            .theme(marker_theme)
            .wrap(false)
            .line_numbers(LineNumbers::Absolute)
            .tab_width(4)
            .render(area, buffer);
        let cursor = find_cursor_marker(buffer, area);
        EditorView::new(&mut input.state)
            .theme(actual_theme)
            .wrap(false)
            .line_numbers(LineNumbers::Absolute)
            .tab_width(4)
            .render(area, buffer);
        cursor
    } else {
        let actual_theme = multiline_editor_theme(input);
        EditorView::new(&mut input.state)
            .theme(actual_theme)
            .wrap(false)
            .line_numbers(LineNumbers::Absolute)
            .tab_width(4)
            .render(area, buffer);
        None
    }
}

impl<'a> Widget for SingleLineTextInput<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) where Self: Sized {
        let new_area = match &self.0.block_title {
            None => area,
            Some(block_title) => {
                let block = THEME.read().panel(block_title.clone(), self.0.highlight_block)
                    .padding(Padding::horizontal(1));

                let block_area = block.inner(area);

                block.render(area, buf);

                block_area
            }
        };


        // The cursor position is read by the main render and pushed to the
        // terminal via `frame.set_cursor_position`; we cannot call that from
        // inside a `Widget::render` (the trait only gives us a `Buffer`), so
        // we stash the cell selected by edtui and the caller picks it up.
        self.0.last_cursor_screen_cell = render_single_line_editor(self.0, new_area, buf);
    }
}

impl<'a> Widget for MultiLineTextInput<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) where Self: Sized {
        self.0.last_cursor_screen_cell = render_multiline_editor(self.0, area, buf);
    }
}

impl TextInput {
    pub fn new(block_title: Option<String>) -> Self {
        let state = EditorState::default();

        Self {
            state,
            event_handler: None,
            default_mode: EditorMode::Insert, // Placeholder
            block_title,
            is_single_line: true, // Placeholder
            insert_mode_only: false, // Placeholder
            highlight_text: false, // Placeholder
            highlight_block: false, // Placeholder
            display_cursor: false, // Placeholder
            saved_text: String::new(),
            vim_pending: String::new(),
            last_cursor_screen_cell: None,
        }
    }

    pub fn update_handler(&mut self) {
        self.vim_pending.clear();
        let editor_handler = EditorEventHandler::new(generate_key_handler(self.is_single_line, self.insert_mode_only));
        self.event_handler = Some(editor_handler);
    }

    pub fn reset_mode(&mut self) {
        self.state.mode = self.default_mode;
    }

    pub fn reset_cursor_position(&mut self) {
        if self.is_single_line {
            self.move_cursor_line_end();
        }
        else {
            self.move_cursor_start();
        }
    }

    pub fn reset_selection(&mut self) {
        self.state.selection = None;
    }

    pub fn clear(&mut self) {
        // A new field/request must not inherit another field's undo history or search.
        self.state = EditorState::default();
        self.state.mode = self.default_mode;
        self.saved_text.clear();
        self.vim_pending.clear();
        if self.event_handler.is_some() { self.update_handler(); }
    }

    pub fn to_string(&self) -> String {
        self.state.lines.to_string()
    }

    pub fn is_modified(&self) -> bool {
        self.to_string() != self.saved_text
    }

    /// Escape ends insertion/selection/search and abandons any pending operator.
    /// It never saves or closes the field, and is idempotent in Normal mode.
    pub fn enter_normal_mode(&mut self) {
        let mode = self.state.mode;
        if mode == EditorMode::Search { self.state.execute(StopSearch); }
        if mode == EditorMode::Insert {
            self.state.cursor.col = self.state.cursor.col.saturating_sub(1);
        }
        self.state.execute(SwitchMode(EditorMode::Normal));
        self.update_handler();
    }

    pub fn to_lines(&self) -> Vec<String> {
        self.state.lines.to_vecs().iter().map(|line| line.iter().collect()).collect()
    }

    pub fn is_in_default_mode(&self) -> bool {
        self.state.mode == self.default_mode
    }

    #[allow(unused)]
    pub fn delete_char_forward(&mut self) {
        self.state.execute(RemoveChar(1));
    }

    #[allow(unused)]
    pub fn delete_char_backward(&mut self) {
        self.state.execute(DeleteChar(1));
    }

    #[allow(unused)]
    pub fn move_cursor_left(&mut self) {
        self.state.execute(MoveBackward(1));
    }

    #[allow(unused)]
    pub fn move_cursor_right(&mut self) {
        self.state.execute(MoveForward(1));
    }

    pub fn move_cursor_start(&mut self) {
        self.state.execute(MoveToFirstRow())
    }

    #[allow(unused)]
    pub fn move_cursor_line_start(&mut self) {
        self.state.execute(MoveToStartOfLine())
    }

    pub fn move_cursor_line_end(&mut self) {
        self.state.execute(MoveToEndOfLine())
    }

    #[allow(unused)]
    pub fn push_char(&mut self, char: char) {
        self.state.execute(InsertChar(char));
    }

    pub fn push_str(&mut self, line: &str) {
        for line in line.split('\n') {
            self.state.execute(PushLine(line));
        }
    }

    pub fn key_event(&mut self, key: KeyCombination, terminal: Option<&mut Terminal<CrosstermBackend<Stdout>>>) {
        let key_event: KeyEvent = key.into();

        if KEY_BINDINGS.read().generic.text_input.mode == TextAreaMode::Vim {
            if key == key!(esc) {
                self.enter_normal_mode();
                return;
            }
            // These commands must not become application pane/tab shortcuts.
            if self.state.mode == EditorMode::Normal && self.vim_pending.is_empty() && key == key!(shift-X) {
                if self.state.cursor.col > 0 { self.state.execute(DeleteChar(1)); }
                return;
            }
            if self.state.mode == EditorMode::Insert && key == key!(ctrl-w) {
                let row = self.state.cursor.row;
                let end = self.state.cursor.col;
                if let Some(line) = self.state.lines.get(edtui::RowIndex::new(row)) {
                    let mut start = end.min(line.len());
                    while start > 0 && line[start - 1].is_whitespace() { start -= 1; }
                    if start > 0 {
                        let word = line[start - 1].is_alphanumeric() || line[start - 1] == '_';
                        while start > 0 && !line[start - 1].is_whitespace()
                            && (line[start - 1].is_alphanumeric() || line[start - 1] == '_') == word { start -= 1; }
                    }
                    self.state.execute(DeleteChar(end.saturating_sub(start)));
                }
                return;
            }
            if self.handle_inner_object(key_event) { return; }
        }

        match key_event.code {
            KeyCode::Char(_) | KeyCode::Enter |
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Delete |
            KeyCode::Tab | KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down |
            KeyCode::Home | KeyCode::End => {}
            _ => return,
        };

        self.event_handler.as_mut().unwrap().on_key_event(key_event, &mut self.state);

        // edtui keeps its operator lookup private. Mirror only its incomplete
        // prefixes so application shortcuts cannot steal ci[ / ci] / yi[ etc.
        if matches!(self.state.mode, EditorMode::Normal | EditorMode::Visual)
            && !key_event.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            && let KeyCode::Char(ch) = key_event.code {
            self.vim_pending.push(ch);
            if !["c", "ci", "d", "di", "y", "yi", "g", "i"].contains(&self.vim_pending.as_str()) {
                self.vim_pending.clear();
            }
        } else { self.vim_pending.clear(); }

        if let Some(terminal) = terminal && system_editor::is_pending(&self.state) {
            system_editor::open(&mut self.state, terminal).ok();
        }
    }

    pub fn paste(&mut self, text: String) {
        if self.state.mode == EditorMode::Search {
            for ch in text.chars().map(|ch| if ch.is_control() { ' ' } else { ch }) {
                self.state.execute(edtui::actions::search::AppendCharToSearch(ch));
            }
            return;
        }
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let text = if self.is_single_line { text.replace('\n', " ") } else { text };
        self.update_handler();
        self.event_handler.as_ref().unwrap().on_paste_event(text, &mut self.state);
    }

    fn handle_inner_object(&mut self, event: KeyEvent) -> bool {
        let operator = match (self.state.mode, self.vim_pending.as_str()) {
            (EditorMode::Normal, "ci") => 'c',
            (EditorMode::Normal, "di") => 'd',
            (EditorMode::Normal, "yi") => 'y',
            (EditorMode::Visual, "i") => 'v',
            _ => return false,
        };
        if event.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) { return false; }
        let (open, close) = match event.code {
            KeyCode::Char('(' | ')') => ('(', ')'),
            KeyCode::Char('[' | ']') => ('[', ']'),
            KeyCode::Char('{' | '}') => ('{', '}'),
            KeyCode::Char('"') => ('"', '"'),
            KeyCode::Char('\'') => ('\'', '\''),
            _ => return false,
        };
        self.update_handler();
        // edtui's delimiter action includes an opening delimiter under the
        // cursor and leaves a change in Visual mode. Resolve the actual inner
        // range here, including empty/nested pairs and multiline JSON bodies.
        let mut chars = Vec::new();
        let mut positions = Vec::new();
        let lines = self.state.lines.to_vecs();
        for (row, line) in lines.iter().enumerate() {
            for (col, ch) in line.iter().enumerate() {
                chars.push(*ch);
                positions.push(edtui::Index2::new(row, col));
            }
            if row + 1 < lines.len() {
                chars.push('\n');
                positions.push(edtui::Index2::new(row, line.len()));
            }
        }
        let Some(cursor) = positions.iter().position(|pos| *pos == self.state.cursor) else { return true; };
        let mut stack = Vec::new();
        let mut pairs = Vec::new();
        let mut quote = None;
        let mut escaped = false;
        for (index, &ch) in chars.iter().enumerate() {
            if escaped { escaped = false; continue; }
            if ch == '\\' { escaped = true; continue; }
            if open == close {
                // Quoted objects stay on the cursor's line, like Vim's i"/i'.
                if positions[index].row != self.state.cursor.row { continue; }
                if ch == open {
                    if let Some(start) = stack.pop() { pairs.push((start, index)); }
                    else { stack.push(index); }
                }
            } else if let Some(delimiter) = quote {
                if ch == delimiter { quote = None; }
            } else if ch == '"' || ch == '\'' { quote = Some(ch); }
            else if ch == open { stack.push(index); }
            else if ch == close && let Some(start) = stack.pop() { pairs.push((start, index)); }
        }
        let range = pairs.iter().copied().filter(|&(start, end)| start <= cursor && cursor <= end).max_by_key(|&(start, _)| start)
            .or_else(|| if open == close { pairs.iter().copied().filter(|&(start, _)| start > cursor).min_by_key(|&(start, _)| start) } else { None });
        let Some((start, end)) = range else { return true; };
        if operator == 'c' { self.state.execute(SwitchMode(EditorMode::Insert)); }
        let empty = end == start + 1;
        if empty { self.state.selection = None; }
        else {
            let mode = self.state.mode;
            self.state.execute(SwitchMode(EditorMode::Visual));
            let selection = self.state.selection.as_mut().unwrap();
            selection.start = positions[start + 1];
            selection.end = positions[end - 1];
            self.state.mode = mode;
        }
        match operator {
            'c' => {
                self.state.execute(edtui::actions::ChangeSelection);
                self.state.cursor = positions[start + 1];
            }
            'd' => self.state.execute(edtui::actions::DeleteSelection),
            'y' => self.state.execute(CopySelection),
            'v' if !empty => { self.state.cursor = positions[end - 1]; }
            _ => {}
        }
        true
    }
}

fn generate_key_handler(is_single_line: bool, insert_mode_only: bool) -> KeyEventHandler {
    let text_input = KEY_BINDINGS.read().generic.text_input;
    match text_input.mode {
        TextAreaMode::Vim => {
            let mut handler = KeyEventHandler::vim_mode();
            // edtui's Vim preset omits these common change-to-end/whole-line commands.
            let change_to_end = Composed::new(SwitchMode(EditorMode::Insert))
                .chain(edtui::actions::delete::DeleteToEndOfLine)
                .chain(MoveToEndOfLine());
            let change_line = Composed::new(MoveToStartOfLine()).chain(change_to_end.clone());
            handler.insert(KeyEventRegister::n(vec![KeyInput::shift('C')]), change_to_end.clone());
            handler.insert(KeyEventRegister::n(vec![KeyInput::new('c'), KeyInput::new('$')]), change_to_end);
            handler.insert(KeyEventRegister::n(vec![KeyInput::new('c'), KeyInput::new('c')]), change_line.clone());
            handler.insert(KeyEventRegister::n(vec![KeyInput::shift('S')]), change_line);
            handler.insert(KeyEventRegister::n(vec![KeyInput::new('d'), KeyInput::new('$')]), edtui::actions::delete::DeleteToEndOfLine);
            handler.insert(KeyEventRegister::n(vec![KeyInput::new('^')]), edtui::actions::MoveToFirst());
            handler.insert(KeyEventRegister::v(vec![KeyInput::new('^')]), edtui::actions::MoveToFirst());
            handler.insert(KeyEventRegister::n(vec![KeyInput::new('s')]),
                Composed::new(SwitchMode(EditorMode::Insert)).chain(edtui::actions::delete::DeleteCharForward(1)));

            // Delimited objects are handled above, including nested pairs.
            for operator in ['d', 'y'] {
                let action = Composed::new(SwitchMode(EditorMode::Normal)).chain(edtui::actions::SelectInnerWord);
                let action = if operator == 'd' { action.chain(edtui::actions::DeleteSelection) }
                    else { action.chain(CopySelection) };
                handler.insert(KeyEventRegister::n(vec![KeyInput::new(operator), KeyInput::new('i'), KeyInput::new('w')]), action.chain(SwitchMode(EditorMode::Normal)));
            }

            if is_single_line {
                // Remove new line
                handler.remove(&KeyEventRegister::i(vec![KeyInput::new(KeyCode::Enter)]));
                handler.remove(&KeyEventRegister::n(vec![KeyInput::new(KeyCode::Char('o'))]));
                handler.remove(&KeyEventRegister::n(vec![KeyInput::shift('O')]));
                handler.remove(&KeyEventRegister::n(vec![KeyInput::shift('J')]));

                // External system editor
                handler.remove(&KeyEventRegister::n(vec![KeyInput::ctrl(KeyCode::Char('e'))]));
            }

            if insert_mode_only {
                handler.remove(&KeyEventRegister::i(vec![KeyInput::new(KeyCode::Esc)]));
            }

            handler
        },
        TextAreaMode::Emacs => {
            let mut handler = KeyEventHandler::emacs_mode();

            if is_single_line {
                // Remove new line
                handler.remove(&KeyEventRegister::i(vec![KeyInput::ctrl(KeyCode::Char('o'))]));
                handler.remove(&KeyEventRegister::i(vec![KeyInput::new(KeyCode::Enter)]));
                handler.remove(&KeyEventRegister::i(vec![KeyInput::ctrl(KeyCode::Char('j'))]));

                // External system editor
                handler.remove(&KeyEventRegister::i(vec![KeyInput::alt(KeyCode::Char('e'))]))
            }

            if insert_mode_only {
                handler.remove(&KeyEventRegister::i(vec![KeyInput::ctrl('s')]));
            }

            handler
        },
        _ => {
            let custom_text_input = match text_input.mode {
                TextAreaMode::Default => CustomTextArea::default(),
                TextAreaMode::Custom(custom_text_input) => custom_text_input,
                _ => unreachable!()
            };

            let copy: KeyEvent = custom_text_input.copy.into();
            let paste: KeyEvent = custom_text_input.paste.into();

            let undo: KeyEvent = custom_text_input.undo.into();
            let redo: KeyEvent = custom_text_input.redo.into();

            let system_editor: KeyEvent = custom_text_input.system_editor.into();

            let search: KeyEvent = custom_text_input.search.into();
            let quit_without_saving: KeyEvent = text_input.quit_without_saving.into();

            let new_line: KeyEvent = custom_text_input.new_line.into();

            let delete_backward: KeyEvent = custom_text_input.delete_backward.into();
            let delete_forward: KeyEvent = custom_text_input.delete_forward.into();

            let skip_word_left: KeyEvent = custom_text_input.skip_word_left.into();
            let skip_word_right: KeyEvent = custom_text_input.skip_word_right.into();

            let move_cursor_right: KeyEvent = custom_text_input.move_cursor_right.into();
            let move_cursor_left: KeyEvent = custom_text_input.move_cursor_left.into();
            let move_cursor_up: KeyEvent = custom_text_input.move_cursor_up.into();
            let move_cursor_down: KeyEvent = custom_text_input.move_cursor_down.into();
            let move_cursor_line_start: KeyEvent = custom_text_input.move_cursor_line_start.into();
            let move_cursor_line_end: KeyEvent = custom_text_input.move_cursor_line_end.into();

            let mut keys = HashMap::from([
                (KeyEventRegister::s(vec![KeyInput::from(delete_forward)]), RemoveCharFromSearch.into()),
                // INSERT
                (KeyEventRegister::i(vec![KeyInput::from(move_cursor_right)]), MoveForward(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(move_cursor_left)]), MoveBackward(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(move_cursor_up)]), MoveUp(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(move_cursor_down)]), MoveDown(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(skip_word_right)]), MoveWordForward(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(skip_word_left)]), MoveWordBackward(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(move_cursor_line_start)]), MoveToStartOfLine().into()),
                (KeyEventRegister::i(vec![KeyInput::from(move_cursor_line_end)]), MoveToEndOfLine().into()),
                (KeyEventRegister::i(vec![KeyInput::from(delete_forward)]), DeleteChar(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(delete_backward)]), RemoveChar(1).into()),
                (KeyEventRegister::i(vec![KeyInput::from(undo)]), Undo.into()),
                (KeyEventRegister::i(vec![KeyInput::from(redo)]), Redo.into()),
                (KeyEventRegister::i(vec![KeyInput::from(copy)]), CopySelection.into()),
                (KeyEventRegister::i(vec![KeyInput::from(paste)]), Paste.into()),
                (KeyEventRegister::i(vec![KeyInput::from(system_editor)]), OpenSystemEditor.into())
            ]);

            if !is_single_line {
                keys.insert(KeyEventRegister::i(vec![KeyInput::from(new_line)]), LineBreak(1).into());
            }

            if !insert_mode_only {
                keys.extend([
                    // SEARCH
                    (
                        KeyEventRegister::i(vec![KeyInput::from(search)]),
                        Composed::new(StartSearch)
                            .chain(SwitchMode(EditorMode::Search))
                            .into()
                    ),
                    (KeyEventRegister::s(vec![KeyInput::from(move_cursor_down)]), FindNext.into()),
                    (KeyEventRegister::s(vec![KeyInput::from(move_cursor_up)]), FindPrevious.into()),
                    (
                        KeyEventRegister::s(vec![KeyInput::from(new_line)]),
                        Composed::new(SelectCurrentSearch)
                            .chain(SwitchMode(EditorMode::Insert))
                            .into()
                    ),
                    (
                        KeyEventRegister::s(vec![KeyInput::from(search)]),
                        Composed::new(StopSearch)
                            .chain(SwitchMode(EditorMode::Insert))
                            .into()
                    ),
                    (
                        KeyEventRegister::s(vec![KeyInput::from(quit_without_saving)]),
                        Composed::new(StopSearch)
                            .chain(SwitchMode(EditorMode::Insert))
                            .into()
                    ),
                ]);
            }

            KeyEventHandler::new(keys, false)
        }
    }
}
