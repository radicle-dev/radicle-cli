use tuirealm::props::{AttrValue, Attribute};
use tuirealm::tui::layout::Rect;

use tuirealm::tui::layout::{Constraint, Direction, Layout};
use tuirealm::MockComponent;

type BoxedComponent = Box<dyn MockComponent>;

/// Trait that should be implemented by layouts that take multiple components
/// and build a tui-realm layout based on the components and their size attributes.
pub trait ComponentLayout {
    fn build(self) -> Vec<(BoxedComponent, Rect)>;
}

/// A layout that packs components horizontally based on their width.
pub struct HorizontalLayout {
    components: Vec<BoxedComponent>,
    area: Rect,
}

impl HorizontalLayout {
    pub fn new(components: Vec<BoxedComponent>, area: Rect) -> Self {
        Self { components, area }
    }
}

impl ComponentLayout for HorizontalLayout {
    fn build(self) -> Vec<(BoxedComponent, Rect)> {
        let constraints = self
            .components
            .iter()
            .map(|c| {
                Constraint::Length(
                    c.query(Attribute::Width)
                        .unwrap_or(AttrValue::Size(0))
                        .unwrap_size(),
                )
            })
            .collect::<Vec<_>>();
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(self.area);

        self.components
            .into_iter()
            .zip(layout.into_iter())
            .collect()
    }
}
