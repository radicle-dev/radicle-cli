use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter;

use anyhow::{anyhow, Context as _, Error, Result};
use either::Either;
use git2::Repository;
use git_repository as git;

use librad::crypto::BoxedSigner;
use librad::git::identities::{self, project, Project};
use librad::git::local::transport;
use librad::git::local::url::LocalUrl;
use librad::git::storage::{ReadOnly, Storage};
use librad::git::types::remote::Remote;
use librad::git::Urn;
use librad::git_ext::RefLike;
use librad::identities::payload::{self, ProjectPayload};
use librad::identities::Person;
use librad::identities::SomeIdentity;
use librad::profile::Profile;
use librad::reflike;
use librad::PeerId;

use rad_identities;

/// Project metadata.
#[derive(Debug)]
pub struct Metadata {
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

pub fn list(storage: &Storage) -> Result<Vec<(Urn, Metadata, Option<git::ObjectId>)>, Error> {
    let repo = git::Repository::open(storage.path())?;
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
    repo: &'r git::Repository,
    urn: &Urn,
    branch: &str,
) -> Result<Option<git::ObjectId>, Error> {
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

pub fn person(storage: &Storage, urn: &Urn, peer: &PeerId) -> anyhow::Result<Option<Person>> {
    use librad::git::types::{Namespace, Reference};

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
