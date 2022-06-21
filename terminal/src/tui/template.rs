use tui::layout::Rect;
use tui::widgets::{Block, Borders};

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