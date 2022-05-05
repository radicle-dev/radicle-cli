use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::tracking::git::tracking;
use librad::git::Urn;
use librad::PeerId;

use rad_common::{keys, profile, project};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "untrack",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad untrack [<peer-id>] [--all]

    Must be run within a project working copy.

Options

    --help   Print help
"#,
};

/// Tool options.
#[derive(Debug)]
pub struct Options {
    pub peer: Option<PeerId>,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut peer: Option<PeerId> = None;
        let mut all = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("all") if peer.is_none() => {
                    all = true;
                }
                Value(val) if peer.is_none() => {
                    let val = val.to_string_lossy();

                    if let Ok(val) = PeerId::from_str(&val) {
                        peer = Some(val);
                    } else {
                        return Err(anyhow!("invalid <peer-id> '{}'", val));
                    }
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        if peer.is_none() && !all {
            return Err(Error::Usage.into());
        }

        Ok((Options { peer }, vec![]))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, _) =
        project::cwd().context("this command must be run in the context of a project")?;

    execute(&urn, options)
}

pub fn execute(urn: &Urn, options: Options) -> anyhow::Result<()> {
    term::info!(
        "Removing tracking relationship for {}...",
        term::format::dim(urn)
    );

    // TODO: Remove remote
    // TODO: Remove tracking branch

    let profile = profile::default()?;
    let signer = keys::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;

    if let Some(peer) = options.peer {
        tracking::untrack(
            &storage,
            urn,
            peer,
            tracking::UntrackArgs {
                policy: tracking::policy::Untrack::MustExist,
                prune: true,
            },
        )??;
        term::success!(
            "Tracking relationship {} removed for {}",
            peer,
            term::format::highlight(urn)
        );
    } else {
        tracking::untrack_all(
            &storage,
            urn,
            tracking::UntrackAllArgs {
                policy: tracking::policy::UntrackAll::Any,
                prune: true,
            },
        )?;
        term::success!(
            "Tracking relationships for {} removed",
            term::format::highlight(urn)
        );
    }

    Ok(())
}
