use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Error, Result};
use timeago;

use tui::backend::Backend;
use tui::layout::{Alignment, Rect};
use tui::style::{Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{ListItem};
use tui::Frame;

use radicle_common::cobs::issue::{Issue, IssueId};
use radicle_terminal as term;

use term::tui::layout::Padding;
use term::tui::store::Store;
use term::tui::strings;
use term::tui::template;
use term::tui::theme::Theme;
use term::tui::window::Widget;

type IssueList = Vec<(IssueId, Issue)>;

#[derive(Clone)]
pub struct BrowserWidget;

impl BrowserWidget {
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

impl<B> Widget<B> for BrowserWidget
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
        let issues = store.get::<IssueList>("project.issue.list")?;
        let active = store.get::<usize>("project.issue.active")?;

        let (block, inner) = template::block(theme, area, Padding { top: 1, left: 2 }, true);
        frame.render_widget(block, area);

        if !issues.is_empty() {
            let items: Vec<ListItem> = issues
                .iter()
                .map(|(id, issue)| self.items(&id, &issue, &theme))
                .collect();

            let (list, mut state) = template::list(items, *active, &theme);
            frame.render_stateful_widget(list, inner, &mut state);
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