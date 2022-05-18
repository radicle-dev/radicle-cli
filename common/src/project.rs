//! Project-related functions and types.
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::iter;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context as _, Result};
use either::Either;
use git2::Repository;
use serde::{Deserialize, Serialize};
use url::Url;

use librad::canonical::Cstring;
use librad::crypto::BoxedSigner;
use librad::git::identities::{self, project, Project};
use librad::git::local::transport;
use librad::git::local::url::LocalUrl;
use librad::git::storage::{ReadOnly, Storage};
use librad::git::tracking;
use librad::git::types::remote::Remote;
use librad::git::types::{Namespace, Reference};
use librad::git::Urn;
use librad::git_ext::RefLike;
use librad::identities::payload::{self, ProjectPayload};
use librad::identities::Person;
use librad::identities::SomeIdentity;
use librad::paths::Paths;
use librad::profile::Profile;
use librad::reflike;
use librad::PeerId;

use lnk_identities;

use crate::person::Ens;
use crate::{git, person, seed};

/// URL scheme for radicle resources.
pub const URL_SCHEME: &str = "rad";

/// Project origin.
///
/// Represents a location from which a project can be fetched.
/// To construct one, use the [`TryFrom<Url>`] or [`FromStr`]
/// instances.
#[derive(Debug, Eq, PartialEq)]
pub struct Origin {
    /// Project URN.
    pub urn: Urn,
    /// If available, the address of a seed which has this project.
    pub seed: Option<seed::Address>,
}

impl Origin {
    /// Create an origin from a URN.
    pub fn from_urn(urn: Urn) -> Self {
        Self { urn, seed: None }
    }

    /// Get the seed URL, if any, of this origin.
    pub fn seed_url(&self) -> Option<Url> {
        self.seed.as_ref().map(|s| s.url())
    }
}

impl FromStr for Origin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(urn) = Urn::from_str(s) {
            Ok(Self { urn, seed: None })
        } else if let Ok(url) = Url::from_str(s) {
            Self::try_from(url)
        } else {
            return Err(anyhow!("invalid origin '{}'", s));
        }
    }
}

impl TryFrom<Url> for Origin {
    type Error = anyhow::Error;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        let mut segments = if let Some(segments) = url.path_segments() {
            segments
        } else {
            anyhow::bail!("invalid radicle URL '{}': missing path", url.to_string());
        };

        if url.scheme() != URL_SCHEME {
            anyhow::bail!("not a radicle URL '{}': invalid scheme", url.to_string());
        }

        let host = url.host();
        let port = url.port();
        let seed = host.map(|host| seed::Address {
            host: host.to_owned(),
            port,
        });

        let urn = if let Some(id) = segments.next() {
            if id.is_empty() {
                anyhow::bail!("invalid radicle URL '{}': empty path", url.to_string());
            }
            Urn::try_from_id(id)?
        } else {
            anyhow::bail!("invalid radicle URL '{}': missing path", url.to_string());
        };

        Ok(Self { urn, seed })
    }
}

/// Project indirect contributor identity.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerIdentity {
    pub urn: Urn,
    pub name: String,
    pub ens: Option<Ens>,
}

impl PeerIdentity {
    /// Get the identity of a peer, and if possible the ENS name.
    pub fn get<S: AsRef<ReadOnly>>(
        urn: &Urn,
        storage: &S,
    ) -> Result<Option<Self>, identities::Error> {
        let person = identities::person::get(&storage, urn)?;
        if let Some(person) = person {
            let ens = match person.payload().get_ext::<Ens>() {
                Ok(e) => e,
                _ => None,
            };

            return Ok(Some(PeerIdentity {
                urn: person.urn(),
                name: person.subject().name.to_string(),
                ens,
            }));
        }
        Ok(None)
    }
}

/// Project peer information.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub struct PeerInfo {
    /// Peer id.
    pub id: PeerId,
    /// Peer identity, if known.
    pub person: Option<PeerIdentity>,
    /// Whether or not this peer belongs to a project delegate.
    pub delegate: bool,
}

impl PeerInfo {
    pub fn name(&self) -> String {
        match &self.person {
            Some(person) => person.name.clone(),
            None => self.id.default_encoding(),
        }
    }

