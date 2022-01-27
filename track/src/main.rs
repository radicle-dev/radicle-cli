use librad::git::tracking;

use rad_common::{keys, profile};
use rad_terminal::components as term;
use rad_track::options::Options;
use rad_track::HELP;

fn main() {
    term::run_command::<Options>(HELP, "Tracking", run);
}

fn run(options: Options) -> anyhow::Result<()> {
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
