use std::io::stdout;
use std::time::Duration;

use anyhow::{Error, Result};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use tui::backend::{Backend, CrosstermBackend};
use tui::Terminal;

pub mod events;
pub mod store;
pub mod window;

use events::{Events, InputEvent, Key};
use store::{Store, Value};
use window::ApplicationWindow;

pub const TICK_RATE: u64 = 200;

/// Internal execution state of a tui-application. Setting the state
/// property `app.state` to `State::Exiting` will exit the
/// application.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum State {
    Running,
    Exiting,
}

/// Basic tui-application with no state.
///
/// # Example
/// ```
/// let mut application = Application::new();
/// application.execute()?;
/// ```
pub struct Application {
    store: Store,
}

impl Application {
    /// Returns a default tui-application that can be quited.
    pub fn new() -> Self {
        let application = Self {
            store: Store::default(),
        };
        application.store(vec![("app.state", Box::new(State::Running))])
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

    /// Starts the render loop, the event thread and re-draws an application window.
    /// Leave render loop if property `app.state` signals exit.
    fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), Error> {
        let window = ApplicationWindow::new();
        let events = Events::new(Duration::from_millis(TICK_RATE));
        loop {
            terminal.draw(|f| {
                let _ = window.draw(f);
            })?;

            match events.next()? {
                InputEvent::Input(key) => self.on_key(key)?,
                InputEvent::Tick => {}
            }

            let state = self.store.get::<State>("app.state")?;
            if *state == State::Exiting {
                return Ok(());
            }
        }
    }

    /// Add all given properties to internal store and return itself.
    pub fn store(mut self, props: Vec<(&str, Value)>) -> Self {
        for (key, prop) in props {
            self.store.set(key, prop);
        }
        self
    }

    /// Handle input key and update store based on type.
    pub fn on_key(&mut self, key: Key) -> Result<(), Error> {
        if let Key::Char('q') = key {
            self.store.set("app.state", Box::new(State::Exiting));
        }
        Ok(())
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
