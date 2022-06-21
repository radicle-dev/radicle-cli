use std::rc::Rc;

use anyhow::{Error, Result};

use tui::backend::Backend;
use tui::layout::{Direction, Rect};
use tui::widgets::{Block, Borders};
use tui::Frame;

use super::layout;
use super::store::Store;

/// Basic application window with layout for shortcut widget.
pub struct ApplicationWindow<B: Backend> {
    pub shortcuts: Rc<dyn Widget<B>>,
}

impl<B> ApplicationWindow<B>
where
    B: Backend,
{
    /// Draw the application window to given `frame`.
    pub fn draw(&self, frame: &mut Frame<B>, store: &Store) -> Result<(), Error> {
        let shortcut_h = self.shortcuts.height(frame.size());
        let areas = layout::split_area(frame.size(), vec![shortcut_h], Direction::Vertical);

        self.shortcuts.draw(frame, areas[0], store)?;
        Ok(())
    }
}

/// Trait that must be implemented by custom application widgets.
pub trait Widget<B: Backend> {
    /// Draw widget to `frame` on the `area` defined and the application `store`
    /// given. Called by the application if drawing is requested.
    fn draw(&self, frame: &mut Frame<B>, area: Rect, store: &Store) -> Result<(), Error>;
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
    fn draw(&self, frame: &mut Frame<B>, area: Rect, _store: &Store) -> Result<(), Error> {
        let block = Block::default().borders(Borders::NONE);
        frame.render_widget(block, area);

        Ok(())
    }

    fn height(&self, _area: Rect) -> u16 {
        0_u16
    }
}
