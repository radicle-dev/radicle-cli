//! Common radicle utilities.
pub mod git;
pub mod keys;
pub mod patch;
pub mod person;
pub mod profile;
pub mod project;
pub mod seed;
pub mod test;

#[cfg(feature = "ethereum")]
pub mod ethereum;

pub use lnk_identities as identities;
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
