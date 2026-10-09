use std::env;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::PathBuf;
use lazy_static::lazy_static;
use nestify::nest;
use parking_lot::RwLock;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders};
use tracing::{trace, warn};
use serde::{Deserialize, Serialize};

use crate::app::app::App;
use crate::app::files::utils::expand_tilde;
use crate::panic_error;

/// LazyDB's Deep Space palette, shared by panes, tabs, dialogs and editors.
/// Optional in existing theme files so older custom themes still load.
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub background: Color,
    pub surface: Color,
    pub surface_raised: Color,
    pub border: Color,
    pub grid_header: Color,
    pub grid_header_text: Color,
    pub grid_border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub action: Color,
    pub selection: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: Color::Rgb(7, 11, 18),
            surface: Color::Rgb(12, 19, 30),
            surface_raised: Color::Rgb(17, 28, 43),
            border: Color::Rgb(43, 66, 86),
            grid_header: Color::Rgb(15, 34, 44),
            grid_header_text: Color::Rgb(184, 235, 229),
            grid_border: Color::Rgb(58, 72, 84),
            text: Color::Rgb(215, 226, 237),
            muted: Color::Rgb(105, 126, 146),
            accent: Color::Rgb(99, 230, 216),
            action: Color::Rgb(101, 167, 255),
            selection: Color::Rgb(26, 55, 70),
            success: Color::Rgb(92, 200, 150),
            warning: Color::Rgb(244, 184, 96),
            error: Color::Rgb(255, 107, 122),
        }
    }
}

nest! {
    #[derive(Serialize, Deserialize)]
    pub struct Theme {
        #[serde(default)]
        pub palette: Palette,
        #[serde(alias = "UI")]
        pub ui:
            #[derive(Serialize, Deserialize)]
            pub struct ThemeUI {
                pub font_color: Color,
                pub app_background: Option<Color>,

                pub main_foreground_color: Color,
                pub secondary_foreground_color: Color,

                pub main_background_color: Color,
                pub secondary_background_color: Color
            },

        #[serde(alias = "Others")]
        pub others:
            #[derive(Serialize, Deserialize)]
            pub struct ThemeOthers {
                pub selection_highlight_color: Color,
                pub environment_variable_highlight_color: Color,
            },

        #[serde(alias = "HTTP")]
        pub http: #[derive(Serialize, Deserialize)]
            pub struct ThemeHttp {
            #[serde(alias = "Methods")]
            pub methods:
                #[derive(Serialize, Deserialize)]
                pub struct ThemeMethods {
                    #[serde(alias = "GET")]
                    pub get: Color,
                    #[serde(alias = "POST")]
                    pub post: Color,
                    #[serde(alias = "PUT")]
                    pub put: Color,
                    #[serde(alias = "PATCH")]
                    pub patch: Color,
                    #[serde(alias = "DELETE")]
                    pub delete: Color,
                    #[serde(alias = "HEAD")]
                    pub head: Color,
                    #[serde(alias = "OPTIONS")]
                    pub options: Color,
                    #[serde(alias = "TRACE")]
                    pub trace: Color,
                    #[serde(alias = "CONNECT")]
                    pub connect: Color
                },
        },

        #[serde(alias = "Websocket")]
        pub websocket:
            #[derive(Serialize, Deserialize)]
            pub struct ThemeWebsocket {
                #[serde(alias = "Connection Statuses")]
                pub connection_status:
                    #[derive(Serialize, Deserialize)]
                    pub struct ThemeConnectionStatuses {
                        #[serde(alias = "Connected")]
                        pub connected: Color,
                        #[serde(alias = "Disconnected")]
                        pub disconnected: Color,
                    },

                #[serde(alias = "Messages")]
                pub messages:
                    #[derive(Serialize, Deserialize)]
                    pub struct ThemeMessages {
                        #[serde(alias = "server_foreground_color")]
                        pub server_foreground_color: Color,
                        #[serde(alias = "server_background_color")]
                        pub server_background_color: Color,
                        #[serde(alias = "you_background_color")]
                        pub you_background_color: Color,
                        #[serde(alias = "details_color")]
                        pub details_color: Color,
                    }
            }
    }
}

impl Default for Theme {
    fn default() -> Self {
        let palette = Palette::default();
        Theme {
            palette,
            ui: ThemeUI {
                font_color: palette.text,
                app_background: Some(palette.background),

                main_foreground_color: palette.border,
                secondary_foreground_color: palette.muted,

                main_background_color: palette.surface,
                secondary_background_color: palette.surface_raised,
            },
            others: ThemeOthers {
                selection_highlight_color: palette.accent,
                environment_variable_highlight_color: palette.accent,
            },
            http: ThemeHttp {
                methods: ThemeMethods {
                    get: palette.success,
                    post: palette.warning,
                    put: palette.action,
                    patch: palette.accent,
                    delete: palette.error,
                    options: palette.action,
                    head: palette.success,
                    trace: palette.warning,
                    connect: palette.action
                },
            },
            websocket: ThemeWebsocket {
                connection_status: ThemeConnectionStatuses {
                    connected: palette.success,
                    disconnected: palette.error
                },

                messages: ThemeMessages {
                    server_foreground_color: palette.text,
                    server_background_color: palette.surface_raised,
                    you_background_color: palette.selection,
                    details_color: palette.muted
                }
            }
        }
    }
}

impl Theme {
    pub fn base(&self) -> Style {
        Style::new().fg(self.ui.font_color).bg(self.ui.app_background.unwrap_or(Color::Reset))
    }

    pub fn panel<'a>(&self, title: impl Into<Line<'a>>, focused: bool) -> Block<'a> {
        Block::new().title(title).borders(Borders::ALL).border_type(BorderType::Rounded)
            .style(Style::new().fg(self.ui.font_color).bg(self.ui.main_background_color))
            .title_style(Style::new().fg(if focused { self.others.selection_highlight_color } else { self.ui.secondary_foreground_color }).bold())
            .border_style(Style::new().fg(if focused { self.others.selection_highlight_color } else { self.ui.main_foreground_color }))
    }

    pub fn selected(&self) -> Style {
        Style::new().fg(self.ui.font_color).bg(self.palette.selection).add_modifier(Modifier::BOLD)
    }

    pub fn active_cell(&self) -> Style {
        Style::new().fg(self.palette.background).bg(self.others.selection_highlight_color).add_modifier(Modifier::BOLD)
    }
}

lazy_static! {
    pub static ref THEME: RwLock<Theme> = RwLock::new(Theme::default());
}

impl App<'_> {
    pub fn parse_theme_file(&mut self) {
        let path = match env::var("LAZYAPI_THEME") {
            // If the LAZYAPI_THEME environment variable exists
            Ok(env_theme) => expand_tilde(PathBuf::from(env_theme)),
            Err(_) => {
                warn!("No theme file found, using default");
                return;
            }
        };

        trace!("Parsing theme file \"{}\"", path.display());

        let mut theme_file = match OpenOptions::new().read(true).open(path) {
            Ok(theme_file) => theme_file,
            Err(e) => panic_error(format!("Could not open theme file\n\t{e}"))
        };

        let mut file_content = String::new();
        theme_file.read_to_string(&mut file_content).expect("\tCould not read key bindings file");

        let theme: Theme = match toml::from_str(&file_content) {
            Ok(theme) => theme,
            Err(e) => panic_error(format!("Could not parse theme file\n\t{e}"))
        };

        *THEME.write() = theme;

        trace!("Theme file parsed!");
    }
}
