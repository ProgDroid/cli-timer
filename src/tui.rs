use crate::app::{App, Result};
use crate::event::Handler;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use std::io;
use tui::Terminal;
use tui::backend::Backend;

#[allow(clippy::partial_pub_fields)]
pub struct Tui<B: Backend> {
    terminal: Terminal<B>,
    pub events: Handler,
}

impl<B: Backend> Tui<B> {
    pub const fn new(terminal: Terminal<B>, events: Handler) -> Self {
        Self { terminal, events }
    }

    /// Puts the terminal in raw mode on the alternate screen.
    ///
    /// # Errors
    /// Returns an error if the terminal cannot be configured.
    pub fn init(&mut self) -> Result<()> {
        terminal::enable_raw_mode()?;
        crossterm::execute!(io::stderr(), EnterAlternateScreen, EnableMouseCapture)?;
        self.terminal.hide_cursor()?;
        self.terminal.clear()?;
        Ok(())
    }

    /// Draws one frame of the app.
    ///
    /// # Errors
    /// Returns an error if writing to the terminal fails.
    pub fn draw(&mut self, app: &mut App) -> Result<()> {
        self.terminal.draw(|frame| app.render(frame))?;
        Ok(())
    }

    /// Restores the terminal to its normal state.
    ///
    /// # Errors
    /// Returns an error if the terminal cannot be restored.
    pub fn exit(&mut self) -> Result<()> {
        terminal::disable_raw_mode()?;
        crossterm::execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}
