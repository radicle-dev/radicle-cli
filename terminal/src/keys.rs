use librad::crypto::keystore::pinentry::{Pinentry, SecUtf8};

pub use radicle_common::keys::*;
pub use radicle_common::signer;

#[derive(Clone)]
pub struct CachedPrompt(pub SecUtf8);

impl CachedPrompt {
    pub fn new(secret: SecUtf8) -> Self {
        Self(secret)
    }
}

impl Pinentry for CachedPrompt {
    type Error = std::io::Error;

    fn get_passphrase(&self) -> Result<SecUtf8, Self::Error> {
        Ok(self.0.clone())
    }
}
