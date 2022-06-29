use std::collections::HashMap;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::crypto::BoxedSigner;
use librad::git::identities;
use librad::git::storage::{ReadOnly, Storage};
use librad::git::tracking;
use librad::profile::Profile;
use librad::PeerId;

use radicle_common::args::Help;
use radicle_common::project::PeerInfo;
use radicle_common::Url;
use radicle_common::{git, keys, project, seed};
use radicle_terminal as term;

mod options;
pub use options::Options;

#[derive(Debug)]
pub struct Peer {
    id: PeerId,
    meta: Option<PeerInfo>,
    branches: Vec<Branch>,
}

#[derive(Debug)]
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
    a remote is created in the repository and an upstream tracking branch is setup. If a seed
    is supplied as well, the seed will be associated with this peer in the local git configuration.

    If no peer id is supplied, show the local or remote tracking graph of the current project.

Options

    --local                Show the local project tracking graph
    --remote               Show the remote project tracking graph from a seed
    --seed <host>          Seed host to fetch refs from
    --no-upstream          Don't setup a tracking branch for the remote
    --no-sync              Don't sync the peer's refs
    --no-fetch             Don't fetch the peer's refs into the working copy
    --verbose, -v          Verbose output
    --help                 Print help
"#,
};

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer.clone())?;

    let (urn, repo) =
        project::cwd().context("this command must be run in the context of a project")?;
    let proj = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("project {} not found in local storage", &urn))?;

    if let Some(peer) = options.peer {
        // Track peer.
        track(peer, proj, repo, storage, profile, signer, options)?;
    } else {
        // Show tracking graph.
        show(proj, repo, storage.read_only(), options)?;
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
        "Establishing 🌱 tracking relationship for {}",
        term::format::highlight(&project.name)
    );
    term::blank();

    let result = tracking::track(
        &storage,
        urn,
        Some(peer),
        tracking::config::Config::default(),
        tracking::policy::Track::MustNotExist,
    )?;
    // Whether or not the tracking existed.
    let existing = matches!(result.err(), Some(tracking::PreviousError::DidExist));

    term::success!(
        "Tracking relationship with {} {}",
        term::format::tertiary(peer),
        if existing { "exists" } else { "established" },
    );

    let seed = options
        .seed
        .as_ref()
        .map(|s| s.url())
        .or_else(|| seed::get_seed(seed::Scope::Any).ok());

    if let Some(seed) = seed {
        if options.sync {
            // Fetch refs from seed...
            let seed_pretty = term::format::highlight(seed.host_str().unwrap_or("seed"));
            let mut spinner = term::spinner(&format!("Syncing peer refs from {}...", seed_pretty));
            if let Err(e) = term::sync::fetch_remotes(&storage, &seed, urn, [&peer], &mut spinner) {
                spinner.failed();
                term::blank();

                return Err(e);
            }

            if let Ok(Some(person)) = project::person(&storage, urn.clone(), &peer) {
                spinner.message(format!(
                    "Syncing peer identity {} from {}...",
                    term::format::tertiary(person.urn()),
                    seed_pretty
                ));

                let monorepo = profile.paths().git_dir();
                match seed::fetch_identity(monorepo, &seed, &person.urn()).and_then(|out| {
                    spinner.finish();
                    spinner.message("Verifying identity...");
                    identities::person::verify(&storage, &person.urn())?;

                    Ok(out)
                }) {
                    Ok(output) => {
                        if options.verbose {
                            spinner.finish();
                            term::blob(output);
                        }
                    }
                    Err(err) => {
                        spinner.failed();
                        term::blank();
                        return Err(err);
                    }
                }
            }

            spinner.finish();
        }
    }

    // If a seed is explicitly specified, associate it with the peer being tracked.
    if let Some(addr) = &options.seed {
        seed::set_peer_seed(&addr.url(), &peer)?;
        term::success!(
            "Saving seed configuration for {} to local git config...",
            term::format::tertiary(radicle_common::fmt::peer(&peer))
        );
    }

    // Don't setup remote if tracking relationship already existed, as the branch
    // probably already exists.
    //
    // TODO: We should allow this anyway if for eg. you want to update a checkout with a peer.
    // There's no other way to setup a remote tracking branch right now..
    //
    if !existing {
        project::SetupRemote {
            project: &project,
            repo: &repo,
            signer,
            fetch: options.fetch,
            upstream: options.upstream,
        }
        .run(&peer, &profile, &storage)?;
    }

    Ok(())
}

