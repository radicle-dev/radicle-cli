use std::path::PathBuf;

use anyhow::{anyhow, Context as _, Error, Result};

use librad::crypto::peer::PeerId;
use librad::git::storage::Storage;
use librad::git::Urn;

pub use librad::profile::{Profile, ProfileId, RadHome};

use rad_profile;

pub fn default() -> Result<Profile, Error> {
    match rad_profile::get(None, None) {
        Ok(Some(profile)) => Ok(profile),
        Ok(None) => Err(anyhow!("could not get active profile")),
        Err(err) => Err(err).context("could not get active profile"),
    }
}

pub fn set(id: &ProfileId) -> Result<(), Error> {
    rad_profile::set(None, id.clone())?;

    Ok(())
}

pub fn repo(home: &RadHome, profile: &Profile) -> Result<PathBuf, Error> {
    match home {
        RadHome::Root(buf) => {
            let mut path = buf.to_path_buf();
            path.push(profile.id());
            path.push("git");
            Ok(path)
        }
        _ => Err(anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Radicle home is not set.",
        ))),
    }
}

pub fn user(storage: &Storage) -> Result<Urn, Error> {
    match storage.config_readonly() {
        Ok(config) => match config.user() {
            Ok(urn) => Ok(urn.unwrap()),
            Err(err) => Err(err).context("could not read user"),
        },
        Err(err) => Err(anyhow::Error::new(err)),
    }
}

pub fn peer_id(storage: &Storage) -> Result<PeerId, Error> {
    match storage.config_readonly() {
        Ok(config) => match config.peer_id() {
            Ok(peer_id) => Ok(peer_id),
            Err(err) => Err(err).context("could not read peer id"),
        },
        Err(err) => Err(anyhow::Error::new(err)),
    }
}
