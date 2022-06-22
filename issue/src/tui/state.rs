use std::convert::{TryFrom, TryInto};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TabError {
    #[error("{0}")]
    InvalidIndex(String),
}

#[derive(Clone, Copy, Debug)]
pub enum Tab {
    Open = 0,
    Closed = 1,
}

impl TryFrom<usize> for Tab {
    type Error = TabError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Tab::Open),
            1 => Ok(Tab::Closed),
            _ => Err(TabError::InvalidIndex("Tab index not allowed!".to_owned())),
        }
    }
}

impl TryInto<usize> for Tab {
    type Error = TabError;

    fn try_into(self) -> Result<usize, Self::Error> {
        Ok(self as usize)
    }
}
