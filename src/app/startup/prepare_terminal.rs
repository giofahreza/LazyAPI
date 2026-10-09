use std::io::stdout;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::ExecutableCommand;
use ratatui::crossterm::event::EnableBracketedPaste;
use ratatui::crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use tracing::trace;
use crate::app::app::App;

impl App<'_> {
    pub fn prepare_terminal(&mut self) -> &mut Self {
        trace!("Preparing terminal...");

        // The full-screen UI needs its explicit white foreground and cursor
        // contrast even when the launching shell disables CLI colors.
        ratatui::crossterm::style::force_color_output(true);

        enable_raw_mode().unwrap();
        stdout().execute(EnterAlternateScreen).unwrap();
        stdout().execute(EnableBracketedPaste).unwrap();
        // Start with a thin bar so the very first frame already looks like a
        // LazyAPI Normal mode prompt. The main render keeps re-issuing the
        // shape, but setting it here avoids a one-frame flash of the user's
        // default terminal cursor.
        let _ = stdout().execute(SetCursorStyle::BlinkingBar);

        trace!("Terminal OK");

        self
    }
}
