use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::PeerId;

use rad_common::seed::SeedOptions;
use rad_terminal::args::{Args, Error};

/// Tool options.
#[derive(Debug)]
pub struct Options {
    pub peer: PeerId,
    pub remote: bool,
    pub upstream: bool,
    pub sync: bool,
    pub fetch: bool,
    pub seed: SeedOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (seed, unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut peer: Option<PeerId> = None;
        let mut remote = true;
        let mut upstream = true;
        let mut sync = true;
        let mut fetch = true;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("peer") => {
                    peer = Some(
                        parser
                            .value()?
                            .parse()
                            .context("invalid value specified for '--peer'")?,
                    );
                }
                Long("remote") => remote = true,
                Long("upstream") => upstream = true,
                Long("sync") => sync = true,
                Long("fetch") => fetch = true,
                Long("no-remote") => remote = false,
                Long("no-upstream") => upstream = false,
                Long("no-sync") => sync = false,
                Long("no-fetch") => fetch = false,

                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if peer.is_none() => {
                    let val = val.to_string_lossy();

                    if let Ok(val) = PeerId::from_str(&val) {
                        peer = Some(val);
                    } else {
                        return Err(anyhow!("invalid <peer-id> '{}'", val));
                    }
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        Ok((
            Options {
                peer: peer.ok_or(Error::Usage)?,
                remote,
                sync,
                fetch,
                upstream,
                seed,
            },
            vec![],
        ))
    }
}