    pub fn get<S: AsRef<ReadOnly>>(peer_id: &PeerId, project: &Metadata, storage: &S) -> PeerInfo {
        let delegate = project.delegates.iter().any(|d| d.contains(peer_id));

        if let Ok(delegate_urn) = Urn::try_from(Reference::rad_self(
            Namespace::from(project.urn.clone()),
            Some(*peer_id),
        )) {
            if let Ok(Some(identity)) = PeerIdentity::get(&delegate_urn, &storage) {
                return PeerInfo {
                    id: *peer_id,
                    person: Some(identity),
                    delegate,
                };
            }
        }
        PeerInfo {
            id: *peer_id,
            person: None,
            delegate,
        }
    }
}

/// Project delegate.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Delegate {
    /// Direct delegation, ie. public key.
    Direct { id: PeerId },
    /// Indirect delegation, ie. a personal identity.
    Indirect { urn: Urn, ids: HashSet<PeerId> },
}

use std::fmt;
impl fmt::Display for Delegate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Direct { id } => write!(f, "{}", id.default_encoding()),
            Self::Indirect { urn, .. } => write!(f, "{}", urn.encode_id()),
        }
    }
}

impl Delegate {
    pub fn contains(&self, other: &PeerId) -> bool {
        match self {
            Self::Direct { id } => id == other,
            Self::Indirect { ids, .. } => ids.contains(other),
        }
    }
}

/// Project metadata.
///
/// Can be constructed from a [`librad::identities::Project`].
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// Project URN.
    pub urn: Urn,
    /// Project name.
    pub name: String,
    /// Project description.
    pub description: String,
    /// Default branch of project.
    pub default_branch: String,
    /// List of delegates.
    pub delegates: Vec<Delegate>,
    /// List of remotes.
    pub remotes: HashSet<PeerId>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("project doesn't have a default branch")]
    MissingDefaultBranch,
}

impl TryFrom<librad::identities::Project> for Metadata {
    type Error = Error;

    fn try_from(project: librad::identities::Project) -> Result<Self, Self::Error> {
        let subject = project.subject();
        let remotes = project
            .delegations()
            .iter()
            .flat_map(|either| match either {
                Either::Left(pk) => Either::Left(std::iter::once(PeerId::from(*pk))),
                Either::Right(indirect) => {
                    Either::Right(indirect.delegations().iter().map(|pk| PeerId::from(*pk)))
                }
            })
            .collect::<HashSet<PeerId>>();
        let default_branch = subject
            .default_branch
            .clone()
            .ok_or(Error::MissingDefaultBranch)?
            .to_string();

        let mut delegates = Vec::new();
        for delegate in project.delegations().iter() {
            match delegate {
                Either::Left(pk) => {
                    delegates.push(Delegate::Direct {
                        id: PeerId::from(*pk),
                    });
                }
                Either::Right(indirect) => {
                    delegates.push(Delegate::Indirect {
                        urn: indirect.urn(),
                        ids: indirect
                            .delegations()
                            .iter()
                            .map(|pk| PeerId::from(*pk))
                            .collect(),
                    });
                }
            }
        }

        Ok(Self {
            urn: project.urn(),
            name: subject.name.to_string(),
            description: subject
                .description
                .clone()
                .map_or_else(|| "".into(), |desc| desc.to_string()),
            default_branch,
            delegates,
            remotes,
        })
    }
}

/// Create a project payload.
pub fn payload(name: String, description: String, default_branch: String) -> payload::Project {
    payload::Project {
        name: Cstring::from(name),
        description: Some(Cstring::from(description)),
        default_branch: Some(Cstring::from(default_branch)),
    }
}

/// Create a new project identity.
pub fn create(payload: payload::Project, storage: &Storage) -> anyhow::Result<Project> {
    let whoami = person::local(storage)?;
    let payload = ProjectPayload::new(payload);
    let delegations = identities::IndirectDelegation::try_from_iter(iter::once(Either::Right(
        whoami.clone().into_inner().into_inner(),
    )))?;
    let project = project::create(storage, whoami, payload, delegations)?;

    Ok(project)
}

