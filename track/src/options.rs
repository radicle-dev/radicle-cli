use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::Urn;
use librad::PeerId;

use rad_terminal::components::Args;

/// Tool options.
/// Nb. These options are also used by the `untrack` tool.
#[derive(Debug)]
pub struct Options {
    pub urn: Urn,
    pub peer: Option<PeerId>,
    pub help: bool,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Options> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();
        let mut urn: Option<Urn> = None;
        let mut peer: Option<PeerId> = None;
        let mut help = false;

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
                Long("help") => {
                    help = true;
                }
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        Ok(Options {
            urn: urn.ok_or_else(|| anyhow!("a tracking URN must be specified"))?,
            peer,
            help,
        })
    }
}
