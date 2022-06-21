use anyhow::{Error, Result};

use tui::backend::Backend;
use tui::style::Style;
use tui::widgets::Block;
use tui::Frame;

/// Basic application window with no widgets.
pub struct ApplicationWindow;

impl ApplicationWindow {
    /// Returns a new application window.
    pub fn new() -> Self {
        Self {}
    }

    /// Draws the application window to given `frame`.
    pub fn draw<B: Backend>(&self, frame: &mut Frame<B>) -> Result<(), Error> {
        let block = Block::default().title("tui").style(Style::default());
        frame.render_widget(block, frame.size());
        Ok(())
    }
}

impl Default for ApplicationWindow {
    fn default() -> Self {
        Self::new()
    }
}
