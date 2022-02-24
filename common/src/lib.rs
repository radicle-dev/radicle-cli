pub mod git;
pub mod keys;
pub mod person;
pub mod profile;
pub mod project;
pub mod seed;

#[cfg(feature = "ethereum")]
pub mod ethereum;

pub use rad_identities as identities;
pub use url::Url;

pub mod fmt {
    use librad::PeerId;

    pub fn peer(peer: &PeerId) -> String {
        let peer = peer.to_string();
        let start = peer.chars().take(7).collect::<String>();
        let end = peer.chars().skip(peer.len() - 7).collect::<String>();

        format!("{}…{}", start, end)
    }
}
