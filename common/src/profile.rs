//! User profile related functions.
use std::{env, fmt, path};

use anyhow::{anyhow, Error, Result};
use serde::{de::DeserializeOwned, Serialize};

pub use librad::profile::{LnkHome, Profile, ProfileId};

use librad::crypto::{
    keystore::{FileStorage, Keystore as _},
    PublicKey, SecretKey,
};
use librad::PeerId;
use librad::{git::storage::ReadOnly, git::Storage, keystore::crypto::Crypto};

use crate::args;
use crate::keys;

/// Environment var that sets the radicle home directory.
pub const RAD_HOME: &str = "RAD_HOME";

/// Create a new profile.
pub fn create<C: Crypto>(home: impl Into<LnkHome>, crypto: C) -> Result<(Profile, PeerId)>
where
    C::Error: fmt::Debug + fmt::Display + Send + Sync + 'static,
    C::SecretBox: Serialize + DeserializeOwned,
{
    let home = home.into();
    let profile = Profile::new(&home)?;

    Profile::set(&home, profile.id().clone())?;

    let key = SecretKey::new();
    let mut store: FileStorage<C, PublicKey, SecretKey, _> =
        FileStorage::new(&profile.paths().keys_dir().join(keys::KEY_FILE), crypto);

    store.put_key(key.clone())?;
    Storage::open(profile.paths(), key.clone())?;

    Ok((profile, PeerId::from(key)))
}

/// Get the radicle home.
pub fn home() -> LnkHome {
    let home = env::var(RAD_HOME)
        .map(|h| LnkHome::Root(path::Path::new(&h).to_path_buf()))
        .ok();

    home.unwrap_or_default()
}

/// Get the default profile. Fails if there is no profile.
pub fn default() -> Result<Profile, Error> {
    let error = args::Error::WithHint {
        err: anyhow!("Could not load radicle profile"),
        hint: "To setup your radicle profile, run `rad auth`.",
    };

    let not_active_error = args::Error::WithHint {
        err: anyhow!("Could not load active radicle profile"),
        hint: "To setup your radicle profile, run `rad auth --init`.",
    };

    match Profile::active(&home()) {
        Ok(Some(profile)) => Ok(profile),
        Ok(None) => Err(not_active_error.into()),
        Err(_) => Err(error.into()),
    }
}

/// Get a profile's name. If none is given, get the default profile's name.
pub fn name(profile: Option<&Profile>) -> Result<String, Error> {
    let default = default()?;
    let read_only = read_only(profile.unwrap_or(&default))?;
    let config = read_only.config()?;

    Ok(config.user_name()?)
}

/// List all profiles.
pub fn list() -> Result<Vec<Profile>, Error> {
    Profile::list(&home()).map_err(|e| e.into())
}

/// Get the count of all profiles.
pub fn count() -> Result<usize, Error> {
    let profiles = list()?;

    Ok(profiles.len())
}

/// Set the default profile.
pub fn set(id: &ProfileId) -> Result<(), Error> {
    Profile::set(&home(), id.clone())?;

    Ok(())
}

/// Open read-only storage.
pub fn read_only(profile: &Profile) -> Result<ReadOnly, Error> {
    let storage = ReadOnly::open(profile.paths())?;

    Ok(storage)
}
