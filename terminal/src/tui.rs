use std::io::stdout;
use std::rc::Rc;
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
pub mod layout;
pub mod store;
pub mod template;
pub mod theme;
pub mod window;

use events::{Events, InputEvent};
use store::{Store, Value};
use theme::Theme;
use window::{ApplicationWindow, ShortcutWidget};

pub const TICK_RATE: u64 = 200;

/// Update callback that must be passed to the application.
pub type Update = dyn Fn(&mut Store, &InputEvent) -> Result<(), Error>;

/// Internal execution state of a tui-application. Setting the state
/// property `app.state` to `State::Exiting` will exit the
/// application.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum State {
    Running,
    Exiting,
}

/// Basic, multi-threaded tui-application with default initialized store.
/// When creating an application, an update callback needs to be passed.
/// This will be called for every input event received from event thread.
///
/// An example application can be found in `examples/tui.rs`.
///
pub struct Application<'a> {
    store: Store,
    update: &'a Update,
}

impl<'a> Application<'a> {
    /// Returns a default tui-application that can be quited.
    pub fn new(update: &'a Update) -> Self {
        let application = Self {
            store: Store::default(),
            update,
        };
        application.store(vec![("app.state", Box::new(State::Running))])
    }

    /// Initializes backend, enters alternative screen, runs application and restores
    /// terminal after application exited.
    pub fn execute(&mut self, theme: &Theme) -> Result<(), Error> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run(&mut terminal, theme);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Starts the render loop, the event thread and re-draws an application window.
    /// Leave render loop if property `app.state` signals exit.
    fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>, theme: &Theme) -> Result<(), Error> {
        let window = ApplicationWindow {
            shortcuts: Rc::new(ShortcutWidget),
        };
        let events = Events::new(Duration::from_millis(TICK_RATE));
        loop {
            let mut error: Option<Error> = None;
            terminal.draw(|frame| {
                error = window.draw(&self.store, frame, theme).err();
            })?;
            if let Some(err) = error {
                return Err(err.into());
            }

            self.on_event(events.next()?)?;

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

    /// Call update function that needs to be passed when creating
    /// the tui-application. It may update the internal store based on
    /// the input event type.
    pub fn on_event(&mut self, event: InputEvent) -> Result<(), Error> {
        (self.update)(&mut self.store, &event)?;
        Ok(())
    }
}
