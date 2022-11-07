//! User profile related functions.
use anyhow::{anyhow, Error, Result};

pub use radicle::profile::{self, home, Profile};

use crate::args;

/// Environment var that sets the radicle home directory.
pub const RAD_HOME: &str = "RAD_HOME";

/// Get the default profile. Fails if there is no profile.
pub fn default() -> Result<Profile, Error> {
    let error = args::Error::WithHint {
        err: anyhow!("Could not load radicle profile"),
        hint: "To setup your radicle profile, run `rad auth`.",
    };

    match Profile::load() {
        Ok(profile) => Ok(profile),
        Err(profile::Error::NotFound(_)) => Err(error.into()),
        Err(err) => anyhow::bail!(err),
    }
}
