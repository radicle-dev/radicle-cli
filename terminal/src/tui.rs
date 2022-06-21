use std::io::stdout;

use anyhow::{Error, Result};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use tui::backend::{Backend, CrosstermBackend};
use tui::Terminal;

pub mod window;

use window::ApplicationWindow;

/// Basic tui-application with no state.
///
/// # Example
/// ```
/// let mut application = Application::new();
/// application.execute()?;
/// ```
pub struct Application;

impl Application {
    /// Returns a default tui-application
    pub fn new() -> Self {
        Self {}
    }

    /// Initializes backend, enters alternative screen, runs application and restores
    /// terminal after application exited.
    pub fn execute(&mut self) -> Result<(), Error> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        self.run(&mut terminal)?;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    // Starts the render loop and re-draws an application window infinitely.
    fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), Error> {
        let window = ApplicationWindow::new();
        loop {
            terminal.draw(|f| {
                let _ = window.draw(f);
            })?;
        }
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
