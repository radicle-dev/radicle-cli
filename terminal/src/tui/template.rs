use tui::layout::Rect;
use tui::style::Style;
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use super::layout;
use super::layout::Padding;
use super::theme::Theme;

pub fn block(theme: &Theme, area: Rect, padding: Padding, borders: bool) -> (Block, Rect) {
    let borders = match borders {
        true => theme.border.borders,
        false => Borders::NONE,
    };
    let block = Block::default()
        .borders(borders)
        .border_style(theme.border.style)
        .border_type(theme.border.border_type);
    let padding = match theme.border.borders {
        Borders::NONE => padding,
        _ => Padding {
            top: padding.top,
            left: padding.left,
        },
    };

    let inner = layout::inner_area(area, padding);
    (block, inner)
}

pub fn paragraph(text: &String, style: Style) -> Paragraph {
    let text = format!("{:1}{}{:1}", "", text, "");
    let text = Span::styled(text, style);

    Paragraph::new(vec![Spans::from(text)])
}

pub fn paragraph_styled(text: &String, style: Style) -> Paragraph {
    let text = format!("{:1}{}{:1}", "", text, "");
    let text = Span::styled(text, style);

    Paragraph::new(vec![Spans::from(text)]).style(style)
}

pub fn list<'a>(
    items: Vec<ListItem<'a>>,
    selected: usize,
    theme: &'a Theme,
) -> (List<'a>, ListState) {
    let mut state = ListState::default();
    state.select(Some(selected));

    let items = List::new(items)
        .highlight_style(theme.highlight)
        .highlight_symbol(&theme.list.symbol)
        .repeat_highlight_symbol(true);

    (items, state)
}
