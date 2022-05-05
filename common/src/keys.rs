//! SSH and key-related functions.
use anyhow::{Context as _, Error, Result};

use librad::crypto::keystore::crypto::Pwhash;
use librad::crypto::BoxedSigner;
use librad::git::storage::Storage;

use librad::keystore::pinentry::Pinentry;
use librad::profile::{Profile, ProfileId};
use librad::PeerId;

use lnk_clib::keys;
use lnk_clib::keys::ssh::SshAuthSock;

use rad_terminal::components as term;

use crate::signer::ToSigner;

/// Get the radicle signer and storage.
pub fn storage(profile: &Profile, signer: impl ToSigner) -> Result<Storage, Error> {
    let signer = match signer.to_signer(profile) {
        Ok(signer) => signer,
        Err(keys::ssh::Error::NoSuchKey(_)) => {
            anyhow::bail!("the radicle ssh key for this profile is not in ssh-agent")
        }
        Err(err) => anyhow::bail!(err),
    };
    let storage = Storage::open(profile.paths(), signer)?;

    Ok(storage)
}

/// Get the signer. First we try getting it from ssh-agent, otherwise we prompt the user.
pub fn signer(profile: &Profile) -> Result<BoxedSigner, Error> {
    let signer = if let Ok(sock) = ssh_auth_sock() {
        sock.to_signer(profile)?
    } else {
        term::secret_key(profile)?.to_signer(profile)?
    };
    Ok(signer)
}

/// Add a profile's radicle signing key to ssh-agent.
pub fn add<P: Pinentry>(
    profile: &Profile,
    pass: Pwhash<P>,
    sock: SshAuthSock,
) -> Result<ProfileId, Error>
where
    <P as Pinentry>::Error: std::fmt::Debug + std::error::Error + Send + Sync + 'static,
{
    lnk_profile::ssh_add(None, profile.id().clone(), sock, pass, Vec::new())
        .context("could not add ssh key")
}

/// Remove a profile's radicle signing key from the ssh-agent
pub fn remove<P: Pinentry>(
    profile: &Profile,
    pass: Pwhash<P>,
    sock: SshAuthSock,
) -> Result<ProfileId, Error>
where
    <P as Pinentry>::Error: std::fmt::Debug + std::error::Error + Send + Sync + 'static,
{
    lnk_profile::ssh_remove(None, profile.id().clone(), sock, pass)
        .context("could not remove ssh key")
}

/// Get the SSH auth socket and error if ssh-agent is not running.
pub fn ssh_auth_sock() -> Result<SshAuthSock, anyhow::Error> {
    if std::env::var("SSH_AGENT_PID").is_err() && std::env::var("SSH_AUTH_SOCK").is_err() {
        anyhow::bail!("ssh-agent does not appear to be running");
    }
    Ok(SshAuthSock::Env)
}

/// Check whether the radicle signing key has been added to ssh-agent.
pub fn is_ready(profile: &Profile, sock: SshAuthSock) -> Result<bool, Error> {
    lnk_profile::ssh_ready(None, profile.id().clone(), sock)
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
