#![allow(clippy::or_fun_call)]
use std::ffi::OsString;

use anyhow::anyhow;

use radicle_common::args::{Args, Error, Help};
use radicle_common::git;
use radicle_common::project;
use radicle_common::seed;
use radicle_common::seed::SeedOptions;
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "pull",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad pull [--seed <host>] [<option>...]

    Pulls changes into the current branch.

Options

    --seed <host>   Seed to clone from
    --help          Print help

"#,
};

#[derive(Debug)]
pub struct Options {
    seed: Option<seed::Address>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (SeedOptions(seed), unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok((Options { seed }, vec![]))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, repo) = project::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let _head = repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(|h| h.to_owned()))
        .ok_or(anyhow!("you must be on a branch to pull"))?;

    rad_sync::run(rad_sync::Options {
        fetch: true,
        origin: Some(project::Origin {
            urn,
            seed: options.seed,
        }),
        seed: None,
        identity: false,
        refs: rad_sync::Refs::All,
        push_self: false,
        verbose: false,
    })?;

    term::blank();
    term::subcommand("git pull");

    let output = git::pull(std::path::Path::new("."), true)?;

    term::info!("{}", output);

    Ok(())
}
