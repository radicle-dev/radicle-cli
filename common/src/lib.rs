//! Common radicle utilities.
#![allow(clippy::or_fun_call)]
pub mod args;
pub mod cobs;
pub mod git;
pub mod keys;
pub mod logger;
pub mod patch;
pub mod person;
pub mod profile;
pub mod project;
pub mod seed;
pub mod signer;
pub mod test;

#[cfg(feature = "ethereum")]
pub mod ethereum;

pub use lnk_identities as identities;
pub use serde_json as json;
pub use url::Url;

/// String formatting of various types.
pub mod fmt {
    use librad::PeerId;

    /// Format a peer id to be more compact.
    pub fn peer(peer: &PeerId) -> String {
        let peer = peer.to_string();
        let start = peer.chars().take(7).collect::<String>();
        let end = peer.chars().skip(peer.len() - 7).collect::<String>();

        format!("{}…{}", start, end)
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Interactive {
    Yes,
    No,
}

impl Default for Interactive {
    fn default() -> Self {
        Interactive::No
    }
}

impl Interactive {
    pub fn yes(&self) -> bool {
        (*self).into()
    }

    pub fn no(&self) -> bool {
        !self.yes()
    }
}

impl From<Interactive> for bool {
    fn from(c: Interactive) -> Self {
        match c {
            Interactive::Yes => true,
            Interactive::No => false,
        }
    }
}

impl From<bool> for Interactive {
    fn from(b: bool) -> Self {
        if b {
            Interactive::Yes
        } else {
            Interactive::No
        }
    }
}
