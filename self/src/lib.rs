use std::ffi::OsString;

use rad_common::args::{Args, Error, Help};
use rad_common::{keys, person, profile};
use rad_terminal as term;

pub const HELP: Help = Help {
    name: "self",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad self [--help]
"#,
};

#[derive(Default, Eq, PartialEq)]
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
    let mut table = term::Table::default();

    let profile = profile::default()?;
    term::info!("Profile {}", term::format::secondary(profile.id()));

    let storage = profile::read_only(&profile)?;

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

    table.render_tree();

    Ok(())
}
