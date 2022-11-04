//! User profile related functions.
use std::fs;
use std::{env, fmt, path};

use anyhow::{anyhow, Error, Result};
use serde::{de::DeserializeOwned, Serialize};

use librad::profile::Profile as OldProfile;

pub use radicle::profile::{home, Profile};

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

/// Get the default profile. Fails if there is no profile.
pub fn default() -> Result<Profile, Error> {
    let error = args::Error::WithHint {
        err: anyhow!("Could not load radicle profile"),
        hint: "To setup your radicle profile, run `rad auth`.",
    };

    // TODO(dave): what to do with this?
    let not_active_error = args::Error::WithHint {
        err: anyhow!("Could not load active radicle profile"),
        hint: "To setup your radicle profile, run `rad auth --init`.",
    };

    match Profile::load() {
        Ok(profile) => Ok(profile),
        //Ok(None) => Err(not_active_error.into()),
        Err(_) => Err(error.into()),
    }
}
