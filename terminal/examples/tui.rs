use std::collections::HashMap;

use anyhow::{Error, Result};
use lazy_static::lazy_static;

use radicle_terminal::tui::events::{InputEvent, Key};
use radicle_terminal::tui::store::Store;
use radicle_terminal::tui::theme::Theme;
use radicle_terminal::tui::{Application, State};

#[derive(Clone, Eq, PartialEq)]
pub enum Action {
    Quit,
}

lazy_static! {
    static ref KEY_BINDINGS: HashMap<Key, Action> =
        [(Key::Char('q'), Action::Quit)].iter().cloned().collect();
}

fn main() -> Result<(), Error> {
    // Create basic application that will call `update` on
    // every input event received from event thread.
    let mut application = Application::new(&on_action);
    let theme = Theme::default_dark();
    application.execute(&theme)?;
    Ok(())
}

fn on_action(store: &mut Store, event: &InputEvent) -> anyhow::Result<(), anyhow::Error> {
    // Set application set to `State::Exiting` when the key 'q' is received.
    // Note that any special tick handling is ignored for now.
    if let InputEvent::Input(key) = *event {
        if let Some(action) = KEY_BINDINGS.get(&key) {
            match action {
                Action::Quit => store.set("app.state", Box::new(State::Exiting)),
            }
        }
    }
    Ok(())
}
