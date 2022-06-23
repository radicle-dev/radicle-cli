use std::any::Any;
use std::collections::HashMap;

use anyhow::Result;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Invalid type requested: {0}")]
    InvalidType(String),
    #[error("Property not found: {0}")]
    NotFound(String),
}

/// A generic value container that stores any type.
pub type Value = Box<dyn Any>;

/// The store properties of a tui-application accessible
/// by a key of type `String`.
#[derive(Default)]
pub struct Store {
    properties: HashMap<String, Value>,
}

impl Store {
    /// Set a value for given property. Overwrite if it already exists.
    pub fn set(&mut self, key: &str, value: Value) {
        self.properties.insert(String::from(key), value);
    }

    /// Return the value of given property or an error if property does
    /// not exist or can't be casted to given type.
    pub fn get<T: Any>(&self, key: &str) -> Result<&T, StoreError> {
        match self.properties.get(key) {
            Some(prop) => match prop.downcast_ref::<T>() {
                Some(value) => Ok(value),
                None => Err(StoreError::InvalidType(key.to_owned())),
            },
            None => Err(StoreError::NotFound(key.to_owned())),
        }
    }

    /// Return a mutable value for given property or an error if property does
    /// not exist or can't be casted to given type.
    pub fn get_mut<T: Any>(&mut self, key: &str) -> Result<&mut T, StoreError> {
        match self.properties.get_mut(key) {
            Some(prop) => match prop.downcast_mut::<T>() {
                Some(value) => Ok(value),
                None => Err(StoreError::InvalidType(key.to_owned())),
            },
            None => Err(StoreError::NotFound(key.to_owned())),
        }
    }
}

/// List of items that keeps track of the selection.
pub struct Items<T> {
    values: Vec<T>,
    index: usize,
}

impl<T> Items<T> {
    pub fn all(&self) -> &Vec<T> {
        &self.values
    }

    pub fn selected(&self) -> Option<&T> {
        self.values.get(self.index)
    }

    pub fn select_index(&mut self, index: usize) {
        self.index = index
    }

    pub fn selected_index(&self) -> usize {
        self.index
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }
}

/// List of items that cycles through the selection.
pub struct TabProperty<T> {
    items: Items<T>,
}

impl<T> TabProperty<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items: Items {
                values: items,
                index: 0,
            },
        }
    }

    pub fn items(&self) -> &Items<T> {
        &self.items
    }

    pub fn select_previous(&mut self) {
        let len = self.items.all().len();
        let index = match self.items.selected_index() == 0 {
            true => len - 1,
            false => self.items.selected_index() - 1,
        };
        self.items.select_index(index);
    }

    pub fn select_next(&mut self) {
        let len = self.items.all().len();
        let index = match self.items.selected_index() >= len - 1 {
            true => 0,
            false => self.items.selected_index() + 1,
        };
        self.items.select_index(index);
    }
}

/// List of items that won't cycle through the selection,
/// but rather stops selecting a new item at the beginning
/// and the end of the list.
pub struct ListProperty<T> {
    items: Items<T>,
}

impl<T> ListProperty<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items: Items {
                values: items,
                index: 0,
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
