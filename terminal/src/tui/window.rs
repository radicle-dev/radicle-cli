use std::rc::Rc;

use anyhow::{Error, Result};

use tui::backend::Backend;
use tui::layout::{Direction, Rect};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Paragraph};
use tui::Frame;

use super::layout;
use super::layout::Padding;
use super::store::Store;
use super::template;
use super::theme::Theme;

/// Basic application window with layout for shortcut widget.
pub struct ApplicationWindow<B: Backend> {
    pub title: Rc<dyn Widget<B>>,
    pub shortcuts: Rc<dyn Widget<B>>,
}

impl<B> ApplicationWindow<B>
where
    B: Backend,
{
    /// Draw the application window to given `frame`.
    pub fn draw(&self, store: &Store, frame: &mut Frame<B>, theme: &Theme) -> Result<(), Error> {
        let title_h = self.title.height(frame.size());
        let shortcut_h = self.shortcuts.height(frame.size());
        let areas =
            layout::split_area(frame.size(), vec![title_h, shortcut_h], Direction::Vertical);

        self.title.draw(store, frame, areas[0], theme)?;
        self.shortcuts.draw(store, frame, areas[1], theme)?;
        Ok(())
    }
}

/// Trait that must be implemented by custom application widgets.
pub trait Widget<B: Backend> {
    /// Draw widget to `frame` on the `area` defined and the application `store`
    /// given. Called by the application if drawing is requested.
    fn draw(
        &self,
        store: &Store,
        frame: &mut Frame<B>,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), Error>;
    /// Return height of widget. Used while layouting.
    fn height(&self, area: Rect) -> u16;
}

/// An empty widget with no height. Can be used as placeholder.
#[derive(Copy, Clone)]
pub struct EmptyWidget;

impl<B> Widget<B> for EmptyWidget
where
    B: Backend,
{
    fn draw(
        &self,
        _store: &Store,
        frame: &mut Frame<B>,
        area: Rect,
        _theme: &Theme,
    ) -> Result<(), Error> {
        let block = Block::default().borders(Borders::NONE);
        frame.render_widget(block, area);

        Ok(())
    }

    fn height(&self, _area: Rect) -> u16 {
        0_u16
    }
}

/// A common title widget. Expects the property `app.title` to
/// be defined.
#[derive(Copy, Clone)]
pub struct TitleWidget;

impl<B> Widget<B> for TitleWidget
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
        let title = store.get::<String>("app.title")?;

        let (block, inner) = template::block(theme, area, Padding { top: 1, left: 4 }, true);
        frame.render_widget(block, area);

        let title = template::paragraph(title, theme.highlight_invert);
        frame.render_widget(title, inner);

        Ok(())
    }

    fn height(&self, _area: Rect) -> u16 {
        3_u16
    }
}

/// A common shortcut widget that will be drawn on the bottom
/// of the application frame. Expects the property `app.shortcuts` to
/// be defined.
#[derive(Copy, Clone)]
pub struct ShortcutWidget;

impl<B> Widget<B> for ShortcutWidget
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
        let shortcuts = store.get::<Vec<String>>("app.shortcuts")?;
        let lengths = shortcuts
            .iter()
            .map(|s| s.len() as u16 + 2)
            .collect::<Vec<_>>();

        let (_, inner) = template::block(theme, area, Padding { top: 1, left: 2 }, false);
        let areas = layout::split_area(inner, lengths, Direction::Horizontal);
        let mut areas = areas.iter();

        let shortcuts = shortcuts
            .iter()
            .map(|s| Span::styled(s, theme.ternary))
            .collect::<Vec<_>>();
        for shortcut in shortcuts {
            if let Some(area) = areas.next() {
                let paragraph = Paragraph::new(Spans::from(shortcut));
                frame.render_widget(paragraph, *area);
            }
        }

        Ok(())
    }

    fn height(&self, _area: Rect) -> u16 {
        3_u16
    }
}
