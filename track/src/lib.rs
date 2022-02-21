use anyhow::anyhow;
use anyhow::Context as _;
use librad::git::tracking;

use rad_common::{git, keys, profile, project, seed};
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

    rad track <peer-id> [--seed <host> | --seed-url <url>] [--no-sync] [--no-remote] [--no-upstream] [--no-fetch]

    Track a peer in the context of the current project. By default, a remote is created in the
    repository and an upstream tracking branch is setup.

Options

    --seed <host>                Seed host to fetch refs from
    --seed-url <url>             Seed URL to fetch refs from
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
        // TODO: Handle conflicts in remote name.
        if let Some(person) = project::person(&storage, &urn, &peer)? {
            let name = person.subject().name.to_string();
            let mut remote = git::remote(&urn, &peer, &name)?;

            // Configure the remote in the repository.
            remote.save(&repo)?;
            // Fetch the refs into the working copy.
            if options.fetch {
                git::fetch_remote(&mut remote, &repo, signer, &profile)?;
            }
            // Setup remote-tracking branch.
            if options.upstream {
                // TODO: If this fails because the branch already exists, suggest how to specify a
                // different branch name or prefix.
                let branch = git::set_upstream(repo.path(), &name, &proj.default_branch)?;

                term::success!(
                    "Remote-tracking branch {} created",
                    term::format::highlight(&branch)
                );
            }
        }
    }

    Ok(())
}
