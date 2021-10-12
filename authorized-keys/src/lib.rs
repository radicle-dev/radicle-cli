pub mod error;
pub mod rad_keys;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use async_trait::async_trait;
use structopt::StructOpt;

/// List of authorized key IDs
pub type KeyRing = Vec<String>;

#[async_trait]
pub trait RadKeyring {
    type Error;
    async fn add(
        &mut self,
        key_type: KeyType,
        key_id: String,
        pub_key: &[u8],
    ) -> Result<(), Self::Error>;
    async fn remove(&mut self, key_type: KeyType, key_id: String) -> Result<(), Self::Error>;
    async fn keyring(&self, key_type: KeyType) -> Result<KeyRing, Self::Error>;
    async fn exists(&self, key_type: KeyType, key_id: String) -> Result<bool, Self::Error>;
}

#[derive(Debug, Clone)]
pub enum KeyRingSource {
    RadKeys,
    RadId,
    ENS,
    Unsupported,
}

impl From<&str> for KeyRingSource {
    fn from(s: &str) -> KeyRingSource {
        match s.to_lowercase().as_ref() {
            "radkeys" => KeyRingSource::RadKeys,
            "radid" => KeyRingSource::RadId,
            "ens" => KeyRingSource::ENS,
            _ => KeyRingSource::Unsupported,
        }
    }
}

#[derive(Debug, Clone, StructOpt)]
pub enum Action {
    /// add a public key to the keyring source
    Add,
    /// remove a public key from the keyring source
    Remove,
    /// list the public keys in the keyring source
    List,
}

/// Supported key types for access control verification;
#[derive(Debug, Clone)]
pub enum KeyType {
    OpenPgp,
    Ed25519,
    Eip155(u64),
    Unsupported,
}

impl From<&str> for KeyType {
    fn from(s: &str) -> KeyType {
        let t = s.to_lowercase();

        // check for Eip155 Chain ID;
        if t.contains("eip155") {
            match t.replace("eip155:", "").parse::<u64>() {
                Ok(chain_id) => return KeyType::Eip155(chain_id),
                Err(_) => return KeyType::Unsupported,
            }
        }

        match t.as_ref() {
            "openpgp" => KeyType::OpenPgp,
            "ed25519" => KeyType::Ed25519,
            _ => KeyType::Unsupported,
        }
    }
}

impl ToString for KeyType {
    fn to_string(&self) -> String {
        match self {
            KeyType::OpenPgp => "openpgp".to_string(),
            KeyType::Ed25519 => "ed25519".to_string(),
            KeyType::Eip155(chain_id) => format!("eip155:{}", chain_id),
            KeyType::Unsupported => "unsupported".to_string(),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "rad-auth-keys",
    about = "Radicle authorized keys CLI tool for managing Radicle git repository authorized keys."
)]
pub struct Options {
    /// Actions to perform by the CLI, options include `add`, `remove` or `list`.
    #[structopt(subcommand)]
    pub action: Action,

    /// Optional, the signing key id (fingerprint) to add or remove to authorized keys list; required for `remove`.
    #[structopt(short, long)]
    pub id: Option<String>,

    /// Optional, the signing key type used for authenticating a request, e.g. `openpgp`, `eip155:<chain_id>`, `ed25519`;
    /// defaults to `openpgp`.
    #[structopt(short, long, parse(from_str))]
    pub key_type: Option<KeyType>,

    /// Optional, the source of the keyring to modify, e.g. `radkeys`, `radid`, `ens`; defaults to `radkeys`.
    #[structopt(short, long, parse(from_str))]
    pub source: Option<KeyRingSource>,

    /// Optional, the path to the public key, otherwise will accept standard input for the public key.
    #[structopt(short, long, parse(from_os_str))]
    pub path: Option<PathBuf>,

    /// Optional, the path for parent directory of `.rad/` directory, defaults to `std::env::current_dir()?`.
    #[structopt(short, long, parse(from_os_str))]
    pub dir: Option<PathBuf>,
}