pub fn show(
    project: project::Metadata,
    repo: git::Repository,
    storage: &ReadOnly,
    options: Options,
) -> anyhow::Result<()> {
    let peers = if options.local {
        term::info!(
            "{} {} {}",
            term::format::highlight(&project.name),
            &project.urn,
            term::format::dim("(local)")
        );
        show_local(&project, storage)?
    } else {
        let seed = &if let Some(seed_url) = options.seed.as_ref().map(|s| s.url()) {
            seed_url
        } else if let Ok(seed) = seed::get_seed(seed::Scope::Any) {
            seed
        } else {
            anyhow::bail!("a seed node must be specified with `--seed`");
        };

        let spinner = term::spinner(&format!(
            "{} {} {}",
            term::format::highlight(&project.name),
            &project.urn,
            term::format::dim(format!("({})", seed.host_str().unwrap_or("seed"))),
        ));
        let peers = show_remote(&project, &repo, seed)?;

        spinner.done();

        peers
    };
    if peers.is_empty() {
        term::info!("{}", term::format::dim("No remotes found for project"));
        return Ok(());
    }

    // TODO: Deterministic ordering of peers when printed.
    for (i, peer) in peers.iter().enumerate() {
        let you = &peer.id == storage.peer_id();
        let mut header = vec![term::format::bold(peer.id)];

        if let Some(meta) = &peer.meta {
            if let Some(name) = meta.person.as_ref().map(|p| &p.name) {
                header.push(term::format::tertiary(name));
            }
            if meta.delegate {
                header.push(term::format::badge_primary("delegate"));
            }
        }
        if you {
            header.push(term::format::badge_secondary("you"));
        }

        if i != peers.len() - 1 {
            term::info!("├── {}", header.join(" "));
        } else {
            term::info!("└── {}", header.join(" "));
        }

        let mut table = term::Table::default();
        for (j, branch) in peer.branches.iter().enumerate() {
            let prefix = if j != peer.branches.len() - 1 {
                " ├──"
            } else {
                " └──"
            };

            let prefix = if i != peers.len() - 1 {
                format!("│  {}", prefix)
            } else {
                format!("   {}", prefix)
            };

            table.push([
                prefix,
                term::format::tertiary(&branch.name),
                term::format::secondary(branch.head.to_string()),
                term::format::italic(&branch.message),
            ]);
        }
        table.render();

        if i != peers.len() - 1 && !peer.branches.is_empty() {
            term::info!("│");
        }
    }
    Ok(())
}

pub fn show_local(project: &project::Metadata, storage: &ReadOnly) -> anyhow::Result<Vec<Peer>> {
    let tracked = project::tracked(project, storage)?;
    let mut peers = Vec::new();

    for (id, meta) in tracked {
        let head = project::get_remote_head(&storage, &project.urn, &id, &project.default_branch)
            .ok()
            .flatten();

        if let Some(head) = head {
            peers.push(Peer {
                id,
                meta: Some(meta),
                branches: vec![Branch {
                    name: project.default_branch.to_string(),
                    head,
                    message: String::new(),
                }],
            });
        } else {
            peers.push(Peer {
                id,
                meta: Some(meta),
                branches: vec![],
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
    let remotes = project::list_seed_heads(repo, seed, urn)?;
    let mut commits: HashMap<_, String> = HashMap::new();

    let remote_metadata = if let Ok(meta) = seed::get_remotes(seed.clone(), urn) {
        meta.into_iter().map(|r| (r.id, r)).collect()
    } else {
        HashMap::new() // Support old seeds that don't have metadata.
    };

    if remotes.is_empty() {
        return Ok(Vec::new());
    }

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
