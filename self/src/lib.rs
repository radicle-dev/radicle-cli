use std::ffi::OsString;

use anyhow::anyhow;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{keys, person, profile};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "self",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad self [<option>...]

Options

    --name       Show name
    --urn        Show URN
    --peer       Show Peer ID
    --profile    Show Profile ID
    --help       Show help
"#,
};

#[derive(Debug)]
enum Show {
    Name,
    Urn,
    Peer,
    Profile,
    All,
}

#[derive(Debug)]
pub struct Options {
    show: Show,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut show: Option<Show> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("name") if show.is_none() => {
                    show = Some(Show::Name);
                }
                Long("urn") if show.is_none() => {
                    show = Some(Show::Urn);
                }
                Long("peer") if show.is_none() => {
                    show = Some(Show::Peer);
                }
                Long("profile") if show.is_none() => {
                    show = Some(Show::Profile);
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                show: show.unwrap_or(Show::All),
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let storage = profile::read_only(&profile)?;

    match options.show {
        Show::Name => {
            if let Some(urn) = storage.config()?.user()? {
                if let Some(person) = person::get(&storage, &urn)? {
                    term::print(&person.subject().name.to_string());
                }
            }
        }
        Show::Profile => {
            term::print(profile.id());
        }
        Show::Peer => {
            term::print(storage.peer_id());
        }
        Show::Urn => {
            term::print(
                storage
                    .config()?
                    .user()?
                    .ok_or_else(|| anyhow!("no user found"))?,
            );
        }
        Show::All => all(&profile)?,
    }

    Ok(())
}

fn all(profile: &profile::Profile) -> anyhow::Result<()> {
    term::info!("Profile {}", term::format::secondary(profile.id()));

    let mut table = term::Table::default();
    let storage = profile::read_only(profile)?;

    if let Some(urn) = storage.config()?.user()? {
        if let Some(person) = person::get(&storage, &urn)? {
            table.push([
                String::from("Name"),
                term::format::tertiary(&person.subject().name),
            ]);
        }
        table.push([String::from("URN"), term::format::tertiary(&urn)]);
    }

    let peer_id = storage.peer_id();
    table.push([String::from("Peer ID"), term::format::tertiary(&peer_id)]);

    let ssh_short = keys::to_ssh_fingerprint(peer_id)?;
    table.push([
        String::from("Key (hash)"),
        term::format::tertiary(ssh_short),
    ]);

    let ssh_long = keys::to_ssh_key(peer_id)?;
    table.push([String::from("Key (full)"), term::format::tertiary(ssh_long)]);

    let git_path = profile.paths().git_dir();
    table.push([
        String::from("Storage (git)"),
        term::format::tertiary(git_path.display()),
    ]);

    let keys_path = profile.paths().keys_dir();
    table.push([
        String::from("Storage (keys)"),
        term::format::tertiary(keys_path.display()),
    ]);

    table.render_tree();

    Ok(())
}
