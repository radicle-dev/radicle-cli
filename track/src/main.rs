use std::env;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::tracking;
use librad::git::Urn;
use librad::PeerId;

use rad_common::{keys, profile};
use rad_terminal::compoments as term;
use rad_terminal::compoments::Args;

const NAME: &str = "rad track";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &str = "Track project peers";
const USAGE: &str = r#"
USAGE
    rad track <urn> [--peer <peer-id>]

OPTIONS
    --peer <peer-id>   Peer ID to track (default: all)
    --help             Print help
"#;

#[derive(Debug)]
struct Options {
    urn: Urn,
    peer: Option<PeerId>,
    help: bool,
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
            urn: urn.ok_or_else(|| anyhow!("a URN to track must be specified"))?,
            peer,
            help,
        })
    }
}

fn main() {
    term::run_command::<Options>("Tracking", run);
}

fn run(options: Options) -> anyhow::Result<()> {
    if options.help {
        term::usage(NAME, VERSION, DESCRIPTION, USAGE);
        return Ok(());
    }

    term::info(&format!(
        "Establishing tracking relationship for {}...",
        term::format::highlight(&options.urn)
    ));

    let cfg = tracking::config::Config::default();
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;

    tracking::track(
        &storage,
        &options.urn,
        options.peer,
        cfg,
        tracking::policy::Track::Any,
    )??;

    if let Some(peer) = options.peer {
        term::success(&format!(
            "Tracking relationship {} established for {}",
            peer, options.urn
        ));
    } else {
        term::success(&format!(
            "Tracking relationship for {} established",
            options.urn
        ));
    }

    Ok(())
}
