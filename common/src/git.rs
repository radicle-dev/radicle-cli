use std::path::PathBuf;
use std::process::Command;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::{crypto::BoxedSigner, git::storage::ReadOnly, git::Urn, paths::Paths, PeerId};

use crate::identities;

pub const CONFIG_SIGNING_KEY: &str = "user.signingkey";
pub const CONFIG_GPG_FORMAT: &str = "gpg.format";
pub const CONFIG_GPG_SSH_PROGRAM: &str = "gpg.ssh.program";
pub const VERSION_REQUIRED: Version = Version {
    major: 2,
    minor: 34,
    patch: 0,
};

#[derive(PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::str::FromStr for Version {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let rest = input
            .strip_prefix("git version ")
            .ok_or(anyhow!("malformed git version string"))?;
        let rest = rest
            .split(' ')
            .next()
            .ok_or(anyhow!("malformed git version string"))?;
        let rest = rest.trim_end();

        let mut parts = rest.split('.');
        let major = parts
            .next()
            .ok_or(anyhow!("malformed git version string"))?
            .parse()?;
        let minor = parts
            .next()
            .ok_or(anyhow!("malformed git version string"))?
            .parse()?;

        let patch = match parts.next() {
            None => 0,
            Some(patch) => patch.parse()?,
        };

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

pub fn version() -> Result<Version, anyhow::Error> {
    let output = Command::new("git").arg("version").output()?;

    if output.status.success() {
        let output = String::from_utf8(output.stdout)?;
        let version = output
            .parse()
            .with_context(|| format!("unable to parse git version string {:?}", output))?;

        return Ok(version);
    }
    Err(anyhow!("failed to run `git version`"))
}

pub fn checkout<S>(
    storage: &S,
    paths: Paths,
    signer: BoxedSigner,
    urn: &Urn,
    peer: Option<PeerId>,
    path: PathBuf,
) -> anyhow::Result<git2::Repository>
where
    S: AsRef<ReadOnly>,
{
    let repo = identities::project::checkout(storage, paths, signer, urn, peer, path)?;
    // The checkout leaves a leftover config section sometimes, we clean it up here.
    git(
        repo.path(),
        ["config", "--remove-section", "remote.__tmp_/rad"],
    )
    .ok();

    Ok(repo)
}

pub fn git<S: AsRef<std::ffi::OsStr>>(
    repo: &std::path::Path,
    args: impl IntoIterator<Item = S>,
) -> Result<String, anyhow::Error> {
    let output = Command::new("git").current_dir(repo).args(args).output()?;

    if output.status.success() {
        let out = if output.stdout.is_empty() {
            &output.stderr
        } else {
            &output.stdout
        };
        return Ok(String::from_utf8_lossy(out).into());
    }

    Err(anyhow::Error::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        String::from_utf8_lossy(&output.stderr),
    )))
}

pub fn configure_monorepo(repo: &std::path::Path, peer_id: &PeerId) -> Result<(), anyhow::Error> {
    let key = crate::keys::to_ssh_key(peer_id)?;

    git(repo, ["config", "--local", CONFIG_SIGNING_KEY, &key])?;
    git(repo, ["config", "--local", CONFIG_GPG_FORMAT, "ssh"])?;
    git(
        repo,
        ["config", "--local", CONFIG_GPG_SSH_PROGRAM, "ssh-keygen"],
    )?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_version_ord() {
        assert!(
            Version {
                major: 2,
                minor: 34,
                patch: 1
            } > Version {
                major: 2,
                minor: 34,
                patch: 0
            }
        );
        assert!(
            Version {
                major: 2,
                minor: 24,
                patch: 12
            } < Version {
                major: 2,
                minor: 34,
                patch: 0
            }
        );
    }

    #[test]
    fn test_version_from_str() {
        assert_eq!(
            Version::from_str("git version 2.34.1\n").ok(),
            Some(Version {
                major: 2,
                minor: 34,
                patch: 1
            })
        );

        assert_eq!(
            Version::from_str("git version 2.34.1 (macOS)").ok(),
            Some(Version {
                major: 2,
                minor: 34,
                patch: 1
            })
        );

        assert_eq!(
            Version::from_str("git version 2.34").ok(),
            Some(Version {
                major: 2,
                minor: 34,
                patch: 0
            })
        );

        assert!(Version::from_str("2.34").is_err());
    }
}
