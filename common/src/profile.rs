//! User profile related functions.

use anyhow::{anyhow, Error, Result};

pub use radicle::profile::{home, Profile};

use crate::args;

/// Environment var that sets the radicle home directory.
pub const RAD_HOME: &str = "RAD_HOME";

/// Get the default profile. Fails if there is no profile.
pub fn default() -> Result<Profile, Error> {
    let error = args::Error::WithHint {
        err: anyhow!("Could not load radicle profile"),
        hint: "To setup your radicle profile, run `rad auth`.",
    };

    // TODO(dave): what to do with this?
    let _not_active_error = args::Error::WithHint {
        err: anyhow!("Could not load active radicle profile"),
        hint: "To setup your radicle profile, run `rad auth --init`.",
    };

    match Profile::load() {
        Ok(profile) => Ok(profile),
        //Ok(None) => Err(not_active_error.into()),
        Err(_) => Err(error.into()),
    }
}
