use crate::app::files::theme::THEME;
use crate::app::files::key_bindings::KEY_BINDINGS;
use crate::models::request::KeyValue;
use crate::tui::utils::stateful::text_input::{SingleLineTextInput, TextInput};
use ratatui::buffer::Buffer;
use ratatui::layout::Direction::{Horizontal, Vertical};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Line, StatefulWidget, Style, Stylize, Widget};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};


pub struct StatefulCustomTable<'a> {
    pub left_state: ListState,
    pub right_state: ListState,
    /// (x, y)
    pub selection: Option<(usize, usize)>,
    pub rows: Vec<KeyValue>,
    pub selection_text_input: TextInput,

    pub is_editing: bool,
    pub empty_rows_lines: Vec<Line<'a>>,
    pub default_key: &'static str,
    pub default_value: &'static str,
}

impl<'a> StatefulCustomTable<'a> {
    pub fn new(empty_rows_lines: Vec<Line<'a>>, default_key: &'static str, default_value: &'static str) -> Self {
        Self {
            left_state: ListState::default(),
            right_state: ListState::default(),
            selection: None,
            rows: vec![],
            selection_text_input: TextInput::new(None),
            is_editing: false,
            empty_rows_lines,
            default_key,
            default_value,
        }
    }

    pub fn clamp_selection(&mut self) {
        let selection = if self.rows.is_empty() { None } else {
            let (row, col) = self.selection.unwrap_or((0, 0));
            Some((row.min(self.rows.len() - 1), col.min(1)))
        };
        self.update_selection(selection);
    }

    pub fn select_column(&mut self, column: usize) {
        if let Some((row, _)) = self.selection { self.update_selection(Some((row, column))); }
    }

    pub fn update_selection(&mut self, selection: Option<(usize, usize)>) {
        match selection {
            None => {
                self.selection = None;
                self.left_state.select(None);
                self.right_state.select(None);
            }
            Some(selection) => {
                self.selection = Some(selection);
                self.left_state.select(Some(selection.0));
                self.right_state.select(Some(selection.0));
            }
        }
    }
    
    fn decrement_x(&self, i: usize) -> usize { i.saturating_sub(1) }

    fn increment_x(&self, i: usize) -> usize { i.saturating_add(1).min(self.rows.len().saturating_sub(1)) }

    pub fn change_y(&mut self) {
        if self.rows.is_empty() || self.selection.is_none() {
            return;
        }

        match self.selection.unwrap() {
            (x, 0) => self.selection = Some((x, 1)),
            (x, 1) => self.selection = Some((x, 0)),
            (x, _) => self.selection = Some((x, 0))
        }

        let x = self.selection.unwrap().0;

        self.right_state.select(Some(x));
        self.left_state.select(Some(x));
    }

    pub fn up(&mut self) {
        if self.rows.is_empty() || self.selection.is_none() {
            return;
        }

        let x = match self.selection.unwrap() {
            (_, 0) => match self.left_state.selected() {
                None => 0,
                Some(i) => self.decrement_x(i)
            },
            (_, 1) => match self.right_state.selected() {
                None => 0,
                Some(i) => self.decrement_x(i)
            },
            (_, _) => 0
        };

        self.left_state.select(Some(x));
        self.right_state.select(Some(x));

        match self.selection.unwrap() {
            (_, y) => self.selection = Some((x, y))
        }
    }

    pub fn down(&mut self) {
        if self.rows.is_empty() || self.selection.is_none() {
            return;
        }

        let x = match self.selection.unwrap() {
            (_, 0) => match self.left_state.selected() {
                None => 0,
                Some(i) => self.increment_x(i)
            },
            (_, 1) => match self.right_state.selected() {
                None => 0,
                Some(i) => self.increment_x(i)
            },
            (_, _) => 0
        };

        self.left_state.select(Some(x));
        self.right_state.select(Some(x));

        match self.selection.unwrap() {
            (_, y) => self.selection = Some((x, y))
        }
    }

    pub fn is_selected(&self) -> bool {
        return self.selection.is_some();
    }
}

