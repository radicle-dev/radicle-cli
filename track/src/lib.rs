use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::tracking;

use rad_common::{keys, profile, project, seed};
use rad_terminal::args::Help;
use rad_terminal::components as term;

mod options;
pub use options::Options;

// TODO: Add `--upstream-prefix` to specify a branch prefix, eg. `remotes/`.
pub const HELP: Help = Help {
    name: "track",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad track <peer-id> [--seed <host>] [--no-sync] [--no-remote] [--no-upstream] [--no-fetch]

    Track a peer in the context of the current project. By default, a remote is created in the
    repository and an upstream tracking branch is setup.

Options

    --seed <host>                Seed host to fetch refs from
    --remote, --no-remote        Setup a remote for the peer (default: yes)
    --upstream, --no-upstream    Setup a tracking branch for the remote (default: yes)
    --sync, --no-sync            Sync the peer's refs (default: yes)
    --fetch, --no-fetch          Fetch the peer's refs into the working copy (default: yes)
    --help                       Print help
"#,
};

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, repo) =
        project::cwd().context("this command must be run in the context of a project")?;

    term::info!(
        "🌱 Establishing tracking relationship for {}...",
        term::format::dim(&urn)
    );

    let cfg = tracking::config::Config::default();
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (signer, storage) = keys::storage(&profile, sock)?;
    let proj = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("project {} not found in local storage", &urn))?;
    let peer = options.peer;

    if &peer == storage.peer_id() {
        anyhow::bail!("you can't track yourself");
    }

    tracking::track(
        &storage,
        &urn,
        Some(peer),
        cfg,
        tracking::policy::Track::Any,
    )??;

    term::success!(
        "Tracking relationship {} established for {}",
        term::format::tertiary(peer),
        term::format::highlight(&urn)
    );

    let seed = options
        .seed
        .seed_url()
        .or_else(|| seed::get_seed(seed::Scope::Any).ok());

    if let Some(seed) = seed {
        if options.sync {
            // Fetch refs from seed...
            let spinner = term::spinner(&format!(
                "Syncing peer refs from {}",
                term::format::highlight(seed.host_str().unwrap_or("seed"))
            ));
            seed::fetch_peers(profile.paths().git_dir(), &seed, &urn, [peer])?;

            spinner.finish();
        } else if options.seed.seed.is_some() {
            term::warning("Ignoring `--seed` argument");
        } else if options.seed.seed_url.is_some() {
            term::warning("Ignoring `--seed-url` argument");
        }
    }

    if options.remote {
        project::SetupRemote {
            project: &proj,
            repo: &repo,
            signer,
            fetch: options.fetch,
            upstream: options.upstream,
        }
        .run(&peer, &profile, &storage)?;
    }

    Ok(())
}
