use std::collections::HashMap;
use std::ffi::OsString;

use anyhow::anyhow;
use librad::git::storage::ReadOnly;
use librad::{profile::Profile, PeerId};

use rad_common::project::RemoteMetadata;
use rad_common::seed::{self, SeedOptions};
use rad_common::{git, profile, project, Url};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub struct Tree {
    peer: PeerId,
    meta: Option<RemoteMetadata>,
    branches: Vec<Branch>,
}

pub struct Branch {
    name: String,
    head: git::Oid,
    message: String,
}

pub const HELP: Help = Help {
    name: "tree",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad tree [--seed <host>] [--remote | --local]

Options

    --local              Show the local project source tree (default)
    --remote             Show the remote project source tree
    --seed <host>        Seed to query for source trees
    --seed-url <url>     Seed URL to query for source trees
    --help               Print help
"#,
};

/// Tool options.
#[derive(Debug)]
pub struct Options {
    seed: SeedOptions,
    local: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (seed, unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut remote = false;
        let mut local = false;

        if let Some(arg) = parser.next()? {
            match arg {
                Long("local") => {
                    local = true;
                }
                Long("remote") => {
                    remote = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        if local && remote {
            // Can't be both local and remote.
            return Err(Error::Usage.into());
        }

        if !local && !remote && seed.seed_url().is_some() {
            // 'Remote' is implied when a seed is specified.
            remote = true;
        }

        Ok((
            Options {
                seed,
                local: !remote,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, repo) = project::cwd()?;

    let profile = profile::default()?;
    let storage = profile::read_only(&profile)?;
    let project = if let Some(p) = project::get(&storage, &urn)? {
        p
    } else {
        anyhow::bail!("project {} not found in local storage", urn);
    };

    let trees = if options.local {
        run_local(&project, &profile, &storage)?
    } else {
        let seed = &if let Some(seed_url) = options.seed.seed_url() {
            seed_url
        } else if let Ok(seed) = seed::get_seed(seed::Scope::Any) {
            seed
        } else {
            anyhow::bail!("a seed node must be specified with `--seed` or `--seed-url`");
        };

        run_remote(&project, &repo, seed)?
    };

    for tree in trees {
        let you = &tree.peer == storage.peer_id();
        let mut header = vec![term::format::bold(tree.peer)];

        if let Some(meta) = tree.meta {
            header.push(format!("({})", meta.name));
            if meta.delegate {
                header.push(term::format::badge_primary("delegate"));
            }
        }
        if you {
            header.push(term::format::badge_secondary("you"));
        }
        term::info!("{}", header.join(" "));

        let mut table = term::Table::default();
        for branch in tree.branches {
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

pub fn run_local(
    project: &project::Metadata,
    profile: &Profile,
    storage: &ReadOnly,
) -> anyhow::Result<Vec<Tree>> {
    let tracked = project::tracked(project, storage)?;
    let monorepo = profile::monorepo(profile)?;
    let mut trees = Vec::new();

    term::info!(
        "Listing {} remotes on local device...",
        term::format::highlight(&project.name),
    );
    term::blank();

    for (peer, meta) in tracked {
        if let Some(head) =
            project::get_remote_head(&monorepo, &project.urn, &peer, &project.default_branch)?
        {
            trees.push(Tree {
                peer,
                meta: Some(meta),
                branches: vec![Branch {
                    name: project.default_branch.clone(),
                    head,
                    message: String::new(),
                }],
            });
        }
    }
    Ok(trees)
}

pub fn run_remote(
    project: &project::Metadata,
    repo: &git::Repository,
    seed: &Url,
) -> anyhow::Result<Vec<Tree>> {
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

    let mut trees = Vec::new();

    for (peer, branches) in remotes {
        let mut tree = Tree {
            peer,
            meta: remote_metadata.get(&peer).cloned(),
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

            tree.branches.push(Branch {
                name: branch,
                head: oid,
                message,
            });
        }
        trees.push(tree);
    }
    Ok(trees)
}
