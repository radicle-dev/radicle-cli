use std::fmt;
use std::path::Path;
use std::{env, error};

use serde::{de::DeserializeOwned, Serialize};

use librad::crypto::keystore::crypto;
use librad::crypto::keystore::crypto::{Pwhash, KDF_PARAMS_TEST};
use librad::crypto::keystore::pinentry::SecUtf8;
use librad::crypto::keystore::FileStorage;
use librad::crypto::BoxedSigner;
use librad::git::identities::local::LocalIdentity;
use librad::git::identities::Project;

use librad::git::Storage;
use librad::keystore::crypto::Crypto;
use librad::keystore::Keystore;
use librad::profile::{Profile, LNK_HOME};
use librad::PublicKey;

use super::{keys, person, profile, project, signer, test};

pub type BoxedError = Box<dyn error::Error>;

pub const USER_PASS: &str = "password";

pub mod setup {
    use super::*;

    pub fn lnk_home() -> Result<(), BoxedError> {
        env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        env::set_var(keys::RAD_PASSPHRASE, USER_PASS);
        Ok(())
    }

    pub fn profile() -> (Storage, Profile, LocalIdentity, Project) {
        let tempdir = env::temp_dir().join("rad").join("home");
        let home = env::var(LNK_HOME)
            .map(|s| Path::new(&s).to_path_buf())
            .unwrap_or_else(|_| tempdir.to_path_buf());

        env::set_var(LNK_HOME, home);

        let name = "cloudhead";
        let pass = Pwhash::new(SecUtf8::from(test::USER_PASS), *KDF_PARAMS_TEST);
        let (profile, _peer_id) = profile::create(profile::home(), pass.clone()).unwrap();
        let signer = test::signer(&profile, pass).unwrap();
        let storage = keys::storage(&profile, signer.clone()).unwrap();
        let person = person::create(&profile, name, signer, &storage).unwrap();

        person::set_local(&storage, &person).unwrap();

        let whoami = person::local(&storage).unwrap();
        let payload = project::payload(
            "nakamoto".to_owned(),
            "Bitcoin light-client".to_owned(),
            "master".to_owned(),
        );
        let project = project::create(payload, &storage).unwrap();

        (storage, profile, whoami, project)
    }
}

pub mod teardown {
    use super::*;
    pub fn profiles() -> Result<(), BoxedError> {
        #[cfg(debug_assertions)]
        let params = *crypto::KDF_PARAMS_TEST;
        #[cfg(not(debug_assertions))]
        let params = crypto::KdfParams::recommended();

        if let Ok(profiles) = profile::list() {
            for profile in profiles {
                let pass = crypto::Pwhash::new(SecUtf8::from(test::USER_PASS), params);
                keys::remove(&profile, pass, keys::ssh_auth_sock()?).ok();
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
