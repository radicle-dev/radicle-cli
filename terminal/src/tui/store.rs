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
}