/// Initialize a repo as a project.
pub fn init(
    project: &Project,
    repo: &git2::Repository,
    storage: &Storage,
    paths: &Paths,
    signer: BoxedSigner,
) -> anyhow::Result<()> {
    if let Some(branch) = project.subject().default_branch.clone() {
        let branch = RefLike::try_from(branch.to_string())?.into();
        let settings = transport::Settings {
            paths: paths.clone(),
            signer,
        };
        let url = LocalUrl::from(project.urn());

        lnk_identities::git::setup_remote(repo, settings, url, &branch)?;
    }
    lnk_identities::git::include::update(storage, paths, project)?;

    Ok(())
}

/// Create a checkout of a radicle project.
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
    let repo = crate::identities::project::checkout(storage, paths, signer, urn, peer, path)?;
    // The checkout leaves a leftover config section sometimes, we clean it up here.
    git::git(
        repo.path(),
        ["config", "--remove-section", "remote.__tmp_/rad"],
    )
    .ok();

    Ok(repo)
}

/// List projects on the local device. Includes the project head if available.
pub fn list<S>(storage: &S) -> anyhow::Result<Vec<(Urn, Metadata, Option<git2::Oid>)>>
where
    S: AsRef<ReadOnly>,
{
    let objs = identities::any::list(storage)?
        .filter_map(|res| {
            res.map(|id| match id {
                SomeIdentity::Project(project) => {
                    let urn = project.urn();
                    let meta: Metadata = project.try_into().ok()?;
                    let head = get_local_head(&storage, &urn, &meta.default_branch)
                        .ok()
                        .flatten();

                    Some((urn, meta, head))
                }
                _ => None,
            })
            .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(objs)
}

/// List the heads of a remote repository.
pub fn list_remote_heads(
    repo: &git2::Repository,
    settings: transport::Settings,
) -> anyhow::Result<HashMap<PeerId, Vec<(String, git2::Oid)>>> {
    let mut remote = self::rad_remote(repo)?;
    let mut remotes = HashMap::new();
    let heads = remote.remote_heads(settings, repo)?;

    for (head, oid) in heads {
        if let Some((peer, r)) = git::parse_remote(&head.to_string()) {
            if let Some(branch) = r.strip_prefix("heads/") {
                let value = (branch.to_owned(), oid);
                remotes.entry(peer).or_insert_with(Vec::new).push(value);
            }
        }
    }
    Ok(remotes)
}

/// Get a local head of a project.
pub fn get_local_head<S>(storage: &S, urn: &Urn, branch: &str) -> anyhow::Result<Option<git2::Oid>>
where
    S: AsRef<ReadOnly>,
{
    let repo = git2::Repository::open_bare(storage.as_ref().path())?;
    let reference = repo.find_reference(&format!(
        "refs/namespaces/{}/refs/heads/{}",
        urn.encode_id(),
        branch
    ))?;

    Ok(reference.target())
}

/// Get the head of a project remote.
pub fn get_remote_head<S>(
    storage: &S,
    urn: &Urn,
    peer: &PeerId,
    branch: &str,
) -> anyhow::Result<Option<git2::Oid>>
where
    S: AsRef<ReadOnly>,
{
    // Open the monorepo.
    let repo = git2::Repository::open_bare(storage.as_ref().path())?;

    // Nb. `git2` doesn't handle namespaces properly, so we specify it manually.
    let reference = repo.find_reference(&format!(
        "refs/namespaces/{}/refs/remotes/{}/heads/{}",
        urn.encode_id(),
        peer,
        branch
    ))?;

    Ok(reference.target())
}

/// Get project metadata.
pub fn get<S>(storage: &S, urn: &Urn) -> anyhow::Result<Option<Metadata>>
where
    S: AsRef<ReadOnly>,
{
    let proj = lnk_identities::project::get(storage, urn)?;
    let meta = proj.map(|p| p.try_into()).transpose()?;

    Ok(meta)
}

/// Get the personal identity associated with a project's peer.
pub fn person<S>(storage: &S, urn: &Urn, peer: &PeerId) -> anyhow::Result<Option<Person>>
where
    S: AsRef<ReadOnly>,
{
    let urn = Urn::try_from(Reference::rad_self(Namespace::from(urn.clone()), *peer))
        .map_err(|e| anyhow!(e))?;

    let person = identities::person::get(&storage, &urn)
        .map_err(|_| identities::Error::NotFound(urn.clone()))?;

    Ok(person)
}

/// Get the repository's "rad" remote.
pub fn rad_remote(repo: &Repository) -> anyhow::Result<Remote<LocalUrl>> {
    match Remote::<LocalUrl>::find(repo, reflike!("rad")) {
        Ok(Some(remote)) => Ok(remote),
        Ok(None) => Err(anyhow!(
            "could not find radicle remote in git config. Did you forget to run `rad init`?"
        )),
        Err(err) => Err(err).context("could not read git remote configuration"),
    }
}

/// Create a git remote for the given project and peer. This does not save the
/// remote to the git configuration.
pub fn remote(urn: &Urn, peer: &PeerId, name: &str) -> Result<Remote<LocalUrl>, anyhow::Error> {
    use librad::git::types::{Flat, Force, GenericRef, Refspec};

    let name = RefLike::try_from(name)?;
    let url = LocalUrl::from(urn.clone());
    let remote = Remote::new(url, name.clone()).with_fetchspecs(vec![Refspec {
        src: Reference::heads(Flat, *peer),
        dst: GenericRef::heads(Flat, name),
        force: Force::True,
    }]);

    Ok(remote)
}

/// Get the project URN and repository of the current working directory.
pub fn cwd() -> anyhow::Result<(Urn, Repository)> {
    let repo = git::repository()?;
    let urn = self::rad_remote(&repo)?.url.urn;

    Ok((urn, repo))
}

/// Get the tracked peers of a project, including information about these peers.
pub fn tracked<S>(project: &Metadata, storage: &S) -> anyhow::Result<HashMap<PeerId, PeerInfo>>
where
    S: AsRef<ReadOnly>,
{
    let entries = tracking::tracked(storage.as_ref(), Some(&project.urn))?;
    let mut remotes = HashMap::new();

    for tracked in entries {
        let tracked = tracked?;
        if let Some(peer) = tracked.peer_id() {
            remotes.insert(peer, PeerInfo::get(&peer, project, storage));
        }
    }
    Ok(remotes)
}

/// Setup a project remote and tracking branch.
pub struct SetupRemote<'a> {
    /// The project.
    pub project: &'a Metadata,
    /// The repository in which to setup the remote.
    pub repo: &'a git2::Repository,
    /// Radicle signer.
    pub signer: BoxedSigner,
    /// Whether or not to fetch the remote immediately.
    pub fetch: bool,
    /// Whether or not to setup an upstream tracking branch.
    pub upstream: bool,
}

impl<'a> SetupRemote<'a> {
    /// Run the setup for the given peer.
    pub fn run(
        &self,
        peer: &PeerId,
        profile: &Profile,
        storage: &Storage,
    ) -> anyhow::Result<Option<String>> {
        let repo = self.repo;
        let urn = &self.project.urn;

        // TODO: Handle conflicts in remote name.
        if let Some(person) = self::person(storage, urn, peer)? {
            let name = format!("peer/{}", person.subject().name);
            let mut remote = self::remote(urn, peer, &name)?;

            // Configure the remote in the repository.
            remote.save(repo)?;
            // Fetch the refs into the working copy.
            if self.fetch {
                git::fetch_remote(&mut remote, repo, self.signer.clone(), profile)?;
            }
            // Setup remote-tracking branch.
            if self.upstream {
                // TODO: If this fails because the branch already exists, suggest how to specify a
                // different branch name or prefix.
                let branch = git::set_upstream(repo.path(), &name, &self.project.default_branch)?;

                return Ok(Some(branch));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_origin_from_url() {
        let url = Url::parse("rad://willow.radicle.garden/hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y")
            .unwrap();

        let origin = Origin::try_from(url).unwrap();

        assert_eq!(
            origin.urn,
            Urn::try_from_id("hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap()
        );
        assert_eq!(
            origin.seed,
            Some(seed::Address {
                host: url::Host::Domain("willow.radicle.garden".to_owned()),
                port: None
            })
        );
    }

    #[test]
    fn test_origin_from_str() {
        let origin = Origin::from_str("rad:git:hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap();
        assert_eq!(
            origin.urn,
            Urn::try_from_id("hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap()
        );
        assert!(origin.seed.is_none());
    }
}
