use std::sync::mpsc::{channel, Receiver, RecvError};
use std::thread;
use std::time::Duration;

use crossterm::event;

/// Abstraction for combinations of crossterm key events
/// with modifiers.
#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Shift(char),
    Up,
    Down,
    Esc,
    Enter,
    Unknown,
}

/// Converts a crossterm event to a key abstraction.
impl From<event::KeyEvent> for Key {
    fn from(key_event: event::KeyEvent) -> Self {
        match key_event {
            event::KeyEvent {
                code: event::KeyCode::Char(c),
                modifiers: event::KeyModifiers::NONE,
            } => Key::Char(c),
            event::KeyEvent {
                code: event::KeyCode::Char(c),
                modifiers: event::KeyModifiers::SHIFT,
            } => Key::Shift(c),
            event::KeyEvent {
                code: event::KeyCode::Char(c),
                modifiers: event::KeyModifiers::CONTROL,
            } => Key::Ctrl(c),
            event::KeyEvent {
                code: event::KeyCode::Up,
                ..
            } => Key::Up,
            event::KeyEvent {
                code: event::KeyCode::Down,
                ..
            } => Key::Down,
            event::KeyEvent {
                code: event::KeyCode::Esc,
                ..
            } => Key::Esc,
            event::KeyEvent {
                code: event::KeyCode::Enter,
                ..
            } => Key::Enter,
            _ => Key::Unknown,
        }
    }
}

/// Encodes the event type known to the application. The event
/// type `Input` wraps an key event and the type `Tick` can be used
/// to update application state constantly if needed.
pub enum InputEvent {
    Input(Key),
    Tick,
}

/// A small event handler that wrap crossterm input and tick event. Each event
/// type is handled in its own thread and returned to a common `Receiver`.
pub struct Events {
    rx: Receiver<InputEvent>,
}

impl Events {
    /// Spawns event thread that sends a key event (if received) and a tick event
    /// to a MPSC channel.
    pub fn new(tick_rate: Duration) -> Events {
        let (tx, rx) = channel();

        thread::spawn(move || loop {
            if crossterm::event::poll(tick_rate).unwrap() {
                if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                    let key = Key::from(key);
                    if tx.send(InputEvent::Input(key)).is_err() {
                        break;
                    }
                }
            }
            if tx.send(InputEvent::Tick).is_err() {
                break;
            }
        });

        Events { rx }
    }

    /// Attempts to read an event. This function will block the current thread.
    pub fn next(&self) -> Result<InputEvent, RecvError> {
        self.rx.recv()
    }
}
