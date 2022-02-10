use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;
use librad::git::Urn;

use rad_common::seed::SeedOptions;
use rad_terminal::args::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "clone",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad clone <urn> [--track] [--seed <host>] [<option>...]

Options

    --track         Track the project after syncing (default: false)
    --seed <host>   Seed to clone from
    --help          Print help

"#,
};

#[derive(Debug)]
pub struct Options {
    urn: Urn,
    track: bool,
    seed: SeedOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (seed, unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut urn: Option<Urn> = None;
        let mut track = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("track") => {
                    track = true;
                }
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                urn: urn.ok_or_else(|| {
                    anyhow!("a URN to clone must be provided; see `rad clone --help`")
                })?,
                track,
                seed,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    rad_sync::run(rad_sync::Options {
        fetch: true,
        urn: Some(options.urn.clone()),
        seed: options.seed,
        verbose: false,
        force: false,
    })?;
    rad_checkout::run(rad_checkout::Options {
        urn: options.urn.clone(),
    })?;

    if options.track {
        rad_track::run(rad_track::Options {
            urn: options.urn,
            peer: None,
        })?;
    }
    Ok(())
}
