use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Error, Result};
use timeago;

use tui::backend::Backend;
use tui::layout::{Alignment, Direction, Rect};
use tui::style::{Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{ListItem, Tabs};
use tui::Frame;

use radicle_common::cobs::issue::{Issue, IssueId};
use radicle_terminal as term;

use term::tui::layout;
use term::tui::layout::Padding;
use term::tui::store::Store;
use term::tui::strings;
use term::tui::template;
use term::tui::theme::Theme;
use term::tui::window::Widget;

use super::state::Tab;

type IssueList = Vec<(IssueId, Issue)>;

#[derive(Clone)]
pub struct BrowserWidget<B: Backend> {
    pub tabs: Rc<dyn Widget<B>>,
}

impl<B> BrowserWidget<B>
where
    B: Backend,
{
    fn items(&self, _id: &IssueId, issue: &Issue, theme: &Theme) -> ListItem {
        let fmt = timeago::Formatter::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timeago = Duration::from_secs(now - issue.comment.timestamp.as_secs());

        let lines = vec![
            Spans::from(Span::styled(issue.title.clone(), theme.primary)),
            Spans::from(vec![
                Span::styled(
                    issue.author.name(),
                    theme.primary_dim.add_modifier(Modifier::ITALIC),
                ),
                Span::raw(strings::whitespaces(1)),
                Span::styled(
                    fmt.convert(timeago),
                    theme.ternary_dim.add_modifier(Modifier::ITALIC),
                ),
            ]),
        ];
        ListItem::new(lines)
    }
}

impl<B> Widget<B> for BrowserWidget<B>
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
        let open = store.get::<IssueList>("project.issue.open.list")?;
        let closed = store.get::<IssueList>("project.issue.closed.list")?;
        let (block, inner) = template::block(theme, area, Padding { top: 0, left: 2 }, true);
        frame.render_widget(block, area);

        if !open.is_empty() || !closed.is_empty() {
            let tab = store.get::<Tab>("app.browser.tab.active")?;
            let (issues, active) = match tab {
                Tab::Open => {
                    let active = store.get::<usize>("project.issue.open.active")?;
                    (open, active)
                }
                Tab::Closed => {
                    let active = store.get::<usize>("project.issue.closed.active")?;
                    (closed, active)
                }
            };
            let items: Vec<ListItem> = issues
                .iter()
                .map(|(id, issue)| self.items(id, issue, theme))
                .collect();

            let tab_h = self.tabs.height(inner);
            let heights = vec![tab_h, inner.height.saturating_sub(tab_h)];
            let areas = layout::split_area(inner, heights, Direction::Vertical);

            self.tabs.draw(store, frame, areas[0], theme)?;

            let (list, mut state) = template::list(items, *active, theme);
            frame.render_stateful_widget(list, areas[1], &mut state);
        } else {
            let message = String::from("No issues found");
            let message =
                template::paragraph(&message, Style::default()).alignment(Alignment::Center);
            frame.render_widget(message, inner);
        }

        Ok(())
    }

    fn height(&self, area: Rect) -> u16 {
        area.height
    }
}

#[derive(Clone)]
pub struct TabWidget;

impl<B> Widget<B> for TabWidget
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
        let open = store.get::<IssueList>("project.issue.open.list")?;
        let closed = store.get::<IssueList>("project.issue.closed.list")?;
        let active = store.get::<Tab>("app.browser.tab.active")?;
        let active = *active as usize;

        let tabs = vec![
            format!("{} open", open.len()),
            format!("{} closed", closed.len()),
        ];
        let divider = "|";

        let (_, inner) = template::block(theme, area, Padding { top: 1, left: 0 }, false);
        let tabs = tabs
            .iter()
            .map(|t| Spans::from(Span::styled(t, Style::default())))
            .collect();

        let tabs = Tabs::new(tabs)
            .style(theme.ternary_dim)
            .highlight_style(theme.ternary)
            .divider(divider)
            .select(active);
        frame.render_widget(tabs, inner);

        Ok(())
    }

    fn height(&self, _area: Rect) -> u16 {
        3_u16
    }
}
