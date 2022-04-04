use std::path::PathBuf;
use std::{env, error, fs};

pub use git2::{Note, Repository};
pub use nonempty::NonEmpty;
pub use serde::{Deserialize, Serialize};

use librad::git::storage::Storage;
use librad::git::Urn;
use librad::crypto::keystore::pinentry::SecUtf8;
use librad::profile::LNK_HOME;

use super::{git, keys, profile, project, test};
use rad_terminal::components as term;

pub type BoxedError = Box<dyn error::Error>;

pub const USER_PASS: &str = "password";

pub mod setup {
    use super::*;

    #[derive(Eq, PartialEq)]
    pub enum Steps {
        CreateLnkHome,
        CreateGitRepo,
        InitGitRepo,
        CreateCommits,
    }

    pub fn with_steps(steps: Vec<Steps>) -> Result<(), BoxedError> {
        if steps.contains(&Steps::CreateLnkHome) {
            env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        }
        if steps.contains(&Steps::CreateGitRepo) {
            fs::create_dir(repo_path())?;
        }
        if steps.contains(&Steps::InitGitRepo) {
            git::git(&repo_path(), ["init"])?;
        }
        if steps.contains(&Steps::CreateCommits) {
            fs::File::create(repo_path().join("README.md"))?;
            git::git(&repo_path(), ["add", "README.md"])?;
            git::git(&repo_path(), ["commit", "-m", "Initial commit"])?;
        }
        Ok(())
    }

    pub fn environment() -> Result<(Storage, Urn, Repository), BoxedError>{
        let repo = git::Repository::open(test::setup::repo_path())?;
        let urn = project::rad_remote(&repo)?.url.urn;

        let profile = profile::default()?;
        let sock = keys::ssh_auth_sock();
        let (_, storage) = keys::storage(&profile, sock)?;

        Ok((storage, urn, repo)) 
    }

    pub fn lnk_home() -> Result<(), BoxedError> {
        env::set_var(LNK_HOME, env::current_dir()?.join("lnk_home"));
        Ok(())
    }

    pub fn repo_path() -> PathBuf {
        env::current_dir().unwrap().join("repo_dir")
    }
}

pub mod teardown {
    use super::*;
    pub fn profiles() -> Result<(), BoxedError> {
        for profile in profile::list()? {
            let pass = term::pwhash(SecUtf8::from(test::USER_PASS));
            keys::remove(&profile, pass, keys::ssh_auth_sock())?;
        }
        Ok(())
    }
}
