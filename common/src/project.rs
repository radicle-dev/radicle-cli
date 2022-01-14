use std::path::Path;

use anyhow::{Error, Result};

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
    let path = Path::new("../").to_path_buf();
    let paths = profile.paths().clone();
    let whoami = project::WhoAmI::from(None);
    let delegations = Vec::new().into_iter().collect();
    match project::create::<payload::Project>(
        storage,
        paths,
        signer,
        whoami,
        delegations,
        payload,
        vec![],
        rad_identities::project::Creation::Existing { path },
    ) {
        Ok(project) => Ok(project),
        Err(err) => {
            term::error("Project could not be initialized.");
            term::format::error_detail(&format!("{}", err));
            Err(err)
        }
    }
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
        Ok(remote) => match remote {
            Some(remote) => Ok(remote),
            None => {
                let msg = "Could not find radicle URL in git config. Did you run `rad init`?";
                term::error(msg);
                Err(anyhow::Error::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    msg,
                )))
            }
        },
        Err(err) => {
            term::error("Could not find radicle entry in git config. Did you run `rad init`?");
            Err(anyhow::Error::new(err))
        }
    }
}
