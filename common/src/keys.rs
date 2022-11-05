//! SSH and key-related functions.
use anyhow::{anyhow, Error, Result};
use zeroize::Zeroizing;

use radicle::crypto::ssh;
use radicle::profile::Profile;
use radicle::Storage;

/// Env var used to pass down the passphrase to the git-remote-helper when
/// ssh-agent isn't present.
pub const RAD_PASSPHRASE: &str = "RAD_PASSPHRASE";

/// Get the radicle signer and storage.
pub fn storage(profile: &Profile) -> Result<Storage, Error> {
    let storage = Storage::open(profile.paths().storage())?;

    Ok(storage)
}

/// Add a profile's radicle signing key to ssh-agent.
pub fn add(profile: &Profile, pass: Zeroizing<String>) -> Result<(), Error> {
    let mut agent = ssh::agent::Agent::connect()?;
    let secret = profile
        .keystore
        .secret_key(pass)?
        .ok_or_else(|| anyhow!("Key not found in {:?}", profile.keystore.path()))?;

    agent.register(&secret)?;

    Ok(())
}

/// Remove a profile's radicle signing key from the ssh-agent
pub fn remove(profile: &Profile) -> Result<(), Error> {
    let mut agent = ssh::agent::Agent::connect()?;
    agent.remove_identity(profile.id())?;

    Ok(())
}

/// Check whether the radicle signing key has been added to ssh-agent.
pub fn is_ready(profile: &Profile) -> Result<bool, radicle::crypto::ssh::agent::Error> {
    let agent = ssh::agent::Agent::connect()?;
    agent.signer(profile.public_key).is_ready()
}
