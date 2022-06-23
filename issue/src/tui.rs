use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{Error, Result};
use lazy_static::lazy_static;

use radicle_common::cobs::issue::{Issue, IssueId, State as IssueState};
use radicle_common::project::Metadata;
use radicle_terminal as term;

use term::tui::events::{InputEvent, Key};
use term::tui::store::{ListProperty, Store, TabProperty};
use term::tui::theme::Theme;
use term::tui::window::{PageWidget, ShortcutWidget, TitleWidget};
use term::tui::{Application, State};

pub mod state;
pub mod store;
pub mod window;

use state::Tab;
use store::ToogleProperty;
use window::{BrowserWidget, InfoWidget, TabWidget};

type TabList = TabProperty<Tab>;
type IssueList = ListProperty<(IssueId, Issue)>;

#[derive(Clone, Eq, PartialEq)]
pub enum InternalCall {
    New,
}

#[derive(Clone, Eq, PartialEq)]
pub enum Action {
    Up,
    Down,
    NextTab,
    Quit,
    ToogleInfo,
    NewIssue,
}

lazy_static! {
    static ref BINDINGS: HashMap<Key, Action> = [
        (Key::Up, Action::Up),
        (Key::Down, Action::Down),
        (Key::Tab, Action::NextTab),
        (Key::Char('q'), Action::Quit),
        (Key::Char('i'), Action::ToogleInfo),
        (Key::Char('n'), Action::NewIssue),
    ]
    .iter()
    .cloned()
    .collect();
}

pub fn run(
    project: &Metadata,
    issues: Vec<(IssueId, Issue)>,
) -> Result<Option<InternalCall>, Error> {
    let mut open = issues.clone();
    let mut closed = issues;

    open.retain(|(_, issue)| issue.state() == IssueState::Open);
    closed.retain(|(_, issue)| issue.state() != IssueState::Open);

    let tabs = vec![Tab::Open, Tab::Closed];
    let mut app = Application::new(&update).store(vec![
        ("app.title", Box::new(project.name.clone())),
        ("app.browser.tabs", Box::new(TabProperty::new(tabs))),
        (
            "app.shortcuts",
            Box::new(vec![
                String::from("n new"),
                String::from("i info"),
                String::from("q quit"),
            ]),
        ),
        ("app.browser.info", Box::new(ToogleProperty::new(false))),
        ("project.issues.open", Box::new(IssueList::new(open))),
        ("project.issues.closed", Box::new(IssueList::new(closed))),
    ]);

    let pages = vec![PageWidget {
        title: Rc::new(TitleWidget),
        widgets: vec![Rc::new(BrowserWidget {
            tabs: Rc::new(TabWidget),
            info: Rc::new(InfoWidget),
        })],
        shortcuts: Rc::new(ShortcutWidget),
    }];

    let theme = Theme::default_dark();
    app.execute(pages, &theme)?;

    match app.store.get::<Option<InternalCall>>("app.call.internal") {
        Ok(Some(call)) => Ok(Some(call.clone())),
        Ok(None) | Err(_) => Ok(None),
    }
}

pub fn update(store: &mut Store, event: &InputEvent) -> Result<(), Error> {
    match event {
        InputEvent::Input(key) => on_action(store, *key)?,
        InputEvent::Tick => {}
    }
    Ok(())
}

pub fn on_action(store: &mut Store, key: Key) -> Result<(), Error> {
    if let Some(action) = BINDINGS.get(&key) {
        match action {
            Action::Quit => {
                quit_application(store)?;
            }
            Action::Up => {
                select_previous_issue(store)?;
            }
            Action::Down => {
                select_next_issue(store)?;
            }
            Action::NextTab => {
                select_next_tab(store)?;
            }
            Action::ToogleInfo => {
                toogle_info(store)?;
            }
            Action::NewIssue => {
                new_issue(store)?;
            }
        }
    }
    Ok(())
}

pub fn quit_application(store: &mut Store) -> Result<(), Error> {
    store.set("app.state", Box::new(State::Exiting));
    Ok(())
}

pub fn select_next_issue(store: &mut Store) -> Result<(), Error> {
    let tabs = store.get::<TabList>("app.browser.tabs")?;
    match tabs.items().selected() {
        Some(Tab::Open) => {
            let issues = store.get_mut::<IssueList>("project.issues.open")?;
            issues.select_next();
        }
        Some(Tab::Closed) => {
            let issues = store.get_mut::<IssueList>("project.issues.closed")?;
            issues.select_next();
        }
        _ => {}
    };
    Ok(())
}

pub fn select_previous_issue(store: &mut Store) -> Result<(), Error> {
    let tabs = store.get::<TabList>("app.browser.tabs")?;
    match tabs.items().selected() {
        Some(Tab::Open) => {
            let issues = store.get_mut::<IssueList>("project.issues.open")?;
            issues.select_previous();
        }
        Some(Tab::Closed) => {
            let issues = store.get_mut::<IssueList>("project.issues.closed")?;
            issues.select_previous();
        }
        _ => {}
    };
    Ok(())
}

pub fn select_next_tab(store: &mut Store) -> Result<(), Error> {
    let tabs = store.get_mut::<TabList>("app.browser.tabs")?;
    tabs.select_next();
    Ok(())
}

pub fn toogle_info(store: &mut Store) -> Result<(), Error> {
    let info = store.get_mut::<ToogleProperty>("app.browser.info")?;
    info.toggle();
    Ok(())
}

pub fn new_issue(store: &mut Store) -> Result<(), Error> {
    store.set("app.call.internal", Box::new(Some(InternalCall::New)));
    quit_application(store)
}
