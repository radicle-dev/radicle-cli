use std::collections::HashMap;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use anyhow::{Error, Result};
use lazy_static::lazy_static;

use radicle_terminal as term;

use term::tui::events::{InputEvent, Key};
use term::tui::store::Store;
use term::tui::theme::Theme;
use term::tui::window::{EmptyWidget, PageWidget, ShortcutWidget, TitleWidget};
use term::tui::{Application, State};

#[derive(Clone, Eq, PartialEq)]
pub enum Action {
    Quit,
    Command,
}

#[derive(Clone, Eq, PartialEq)]
pub enum InternalCall {
    Echo(String),
}

lazy_static! {
    static ref KEY_BINDINGS: HashMap<Key, Action> = [
        (Key::Char('q'), Action::Quit),
        (Key::Char('c'), Action::Command)
    ]
    .iter()
    .cloned()
    .collect();
}

fn main() -> Result<(), Error> {
    // Run application, execute logic and re-run until no internal call
    // is returned by application.
    while let Some(call) = run()? {
        match call {
            InternalCall::Echo(message) => {
                println!(
                    "Message: {}, sleeping for 5 seconds and re-running application",
                    message
                );
                thread::sleep(Duration::from_secs(5));
            }
        }
    }
    Ok(())
}

fn run() -> Result<Option<InternalCall>, Error> {
    // Create basic application that will call `update` on
    // every input event received from event thread.
    let call: Option<InternalCall> = None;
    let mut application = Application::new(&on_action).store(vec![
        (
            "app.shortcuts",
            Box::new(vec![String::from("q quit"), String::from("c command")]),
        ),
        ("app.title", Box::new(String::from("tui-internal-call"))),
        ("app.call.internal", Box::new(call)),
    ]);

    // Create a single-page application
    let pages = vec![PageWidget {
        title: Rc::new(TitleWidget),
        widgets: vec![Rc::new(EmptyWidget)],
        shortcuts: Rc::new(ShortcutWidget),
    }];

    // Use default, borderless theme
    let theme = Theme::default_dark();

    // Run application
    application.execute(pages, &theme)?;

    // If application set an interal call, return it to signal that it should
    // be executed and application re-run after
    match application
        .store
        .get::<Option<InternalCall>>("app.call.internal")
    {
        Ok(Some(call)) => return Ok(Some(call.clone())),
        Ok(None) | Err(_) => return Ok(None),
    }
}

fn on_action(store: &mut Store, event: &InputEvent) -> anyhow::Result<(), anyhow::Error> {
    // Set application set to `State::Exiting` when the key 'q' is received.
    // Note that any special tick handling is ignored for now.
    if let InputEvent::Input(key) = *event {
        if let Some(action) = KEY_BINDINGS.get(&key) {
            match action {
                Action::Quit => quit(store),
                Action::Command => run_command(store),
            }
        }
    }
    Ok(())
}

fn quit(store: &mut Store) {
    store.set("app.state", Box::new(State::Exiting))
}

fn run_command(store: &mut Store) {
    let message = String::from("tui-internal-call");
    store.set(
        "app.call.internal",
        Box::new(Some(InternalCall::Echo(message))),
    );
    store.set("app.state", Box::new(State::Exiting))
}
