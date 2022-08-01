use std::collections::HashMap;
use std::convert::TryInto;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::crypto::BoxedSigner;
use librad::git::storage::{ReadOnly, Storage};
use librad::git::tracking;
use librad::profile::Profile;
use librad::PeerId;

use radicle_common::args::Help;
use radicle_common::nonempty::NonEmpty;
use radicle_common::project::PeerInfo;
use radicle_common::tokio;
use radicle_common::Url;
use radicle_common::{git, keys, project, seed, sync, Urn};
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
    rad track           [--seed <url>]
    rad track <peer-id> [--seed <url>] [--no-sync] [--no-upstream] [--no-fetch]

    If a peer id is supplied, track this peer in the context of the current project. By default,
    a remote is created in the repository and an upstream tracking branch is setup. If a seed
    is supplied as well, the seed will be associated with this peer in the local git configuration.

    If no peer id is supplied, show the local or remote tracking graph of the current project.

Options

    --local                Show the local project tracking graph
    --remote               Show the remote project tracking graph from a seed
    --seed <url>           Seed URL to fetch refs from
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

    if options.sync {
        let seeds = if let Some(addr) = &options.seed {
            let seed = addr
                .clone()
                .try_into()
                .map_err(|e| anyhow!("invalid seed specified: {}", e))?;
            NonEmpty::new(seed)
        } else {
            sync::seeds(&profile)?
        };

        let rt = tokio::runtime::Runtime::new()?;
        term::sync::sync(
            project.urn.clone(),
            seeds,
            sync::Mode::Fetch,
            &profile,
            signer.clone(),
            &rt,
        )?;
    }

    // If a seed is explicitly specified, associate it with the peer being tracked.
    if let Some(addr) = &options.seed {
        let seed = addr
            .clone()
            .try_into()
            .map_err(|e| anyhow!("invalid seed specified: {}", e))?;

        seed::set_peer_seed(&seed, &peer)?;
        term::success!(
            "Saving seed configuration for {} to local git config...",
            term::format::tertiary(radicle_common::fmt::peer(&peer))
        );
    }

    if options.upstream {
        let name = if let Some(person) = project::person(&storage, urn.clone(), &peer)? {
            person.subject().name.to_string()
        } else {
            term::warning("peer identity document not found, using id as remote name");
            peer.default_encoding()
        };

        let branch = project::SetupRemote {
            project: &project,
            repo: &repo,
            signer,
            fetch: options.fetch,
            upstream: options.upstream,
        }
        .run(&peer, &name, &profile)?;

        if let Some((remote, branch)) = branch {
            term::success!("Remote {} set", term::format::highlight(remote.name),);
            term::success!(
                "Remote-tracking branch {} set",
                term::format::highlight(branch),
            );
        }
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
        let seed = if let Some(seed) = &options.seed {
            seed.clone()
        } else {
            anyhow::bail!("a seed node must be specified with `--seed`");
        };

        if !matches!(seed.protocol, seed::Protocol::Git { .. }) {
            anyhow::bail!(
                "invalid seed specified with `--seed`: must start with `http` or `https`"
            );
        }

        let spinner = term::spinner(&format!(
            "{} {} {}",
            term::format::highlight(&project.name),
            &project.urn,
            term::format::dim(format!("({})", seed.host)),
        ));
        let peers = show_remote(&project, &repo, &seed.url())?;

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

/// Return the Peer's branches with their Oid's by parsing references in the storage's git
/// repository.
fn get_peer_branches(
    storage: &ReadOnly,
    peer: PeerId,
    urn: &Urn,
) -> anyhow::Result<Vec<(String, git::Oid)>> {
    // Open the monorepo.
    let repo = git::Repository::open_bare(storage.as_ref().path())?;

    let ref_name_prefix = &format!(
        "refs/namespaces/{}/refs/remotes/{}/heads/",
        urn.encode_id(),
        peer,
    );

    let mut branches = vec![];
    for r in repo.references()?.flatten() {
        let (head, ref_name) = if let (Some(target), Some(name)) = (r.target(), r.name()) {
            (target, name)
        } else {
            continue;
        };

        if let Some(branch_name) = ref_name.strip_prefix(ref_name_prefix) {
            branches.push((branch_name.to_string(), head));
        }
    }

    Ok(branches)
}

pub fn show_local(project: &project::Metadata, storage: &ReadOnly) -> anyhow::Result<Vec<Peer>> {
    let tracked = project::tracked(project, storage)?;
    let mut peers = Vec::new();

    for (id, meta) in tracked {
        let mut branches = vec![];
        for (branch_name, head) in get_peer_branches(storage, id, &project.urn)? {
            branches.push(Branch {
                name: branch_name,
                head,
                message: String::new(),
            });
        }

        peers.push(Peer {
            id,
            meta: Some(meta),
            branches,
        });
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
