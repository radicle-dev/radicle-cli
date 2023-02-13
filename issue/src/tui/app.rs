use anyhow::Result;

use librad::git::storage::ReadOnly;

use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute};
use tuirealm::tui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::{Frame, Sub, SubClause, SubEventClause};

use radicle_common::cobs::issue::*;
use radicle_common::project;

use radicle_terminal_tui as tui;

use tui::components::{ApplicationTitle, Shortcut, ShortcutBar, TabContainer};
use tui::{App, Tui};

use super::components::{CommentList, GlobalListener, IssueList};

use super::issue;
use super::issue::{GroupedIssues, WrappedComment};

/// Messages handled by this tui-application.
#[derive(Debug, Eq, PartialEq)]
pub enum Message {
    TabChanged(usize),
    EnterDetail(IssueId),
    LeaveDetail,
    Quit,
}

/// All components known to this tui-application.
#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub enum Id {
    Global,
    Title,
    Browser,
    Detail,
    Shortcuts,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Mode {
    Browser,
    Detail,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Browser
    }
}

/// App-window used by this application.
#[derive(Default)]
pub struct IssueTui {
    /// Issues currently displayed by this tui.
    issues: GroupedIssues,
    /// Represents the active view
    mode: Mode,
    /// True if application should quit.
    quit: bool,
}

impl IssueTui {
    pub fn new<S: AsRef<ReadOnly>>(
        storage: &S,
        metadata: &project::Metadata,
        store: &IssueStore,
    ) -> Self {
        let issues = match issue::load(storage, metadata, store) {
            Ok(issues) => issues,
            Err(_) => vec![],
        };

        Self {
            issues: GroupedIssues::from(&issues),
            mode: Mode::default(),
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
                    Constraint::Length(container_h.saturating_sub(2)),
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
            Id::Browser,
            TabContainer::default()
                .child(
                    format!("{} Open", self.issues.open.len()),
                    IssueList::new(self.issues.open.clone()),
                )
                .child(
                    format!("{} Closed", self.issues.closed.len()),
                    IssueList::new(self.issues.closed.clone()),
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

        app.mount(Id::Detail, CommentList::<()>::new(None, vec![]), vec![])?;

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
        app.activate(Id::Browser)?;

        Ok(())
    }

    fn view(&mut self, app: &mut App<Id, Message>, frame: &mut Frame) {
        let layout = Self::layout(app, frame);

        match self.mode {
            Mode::Browser => {
                app.view(Id::Title, frame, layout[0]);
                app.view(Id::Browser, frame, layout[1]);
            }
            Mode::Detail => {
                app.view(Id::Detail, frame, layout[1]);
            }
        }
        app.view(Id::Shortcuts, frame, layout[2]);
    }

    fn update(&mut self, app: &mut App<Id, Message>) {
        for message in app.poll() {
            match message {
                Message::Quit => self.quit = true,
                Message::EnterDetail(issue_id) => {
                    let issues = Vec::<(IssueId, Issue)>::from(&self.issues);
                    if let Some((id, issue)) = issues.iter().find(|(id, _)| *id == issue_id) {
                        let comments = issue
                            .comments()
                            .iter()
                            .map(|comment| WrappedComment::Reply {
                                comment: comment.clone(),
                            })
                            .collect::<Vec<_>>();

                        self.mode = Mode::Detail;

                        let comments = [
                            vec![WrappedComment::Root {
                                comment: issue.comment.clone(),
                            }],
                            comments,
                        ]
                        .concat();

                        app.remount(
                            Id::Detail,
                            CommentList::new(Some((*id, issue.clone())), comments),
                            vec![],
                        )
                        .ok();
                        app.activate(Id::Detail).ok();
                    }
                }
                Message::LeaveDetail => {
                    self.mode = Mode::Browser;
                    app.blur().ok();
                }
                _ => {}
            }
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }
}
