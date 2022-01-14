use anyhow::{Error, Result};

use librad::crypto::keystore::crypto::Pwhash;
use librad::git::storage::Storage;

use librad::profile::{Profile, ProfileId};

use rad_clib::keys::ssh::SshAuthSock;
use rad_clib::storage::ssh;

use rad_terminal::compoments as term;
use rad_terminal::keys::CachedPrompt;

pub fn storage(profile: &Profile, sock: SshAuthSock) -> Result<Storage, Error> {
    match ssh::storage(profile, sock) {
        Ok((_, storage)) => Ok(storage),
        Err(err) => {
            term::error("Could not read ssh key:");
            term::format::error_detail(&format!("{}", err));
            Err(anyhow::Error::new(err))
        }
    }
}

pub fn add(
    profile: &Profile,
    pass: Pwhash<CachedPrompt>,
    sock: SshAuthSock,
) -> Result<ProfileId, Error> {
    match rad_profile::ssh_add(None, profile.id().clone(), sock, pass, &Vec::new()) {
        Ok(id) => Ok(id),
        Err(err) => {
            term::error(&format!("Could not add ssh key. {:?}", err));
            Err(anyhow::Error::new(err))
        }
    }
}
