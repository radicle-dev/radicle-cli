/// List of items that keeps track of the selection.
pub struct Items<T> {
    values: Vec<T>,
    selected: usize,
}

impl<T> Items<T> {
    pub fn all(&self) -> &Vec<T> {
        &self.values
    }

    pub fn selected(&self) -> Option<&T> {
        self.values.get(self.selected)
    }

    pub fn select_index(&mut self, index: usize) {
        self.selected = index
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }
}

/// List of items that won't cycle through the selection,
/// but rather stops selecting a new item at the beginning
/// and the end of the list.
pub struct ListState<T> {
    items: Items<T>,
}

impl<T> ListState<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items: Items {
                values: items,
                selected: 0,
            },
        }
    }

    pub fn items(&self) -> &Items<T> {
        &self.items
    }

    pub fn select_previous(&mut self) {
        let index = match self.items.selected_index() == 0 {
            true => 0,
            false => self.items.selected_index() - 1,
        };
        self.items.select_index(index);
    }

    pub fn select_next(&mut self) {
        let len = self.items.all().len();
        let index = match self.items.selected_index() >= len - 1 {
            true => len - 1,
            false => self.items.selected_index() + 1,
        };
        self.items.select_index(index);
    }
}
