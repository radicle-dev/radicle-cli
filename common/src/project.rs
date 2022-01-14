use std::path::Path;

use anyhow::{Error, Result};

use git2::Repository;

use librad::{
    crypto::BoxedSigner,
    git::{identities::Project, storage::Storage},
    identities::payload::{self},
    profile::Profile,
};

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
