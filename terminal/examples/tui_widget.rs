use std::collections::HashMap;
use std::rc::Rc;

use anyhow::{Error, Result};
use lazy_static::lazy_static;

use tui::backend::Backend;
use tui::layout::Rect;
use tui::style::Style;
use tui::Frame;

use radicle_terminal as term;

use term::tui::events::{InputEvent, Key};
use term::tui::layout::Padding;
use term::tui::store::Store;
use term::tui::template;
use term::tui::theme::Theme;
use term::tui::window::{PageWidget, ShortcutWidget, TitleWidget, Widget};
use term::tui::{Application, State};

#[derive(Clone, Eq, PartialEq)]
pub enum Action {
    Quit,
}

lazy_static! {
    static ref KEY_BINDINGS: HashMap<Key, Action> =
        [(Key::Char('q'), Action::Quit)].iter().cloned().collect();
}

fn main() -> Result<(), Error> {
    // Create basic application that will call `update` on
    // every input event received from event thread.
    let mut application = Application::new(&on_action).store(vec![
        ("app.shortcuts", Box::new(vec![String::from("q quit")])),
        ("app.title", Box::new(String::from("tui-example"))),
        ("app.content", Box::new(String::from("Hello Tui!"))),
    ]);

    // Create a single-page application
    let pages = vec![PageWidget {
        title: Rc::new(TitleWidget),
        widgets: vec![Rc::new(ExampleWidget)],
        shortcuts: Rc::new(ShortcutWidget),
    }];

    // Use default, borderless theme
    let theme = Theme::default_dark();

    // Run application
    application.execute(pages, &theme)?;
    Ok(())
}

fn on_action(store: &mut Store, event: &InputEvent) -> anyhow::Result<(), anyhow::Error> {
    // Set application set to `State::Exiting` when the key 'q' is received.
    // Note that any special tick handling is ignored for now.
    if let InputEvent::Input(key) = *event {
        if let Some(action) = KEY_BINDINGS.get(&key) {
            match action {
                Action::Quit => store.set("app.state", Box::new(State::Exiting)),
            }
        }
    }
    Ok(())
}

/// A widget example.
#[derive(Copy, Clone)]
pub struct ExampleWidget;

impl<B> Widget<B> for ExampleWidget
where
    B: Backend,
{
    fn draw(
        &self,
        store: &Store,
        frame: &mut Frame<B>,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), Error> {
        let content = store.get::<String>("app.content")?;

        let (block, inner) = template::block(theme, area, Padding { top: 1, left: 2 }, true);
        frame.render_widget(block, area);

        let title = template::paragraph(content, Style::default());
        frame.render_widget(title, inner);

        Ok(())
    }

    fn height(&self, _area: Rect) -> u16 {
        3_u16
    }
}
