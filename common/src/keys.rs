//! SSH and key-related functions.
use anyhow::{anyhow, Context as _, Error, Result};

use librad::crypto::keystore::crypto::Pwhash;
use librad::crypto::BoxedSigner;
use librad::git::storage::Storage;

use librad::profile::{Profile, ProfileId};
use librad::PeerId;

use rad_clib::keys;
use rad_clib::keys::ssh::SshAuthSock;
use rad_clib::storage;
use rad_clib::storage::ssh;

use rad_terminal::keys::CachedPrompt;

/// Get the radicle signer and storage.
pub fn storage(profile: &Profile, sock: SshAuthSock) -> Result<(BoxedSigner, Storage), Error> {
    match ssh::storage(profile, sock) {
        Ok(result) => Ok(result),
        Err(storage::Error::SshKeys(keys::ssh::Error::NoSuchKey(_))) => Err(anyhow!(
            "the radicle ssh key for this profile is not in ssh-agent"
        )),
        Err(err) => Err(anyhow!(err)),
    }
}

/// Add a profile's radicle signing key to ssh-agent.
pub fn add(
    profile: &Profile,
    pass: Pwhash<CachedPrompt>,
    sock: SshAuthSock,
) -> Result<ProfileId, Error> {
    rad_profile::ssh_add(None, profile.id().clone(), sock, pass, &Vec::new())
        .context("could not add ssh key")
}

/// Get the SSH auth socket and warn if ssh-agent is not running.
pub fn ssh_auth_sock() -> SshAuthSock {
    if std::env::var("SSH_AGENT_PID").is_err() && std::env::var("SSH_AUTH_SOCK").is_err() {
        rad_terminal::components::warning("Warning: ssh-agent does not appear to be running!");
    }
    SshAuthSock::default()
}

/// Check whether the radicle signing key has been added to ssh-agent.
pub fn is_ready(profile: &Profile, sock: SshAuthSock) -> Result<bool, Error> {
    rad_profile::ssh_ready(None, profile.id().clone(), sock)
        .context("could not lookup ssh key, is ssh-agent running?")
        .map(|(_, is_ready)| is_ready)
}

/// Get the SSH long key from a peer id.
/// This is the output of `ssh-add -L`.
pub fn to_ssh_key(peer_id: &PeerId) -> Result<String, std::io::Error> {
    use byteorder::{BigEndian, WriteBytesExt};

    let mut buf = Vec::new();
    let key = peer_id.as_public_key().as_ref();
    let len = key.len();

    buf.write_u32::<BigEndian>(len as u32)?;
    buf.extend_from_slice(key);

    // Despite research, I have no idea what this string is, but it seems
    // to be the same for all Ed25519 keys.
    let mut encoded = String::from("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5");
    encoded.push_str(&base64::encode(buf));

    Ok(encoded)
}

/// Get the SSH key fingerprint from a peer id.
/// This is the output of `ssh-add -l`.
pub fn to_ssh_fingerprint(peer_id: &PeerId) -> Result<String, std::io::Error> {
    use byteorder::{BigEndian, WriteBytesExt};
    use sha2::Digest;

    let mut buf = Vec::new();
    let name = b"ssh-ed25519";
    let key = peer_id.as_public_key().as_ref();

    buf.write_u32::<BigEndian>(name.len() as u32)?;
    buf.extend_from_slice(name);
    buf.write_u32::<BigEndian>(key.len() as u32)?;
    buf.extend_from_slice(key);

    let sha = sha2::Sha256::digest(&buf).to_vec();
    let encoded = base64::encode(sha);

    Ok(format!("SHA256:{}", encoded.trim_end_matches('=')))
}
