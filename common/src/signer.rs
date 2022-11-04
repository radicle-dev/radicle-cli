use zeroize::Zeroizing;

use crate::keys;
use crate::keys::link_crypto::BoxedSignError;
use crate::keys::link_crypto::BoxedSigner;
use crate::keys::link_crypto::SecretKey;
use crate::keys::radicle_keystore as ed25519;
use crate::keys::ssh_super::SshAuthSock;

use radicle::profile::Profile;

/// A trait for types that can be converted to signers.
pub trait ToSigner {
    /// Convert to a signer.
    fn to_signer(self, profile: &Profile) -> Result<BoxedSigner, keys::ssh::Error>;
}

impl ToSigner for BoxedSigner {
    fn to_signer(self, _profile: &Profile) -> Result<BoxedSigner, keys::ssh::Error> {
        Ok(self)
    }
}

impl ToSigner for SshAuthSock {
    fn to_signer(self, profile: &Profile) -> Result<BoxedSigner, keys::ssh::Error> {
        let signer = keys::ssh::signer(profile, self)?;
        Ok(signer)
    }
}

impl ToSigner for SecretKey {
    fn to_signer(self, _profile: &Profile) -> Result<BoxedSigner, keys::ssh::Error> {
        Ok(self.into())
    }
}

impl ToSigner for ZeroizingSecretKey {
    fn to_signer(self, _profile: &Profile) -> Result<BoxedSigner, keys::ssh::Error> {
        Ok(BoxedSigner::new(self))
    }
}

/// Secret key that is zeroed when dropped.
#[derive(Clone)]
pub struct ZeroizingSecretKey {
    key: Zeroizing<SecretKey>,
}

impl ZeroizingSecretKey {
    pub fn new(key: SecretKey) -> Self {
        Self {
            key: Zeroizing::new(key),
        }
    }
}

impl ed25519::SignerTrait for ZeroizingSecretKey {
    type Error = BoxedSignError;

    fn public_key(&self) -> ed25519::PublicKey {
        self.key.public_key()
    }

    fn sign(&self, data: &[u8]) -> Result<ed25519::Signature, Self::Error> {
        <SecretKey as ed25519::SignerTrait>::sign(&self.key, data)
            .map_err(BoxedSignError::from_std_error)
    }
}

impl keys::link_crypto::Signer for ZeroizingSecretKey {
    fn sign_blocking(
        &self,
        data: &[u8],
    ) -> Result<ed25519::Signature, <Self as ed25519::SignerTrait>::Error> {
        self.key
            .sign_blocking(data)
            .map_err(BoxedSignError::from_std_error)
    }
}
