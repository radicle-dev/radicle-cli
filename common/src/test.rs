use std::fmt;
use std::{env, error};

use serde::{de::DeserializeOwned, Serialize};

use librad::crypto::keystore::crypto;
use librad::crypto::keystore::pinentry::SecUtf8;
use librad::crypto::keystore::FileStorage;
use librad::crypto::BoxedSigner;
use librad::keystore::crypto::Crypto;
use librad::keystore::Keystore;
use librad::profile::{Profile, LNK_HOME};
use librad::PublicKey;

use super::{keys, profile, signer, test};

pub type BoxedError = Box<dyn error::Error>;

pub const USER_PASS: &str = "password";

pub mod setup {
    use super::*;
    pub fn lnk_home() -> Result<(), BoxedError> {
        env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        Ok(())
    }
}

pub mod teardown {
    use super::*;
    pub fn profiles() -> Result<(), BoxedError> {
        #[cfg(test)]
        let params = *crypto::KDF_PARAMS_TEST;
        #[cfg(not(test))]
        let params = crypto::KdfParams::recommended();

        if let Ok(profiles) = profile::list() {
            for profile in profiles {
                let pass = crypto::Pwhash::new(SecUtf8::from(test::USER_PASS), params);
                keys::remove(&profile, pass, keys::ssh_auth_sock()?)?;
            }
        }
        Ok(())
    }
}

/// Signer useful for testing.
pub fn signer<C: Crypto>(profile: &Profile, crypto: C) -> Result<BoxedSigner, anyhow::Error>
where
    C::Error: fmt::Debug + fmt::Display + Send + Sync + 'static,
    C::SecretBox: Serialize + DeserializeOwned,
{
    let file_storage: FileStorage<_, PublicKey, _, _> =
        FileStorage::new(&profile.paths().keys_dir().join(keys::KEY_FILE), crypto);
    let keystore = file_storage.get_key()?;

    Ok(BoxedSigner::new(signer::ZeroizingSecretKey::new(
        keystore.secret_key,
    )))
}
