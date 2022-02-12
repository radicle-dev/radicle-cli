use anyhow::Context as _;
use librad::git::tracking;

use rad_common::{keys, profile, project};
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

    rad track [<urn>] [--peer <peer-id>]

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
        "Establishing tracking relationship for {}...",
        term::format::highlight(&urn)
    );

    let cfg = tracking::config::Config::default();
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;

    tracking::track(
        &storage,
        &urn,
        options.peer,
        cfg,
        tracking::policy::Track::Any,
    )??;

    if let Some(peer) = options.peer {
        term::success!("Tracking relationship {} established for {}", peer, urn);
    } else {
        term::success!("Tracking relationship for {} established", urn);
    }

    Ok(())
}
