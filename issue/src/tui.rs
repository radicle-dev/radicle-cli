use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{Error, Result};
use lazy_static::lazy_static;

use radicle_common::cobs::issue::{Issue, IssueId};
use radicle_common::project::Metadata;
use radicle_terminal as term;

use term::tui::events::{InputEvent, Key};
use term::tui::store::Store;
use term::tui::theme::Theme;
use term::tui::window::{PageWidget, ShortcutWidget, TitleWidget};
use term::tui::{Application, State};

pub mod window;
use window::BrowserWidget;

type IssueList = Vec<(IssueId, Issue)>;

#[derive(Clone, Eq, PartialEq)]
pub enum InternalCall {}

#[derive(Clone, Eq, PartialEq)]
pub enum Action {
    Up,
    Down,
    Quit,
}

lazy_static! {
    static ref BINDINGS: HashMap<Key, Action> = [
        (Key::Up, Action::Up),
        (Key::Down, Action::Down),
        (Key::Char('q'), Action::Quit)
    ]
    .iter()
    .cloned()
    .collect();
}

pub fn run(project: &Metadata, issues: IssueList) -> Result<Option<InternalCall>, Error> {
    let call: Option<InternalCall> = None;
    let mut app = Application::new(&update).store(vec![
        ("app.title", Box::new(project.name.clone())),
        ("app.call.internal", Box::new(call)),
        ("app.shortcuts", Box::new(vec![String::from("q quit")])),
        ("project.issue.list", Box::new(issues)),
        ("project.issue.active", Box::new(0_usize)),
    ]);

    let pages = vec![PageWidget {
        title: Rc::new(TitleWidget),
        widgets: vec![Rc::new(BrowserWidget)],
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
        }
    }
    Ok(())
}

pub fn quit_application(store: &mut Store) -> Result<(), Error> {
    store.set("app.state", Box::new(State::Exiting));
    Ok(())
}

pub fn select_next_issue(store: &mut Store) -> Result<(), Error> {
    let issues = store.get::<IssueList>("project.issue.list")?;
    let active = store.get::<usize>("project.issue.active")?;
    let active = match *active >= issues.len() - 1 {
        true => issues.len() - 1,
        false => active + 1,
    };
    store.set("project.issue.active", Box::new(active));

    Ok(())
}

pub fn select_previous_issue(store: &mut Store) -> Result<(), Error> {
    let active = store.get::<usize>("project.issue.active")?;
    let active = match *active == 0 {
        true => 0,
        false => active - 1,
    };
    store.set("project.issue.active", Box::new(active));

    Ok(())
}
