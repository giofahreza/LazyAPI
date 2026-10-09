use std::sync::Arc;
use lazy_static::lazy_static;
use ratatui::prelude::Color;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeItem, StyleModifier};
use syntect::parsing::{SyntaxReference, SyntaxSet};

#[derive(Default)]
pub struct SyntaxHighlighting {
    pub highlighted_body: Option<Vec<Line<'static>>>,
    pub highlighted_console_output: Vec<Line<'static>>,
}

lazy_static! {
    pub static ref SYNTAX_SET: Arc<SyntaxSet> = Arc::new(SyntaxSet::load_defaults_newlines());
    pub static ref JSON_SYNTAX_REF: &'static SyntaxReference = SYNTAX_SET.find_syntax_by_extension("json").unwrap();
    pub static ref XML_SYNTAX_REF: &'static SyntaxReference = SYNTAX_SET.find_syntax_by_extension("xml").unwrap();
    pub static ref HTML_SYNTAX_REF: &'static SyntaxReference = SYNTAX_SET.find_syntax_by_extension("html").unwrap();
    pub static ref JS_SYNTAX_REF: &'static SyntaxReference = SYNTAX_SET.find_syntax_by_extension("js").unwrap();

}

fn syntax_theme() -> Theme {
    let ui = crate::app::files::theme::THEME.read();
    let color = |color: Color| match color {
        Color::Rgb(r, g, b) => syntect::highlighting::Color { r, g, b, a: 255 },
        _ => syntect::highlighting::Color::WHITE,
    };
    let mut theme = Theme::default();
    theme.settings.foreground = Some(color(ui.ui.font_color));
    theme.scopes = [
        ("comment", ui.palette.muted),
        ("keyword, storage, support.type", ui.palette.accent),
        ("string", ui.palette.success),
        ("constant.numeric, constant.language, variable.parameter", ui.palette.action),
        ("entity.name.function, support.function", Color::Rgb(130, 170, 255)),
        ("invalid", ui.palette.error),
    ].into_iter().map(|(scope, foreground)| ThemeItem {
        scope: scope.parse().unwrap(),
        style: StyleModifier { foreground: Some(color(foreground)), ..StyleModifier::default() },
    }).collect();
    theme
}

pub fn highlight(string: &str, extension: &str) -> Option<Vec<Line<'static>>> {
    let syntax = match extension {
        "json" => &*JSON_SYNTAX_REF,
        "xml" => &*XML_SYNTAX_REF,
        "html" => &*HTML_SYNTAX_REF,
        "js" => &*JS_SYNTAX_REF,
        _ => match SYNTAX_SET.find_syntax_by_extension(&extension) {
            None => return None,
            Some(syntax) => syntax
        }
    };

    let theme = syntax_theme();
    let mut highlight = HighlightLines::new(syntax, &theme);

    let mut lines: Vec<Line> = vec![];


    for line in string.lines() {
        let result = highlight.highlight_line(line, &SYNTAX_SET).unwrap();

        let mut highlighted_line: Vec<Span> = vec![];

        for &(ref style, text) in result.iter() {
            highlighted_line.push(Span::raw(text.to_string()).fg(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b)));
        }

        lines.push(Line::from(highlighted_line));
    }

    return Some(lines);
}
