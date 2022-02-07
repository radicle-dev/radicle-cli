use librad::git::tracking;

use rad_common::{keys, profile};
use rad_terminal::args::Help;
use rad_terminal::components as term;

mod options;
pub use options::Options;

pub const HELP: Help = Help {
    name: "track",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad track <urn> [--peer <peer-id>]

Options

    --peer <peer-id>   Peer ID to track (default: all)
    --help             Print help
"#,
};

pub fn run(options: Options) -> anyhow::Result<()> {
    term::info!(
        "Establishing tracking relationship for {}...",
        term::format::highlight(&options.urn)
    );

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
        term::success!(
            "Tracking relationship {} established for {}",
            peer,
            options.urn
        );
    } else {
        term::success!("Tracking relationship for {} established", options.urn);
    }

    Ok(())
}
