use anyhow::Result;

use tui_realm_stdlib::Textarea;
use tuirealm::props::{AttrValue, Attribute, BorderSides, Borders, Color, TextSpan};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::Frame;

use radicle_terminal_tui as tui;
use tui::components::{ApplicationTitle, Shortcut, ShortcutBar, TabContainer};
use tui::{App, Tui};

/// Messages handled by this tui-application.
#[derive(Debug, Eq, PartialEq)]
pub enum Message {
    Quit,
}

/// All components known to the application.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    Title,
    Content,
    Shortcuts,
}

/// App-window used by this application.
pub struct Demo {
    quit: bool,
}

/// Creates a new application using a tui-realm-application, mounts all
/// components and sets focus to a default one.
impl Demo {
    pub fn welcome_content() -> Vec<TextSpan> {
        vec![
            TextSpan::new("# Welcome").fg(Color::Cyan),
            TextSpan::new(String::new()),
            TextSpan::from("This is a basic Radicle TUI application."),
        ]
    }

    pub fn help_content() -> Vec<TextSpan> {
        vec![
            TextSpan::new("# Help").fg(Color::Cyan),
            TextSpan::new(String::new()),
            TextSpan::from("Please see https://radicle.xyz for further information."),
        ]
    }

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

impl Default for Demo {
    fn default() -> Self {
        Self { quit: false }
    }
}

impl Tui<Id, Message> for Demo {
    fn init(&mut self, app: &mut App<Id, Message>) -> Result<()> {
        app.mount(Id::Title, ApplicationTitle::new("my-project"))?;
        app.mount(
            Id::Content,
            TabContainer::default()
                .child(
                    String::from("Welcome"),
                    Textarea::default()
                        .borders(Borders::default().sides(BorderSides::NONE))
                        .text_rows(&Self::welcome_content()),
                )
                .child(
                    String::from("Help"),
                    Textarea::default()
                        .borders(Borders::default().sides(BorderSides::NONE))
                        .text_rows(&Self::help_content()),
                ),
        )?;
        app.mount(
            Id::Shortcuts,
            ShortcutBar::default()
                .child(Shortcut::new("q", "quit"))
                .child(Shortcut::new("?", "help")),
        )?;
        // We need to give focus to a component then
        app.activate(Id::Content)?;

        Ok(())
    }

    fn view(&mut self, app: &mut App<Id, Message>, frame: &mut Frame) {
        let layout = Self::layout(app, frame);

        app.view(Id::Title, frame, layout[0]);
        app.view(Id::Content, frame, layout[1]);
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
