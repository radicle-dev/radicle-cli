use anyhow::{Error, Result};

use radicle_terminal::tui::events::{InputEvent, Key};
use radicle_terminal::tui::store::Store;
use radicle_terminal::tui::{Application, State};

fn main() -> Result<(), Error> {
    // Create basic application that will call `update` on
    // every input event received from event thread.
    let mut application = Application::new(&update);
    application.execute()?;
    Ok(())
}

fn update(store: &mut Store, event: &InputEvent) -> anyhow::Result<(), anyhow::Error> {
    // Set application set to `State::Exiting` when the key 'q' is received.
    // Note that any special tick handling is ignored for now.
    if let InputEvent::Input(Key::Char('q')) = *event {
        store.set("app.state", Box::new(State::Exiting));
    }
    Ok(())
}
