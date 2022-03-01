use std::ffi::OsString;

use anyhow::anyhow;

use rad_common::git;
use rad_common::project;
use rad_common::seed;
use rad_common::seed::SeedOptions;
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

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
    let (urn, _) = project::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    rad_sync::run(rad_sync::Options {
        fetch: true,
        origin: Some(project::Origin {
            urn,
            seed: options.seed,
        }),
        seed: None,
        identity: false,
        push_self: false,
        verbose: false,
        force: false,
    })?;

    term::blank();
    term::subcommand("git pull");

    let output = git::pull(std::path::Path::new("."), true)?;

    term::info!("{}", output);

    Ok(())
}
