// TODO: Support '--help' flag
// TODO: Take default branch from current git branch
use std::thread::sleep;
use std::time::Duration;

use anyhow::bail;

use librad::canonical::Cstring;
use librad::identities::payload::{self};

use rad_clib::keys::ssh::SshAuthSock;

use rad_common::{keys, profile, project};
use rad_terminal::compoments as term;

fn main() -> anyhow::Result<()> {
    match run() {
        Ok(()) => Ok(()),
        Err(err) => {
            term::format::error("Project initialization failed", &err);
            term::blank();

            std::process::exit(1);
        }
    }
}

fn run() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let path = cwd.as_path();
    let name = path.file_name().unwrap().to_string_lossy().to_string();

    term::headline(&format!(
        "Initializing local 🌱 project {}",
        term::format::highlight(&name)
    ));

    let _repo = project::repository()?;
    let profile = profile::default()?;
    let (signer, storage) = keys::storage(&profile, SshAuthSock::default())?;

    let description = term::text_input("Description", None);
    let branch = term::text_input("Default branch", Some("master".to_string()));

    term::blank();

    let spinner = term::spinner(&format!(
        "Initializing new project in {}...",
        path.display()
    ));
    sleep(Duration::from_secs(1));

    let payload = payload::Project {
        name: Cstring::from(name),
        description: Some(Cstring::from(description)),
        default_branch: Some(Cstring::from(branch.clone())),
    };

    match project::create(&storage, signer, &profile, payload) {
        Ok(proj) => {
            let urn = proj.urn();

            spinner.finish_and_clear();

            term::success(&format!(
                "Project initialized with URN {}",
                term::format::highlight(&urn.to_string())
            ));
            term::info(&format!(
                "To publish, run `rad publish` or `git push rad {}`",
                branch
            ));
        }
        Err(err) => {
            spinner.finish_and_clear();

            use rad_common::identities::git::existing::Error;
            use rad_common::identities::git::validation;

            match err.downcast_ref::<Error>() {
                Some(Error::Validation(validation::Error::UrlMismatch { found, .. })) => {
                    bail!(
                        "this repository is already initialized with remote {}",
                        found
                    );
                }
                Some(Error::Validation(validation::Error::MissingDefaultBranch { .. })) => bail!(
                    "the `{}` branch was either not found, or has no commits",
                    branch
                ),
                Some(_) | None => return Err(err),
            }
        }
    }

    Ok(())
}
