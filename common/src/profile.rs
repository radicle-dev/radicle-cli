//! User profile related functions.
use anyhow::{anyhow, Error, Result};

use librad::git::storage::ReadOnly;
pub use librad::profile::{Profile, ProfileId};

use lnk_profile;

/// Get the default profile. Fails if there is no profile.
pub fn default() -> Result<Profile, Error> {
    use rad_terminal::args;

    let error = args::Error::WithHint {
        err: anyhow!("failed to load radicle profile"),
        hint: "To setup your radicle profile, run `rad auth`.",
    };

    match lnk_profile::get(None, None) {
        Ok(Some(profile)) => Ok(profile),
        Ok(None) | Err(_) => Err(error.into()),
    }
}

/// List all profiles.
pub fn list() -> Result<Vec<Profile>, Error> {
    lnk_profile::list(None).map_err(|e| e.into())
}

/// Set the default profile.
pub fn set(id: &ProfileId) -> Result<(), Error> {
    lnk_profile::set(None, id.clone())?;

    Ok(())
}

/// Open read-only storage.
pub fn read_only(profile: &Profile) -> Result<ReadOnly, Error> {
    let storage = ReadOnly::open(profile.paths())?;

    Ok(storage)
}
