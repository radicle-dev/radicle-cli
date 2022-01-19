use std::path::Path;

use anyhow::{anyhow, Context as _, Error, Result};

use git2::Repository;

use librad::crypto::BoxedSigner;
use librad::git::identities::Project;
use librad::git::local::url::LocalUrl;
use librad::git::storage::Storage;
use librad::git::types::remote::Remote;
use librad::identities::payload::{self};
use librad::profile::Profile;
use librad::reflike;

use rad_identities::{self, project};
use rad_terminal::compoments as term;

pub fn create(
    storage: &Storage,
    signer: BoxedSigner,
    profile: &Profile,
    payload: payload::Project,
) -> Result<Project, Error> {
    // Currently, radicle link adds the project name to the path, so we're forced to
    // have them match, and specify the parent folder instead of the current folder.
    let path = Path::new("..").to_path_buf();
    let paths = profile.paths().clone();
    let whoami = project::WhoAmI::from(None);
    let delegations = Vec::new().into_iter().collect();

    project::create::<payload::Project>(
        storage,
        paths,
        signer,
        whoami,
        delegations,
        payload,
        vec![],
        rad_identities::project::Creation::Existing { path },
    )
}

pub fn repository() -> Result<Repository, Error> {
    match Repository::open(".") {
        Ok(repo) => Ok(repo),
        Err(err) => {
            term::error("This is not a git repository.");
            Err(anyhow::Error::new(err))
        }
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
