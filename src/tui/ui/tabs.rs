use crate::app::files::theme::THEME;
use ratatui::{Frame, layout::Rect, style::{Style, Stylize}, text::{Line, Span}, widgets::Paragraph};

pub(super) fn text_width(text: &str) -> usize { Line::from(text).width() }

pub(super) fn clip_text(text: &str, width: usize) -> String {
    let text: String = text.chars().map(|ch| if ch.is_control() { ' ' } else { ch }).collect();
    if text_width(&text) <= width { return text; }
    if width == 0 { return String::new(); }
    let mut clipped = String::new();
    for ch in text.chars() {
        let mut next = clipped.clone();
        next.push(ch);
        if text_width(&next) + 1 > width { break; }
        clipped = next;
    }
    clipped.push('…');
    clipped
}

/// Natural-width tabs with an active tab that stays visible on narrow screens.
/// Workspace tabs use filled labels; section tabs use LazyDB's accent underline.
pub(super) fn render_tabs(frame: &mut Frame, area: Rect, labels: &[String], active: usize, workspace: bool) {
    if area.is_empty() || labels.is_empty() { return; }
    let theme = THEME.read();
    frame.render_widget(Paragraph::new("").style(theme.base()), area);
    let active = active.min(labels.len() - 1);
    let widths: Vec<usize> = labels.iter().map(|label| text_width(label) + 2).collect();
    let overflow = widths.iter().sum::<usize>() > area.width as usize;
    let available = (area.width as usize).saturating_sub(if overflow { 2 } else { 0 });
    if available == 0 { return; }
    let (mut start, mut end) = (0, labels.len());
    if overflow {
        start = active;
        end = active + 1;
        let mut used = widths[active].min(available);
        while end < widths.len() && used + widths[end] <= available { used += widths[end]; end += 1; }
        while start > 0 && used + widths[start - 1] <= available { start -= 1; used += widths[start]; }
    }
    let x = area.x + u16::from(overflow);
    let mut offset = 0;
    let mut spans = Vec::new();
    for index in start..end {
        let width = widths[index].min(available.saturating_sub(offset));
        let label = clip_text(&format!(" {} ", labels[index]), width);
        let style = if workspace {
            if index == active { theme.active_cell() }
            else { Style::new().fg(theme.ui.secondary_foreground_color).bg(theme.ui.main_background_color) }
        } else {
            Style::new().fg(if index == active { theme.others.selection_highlight_color } else { theme.ui.secondary_foreground_color }).bold()
        };
        spans.push(Span::styled(label, style));
        if index == active && area.height > 1 && !workspace {
            frame.render_widget(Paragraph::new("━".repeat(width)).fg(theme.others.selection_highlight_color),
                Rect::new(x + offset as u16, area.y + 1, width as u16, 1));
        }
        offset += width;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.base()), Rect::new(x, area.y, available as u16, 1));
    if overflow {
        for (x, glyph, enabled) in [(area.x, "‹", start > 0), (area.right() - 1, "›", end < labels.len())] {
            frame.render_widget(Paragraph::new(glyph).fg(if enabled { theme.palette.action } else { theme.palette.border }), Rect::new(x, area.y, 1, 1));
        }
    }
}
