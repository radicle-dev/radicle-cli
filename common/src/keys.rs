use anyhow::{anyhow, Context as _, Error, Result};

use librad::crypto::keystore::crypto::Pwhash;
use librad::crypto::BoxedSigner;
use librad::git::storage::Storage;

use librad::profile::{Profile, ProfileId};

use rad_clib::keys;
use rad_clib::keys::ssh::SshAuthSock;
use rad_clib::storage;
use rad_clib::storage::ssh;

use rad_terminal::keys::CachedPrompt;

pub fn storage(profile: &Profile, sock: SshAuthSock) -> Result<(BoxedSigner, Storage), Error> {
    match ssh::storage(profile, sock) {
        Ok(result) => Ok(result),
        Err(storage::Error::SshKeys(keys::ssh::Error::NoSuchKey(_))) => Err(anyhow!(
            "the radicle ssh key for this profile is not in ssh-agent"
        )),
        Err(err) => Err(anyhow!(err)),
    }
}

pub fn add(
    profile: &Profile,
    pass: Pwhash<CachedPrompt>,
    sock: SshAuthSock,
) -> Result<ProfileId, Error> {
    rad_profile::ssh_add(None, profile.id().clone(), sock, pass, &Vec::new())
        .context("could not add ssh key")
}

pub fn is_ready(profile: &Profile, sock: SshAuthSock) -> Result<bool, Error> {
    rad_profile::ssh_ready(None, profile.id().clone(), sock)
        .context("could not lookup ssh key")
        .map(|(_, is_ready)| is_ready)
}
