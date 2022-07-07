use anyhow::Result;

use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::{Frame, Sub, SubClause, SubEventClause};

use librad::git::storage::ReadOnly;

use radicle_common::cobs::issue::State as IssueState;
use radicle_common::cobs::issue::*;
use radicle_common::project;

use radicle_terminal_tui as tui;
use tui::components::{ApplicationTitle, Shortcut, ShortcutBar, TabContainer};
use tui::{App, Tui};

use super::components::{GlobalListener, IssueList};

/// Messages handled by this tui-application.
#[derive(Debug, Eq, PartialEq)]
pub enum Message {
    TabChanged(usize),
    Quit,
}

/// All components known to this tui-application.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    Global,
    Title,
    Content,
    Shortcuts,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash)]
pub enum Group {
    Open,
    Closed,
}

#[derive(Default)]
pub struct IssueGroups {
    open: Vec<(IssueId, Issue)>,
    closed: Vec<(IssueId, Issue)>,
}

/// App-window used by this application.
#[derive(Default)]
pub struct IssueTui {
    /// Issues currently displayed by this tui.
    issues: IssueGroups,
    /// Current issue.
    active: Option<(Group, IssueId)>,
    /// True if application should quit.
    quit: bool,
}

impl IssueTui {
    pub fn new<S: AsRef<ReadOnly>>(
        storage: &S,
        metadata: &project::Metadata,
        store: &IssueStore,
    ) -> Self {
        let issues = match Self::load_issues(storage, metadata, store) {
            Ok(issues) => issues,
            Err(_) => vec![],
        };

        Self {
            issues: Self::group_issues(&issues),
            active: None,
            quit: false,
        }
    }
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

    fn load_issues<S: AsRef<ReadOnly>>(
        storage: &S,
        metadata: &project::Metadata,
        store: &IssueStore,
    ) -> Result<Vec<(IssueId, Issue)>> {
        let mut issues = store.all(&metadata.urn)?;
        Self::resolve_issues(storage, &mut issues);
        Ok(issues)
    }

    fn resolve_issues<S: AsRef<ReadOnly>>(storage: &S, issues: &mut Vec<(IssueId, Issue)>) {
        let _ = issues
            .iter_mut()
            .map(|(_, issue)| issue.resolve(&storage).ok())
            .collect::<Vec<_>>();
    }

    fn group_issues(issues: &Vec<(IssueId, Issue)>) -> IssueGroups {
        let mut open = issues.clone();
        let mut closed = issues.clone();

        open.retain(|(_, issue)| issue.state() == IssueState::Open);
        closed.retain(|(_, issue)| issue.state() != IssueState::Open);

        IssueGroups {
            open: open,
            closed: closed,
        }
    }
}

impl Tui<Id, Message> for IssueTui {
    fn init(&mut self, app: &mut App<Id, Message>) -> Result<()> {
        app.mount(Id::Title, ApplicationTitle::new("my-project"), vec![])?;
        app.mount(
            Id::Content,
            TabContainer::default()
                .child(
                    format!("{} Open", self.issues.open.len()),
                    IssueList::new(self.issues.open.clone(), Group::Open),
                )
                .child(
                    format!("{} Closed", self.issues.closed.len()),
                    IssueList::new(self.issues.closed.clone(), Group::Closed),
                ),
            vec![
                Sub::new(
                    SubEventClause::Keyboard(KeyEvent {
                        code: Key::Tab,
                        modifiers: KeyModifiers::NONE,
                    }),
                    SubClause::Always,
                ),
                Sub::new(
                    SubEventClause::Keyboard(KeyEvent {
                        code: Key::Up,
                        modifiers: KeyModifiers::NONE,
                    }),
                    SubClause::Always,
                ),
                Sub::new(
                    SubEventClause::Keyboard(KeyEvent {
                        code: Key::Down,
                        modifiers: KeyModifiers::NONE,
                    }),
                    SubClause::Always,
                ),
            ],
        )?;

        app.mount(
            Id::Shortcuts,
            ShortcutBar::default().child(Shortcut::new("q", "quit")),
            vec![],
        )?;

        app.mount(
            Id::Global,
            GlobalListener::default(),
            vec![Sub::new(
                SubEventClause::Keyboard(KeyEvent {
                    code: Key::Char('q'),
                    modifiers: KeyModifiers::NONE,
                }),
                SubClause::Always,
            )],
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
                _ => {}
            }
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }
}
