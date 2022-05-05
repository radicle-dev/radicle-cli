use librad::crypto::BoxedSigner;
use librad::profile::Profile;
use librad::SecretKey;

use lnk_clib::keys;
use lnk_clib::keys::ssh::SshAuthSock;

use rad_terminal::keys::ZeroizingSecretKey;

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
