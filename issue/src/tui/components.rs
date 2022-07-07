use std::time::{Duration, SystemTime, UNIX_EPOCH};

use timeago;

use tui_realm_stdlib::Phantom;

use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::props::{AttrValue, Attribute, Color, Props, Style};
use tuirealm::tui::layout::Rect;
use tuirealm::tui::style::Modifier;
use tuirealm::tui::text::{Span, Spans};
use tuirealm::tui::widgets::{List as TuiList, ListItem, ListState as TuiListState};
use tuirealm::{Component, Frame, MockComponent, NoUserEvent, State, StateValue};

use radicle_common::cobs::issue::*;
use radicle_terminal_tui as tui;
use tui::components::{ApplicationTitle, ShortcutBar, TabContainer};
use tui::state::ListState;

use super::app::Group;
use super::app::Message;

/// Since `terminal-tui` does not know the type of messages that are being
/// passed around in the app, the following handlers need to be implemented for
/// every component used.
#[derive(Default, MockComponent)]
pub struct GlobalListener {
    component: Phantom,
}

impl Component<Message, NoUserEvent> for GlobalListener {
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

impl Component<Message, NoUserEvent> for ApplicationTitle {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl Component<Message, NoUserEvent> for ShortcutBar {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

impl Component<Message, NoUserEvent> for TabContainer {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent { code: Key::Tab, .. }) => {
                match self.perform(Cmd::Move(Direction::Right)) {
                    CmdResult::Changed(State::One(StateValue::Usize(index))) => {
                        Some(Message::TabChanged(index))
                    }
                    _ => None,
                }
            }
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                self.perform(Cmd::Move(Direction::Up));
                None
            }
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => {
                self.perform(Cmd::Move(Direction::Down));
                None
            }
            _ => None,
        }
    }
}

pub struct IssueList {
    props: Props,
    group: Group,
    issues: ListState<(IssueId, Issue)>,
}

impl IssueList {
    pub fn new(issues: Vec<(IssueId, Issue)>, group: Group) -> Self {
        Self {
            props: Props::default(),
            group: group,
            issues: ListState::new(issues),
        }
    }

    fn items(&self, _id: &IssueId, issue: &Issue) -> ListItem {
        let fmt = timeago::Formatter::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timeago = Duration::from_secs(now - issue.comment.timestamp.as_secs());

        let lines = vec![
            Spans::from(Span::styled(
                issue.title.clone(),
                Style::default().fg(Color::Rgb(117, 113, 249)),
            )),
            Spans::from(vec![
                Span::styled(
                    issue.author.name(),
                    Style::default()
                        .fg(Color::Rgb(79, 75, 187))
                        .add_modifier(Modifier::ITALIC),
                ),
                Span::raw(String::from(" ")),
                Span::styled(
                    fmt.convert(timeago),
                    Style::default()
                        .fg(Color::Rgb(70, 70, 70))
                        .add_modifier(Modifier::ITALIC),
                ),
            ]),
        ];
        ListItem::new(lines)
    }
}

impl MockComponent for IssueList {
    fn view(&mut self, render: &mut Frame, area: Rect) {
        let items = self
            .issues
            .items()
            .all()
            .iter()
            .map(|(id, issue)| self.items(id, issue))
            .collect::<Vec<_>>();

        let list = TuiList::new(items)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Rgb(238, 111, 248)))
            .highlight_symbol("│ ")
            .repeat_highlight_symbol(true);

        let mut state: TuiListState = TuiListState::default();

        state.select(Some(self.issues.items().selected_index()));
        render.render_stateful_widget(list, area, &mut state);
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.props.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.props.set(attr, value);
    }

    fn state(&self) -> State {
        State::One(StateValue::Usize(self.issues.items().selected_index()))
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(Direction::Up) => {
                self.issues.select_previous();
            }
            Cmd::Move(Direction::Down) => {
                self.issues.select_next();
            }
            _ => {}
        }
        CmdResult::None
    }
}

impl Component<Message, NoUserEvent> for IssueList {
    fn on(&mut self, ev: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}
