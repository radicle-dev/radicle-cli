use anyhow::Context as _;

use librad::git::tracking::git::tracking;
use rad_terminal::args::Help;

use rad_common::{keys, profile, project};
use rad_terminal::components as term;

pub use rad_track::Options;

pub const HELP: Help = Help {
    name: "untrack",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad untrack [<urn>] [--peer <peer-id>]

    If <urn> isn't specified, the working copy project will be used.

Options

    --peer <peer-id>   Peer ID to track (default: all)
    --help             Print help
"#,
};

pub fn run(options: Options) -> anyhow::Result<()> {
    let urn = if let Some(urn) = &options.urn {
        urn.clone()
    } else {
        project::urn().context("a URN must be specified")?
    };

    term::info!(
        "Removing tracking relationship for {}...",
        term::format::dim(&urn)
    );

    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;

    if let Some(peer) = options.peer {
        tracking::untrack(&storage, &urn, peer, tracking::policy::Untrack::MustExist)??;
        term::success!(
            "Tracking relationship {} removed for {}",
            peer,
            term::format::highlight(urn)
        );
    } else {
        tracking::untrack_all(&storage, &urn, tracking::policy::UntrackAll::Any)?.for_each(drop);
        term::success!(
            "Tracking relationships for {} removed",
            term::format::highlight(urn)
        );
    }

    Ok(())
}
