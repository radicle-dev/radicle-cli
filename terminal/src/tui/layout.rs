use tui::layout::{Constraint, Direction, Layout, Rect};

pub fn split_area(area: Rect, lengths: Vec<u16>, direction: Direction) -> Vec<Rect> {
    let constraints = lengths
        .iter()
        .map(|l| Constraint::Length(*l))
        .collect::<Vec<_>>();
    Layout::default().direction(direction).constraints(constraints).split(area)
}