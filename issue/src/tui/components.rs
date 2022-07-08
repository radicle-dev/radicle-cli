use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use timeago;

use librad::collaborative_objects::ObjectId;

use tui_realm_stdlib::{utils, Phantom};

use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::event::{Event, Key, KeyEvent};
use tuirealm::props::{AttrValue, Attribute, Color, Props, Style, TextSpan};
use tuirealm::tui::layout::{Constraint, Layout, Rect};
use tuirealm::tui::style::Modifier;
use tuirealm::tui::text::{Span, Spans};
use tuirealm::tui::widgets::{List as TuiList, ListItem, ListState as TuiListState};
use tuirealm::{Component, Frame, MockComponent, NoUserEvent, State, StateValue};

use radicle_common::cobs::issue::*;
use radicle_common::cobs::Timestamp;

use radicle_terminal_tui as tui;

use tui::components::{ApplicationTitle, ContextBar, ShortcutBar, TabContainer};
use tui::state::ListState;

use super::app::Message;
use super::issue::WrappedComment;

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
            Event::Keyboard(KeyEvent {
                code: Key::Enter, ..
            }) => match self.perform(Cmd::Submit) {
                CmdResult::Batch(batch) => batch.iter().fold(None, |_, result| match result {
                    CmdResult::Submit(State::One(StateValue::String(id))) => {
                        match ObjectId::from_str(&id) {
                            Ok(id) => Some(Message::EnterDetail(id)),
                            Err(_) => None,
                        }
                    }
                    _ => None,
                }),
                _ => None,
            },
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
    attributes: Props,
    issues: ListState<(IssueId, Issue)>,
}

impl IssueList {
    pub fn new(issues: Vec<(IssueId, Issue)>) -> Self {
        Self {
            attributes: Props::default(),
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
    fn view(&mut self, frame: &mut Frame, area: Rect) {
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
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value);
    }

    fn state(&self) -> State {
        State::One(StateValue::Usize(self.issues.items().selected_index()))
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(Direction::Up) => {
                self.issues.select_previous();
                let selected = self.issues.items().selected_index();
                CmdResult::Changed(State::One(StateValue::Usize(selected)))
            }
            Cmd::Move(Direction::Down) => {
                self.issues.select_next();
                let selected = self.issues.items().selected_index();
                CmdResult::Changed(State::One(StateValue::Usize(selected)))
            }
            Cmd::Submit => {
                let (id, _) = self.issues.items().selected().unwrap();
                CmdResult::Submit(State::One(StateValue::String(id.to_string())))
            }
            _ => CmdResult::None,
        }
    }
}

impl Component<Message, NoUserEvent> for IssueList {
    fn on(&mut self, _event: Event<NoUserEvent>) -> Option<Message> {
        None
    }
}

pub struct CommentList<R> {
    attributes: Props,
    comments: ListState<WrappedComment<R>>,
    issue: Option<(IssueId, Issue)>,
}

impl<R> CommentList<R> {
    pub fn new(issue: Option<(IssueId, Issue)>, comments: Vec<WrappedComment<R>>) -> Self {
        Self {
            attributes: Props::default(),
            comments: ListState::new(comments),
            issue: issue,
        }
    }

    fn items(&self, comment: &WrappedComment<R>, width: u16) -> ListItem {
        let (author, body, reactions, timestamp, indent) = comment.author_info();
        let reactions = reactions
            .iter()
            .map(|(r, _)| format!("{} ", r.emoji))
            .collect::<String>();

        let lines = [
            Self::body(body, indent, width),
            vec![
                Spans::from(String::new()),
                Spans::from(Self::meta(author, reactions, timestamp, indent)),
                Spans::from(String::new()),
            ],
        ]
        .concat();
        ListItem::new(lines)
    }

    fn body<'a>(body: String, indent: u16, width: u16) -> Vec<Spans<'a>> {
        let props = Props::default();
        let body = TextSpan::new(body).fg(Color::Rgb(150, 150, 150));

        let lines = utils::wrap_spans(&[body], (width - indent) as usize, &props)
            .iter()
            .map(|line| Spans::from(format!("{}{}", whitespaces(indent), line.0[0].content)))
            .collect::<Vec<_>>();
        lines
    }

    fn meta<'a>(
        author: String,
        reactions: String,
        timestamp: Timestamp,
        indent: u16,
    ) -> Vec<Span<'a>> {
        let fmt = timeago::Formatter::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timeago = Duration::from_secs(now - timestamp.as_secs());

        vec![
            Span::raw(whitespaces(indent)),
            Span::styled(
                author,
                Style::default()
                    .fg(Color::Rgb(79, 75, 187))
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::raw(whitespaces(1)),
            Span::styled(
                fmt.convert(timeago),
                Style::default()
                    .fg(Color::Rgb(70, 70, 70))
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::raw(whitespaces(1)),
            Span::raw(reactions),
        ]
    }
}

impl<R> MockComponent for CommentList<R> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        use tuirealm::tui::layout::Direction;

        let mut context = match &self.issue {
            Some((id, issue)) => ContextBar::new(
                "Issue",
                &format!("{}", id),
                issue.title(),
                &issue.author().name(),
                &format!("{}", self.comments.items().count()),
            ),
            None => ContextBar::new("Issue", "", "", "", ""),
        };
        let context_h = context.query(Attribute::Height).unwrap().unwrap_size();
        let spacer_h = 1;

        let list_h = area.height.saturating_sub(context_h);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(list_h.saturating_sub(spacer_h)),
                    Constraint::Length(context_h),
                    Constraint::Length(spacer_h),
                ]
                .as_ref(),
            )
            .split(area);

        let items = self
            .comments
            .items()
            .all()
            .iter()
            .map(|comment| self.items(comment, area.width))
            .collect::<Vec<_>>();

        let list = TuiList::new(items)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Rgb(238, 111, 248)))
            .highlight_symbol("│ ")
            .repeat_highlight_symbol(true);

        let mut state: TuiListState = TuiListState::default();
        state.select(Some(self.comments.items().selected_index()));
        frame.render_stateful_widget(list, layout[0], &mut state);

        context.view(frame, layout[1]);
    }

    fn query(&self, attr: Attribute) -> Option<AttrValue> {
        self.attributes.get(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.attributes.set(attr, value);
    }

    fn state(&self) -> State {
        State::One(StateValue::Usize(self.comments.items().selected_index()))
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Move(Direction::Up) => {
                self.comments.select_previous();
            }
            Cmd::Move(Direction::Down) => {
                self.comments.select_next();
            }
            _ => {}
        }
        CmdResult::None
    }
}

impl<R> Component<Message, NoUserEvent> for CommentList<R> {
    fn on(&mut self, event: Event<NoUserEvent>) -> Option<Message> {
        match event {
            Event::Keyboard(KeyEvent {
                code: Key::Down, ..
            }) => {
                self.perform(Cmd::Move(Direction::Down));
                None
            }
            Event::Keyboard(KeyEvent { code: Key::Up, .. }) => {
                self.perform(Cmd::Move(Direction::Up));
                None
            }
            Event::Keyboard(KeyEvent { code: Key::Esc, .. }) => Some(Message::LeaveDetail),
            _ => None,
        }
    }
}

pub fn whitespaces(indent: u16) -> String {
    match String::from_utf8(vec![b' '; indent as usize]) {
        Ok(spaces) => spaces,
        Err(_) => String::new(),
    }
}
