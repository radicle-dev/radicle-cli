use std::path::PathBuf;

use anyhow::{Error, Result};

use librad::crypto::peer::PeerId;
use librad::git::storage::Storage;
use librad::git::Urn;
use librad::profile::{Profile, RadHome};

use rad_profile;
use rad_terminal::compoments as term;

pub fn default() -> Result<Profile, Error> {
    match rad_profile::get(None, None) {
        Ok(profile) => Ok(profile.unwrap()),
        Err(err) => {
            term::error(&format!("Could not get active profile. {:?}", err));
            Err(anyhow::Error::new(err))
        }
    }
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
            Err(err) => {
                term::error(&format!("Could not read user. {:?}", err));
                Err(anyhow::Error::new(err))
            }
        },
        Err(err) => Err(anyhow::Error::new(err)),
    }
}

pub fn peer_id(storage: &Storage) -> Result<PeerId, Error> {
    match storage.config_readonly() {
        Ok(config) => match config.peer_id() {
            Ok(peer_id) => Ok(peer_id),
            Err(err) => {
                term::error(&format!("Could not read peer id. {:?}", err));
                Err(anyhow::Error::new(err))
            }
        },
        Err(err) => Err(anyhow::Error::new(err)),
    }
}
