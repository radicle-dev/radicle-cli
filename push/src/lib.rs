use std::ffi::OsString;

use radicle_common::args::{Args, Error, Help};
use radicle_common::seed;
use radicle_common::seed::SeedOptions;

use anyhow::anyhow;

pub const HELP: Help = Help {
    name: "push",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad push [--seed <host>] [--all] [--[no-]sync] [<option>...]

    By default, only the current branch is synced.

Options

    --seed <host>       Use the given seed node for syncing
    --all               Push and sync all branches (default: false)
    --sync              Sync after pushing to the "rad" remote (default: true)
    --no-sync           Do not sync after pushing to the "rad" remote
    --help              Print help

Git options

    -f, --force           Force push
    -u, --set-upstream    Set upstream tracking branch

"#,
};

#[derive(Default, Debug)]
pub struct Options {
    pub seed: Option<seed::Address>,
    pub verbose: bool,
    pub force: bool,
    pub all: bool,
    pub identity: bool,
    pub set_upstream: bool,
    pub sync: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (SeedOptions(seed), unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut verbose = false;
        let mut force = false;
        let mut identity = true;
        let mut all = false;
        let mut sync = true;
        let mut set_upstream = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("all") => {
                    all = true;
                }
                Long("set-upstream") | Short('u') => {
                    set_upstream = true;
                }
                Long("sync") => {
                    sync = true;
                }
                Long("no-sync") => {
                    sync = false;
                }
                Long("identity") => {
                    identity = true;
                }
                Long("no-identity") => {
                    identity = false;
                }
                Long("force") | Short('f') => {
                    force = true;
                }
                arg => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        Ok((
            Options {
                seed,
                force,
                all,
                set_upstream,
                identity,
                sync,
                verbose,
            },
            vec![],
        ))
    }
}
