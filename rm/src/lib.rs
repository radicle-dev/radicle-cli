use std::convert::From;
use std::ffi::OsString;
use std::fs;
use std::str::FromStr;

use anyhow::anyhow;

use librad::git::tracking::git::tracking;
use librad::git::Urn;
use librad::PeerId;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{keys, profile, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "rm",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad rm <urn | peer-id> [<option>...]

    Removes a project if URN is given or user if Peer ID is given.

Options

    --no-confirm        Do not ask for confirmation before removal
                        (default: false)
    --no-passphrase     If Peer ID is given, bypass passphrase prompt and
                        neither read environment variable `RAD_PASSPHRASE`
                        nor standard input stream (default: false)
    --stdin             Read passphrase from stdin (default: false)
    --help              Print help
"#,
};

enum Object {
    Project(Urn),
    User(PeerId),
    Unknown(String),
}

impl From<&str> for Object {
    fn from(value: &str) -> Self {
        if let Ok(urn) = Urn::from_str(value) {
            Object::Project(urn)
        } else if let Ok(peer_id) = PeerId::from_str(value) {
            Object::User(peer_id)
        } else {
            Object::Unknown(value.to_owned())
        }
    }
}

pub struct Options {
    object: Object,
    confirm: bool,
    passphrase: bool,
    stdin: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut object: Option<Object> = None;
        let mut confirm = true;
        let mut passphrase = true;
        let mut stdin = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("no-confirm") => {
                    confirm = false;
                }
                Long("no-passphrase") => {
                    passphrase = false;
                }
                Long("stdin") => {
                    stdin = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if object.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Object::from(val.as_ref());
                    object = Some(val);
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                object: object.ok_or_else(|| {
                    anyhow!("Urn or peer id to remove must be provided; see `rad rm --help`")
                })?,
                confirm,
                passphrase,
                stdin,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    term::warning("Experimental tool; use at your own risk!");

    let profile = ctx.profile()?;

    match &options.object {
        Object::Project(urn) => {
            let signer = term::signer(&profile)?;
            let storage = keys::storage(&profile, signer)?;

            if project::get(&storage, urn)?.is_none() {
                anyhow::bail!("project {} does not exist", &urn);
            }
            let monorepo = profile.paths().git_dir();
            let namespace = monorepo
                .join("refs")
                .join("namespaces")
                .join(&urn.encode_id());

            if options.confirm
                && !term::confirm(format!(
                    "Are you sure you would like to delete {}?",
                    term::format::dim(namespace.display())
                ))
            {
                return Ok(());
            }

            let all_untracked = tracking::untrack_all(
                &storage,
                urn,
                tracking::UntrackAllArgs {
                    policy: tracking::policy::UntrackAll::Any,
                    prune: true,
                },
            )?;
            fs::remove_dir_all(namespace)?;

            if let Ok((cwd_urn, cwd_repo)) = project::cwd() {
                if cwd_urn == *urn {
                    for p in all_untracked.untracked.flatten() {
                        let err =
                            term::remote::remove(&p.remote.to_string(), &storage, &cwd_repo, urn);
                        if let Err(err) = err {
                            eprintln!("unable to remove git remote '{}': {}", p.remote, err);
                        }
                    }
                }
            };

            term::success!("Successfully removed project {}", &urn);
        }
        Object::User(peer_id) => {
            let profiles = profile::list()?;
            let storage = profile::read_only(&profile)?;
            if storage.peer_id() != peer_id {
                if let Some((storage, other)) = profiles
                    .iter()
                    .map(|p| (profile::read_only(p), p))
                    .find(|(s, _)| match s {
                        Ok(s) => s.peer_id() == peer_id,
                        Err(_) => false,
                    })
                {
                    let read_only = storage?;
                    let config = read_only.config()?;
                    let username = config.user_name()?;
                    if options.confirm
                        && !term::confirm(format!(
                            "Are you sure you would like to remove {} ({})?",
                            term::format::dim(peer_id),
                            term::format::dim(username)
                        ))
                    {
                        return Ok(());
                    }
                    if options.passphrase {
                        let passphrase = term::read_passphrase(options.stdin, false)?;
                        if keys::load_secret_key(other, passphrase).is_err() {
                            anyhow::bail!(format!("Invalid passphrase supplied."));
                        }
                    }
                    profile::remove(other)?;
                    term::success!("Successfully removed user {}", peer_id);
                } else {
                    anyhow::bail!("No user found with Peer ID: {}", peer_id);
                }
            } else {
                anyhow::bail!("Cannot remove active user; see `rad rm --help`");
            }
        }
        Object::Unknown(arg) => {
            anyhow::bail!(format!("Object must be an URN or a Peer ID: {}", arg));
        }
    }

    Ok(())
}