impl<'a> StatefulWidget for &'a mut StatefulCustomTable<'_> {
    type State = (Vec<ListItem<'a>>, Vec<ListItem<'a>>);

    fn render(self, area: Rect, buf: &mut Buffer, rows: &mut Self::State) where Self: Sized {
        match self.selection {
            None => {
                let mut lines = self.empty_rows_lines.clone();
                lines.push(Line::from(format!("(Add one with {})", KEY_BINDINGS.read().generic.list_and_table_actions.create_element))
                    .fg(ratatui::style::Color::White));
                let headers_paragraph = Paragraph::new(lines).centered();

                headers_paragraph.render(area, buf);
            },
            Some(selection) => {
                let layout = Layout::new(
                    Vertical,
                    [
                        Constraint::Length(2),
                        Constraint::Fill(1)
                    ]
                )
                    .split(area);

                let inner_layout = Layout::new(
                    Horizontal,
                    [
                        Constraint::Percentage(50),
                        Constraint::Percentage(50)
                    ]
                )
                    .split(layout[0]);

                let title = Paragraph::new(self.default_key)
                    .centered()
                    .block(
                        Block::new()
                            .borders(Borders::BOTTOM | Borders::RIGHT)
                            .border_style(Style::new().fg(THEME.read().palette.grid_border))
                    )
                    .fg(THEME.read().palette.grid_header_text).bg(THEME.read().palette.grid_header).bold();

                let form_value = Paragraph::new(self.default_value)
                    .centered()
                    .block(
                        Block::new()
                            .borders(Borders::BOTTOM)
                            .border_style(Style::new().fg(THEME.read().palette.grid_border))
                    )
                    .fg(THEME.read().palette.grid_header_text).bg(THEME.read().palette.grid_header).bold();

                title.render(inner_layout[0], buf);
                form_value.render(inner_layout[1], buf);

                let horizontal_margin = 2;

                let table_layout = Layout::new(
                    Horizontal,
                    [
                        Constraint::Percentage(50),
                        Constraint::Percentage(50)
                    ]
                )
                    .horizontal_margin(horizontal_margin)
                    .split(layout[1]);

                let left_list_style = THEME.read().selected();
                let right_list_style = THEME.read().selected();

                let left_list = List::new(rows.0.to_owned())
                    .highlight_style(left_list_style)
                    .fg(THEME.read().ui.font_color);

                let right_list = List::new(rows.1.to_owned())
                    .highlight_style(right_list_style)
                    .fg(THEME.read().ui.font_color);

                StatefulWidget::render(left_list, table_layout[0], buf, &mut self.left_state);
                StatefulWidget::render(right_list, table_layout[1], buf, &mut self.right_state);

                // Form input & cursor

                let cell_with = layout[1].width / 2;

                let width_adjustment = match selection.1 {
                    0 => 0,
                    1 => {
                        let even_odd_adjustment = match layout[1].width % 2 {
                            1 => 1,
                            0 => 2,
                            _ => 0
                        };

                        cell_with.saturating_sub(even_odd_adjustment)
                    },
                    _ => 0
                };

                if layout[1].height == 0 { return; }
                let height_adjustment = selection.0.saturating_sub(self.left_state.offset()) as u16 % layout[1].height;

                let selection_position_x = layout[1].x + width_adjustment + horizontal_margin;
                let selection_position_y = layout[1].y + height_adjustment;

                let text_rect = Rect::new(selection_position_x, selection_position_y, cell_with.saturating_sub(horizontal_margin), 1);

                if !self.is_editing {
                    buf.set_style(text_rect.intersection(area), THEME.read().active_cell());
                }
                if self.is_editing {
                    self.selection_text_input.display_cursor = true;
                    self.selection_text_input.highlight_text = true;
                    " ".repeat(text_rect.width as usize).render(text_rect, buf);
                    SingleLineTextInput(&mut self.selection_text_input).render(text_rect, buf);
                } else {
                    self.selection_text_input.display_cursor = false;
                    self.selection_text_input.highlight_text = false;
                }
            }
        }
    }
}
