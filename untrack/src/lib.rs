use librad::git::tracking::git::tracking;
use rad_terminal::args::Help;

use rad_common::{keys, profile};
use rad_terminal::components as term;

pub use rad_track::Options;

pub const HELP: Help = Help {
    name: "untrack",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad untrack <urn> [--peer <peer-id>]

Options

    --peer <peer-id>   Peer ID to track (default: all)
    --help             Print help
"#,
};

pub fn run(options: Options) -> anyhow::Result<()> {
    term::info!(
        "Removing tracking relationship for {}...",
        term::format::highlight(&options.urn)
    );

    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;

    if let Some(peer) = options.peer {
        tracking::untrack(
            &storage,
            &options.urn,
            peer,
            tracking::policy::Untrack::MustExist,
        )??;

        term::success!("Tracking relationship {} removed for {}", peer, options.urn);
    } else {
        tracking::untrack_all(&storage, &options.urn, tracking::policy::UntrackAll::Any)?
            .for_each(drop);

        term::success!("Tracking relationships for {} removed", options.urn);
    }

    Ok(())
}
