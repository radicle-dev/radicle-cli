use tui::layout::{Constraint, Direction, Layout, Rect};

pub struct Padding {
    pub top: u16,
    pub left: u16,
}

pub fn inner_area(area: Rect, padding: Padding) -> Rect {
    Rect::new(
        area.x + padding.left,
        area.y + padding.top,
        area.width - padding.left * 2,
        area.height - padding.top * 2,
    )
}

pub fn split_area(area: Rect, lengths: Vec<u16>, direction: Direction) -> Vec<Rect> {
    let constraints = lengths
        .iter()
        .map(|l| Constraint::Length(*l))
        .collect::<Vec<_>>();
    Layout::default()
        .direction(direction)
        .constraints(constraints)
        .split(area)
}
