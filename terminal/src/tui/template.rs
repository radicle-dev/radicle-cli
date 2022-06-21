use tui::layout::Rect;
use tui::style::Style;
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Paragraph};

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
