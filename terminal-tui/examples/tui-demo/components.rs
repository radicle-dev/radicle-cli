use tui_realm_stdlib::Textarea;

use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::{Component, MockComponent, NoUserEvent};

use radicle_terminal_tui as tui;
use tui::components::{ApplicationTitle, ShortcutBar, TabContainer};

use super::app::Message;

/// Since `terminal-tui` does not know the type of messages that are being
/// passed around in the app, the following handlers need to be implemented for
/// each component used.
impl Component<Message, NoUserEvent> for ApplicationTitle {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Message::Quit),
            _ => None,
        }
    }
}

impl Component<Message, NoUserEvent> for Textarea {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Message::Quit),
            _ => None,
        }
    }
}

impl Component<Message, NoUserEvent> for ShortcutBar {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => Some(Message::Quit),
            _ => None,
        }
    }
}

impl Component<Message, NoUserEvent> for TabContainer {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        let _ = match event {
            Event::Keyboard(KeyEvent { code: Key::Tab, .. }) => {
                self.perform(Cmd::Move(Direction::Right))
            }
            Event::Keyboard(KeyEvent {
                code: Key::Char('q'),
                ..
            }) => return Some(Message::Quit),
            _ => CmdResult::None,
        };
        None
    }
}
