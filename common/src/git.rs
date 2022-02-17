use std::collections::HashMap;
use std::convert::TryFrom as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::local::url::LocalUrl;
use librad::git::types::{remote::Remote, Flat, Force, GenericRef, Reference, Refspec};
use librad::git_ext::RefLike;
use librad::profile::Profile;
use librad::{crypto::BoxedSigner, git::storage::ReadOnly, git::Urn, paths::Paths, PeerId};

pub use librad::git::local::transport;
pub use librad::git::types::remote::LocalFetchspec;

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

pub fn repository(path: &Path) -> Result<git_repository::Repository, git_repository::open::Error> {
    git_repository::Repository::open(path)
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

pub fn configure_monorepo(repo: &Path, peer_id: &PeerId) -> Result<(), anyhow::Error> {
    let key = crate::keys::to_ssh_key(peer_id)?;

    git(repo, ["config", "--local", CONFIG_SIGNING_KEY, &key])?;
    git(repo, ["config", "--local", CONFIG_GPG_FORMAT, "ssh"])?;
    git(
        repo,
        ["config", "--local", CONFIG_GPG_SSH_PROGRAM, "ssh-keygen"],
    )?;

    Ok(())
}

pub fn remote(urn: &Urn, peer: &PeerId, name: &str) -> Result<Remote<LocalUrl>, anyhow::Error> {
    let name = RefLike::try_from(name)?;
    let url = LocalUrl::from(urn.clone());
    let remote = Remote::new(url, name.clone()).with_fetchspecs(vec![Refspec {
        src: Reference::heads(Flat, *peer),
        dst: GenericRef::heads(Flat, name),
        force: Force::True,
    }]);

    Ok(remote)
}

pub fn remotes(repo: &git2::Repository) -> anyhow::Result<Vec<(String, PeerId)>> {
    let mut remotes = Vec::new();

    for name in repo.remotes().iter().flatten().flatten() {
        let remote = repo.find_remote(name)?;
        for refspec in remote.refspecs() {
            if refspec.direction() != git2::Direction::Fetch {
                continue;
            }
            if let Some((peer, _)) = refspec.src().and_then(self::parse_remote) {
                remotes.push((name.to_owned(), peer));
            }
        }
    }

    Ok(remotes)
}

pub fn set_upstream(repo: &Path, name: &str, branch: &str) -> anyhow::Result<String> {
    let branch_name = format!("{}/{}", name, branch);

    git(
        repo,
        [
            "branch",
            &branch_name,
            &format!("{}/heads/{}", name, branch),
        ],
    )?;

    Ok(branch_name)
}

pub fn list_remotes(
    repo: &git2::Repository,
    url: &url::Url,
    urn: &Urn,
) -> anyhow::Result<HashMap<PeerId, Vec<(String, git2::Oid)>>> {
    // TODO: Use `Remote::remote_heads`.
    let url = url.join(&urn.encode_id())?;
    let mut remote = repo.remote_anonymous(url.as_str())?;
    let mut remotes = HashMap::new();

    remote.connect(git2::Direction::Fetch)?;

    let heads = remote.list()?;
    for head in heads {
        if let Some((peer, r)) = parse_remote(head.name()) {
            if let Some(branch) = r.strip_prefix("heads/") {
                let value = (branch.to_owned(), head.oid());
                remotes.entry(peer).or_insert_with(Vec::new).push(value);
            }
        }
    }
    Ok(remotes)
}

/// Fetch refs into working copy.
pub fn fetch_remote(
    remote: &mut Remote<LocalUrl>,
    repo: &git2::Repository,
    signer: BoxedSigner,
    profile: &Profile,
) -> anyhow::Result<()> {
    let settings = transport::Settings {
        paths: profile.paths().clone(),
        signer,
    };
    remote
        .fetch(settings, repo, LocalFetchspec::Configured)?
        .for_each(drop);

    Ok(())
}

fn parse_remote(refspec: &str) -> Option<(PeerId, &str)> {
    refspec
        .strip_prefix("refs/remotes/")
        .and_then(|s| s.split_once('/'))
        .and_then(|(peer, r)| PeerId::from_str(peer).ok().map(|p| (p, r)))
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
