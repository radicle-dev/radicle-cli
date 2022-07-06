use anyhow::Result;

use tuirealm::props::{AttrValue, Attribute};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::Frame;

use radicle_terminal_tui as tui;
use tui::components::{ApplicationTitle, Shortcut, ShortcutBar};
use tui::{App, Tui};

/// Messages handled by this tui-application.
#[derive(Debug, Eq, PartialEq)]
pub enum Message {
    Quit,
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    Title,
    Shortcuts,
}

/// App-window used by this application.
#[derive(Default)]
pub struct IssueTui {
    quit: bool,
}

/// Creates a new application using a tui-realm-application, mounts all
/// components and sets focus to a default one.
impl IssueTui {
    fn layout(app: &mut App<Id, Message>, frame: &mut Frame) -> Vec<Rect> {
        let area = frame.size();
        let title_h = app
            .query(Id::Title, Attribute::Height)
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let shortcuts_h = app
            .query(Id::Shortcuts, Attribute::Height)
            .unwrap_or(AttrValue::Size(0))
            .unwrap_size();
        let container_h = area.height.saturating_sub(title_h + shortcuts_h);

        Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(title_h),
                    Constraint::Length(container_h - 2),
                    Constraint::Length(shortcuts_h),
                ]
                .as_ref(),
            )
            .split(area)
    }
}

impl Tui<Id, Message> for IssueTui {
    fn init(&mut self, app: &mut App<Id, Message>) -> Result<()> {
        app.mount(Id::Title, ApplicationTitle::new("my-project"), vec![])?;
        app.mount(
            Id::Shortcuts,
            ShortcutBar::default().child(Shortcut::new("q", "quit")),
            vec![],
        )?;

        // We need to give focus to a component then
        app.activate(Id::Title)?;

        Ok(())
    }

    fn view(&mut self, app: &mut App<Id, Message>, frame: &mut Frame) {
        let layout = Self::layout(app, frame);

        app.view(Id::Title, frame, layout[0]);
        app.view(Id::Shortcuts, frame, layout[2]);
    }

    fn update(&mut self, app: &mut App<Id, Message>) {
        for message in app.poll() {
            match message {
                Message::Quit => self.quit = true,
            }
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }
}
