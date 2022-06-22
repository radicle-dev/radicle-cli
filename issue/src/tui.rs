use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{Error, Result};
use lazy_static::lazy_static;

use radicle_common::cobs::issue::{Issue, IssueId, State as IssueState};
use radicle_common::project::Metadata;
use radicle_terminal as term;

use term::tui::events::{InputEvent, Key};
use term::tui::store::Store;
use term::tui::theme::Theme;
use term::tui::window::{PageWidget, ShortcutWidget, TitleWidget};
use term::tui::{Application, State};

pub mod state;
pub mod window;

use state::Tab;
use window::{BrowserWidget, TabWidget};

type IssueList = Vec<(IssueId, Issue)>;

#[derive(Clone, Eq, PartialEq)]
pub enum InternalCall {}

#[derive(Clone, Eq, PartialEq)]
pub enum Action {
    Up,
    Down,
    NextTab,
    Quit,
}

lazy_static! {
    static ref BINDINGS: HashMap<Key, Action> = [
        (Key::Up, Action::Up),
        (Key::Down, Action::Down),
        (Key::Tab, Action::NextTab),
        (Key::Char('q'), Action::Quit)
    ]
    .iter()
    .cloned()
    .collect();
}

pub fn run(project: &Metadata, issues: IssueList) -> Result<Option<InternalCall>, Error> {
    let call: Option<InternalCall> = None;
    let mut open = issues.clone();
    let mut closed = issues;

    open.retain(|(_, issue)| issue.state() == IssueState::Open);
    closed.retain(|(_, issue)| issue.state() != IssueState::Open);

    let mut app = Application::new(&update).store(vec![
        ("app.title", Box::new(project.name.clone())),
        ("app.call.internal", Box::new(call)),
        ("app.browser.tab.active", Box::new(Tab::Open)),
        ("app.shortcuts", Box::new(vec![String::from("q quit")])),
        ("project.issue.open.list", Box::new(open)),
        ("project.issue.open.active", Box::new(0_usize)),
        ("project.issue.closed.list", Box::new(closed)),
        ("project.issue.closed.active", Box::new(0_usize)),
    ]);

    let pages = vec![PageWidget {
        title: Rc::new(TitleWidget),
        widgets: vec![Rc::new(BrowserWidget {
            tabs: Rc::new(TabWidget),
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
        }
    }
    Ok(())
}

pub fn quit_application(store: &mut Store) -> Result<(), Error> {
    store.set("app.state", Box::new(State::Exiting));
    Ok(())
}

pub fn select_next_issue(store: &mut Store) -> Result<(), Error> {
    let tab = store.get::<Tab>("app.browser.tab.active")?;
    let (issues, active) = match tab {
        Tab::Open => {
            let issues = store.get::<IssueList>("project.issue.open.list")?;
            let active = store.get::<usize>("project.issue.open.active")?;
            (issues, active)
        }
        Tab::Closed => {
            let issues = store.get::<IssueList>("project.issue.closed.list")?;
            let active = store.get::<usize>("project.issue.closed.active")?;
            (issues, active)
        }
    };
    let active = match *active >= issues.len() - 1 {
        true => issues.len() - 1,
        false => active + 1,
    };
    match tab {
        Tab::Open => store.set("project.issue.open.active", Box::new(active)),
        Tab::Closed => store.set("project.issue.closed.active", Box::new(active)),
    }

    Ok(())
}

pub fn select_previous_issue(store: &mut Store) -> Result<(), Error> {
    let tab = store.get::<Tab>("app.browser.tab.active")?;
    let active = match tab {
        Tab::Open => store.get::<usize>("project.issue.open.active")?,
        Tab::Closed => store.get::<usize>("project.issue.closed.active")?,
    };

    let active = match *active == 0 {
        true => 0,
        false => active - 1,
    };
    match tab {
        Tab::Open => store.set("project.issue.open.active", Box::new(active)),
        Tab::Closed => store.set("project.issue.closed.active", Box::new(active)),
    }

    Ok(())
}

pub fn select_next_tab(store: &mut Store) -> Result<(), Error> {
    let tab = store.get::<Tab>("app.browser.tab.active")?;
    match tab {
        Tab::Open => store.set("app.browser.tab.active", Box::new(Tab::Closed)),
        Tab::Closed => store.set("app.browser.tab.active", Box::new(Tab::Open)),
    }
    Ok(())
}
