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

/// Application window with layout that supports multiple pages.
/// Expects the property `app.page.active` to be defined.
pub struct ApplicationWindow<B: Backend> {
    pub pages: Vec<PageWidget<B>>,
}

impl<B> ApplicationWindow<B>
where
    B: Backend,
{
    /// Draw the application window to given `frame`.
    pub fn draw(&self, store: &Store, frame: &mut Frame<B>, theme: &Theme) -> Result<(), Error> {
        let page_h = frame.size().height;
        let areas = layout::split_area(frame.size(), vec![page_h], Direction::Vertical);

        self.draw_active_page(store, frame, areas[0], theme)?;

        Ok(())
    }

    pub fn draw_active_page(
        &self,
        store: &Store,
        frame: &mut Frame<B>,
        area: Rect,
        theme: &Theme,
    ) -> Result<(), Error> {
        let active = store.get::<usize>("app.page.active")?;
        if let Some(page) = self.pages.get(*active) {
            page.draw(store, frame, area, theme)?;
        }
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

/// A common page widget that can hold a title and arbitrary child
/// widgets.
#[derive(Clone)]
pub struct PageWidget<B: Backend> {
    pub title: Rc<dyn Widget<B>>,
    pub widgets: Vec<Rc<dyn Widget<B>>>,
    pub shortcuts: Rc<dyn Widget<B>>,
}

impl<B> Widget<B> for PageWidget<B>
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
        let title_h = self.title.height(area);
        let shortcut_h = self.shortcuts.height(area);

        let area_h = area.height.saturating_sub(title_h + shortcut_h);
        let widget_h = area_h.checked_div(self.widgets.len() as u16).unwrap_or(0);

        let lengths = [
            vec![title_h],
            vec![widget_h; self.widgets.len()],
            vec![shortcut_h],
        ]
        .concat();

        let areas = layout::split_area(area, lengths, Direction::Vertical);
        let mut areas = areas.iter();

        if let Some(area) = areas.next() {
            self.title.draw(store, frame, *area, theme)?;
        }
        for widget in &self.widgets {
            if let Some(area) = areas.next() {
                widget.draw(store, frame, *area, theme)?;
            }
        }
        if let Some(area) = areas.next() {
            self.shortcuts.draw(store, frame, *area, theme)?;
        }

        Ok(())
    }

    fn height(&self, area: Rect) -> u16 {
        area.height
    }
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
