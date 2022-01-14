use anyhow::{Error, Result};

use librad::profile::Profile;

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
