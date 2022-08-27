use std::convert::From;
use std::ffi::OsString;
use std::fs;
use std::str::FromStr;

use anyhow::anyhow;

use librad::git::Urn;

use radicle_common::args::{Args, Error, Help};
use radicle_common::profile::ProfileId;
use radicle_common::{keys, profile, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "rm",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad rm <urn | profile-id> [<option>...]

Options

    --no-confirm        Do not ask for confirmation before removal
    --no-passphrase     If profile id given, bypass passphrase prompt and
                        do not read environment variable `RAD_PASSPHRASE`
                        (default: false)
    --help              Print help
"#,
};

enum Object {
    Project(Urn),
    Profile(ProfileId),
    Unknown(String),
}

impl From<&str> for Object {
    fn from(value: &str) -> Self {
        if let Ok(urn) = Urn::from_str(value) {
            Object::Project(urn)
        } else if let Ok(id) = ProfileId::from_str(value) {
            Object::Profile(id)
        } else {
            Object::Unknown(value.to_owned())
        }
    }
}

pub struct Options {
    object: Object,
    confirm: bool,
    passphrase: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut object: Option<Object> = None;
        let mut confirm = true;
        let mut passphrase = true;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("no-confirm") => {
                    confirm = false;
                }
                Long("no-passphrase") => {
                    passphrase = false;
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
                    anyhow!("Urn or profile id to remove must be provided; see `rad rm --help`")
                })?,
                confirm,
                passphrase,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    term::warning("Experimental tool; use at your own risk!");

    match &options.object {
        Object::Project(urn) => {
            let profile = ctx.profile()?;
            let storage = profile::read_only(&profile)?;
            let monorepo = profile.paths().git_dir();

            if project::get(&storage, urn)?.is_none() {
                anyhow::bail!("project {} does not exist", &urn);
            }
            let namespace = monorepo
                .join("refs")
                .join("namespaces")
                .join(&urn.encode_id());

            if !options.confirm
                || term::confirm(format!(
                    "Are you sure you would like to delete {}?",
                    term::format::dim(namespace.display())
                ))
            {
                rad_untrack::execute(urn, rad_untrack::Options { peer: None }, &profile)?;
                fs::remove_dir_all(namespace)?;
                term::success!("Successfully removed project {}", &urn);
            }
        }
        Object::Profile(id) => {
            let profile = ctx.profile()?;
            if profile.id() == id {
                anyhow::bail!("Cannot remove active profile; see `rad auth --help`");
            } else {
                let profile = profile::get(id)?;
                let read_only = profile::read_only(&profile)?;
                let config = read_only.config()?;
                let username = config.user_name()?;

                if !options.confirm
                    || term::confirm(format!(
                        "Are you sure you would like to delete {} ({})?",
                        term::format::dim(id),
                        term::format::dim(username)
                    ))
                {
                    if options.passphrase {
                        let is_tty = atty::is(atty::Stream::Stdin);
                        let secret_input = match keys::read_env_passphrase() {
                            Ok(input) => input,
                            _ => term::switch_secret_input(is_tty)?,
                        };

                        if keys::load_secret_key(&profile, secret_input).is_ok() {
                            profile::remove(&profile)?;
                        } else {
                            anyhow::bail!(format!("Invalid passphrase supplied."));
                        }
                    } else {
                        profile::remove(&profile)?;
                    }

                    term::success!("Successfully removed profile {}", id);
                }
            }
        }
        Object::Unknown(arg) => {
            anyhow::bail!(format!("Object must be an Urn or a profile id: {}", arg));
        }
    }

    Ok(())
}
