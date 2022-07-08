#![allow(clippy::or_fun_call)]
use std::ffi::OsString;

use anyhow::anyhow;

use radicle_common::{
    args::{Args, Error, Help},
    git, identity, project,
    seed::{self},
    sync,
};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "pull",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad pull [--seed <addr>]... [<option>...]

    Pulls changes into the current branch after optionally syncing.

Options

    --seed <addr>   Seed to sync from (may be specified multiple times)
    --help          Print help

"#,
};

#[derive(Debug)]
pub struct Options {
    seeds: Vec<sync::Seed<String>>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut seeds = Vec::new();

        if let Some(arg) = parser.next()? {
            match arg {
                Long("seed") => {
                    let seed = seed::parse_value(&mut parser)?;
                    seeds.push(seed);
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok((Options { seeds }, vec![]))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let (urn, repo) = project::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let _head = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|h| h.to_owned()))
        .ok_or(anyhow!("you must be on a branch to pull"))?;

    rad_sync::run(
        rad_sync::Options {
            origin: Some(identity::Origin::from_urn(urn)),
            seeds: options.seeds,
            mode: sync::Mode::Fetch,
            ..rad_sync::Options::default()
        },
        ctx,
    )?;

    term::blank();
    term::subcommand("git pull");

    let output = git::pull(std::path::Path::new("."), true)?;

    term::info!("{}", output);

    Ok(())
}
