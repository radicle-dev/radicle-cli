//! Project-related functions and types.
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::iter;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use either::Either;
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
use librad::git_ext::{OneLevel, RefLike};
use librad::identities::payload::{self, ProjectPayload};
use librad::identities::SomeIdentity;
use librad::identities::{Person, VerifiedProject};
use librad::paths::Paths;
use librad::profile::Profile;
use librad::PeerId;

use lnk_identities;
use lnk_identities::working_copy_dir::WorkingCopyDir;

use crate as common;
use crate::person::Ens;
use crate::{git, person};

/// URL scheme for radicle resources.
pub const URL_SCHEME: &str = "rad";

/// Prefix for remote tracking branches from peers.
pub const PEER_PREFIX: &str = "peers";

/// Project indirect contributor identity.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PeerIdentity {
    #[serde(deserialize_with = "deserialize_urn")]
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
#[serde(rename_all = "camelCase")]
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
            None => common::fmt::peer(&self.id),
        }
    }

    pub fn get<S: AsRef<ReadOnly>>(peer_id: &PeerId, project: &Metadata, storage: &S) -> PeerInfo {
        let delegate = project.delegates.iter().any(|d| d.contains(peer_id));
        let reference = project.peer_self(peer_id, storage);

        if let Ok(urn) = Urn::try_from(reference) {
            if let Ok(Some(identity)) = PeerIdentity::get(&urn, &storage) {
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
    pub default_branch: OneLevel,
    /// List of delegates.
    pub delegates: Vec<Delegate>,
    /// List of remotes.
    pub remotes: HashSet<PeerId>,
}

impl Metadata {
    /// Get the local head of a project's default branch.
    pub fn local_head(&self, branch: impl Into<RefLike>) -> Reference<RefLike> {
        let namespace = Namespace::from(self.urn.clone());
        Reference::head(Some(namespace), None, branch.into())
    }

    /// Get the head of a project's default branch under a remote.
    pub fn remote_head(&self, remote: &PeerId) -> Reference<RefLike> {
        let namespace = Namespace::from(self.urn.clone());

        Reference::head(
            Some(namespace),
            Some(*remote),
            RefLike::from(self.default_branch.clone()),
        )
    }

    /// Get the reference to a project peer's `rad/self`.
    pub fn peer_self<S>(&self, peer: &PeerId, storage: &S) -> Reference<RefLike>
    where
        S: AsRef<ReadOnly>,
    {
        peer_self(storage, self.urn.clone(), peer)
    }

    /// Get a [`VerifiedProject`] from project metadata.
    pub fn verified(&self, storage: &Storage) -> anyhow::Result<VerifiedProject> {
        identities::project::verify(storage, &self.urn)?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", self.urn))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("project doesn't have a default branch")]
    MissingDefaultBranch,
    #[error("default branch error: {0}")]
    DefaultBranchName(#[from] radicle_git_ext::name::Error),
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
        let default_branch = RefLike::try_from(default_branch)?;
        let default_branch = OneLevel::from(default_branch);

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
    repo: &git::Repository,
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
) -> anyhow::Result<git::Repository>
where
    S: AsRef<ReadOnly>,
{
    let repo = crate::identities::project::checkout(
        storage,
        paths,
        signer,
        urn,
        peer,
        WorkingCopyDir::At(path),
    )?;
    // The checkout leaves a leftover config section sometimes, we clean it up here.
    git::git(
        repo.path(),
        ["config", "--remove-section", "remote.__tmp_/rad"],
    )
    .ok();

    Ok(repo)
}

/// List projects on the local device. Includes the project head if available.
pub fn list<S>(storage: &S) -> anyhow::Result<Vec<(Urn, Metadata, Option<git::Oid>)>>
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

/// List the heads of the rad remote.
pub fn list_rad_remote_heads(
    repo: &git::Repository,
    settings: transport::Settings,
) -> anyhow::Result<HashMap<PeerId, Vec<(String, git::Oid)>>> {
    let mut remote = git::rad_remote(repo)?;
    let mut remotes = HashMap::new();
    let heads = remote.remote_heads(settings, repo)?;

    for (head, oid) in heads {
        if let Some((peer, r)) = git::parse_remote(&head) {
            if let Some(branch) = r.strip_prefix("heads/") {
                let value = (branch.to_owned(), oid);
                remotes.entry(peer).or_insert_with(Vec::new).push(value);
            }
        }
    }
    Ok(remotes)
}

/// Get a local head of a project.
pub fn get_local_head<S>(storage: &S, urn: &Urn, branch: &str) -> anyhow::Result<Option<git::Oid>>
where
    S: AsRef<ReadOnly>,
{
    let repo = git::Repository::open_bare(storage.as_ref().path())?;
    let reference = repo
        .find_reference(&format!(
            "refs/namespaces/{}/refs/heads/{}",
            urn.encode_id(),
            branch
        ))
        .ok();

    Ok(reference.and_then(|r| r.target()))
}

/// Get the head of a project remote.
pub fn get_remote_head<S>(
    storage: &S,
    urn: &Urn,
    peer: &PeerId,
    branch: &str,
) -> anyhow::Result<Option<git::Oid>>
where
    S: AsRef<ReadOnly>,
{
    // Open the monorepo.
    let repo = git::Repository::open_bare(storage.as_ref().path())?;

    // Nb. the git2 crate doesn't handle namespaces properly, so we specify it manually.
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
pub fn person<S>(storage: &S, project: Urn, peer: &PeerId) -> anyhow::Result<Option<Person>>
where
    S: AsRef<ReadOnly>,
{
    let reference = peer_self(storage, project, peer);
    let urn = Urn::try_from(reference).map_err(|e| anyhow!(e))?;
    let person = identities::person::get(&storage, &urn)
        .map_err(|_| identities::Error::NotFound(urn.clone()))?;

    Ok(person)
}

/// Get a reference to `rad/self` for a project's peer.
pub fn peer_self<S>(storage: &S, project: Urn, peer: &PeerId) -> Reference<RefLike>
where
    S: AsRef<ReadOnly>,
{
    if storage.as_ref().peer_id() == peer {
        Reference::rad_self(Namespace::from(project), None)
    } else {
        Reference::rad_self(Namespace::from(project), *peer)
    }
}

/// List project seed heads.
pub fn list_seed_heads(
    repo: &git::Repository,
    url: &Url,
    project: &Urn,
) -> anyhow::Result<HashMap<PeerId, Vec<(String, git::Oid)>>> {
    let url = url.join(&project.encode_id())?;
    let mut remote = repo.remote_anonymous(url.as_str())?;
    let mut remotes = HashMap::new();

    remote.connect(git::Direction::Fetch)?;

    for head in remote.list()? {
        if let Some((peer, r)) = git::parse_remote(head.name()) {
            if let Some(branch) = r.strip_prefix("heads/") {
                let value = (branch.to_owned(), head.oid());
                remotes.entry(peer).or_insert_with(Vec::new).push(value);
            }
        }
    }
    Ok(remotes)
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
pub fn cwd() -> anyhow::Result<(Urn, git::Repository)> {
    let repo = git::repository()?;
    let urn = git::rad_remote(&repo)?.url.urn;

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
    pub repo: &'a git::Repository,
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
        name: &str,
        profile: &Profile,
    ) -> anyhow::Result<Option<(Remote<LocalUrl>, String)>> {
        let repo = self.repo;
        let urn = &self.project.urn;

        // TODO: Handle conflicts in remote name.
        let peer_prefix = format!("{}/{}", PEER_PREFIX, name);
        let remote_name = format!("{}/rad", peer_prefix);
        let mut remote = self::remote(urn, peer, &remote_name)?;

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
            let branch =
                git::set_tracking(repo.path(), &peer_prefix, &self.project.default_branch)?;

            return Ok(Some((remote, branch)));
        }
        Ok(None)
    }
}

pub fn deserialize_urn<'de, D>(deserializer: D) -> Result<Urn, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}
