use std::ffi::OsString;

use anyhow::bail;

use librad::canonical::Cstring;
use librad::identities::payload::{self};

use rad_common::{keys, person, profile, project};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "init",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad init [<option>...]

Options

    --help    Print help
"#,
};

pub struct Options {}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options {}, vec![]))
    }
}

pub fn run(_options: Options) -> anyhow::Result<()> {
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
    let identity = person::local(&storage)?;

    term::headline(&format!(
        "Initializing local 🌱 project {}",
        term::format::highlight(&name)
    ));

    let head: String = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|h| h.to_owned()))
        .unwrap_or_else(|| String::from("master"));
    let description: String = term::text_input("Description", None)?;
    let branch = term::text_input("Default branch", Some(head))?;
    let spinner = term::spinner("Initializing...");

    let payload = payload::Project {
        name: Cstring::from(name),
        description: Some(Cstring::from(description)),
        default_branch: Some(Cstring::from(branch.clone())),
    };

    match project::create(&repo, identity, &storage, signer, &profile, payload) {
        Ok(proj) => {
            let urn = proj.urn();

            spinner.finish();

            term::blank();
            term::info!(
                "Your project id is {}. You can show it any time by running:",
                term::format::highlight(&urn.to_string())
            );
            term::indented(&term::format::secondary("rad show --project"));

            term::blank();
            term::info!("To publish your project to the network, run:");
            term::indented(&term::format::secondary("rad push"));
        }
        Err(err) => {
            spinner.failed();
            term::blank();

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
