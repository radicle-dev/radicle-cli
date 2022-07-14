#![allow(clippy::or_fun_call)]
use std::convert::TryFrom;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context as _};

use librad::PeerId;

use radicle_common::args::{Args, Error, Help};
use radicle_common::json;
use radicle_common::Interactive;
use radicle_common::{git, keys, profile, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "init",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad init [<path>] [<option>...]

Options

    --name               Name of the project
    --description        Description of the project
    --default-branch     The default branch of the project
    --set-upstream, -u   Setup the upstream of the default branch
    --no-confirm         Don't ask for confirmation during setup
    --help               Print help
"#,
};

#[derive(Default)]
pub struct Options {
    pub path: Option<PathBuf>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub branch: Option<String>,
    pub interactive: Interactive,
    pub set_upstream: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut path: Option<PathBuf> = None;

        let mut name = None;
        let mut description = None;
        let mut branch = None;
        let mut interactive = Interactive::Yes;
        let mut set_upstream = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("name") if name.is_none() => {
                    let value = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!(
                            "invalid project name specified with `--name`"
                        ))?
                        .to_owned();
                    name = Some(value);
                }
                Long("description") if description.is_none() => {
                    let value = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!(
                            "invalid project description specified with `--description`"
                        ))?
                        .to_owned();

                    description = Some(value);
                }
                Long("default-branch") if branch.is_none() => {
                    let value = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow::anyhow!(
                            "invalid branch specified with `--default-branch`"
                        ))?
                        .to_owned();

                    branch = Some(value);
                }
                Long("set-upstream") | Short('u') => {
                    set_upstream = true;
                }
                Long("no-confirm") => {
                    interactive = Interactive::No;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if path.is_none() => {
                    path = Some(val.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                path,
                name,
                description,
                branch,
                interactive,
                set_upstream,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;

    if git::check_version().is_err() {
        term::warning(&format!(
            "Your git version is unsupported, please upgrade to {} or later",
            git::VERSION_REQUIRED,
        ));
        term::blank();
    }
    init(options, &profile)
}

pub fn init(options: Options, profile: &profile::Profile) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let path = options.path.unwrap_or_else(|| cwd.clone());
    let path = path.as_path().canonicalize()?;
    let interactive = options.interactive;

    term::headline(&format!(
        "Initializing local 🌱 project in {}",
        if path == cwd {
            term::format::highlight(".")
        } else {
            term::format::highlight(&path.display())
        }
    ));

    let repo = git::Repository::open(&path)?;
    if let Ok(remote) = git::rad_remote(&repo) {
        bail!(
            "repository is already initialized with remote {}",
            remote.url
        );
    }

    let signer = term::signer(profile)?;
    let storage = keys::storage(profile, signer.clone())?;

    let head: String = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|h| h.to_owned()))
        .ok_or_else(|| anyhow!("error: repository head does not point to any commits"))?;

    let name = options.name.unwrap_or_else(|| {
        let default = path.file_name().map(|f| f.to_string_lossy().to_string());
        term::text_input("Name", default).unwrap()
    });
    let description = options
        .description
        .unwrap_or_else(|| term::text_input("Description", None).unwrap());
    let branch = options.branch.unwrap_or_else(|| {
        if interactive.yes() {
            term::text_input("Default branch", Some(head)).unwrap()
        } else {
            head
        }
    });

    let mut spinner = term::spinner("Initializing...");
    let payload = project::payload(name, description, branch.clone());

    match project::create(payload, &storage).and_then(|proj| {
        project::init(&proj, &repo, &storage, profile.paths(), signer).map(|_| proj)
    }) {
        Ok(proj) => {
            let urn = proj.urn();

            spinner.message(format!(
                "Project {} created",
                term::format::highlight(&proj.subject().name)
            ));
            spinner.finish();

            if interactive.no() {
                term::blob(json::to_string_pretty(&proj.payload())?);
                term::blank();
            }

            if options.set_upstream || git::branch_remote(&repo, &branch).is_err() {
                let branch = git::RefLike::try_from(branch)?;
                let branch = git::OneLevel::from(branch);

                // Setup eg. `master` -> `rad/master`
                git::set_upstream(&repo, &git::rad_remote(&repo)?, branch)?;
            }

            // Setup radicle signing key.
            self::setup_signing(storage.peer_id(), &repo, interactive)?;

            term::blank();
            term::info!(
                "Your project id is {}. You can show it any time by running:",
                term::format::highlight(&urn.to_string())
            );
            term::indented(&term::format::secondary("rad ."));

            term::blank();
            term::info!("To publish your project to the network, run:");
            term::indented(&term::format::secondary("rad push"));
            term::blank();
        }
        Err(err) => {
            spinner.failed();
            term::blank();

            use radicle_common::identities::git::validation;
            use radicle_common::identities::git::Error;

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

/// Setup radicle key as commit signing key in repository.
pub fn setup_signing(
    peer_id: &PeerId,
    repo: &git::Repository,
    interactive: Interactive,
) -> anyhow::Result<()> {
    let repo = repo
        .workdir()
        .ok_or(anyhow!("cannot setup signing in bare repository"))?;
    let key = keys::to_ssh_fingerprint(peer_id)?;
    let yes = if !git::is_signing_configured(repo)? {
        term::headline(&format!(
            "Configuring 🌱 signing key {}...",
            term::format::tertiary(key)
        ));
        true
    } else if interactive.yes() {
        term::confirm(&format!(
            "Configure 🌱 signing key {} in local checkout?",
            term::format::tertiary(key),
        ))
    } else {
        true
    };

    if yes {
        match git::write_gitsigners(repo, [peer_id]) {
            Ok(file) => {
                git::ignore(repo, file.as_path())?;

                term::success!("Created {} file", term::format::tertiary(file.display()));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let ssh_key = keys::to_ssh_key(peer_id)?;
                let gitsigners = term::format::tertiary(".gitsigners");
                term::success!("Found existing {} file", gitsigners);

                let ssh_keys =
                    git::read_gitsigners(repo).context("error reading .gitsigners file")?;

                if ssh_keys.contains(&ssh_key) {
                    term::success!("Signing key is already in {} file", gitsigners);
                } else if term::confirm(&format!("Add signing key to {}?", gitsigners)) {
                    git::add_gitsigners(repo, [peer_id])?;
                }
            }
            Err(err) => {
                return Err(err.into());
            }
        }
        git::configure_signing(repo, peer_id)?;

        term::success!(
            "Signing configured in {}",
            term::format::tertiary(".git/config")
        );
    }
    Ok(())
}
