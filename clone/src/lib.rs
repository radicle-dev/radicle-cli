use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;
use librad::git::Urn;

use rad_terminal::args;
use rad_terminal::args::{Args, Error, Help};

pub const HELP: Help = Help {
    name: "clone",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad clone <urn> [--track] [<sync-option>...] [<option>...]

Options

    --track   Track the project after syncing (default: false)
    --help    Print help

Sync options

    See `rad sync --help`
"#,
};

#[derive(Debug)]
pub struct Options {
    urn: Urn,
    track: bool,
    sync: rad_sync::Options,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut urn: Option<Urn> = None;
        let mut track = false;
        let mut sync = rad_sync::Options::default();

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
                _ => {
                    let unparsed = std::iter::once(args::format(arg))
                        .chain(std::iter::from_fn(|| parser.value().ok()))
                        .collect();
                    let (sync_opts, unparsed) = rad_sync::Options::from_args(unparsed)?;

                    args::finish(unparsed)?;
                    sync = sync_opts;

                    break;
                }
            }
        }

        Ok((
            Options {
                urn: urn.ok_or_else(|| {
                    anyhow!("a URN to clone must be provided; see `rad clone --help`")
                })?,
                track,
                sync,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    rad_sync::run(rad_sync::Options {
        fetch: true,
        urn: Some(options.urn.clone()),
        ..options.sync
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
