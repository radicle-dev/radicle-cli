use anyhow::bail;

use librad::canonical::Cstring;
use librad::identities::payload::{self};

use rad_common::{keys, profile, project};
use rad_init::{Options, HELP};
use rad_terminal::components as term;

fn main() {
    term::run_command::<Options>(HELP, "Project initialization", run);
}

fn run(_options: Options) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let path = cwd.as_path();
    let name = path.file_name().unwrap().to_string_lossy().to_string();

    let repo = project::repository()?;
    if let Ok(remote) = project::remote(&repo) {
        bail!(
            "repository is already initialized with remote {}",
            remote.url
        );
    }

    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (signer, storage) = keys::storage(&profile, sock)?;

    term::headline(&format!(
        "Initializing local 🌱 project {}",
        term::format::highlight(&name)
    ));

    let head: String = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|h| h.to_owned()))
        .unwrap_or_else(|| String::from("master"));
    let description = term::text_input("Description", None);
    let branch = term::text_input("Default branch", Some(head));

    let spinner = term::spinner(&format!(
        "Initializing new project in {}...",
        path.display()
    ));

    let payload = payload::Project {
        name: Cstring::from(name),
        description: Some(Cstring::from(description)),
        default_branch: Some(Cstring::from(branch.clone())),
    };

    match project::create(&storage, signer, &profile, payload) {
        Ok(proj) => {
            let urn = proj.urn();

            spinner.finish();

            term::success(&format!(
                "Project initialized: {}",
                term::format::highlight(&urn.to_string())
            ));
            term::blank();
            term::tip("To publish, run `rad push`.");
        }
        Err(err) => {
            spinner.finish();

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
