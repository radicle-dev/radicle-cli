use std::collections::HashMap;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::crypto::BoxedSigner;
use librad::git::storage::{ReadOnly, Storage};
use librad::git::tracking;
use librad::profile::Profile;
use librad::PeerId;

use rad_common::project::RemoteMetadata;
use rad_common::Url;
use rad_common::{git, keys, profile, project, seed};
use rad_terminal::args::Help;
use rad_terminal::components as term;

mod options;
pub use options::Options;

pub struct Peer {
    id: PeerId,
    meta: Option<RemoteMetadata>,
    branches: Vec<Branch>,
}

pub struct Branch {
    name: String,
    head: git::Oid,
    message: String,
}

// TODO: Add `--upstream-prefix` to specify a branch prefix, eg. `remotes/`.
pub const HELP: Help = Help {
    name: "track",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad track           [--local | --remote]
    rad track           [--seed <host>]
    rad track <peer-id> [--seed <host>] [--no-sync] [--no-upstream] [--no-fetch]

    If a peer id is supplied, track this peer in the context of the current project. By default,
    a remote is created in the repository and an upstream tracking branch is setup.

    If no peer id is supplied, show the local or remote tracking graph of the current project.

Options

    --local                Show the local project tracking graph
    --remote               Show the remote project tracking graph from a seed
    --seed <host>          Seed host to fetch refs from
    --no-upstream          Don't setup a tracking branch for the remote
    --no-sync              Don't sync the peer's refs
    --no-fetch             Don't fetch the peer's refs into the working copy
    --help                 Print help
"#,
};

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (signer, storage) = keys::storage(&profile, sock)?;

    let (urn, repo) =
        project::cwd().context("this command must be run in the context of a project")?;
    let proj = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("project {} not found in local storage", &urn))?;

    if let Some(peer) = options.peer {
        // Track peer.
        track(peer, proj, repo, storage, profile, signer, options)?;
    } else {
        // Show tracking graph.
        show(proj, repo, profile, storage.read_only(), options)?;
    }

    Ok(())
}

pub fn track(
    peer: PeerId,
    project: project::Metadata,
    repo: git::Repository,
    storage: Storage,
    profile: Profile,
    signer: BoxedSigner,
    options: Options,
) -> anyhow::Result<()> {
    if &peer == storage.peer_id() {
        anyhow::bail!("you can't track yourself");
    }
    let urn = &project.urn;

    term::info!(
        "🌱 Establishing tracking relationship for {}...",
        term::format::dim(&urn)
    );

    tracking::track(
        &storage,
        urn,
        Some(peer),
        tracking::config::Config::default(),
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
            seed::fetch_peers(profile.paths().git_dir(), &seed, urn, [peer])?;

            spinner.finish();
        } else if options.seed.seed.is_some() {
            term::warning("Ignoring `--seed` argument");
        } else if options.seed.seed_url.is_some() {
            term::warning("Ignoring `--seed-url` argument");
        }
    }

    project::SetupRemote {
        project: &project,
        repo: &repo,
        signer,
        fetch: options.fetch,
        upstream: options.upstream,
    }
    .run(&peer, &profile, &storage)?;

    Ok(())
}

pub fn show(
    project: project::Metadata,
    repo: git::Repository,
    profile: Profile,
    storage: &ReadOnly,
    options: Options,
) -> anyhow::Result<()> {
    let peers = if options.local {
        show_local(&project, &profile, storage)?
    } else {
        let seed = &if let Some(seed_url) = options.seed.seed_url() {
            seed_url
        } else if let Ok(seed) = seed::get_seed(seed::Scope::Any) {
            seed
        } else {
            anyhow::bail!("a seed node must be specified with `--seed` or `--seed-url`");
        };

        show_remote(&project, &repo, seed)?
    };

    for peer in peers {
        let you = &peer.id == storage.peer_id();
        let mut header = vec![term::format::bold(peer.id)];

        if let Some(meta) = peer.meta {
            header.push(term::format::tertiary(meta.name.to_string()));
            if meta.delegate {
                header.push(term::format::badge_primary("delegate"));
            }
        }
        if you {
            header.push(term::format::badge_secondary("you"));
        }
        term::info!("{}", header.join(" "));

        let mut table = term::Table::default();
        for branch in peer.branches {
            table.push([
                term::format::tertiary(branch.name),
                term::format::secondary(branch.head.to_string()),
                term::format::italic(branch.message),
            ]);
        }
        table.render_tree();
        term::blank();
    }
    Ok(())
}

pub fn show_local(
    project: &project::Metadata,
    profile: &Profile,
    storage: &ReadOnly,
) -> anyhow::Result<Vec<Peer>> {
    let tracked = project::tracked(project, storage)?;
    let monorepo = profile::monorepo(profile)?;
    let mut peers = Vec::new();

    term::info!(
        "Listing {} remotes on local device...",
        term::format::highlight(&project.name),
    );
    term::blank();

    for (id, meta) in tracked {
        if let Some(head) =
            project::get_remote_head(&monorepo, &project.urn, &id, &project.default_branch)?
        {
            peers.push(Peer {
                id,
                meta: Some(meta),
                branches: vec![Branch {
                    name: project.default_branch.clone(),
                    head,
                    message: String::new(),
                }],
            });
        }
    }
    Ok(peers)
}

pub fn show_remote(
    project: &project::Metadata,
    repo: &git::Repository,
    seed: &Url,
) -> anyhow::Result<Vec<Peer>> {
    let urn = &project.urn;
    let spinner = term::spinner(&format!(
        "Listing {} remotes on {}...",
        term::format::highlight(&project.name),
        term::format::highlight(seed.host_str().unwrap_or("seed"))
    ));
    let remotes = git::list_remotes(repo, seed, urn)?;
    let mut commits: HashMap<_, String> = HashMap::new();

    let remote_metadata = if let Ok(meta) = seed::get_remotes(seed.clone(), urn) {
        meta.into_iter().map(|r| (r.id, r)).collect()
    } else {
        HashMap::new() // Support old seeds that don't have metadata.
    };
    spinner.finish();

    if remotes.is_empty() {
        term::info!("{}", term::format::dim("No remotes found."));
        return Ok(Vec::new());
    }
    term::blank();

    let mut peers = Vec::new();

    for (id, branches) in remotes {
        let mut peer = Peer {
            id,
            meta: remote_metadata.get(&id).cloned(),
            branches: Vec::new(),
        };

        for (branch, oid) in branches {
            let message: String = if let Some(m) = commits.get(&oid) {
                m.to_owned()
            } else if let Ok(commit) = seed::get_commit(seed.clone(), urn, &oid) {
                commits.insert(oid, commit.header.summary.clone());
                commit.header.summary
            } else {
                String::new()
            };

            peer.branches.push(Branch {
                name: branch,
                head: oid,
                message,
            });
        }
        peers.push(peer);
    }
    Ok(peers)
}
