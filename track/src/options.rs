use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::PeerId;

use radicle_common::args::{Args, Error};
use radicle_common::seed;

/// Tool options.
#[derive(Debug)]
pub struct Options {
    pub peer: Option<PeerId>,
    pub upstream: bool,
    pub sync: bool,
    pub fetch: bool,
    pub local: bool,
    pub seed: Option<seed::Address>,
    pub verbose: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut peer: Option<PeerId> = None;
        let mut local: Option<bool> = None;
        let mut upstream = true;
        let mut sync = true;
        let mut fetch = true;
        let mut verbose = false;
        let mut seed = None;

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
                Long("seed") if seed.is_none() => {
                    let value = parser.value()?;
                    let value = value.to_string_lossy();
                    let value = value.as_ref();

                    seed = Some(seed::Address::from_str(value)?);
                }
                Long("sync") => sync = true,
                Long("local") => local = Some(true),
                Long("remote") => local = Some(false),
                Long("no-upstream") => upstream = false,
                Long("no-sync") => sync = false,
                Long("no-fetch") => fetch = false,

                Long("verbose") | Short('v') => verbose = true,
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

        // If a seed is specified, and `--local` isn't, we assume remote.
        // Otherwise, we assume local.
        let local = if let Some(local) = local {
            local
        } else {
            seed.is_none()
        };

        Ok((
            Options {
                peer,
                sync,
                fetch,
                upstream,
                local,
                seed,
                verbose,
            },
            vec![],
        ))
    }
}
