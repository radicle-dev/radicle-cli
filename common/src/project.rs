use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::iter;
use std::path::Path;

use anyhow::{anyhow, Context as _, Error, Result};
use either::Either;
use git2::Repository;

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
use librad::profile::Profile;
use librad::reflike;
use librad::PeerId;

use rad_identities;
use rad_terminal::components as term;

/// Project delegate.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub struct RemoteMetadata {
    pub id: PeerId,
    pub name: String,
    pub delegate: bool,
}

/// Project metadata.
#[derive(Debug)]
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
    pub delegates: HashSet<Urn>,
    /// List of remotes.
    pub remotes: HashSet<PeerId>,
}

impl TryFrom<librad::identities::Project> for Metadata {
    type Error = anyhow::Error;

    fn try_from(project: librad::identities::Project) -> Result<Self, Self::Error> {
        let subject = project.subject();
        let delegates = project
            .delegations()
            .iter()
            .indirect()
            .map(|indirect| indirect.urn())
            .collect();
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
            .ok_or(anyhow!("project is missing a default branch"))?
            .to_string();

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

pub fn create(
    repo: &git2::Repository,
    identity: identities::local::LocalIdentity,
    storage: &Storage,
    signer: BoxedSigner,
    profile: &Profile,
    payload: payload::Project,
) -> Result<Project, Error> {
    let paths = profile.paths().clone();
    let payload = ProjectPayload::new(payload);

    let delegations = identities::IndirectDelegation::try_from_iter(iter::once(Either::Right(
        identity.clone().into_inner().into_inner(),
    )))?;

    let urn = project::urn(storage, payload.clone(), delegations.clone())?;
    let url = LocalUrl::from(urn);
    let project = project::create(storage, identity, payload, delegations)?;

    if let Some(branch) = project.subject().default_branch.clone() {
        let branch = RefLike::try_from(branch.to_string())?.into();
        let settings = transport::Settings {
            paths: paths.clone(),
            signer,
        };
        rad_identities::git::setup_remote(repo, settings, url, &branch)?;
    }
    rad_identities::git::include::update(storage, &paths, &project)?;

    Ok(project)
}

pub fn list(
    storage: &Storage,
) -> Result<Vec<(Urn, Metadata, Option<git_repository::ObjectId>)>, Error> {
    let repo = git_repository::Repository::open(storage.path())?;
    let objs = identities::any::list(storage)?
        .filter_map(|res| {
            res.map(|id| match id {
                SomeIdentity::Project(project) => {
                    let urn = project.urn();
                    let meta: Metadata = project.try_into().ok()?;
                    let head = get_local_head(&repo, &urn, &meta.default_branch)
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

pub fn get_local_head<'r>(
    repo: &'r git_repository::Repository,
    urn: &Urn,
    branch: &str,
) -> Result<Option<git_repository::ObjectId>, Error> {
    let mut repo = repo.to_easy();
    repo.set_namespace(urn.encode_id())?;

    let reference = repo.try_find_reference(format!("heads/{}", branch))?;

    Ok(reference.map(|r| r.id().detach()))
}

pub fn get<S>(storage: &S, urn: &Urn) -> Result<Option<Metadata>, Error>
where
    S: AsRef<ReadOnly>,
{
    let proj = rad_identities::project::get(storage, urn)?;
    let meta = proj.map(|p| p.try_into()).transpose()?;

    Ok(meta)
}

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

pub fn repository() -> Result<Repository, Error> {
    match Repository::open(".") {
        Ok(repo) => Ok(repo),
        Err(err) => Err(err).context("the current working directory is not a git repository"),
    }
}

pub fn repository_from(path: &Path) -> Result<Repository, Error> {
    match Repository::open(path) {
        Ok(repo) => Ok(repo),
        Err(err) => Err(err).context(format!("{} is not a git repository", path.display())),
    }
}

pub fn remote(repo: &Repository) -> Result<Remote<LocalUrl>, Error> {
    match Remote::<LocalUrl>::find(repo, reflike!("rad")) {
        Ok(Some(remote)) => Ok(remote),
        Ok(None) => Err(anyhow!(
            "could not find radicle remote in git config. Did you forget to run `rad init`?"
        )),
        Err(err) => Err(err).context("could not read git remote configuration"),
    }
}

pub fn urn() -> Result<Urn, Error> {
    let repo = self::repository()?;
    Ok(self::remote(&repo)?.url.urn)
}

pub fn cwd() -> Result<(Urn, Repository), Error> {
    let repo = self::repository()?;
    let urn = self::remote(&repo)?.url.urn;

    Ok((urn, repo))
}

pub fn tracked<S>(
    project: &Metadata,
    storage: &S,
) -> anyhow::Result<HashMap<PeerId, RemoteMetadata>>
where
    S: AsRef<ReadOnly>,
{
    let entries = tracking::tracked(storage.as_ref(), Some(&project.urn))?;
    let mut remotes = HashMap::new();

    for tracked in entries {
        let tracked = tracked?;
        if let Some(peer) = tracked.peer_id() {
            if let Some(person) = self::person(storage, &project.urn, &peer)? {
                let delegate = project.remotes.contains(&peer);

                remotes.insert(
                    peer,
                    RemoteMetadata {
                        id: peer,
                        name: person.subject().name.to_string(),
                        delegate,
                    },
                );
            }
        }
    }
    Ok(remotes)
}

pub fn get_remote_head(
    repo: &Repository,
    urn: &Urn,
    peer: &PeerId,
    branch: &str,
) -> Result<Option<git2::Oid>, Error> {
    // Nb. `git2` doesn't handle namespaces properly, so we specify it manually.
    let reference = repo.find_reference(&format!(
        "refs/namespaces/{}/refs/remotes/{}/heads/{}",
        urn.encode_id(),
        peer,
        branch
    ))?;

    Ok(reference.target())
}

pub struct SetupRemote<'a> {
    pub project: &'a Metadata,
    pub repo: &'a git2::Repository,
    pub signer: BoxedSigner,
    pub fetch: bool,
    pub upstream: bool,
}

impl<'a> SetupRemote<'a> {
    pub fn run(&self, peer: &PeerId, profile: &Profile, storage: &Storage) -> anyhow::Result<()> {
        use crate::git;

        let repo = self.repo;
        let urn = &self.project.urn;

        // TODO: Handle conflicts in remote name.
        if let Some(person) = self::person(storage, urn, peer)? {
            let name = format!("peer/{}", person.subject().name);
            let mut remote = git::remote(urn, peer, &name)?;

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

                term::success!(
                    "Remote-tracking branch {} created for {}",
                    term::format::highlight(&branch),
                    term::format::tertiary(crate::fmt::peer(peer))
                );
            }
        }
        Ok(())
    }
}
