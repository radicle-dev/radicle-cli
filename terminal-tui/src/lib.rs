use std::hash::Hash;
use std::time::Duration;

use anyhow::{Error, Result};

use tuirealm::terminal::TerminalBridge;

use tuirealm::application::PollStrategy;
use tuirealm::props::{AttrValue, Attribute};
use tuirealm::tui::layout::Rect;
use tuirealm::{Application, EventListenerCfg, NoUserEvent};
use tuirealm::{Component, Frame};

pub mod components;
pub mod state;
pub mod layout;
pub mod theme;

/// A proxy that abstracts the tui-realm-specific application.
pub struct App<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    backend: Application<Id, Message, NoUserEvent>,
}

impl<Id, Message> Default for App<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Id, Message> App<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    pub fn new() -> Self {
        let backend = Application::init(
            EventListenerCfg::default().default_input_listener(Duration::from_millis(10)),
        );
        Self { backend }
    }

    pub fn mount<C>(&mut self, id: Id, component: C) -> Result<(), Error>
    where
        C: Component<Message, NoUserEvent> + 'static,
    {
        self.backend.mount(id, Box::new(component), vec![])?;
        Ok(())
    }

    pub fn activate(&mut self, id: Id) -> Result<(), Error> {
        self.backend.active(&id)?;
        Ok(())
    }

    pub fn query(&self, id: Id, attr: Attribute) -> Option<AttrValue> {
        self.backend.query(&id, attr).ok().flatten()
    }

    pub fn view(&mut self, id: Id, frame: &mut Frame, area: Rect) {
        self.backend.view(&id, frame, area);
    }

    pub fn poll(&mut self) -> Vec<Message> {
        match self.backend.tick(PollStrategy::Once) {
            Ok(messages) => messages,
            _ => vec![],
        }
    }
}

/// Trait that must be implemented by client applications in order to be run
/// as tui-application using tui-realm. Implementors act as models to the
/// tui-realm application that can be polled for new messages, updated
/// accordingly and rendered with new state.
///
/// Please see `examples/` for further information on how to use it.
pub trait Tui<Id, Message>
where
    Id: Eq + PartialEq + Clone + Hash,
    Message: Eq,
{
    /// Should initialize an application by mounting and activating components.
    fn init(&mut self, app: &mut App<Id, Message>) -> Result<()>;

    /// Should update the current state by handling a message from the view.
    fn update(&mut self, app: &mut App<Id, Message>);

    /// Should draw the application to a frame.
    fn view(&mut self, app: &mut App<Id, Message>, frame: &mut Frame);

    /// Should return true if the application is requested to quit.
    fn quit(&self) -> bool;
}

/// A tui-window using the cross-platform Terminal helper provided
/// by tui-realm.
pub struct Window {
    /// Helper around `Terminal` to quickly setup and perform on terminal.
    pub terminal: TerminalBridge,
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

/// Provides a way to create and run a new tui-application.
impl Window {
    /// Creates a tui-window using the default cross-platform Terminal
    /// helper and panics if its creation fails.
    pub fn new() -> Self {
        Self {
            terminal: TerminalBridge::new().expect("Cannot create terminal bridge"),
        }
    }

    /// Runs this tui-window with the tui-application given and performs the
    /// following steps:
    /// 1. Enter alternative terminal screen
    /// 2. Run main loop until application should quit and with each iteration
    ///    - poll new events (tick or user event)
    ///    - update application state
    ///    - redraw view
    /// 3. Leave alternative terminal screen
    pub fn run<T, Id, Message>(&mut self, tui: &mut T) -> Result<()>
    where
        T: Tui<Id, Message>,
        Id: Eq + PartialEq + Clone + Hash,
        Message: Eq,
    {
        let _ = self.terminal.enable_raw_mode();
        let _ = self.terminal.enter_alternate_screen();
        let mut app = App::default();

        tui.init(&mut app)?;

        while !tui.quit() {
            tui.update(&mut app);

            self.terminal.raw_mut().draw(|frame| {
                tui.view(&mut app, frame);
            })?;
        }

        let _ = self.terminal.leave_alternate_screen();
        let _ = self.terminal.disable_raw_mode();

        Ok(())
    }
}
