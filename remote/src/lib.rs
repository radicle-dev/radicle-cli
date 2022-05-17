use std::collections::HashSet;
use std::ffi::OsString;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::tracking;
use librad::PeerId;

use radicle_common::args::{Args, Error, Help};
use radicle_common::{git, keys, profile, project, seed};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "remote",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad remote add <name> <peer-id> [-f | --fetch]
    rad remote rm <name | peer-id>
    rad remote ls

Examples

    rad remote add cloudhead hyn9diwfnytahjq8u3iw63h9jte1ydcatxax3saymwdxqu1zo645pe

Options

    -f, --fetch     Fetch the remote immediately after it is setup
        --help      Print help
"#,
};

#[derive(Debug)]
pub enum Operation {
    Add {
        name: String,
        peer: PeerId,
        fetch: bool,
    },
    Remove {
        remote: String,
    },
    List,
}

/// Tool options.
#[derive(Debug)]
pub struct Options {
    pub op: Operation,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut peer: Option<PeerId> = None;
        let mut remote: Option<String> = None;
        let mut op: Option<String> = None;
        let mut fetch = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("fetch") | Short('f') if op.is_some() => {
                    fetch = true;
                }
                Value(val) if op.is_none() => {
                    op = Some(val.to_string_lossy().to_string());
                }
                Value(val) if remote.is_none() => {
                    remote = Some(val.to_string_lossy().to_string());
                }
                Value(val) if peer.is_none() => {
                    peer = Some(val.parse().context("invalid value specified for peer")?);
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        let op = match op {
            Some(op) => match op.as_str() {
                "add" => Operation::Add {
                    name: remote.ok_or(Error::Usage)?,
                    peer: peer.ok_or(Error::Usage)?,
                    fetch,
                },
                "rm" => Operation::Remove {
                    remote: remote.ok_or_else(|| anyhow!("a remote name must be specified"))?,
                },
                "ls" => Operation::List,

                unknown => anyhow::bail!("unknown operation '{}'", unknown),
            },
            None => Operation::List,
        };

        Ok((Options { op }, vec![]))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer.clone())?;
    let (urn, repo) = project::cwd()?;

    match options.op {
        Operation::Add { name, peer, fetch } => {
            let mut remote = project::remote(&urn, &peer, &name)?;
            remote.save(&repo)?;

            tracking::track(
                &storage,
                &urn,
                Some(peer),
                tracking::config::Config::default(),
                tracking::policy::Track::Any,
            )??;

            // TODO: Only show this if new.
            term::success!(
                "Tracking relationship established with {}",
                term::format::highlight(peer)
            );

            if fetch {
                let seed = if let Ok(seed) = seed::get_seed(seed::Scope::Any) {
                    seed
                } else {
                    anyhow::bail!("a seed node must be specified with `--seed`");
                };

                let mut spinner = term::spinner(&format!(
                    "Syncing remotes from {}...",
                    term::format::highlight(seed.host_str().unwrap_or("seed"))
                ));

                if let Err(e) =
                    term::sync::fetch_remotes(&storage, &seed, &urn, [&peer], &mut spinner)
                {
                    spinner.failed();
                    term::blank();

                    return Err(e);
                }
                git::fetch_remote(&mut remote, &repo, signer, &profile)?;
            }
            term::success!(
                "Remote {} successfully added",
                term::format::highlight(&name)
            );
        }
        Operation::Remove { remote } => {
            if let Ok(peer_) = remote.parse() {
                // Delete by peer id.
                for (name, peer) in git::remotes(&repo)? {
                    if peer == peer_ {
                        repo.remote_delete(&name)?;
                        term::success!(
                            "Remote {} {} removed",
                            term::format::highlight(&remote),
                            term::format::dim(format!("{:?}", name)),
                        );
                    }
                }
            } else {
                // Delete by peer name.
                for (name, peer) in git::remotes(&repo)? {
                    if let Some(person) = project::person(&storage, &urn, &peer)? {
                        if person.subject().name.to_string() == remote {
                            repo.remote_delete(&name)?;
                            term::success!(
                                "Remote {} {} removed",
                                term::format::highlight(&remote),
                                term::format::dim(format!("{:?}", name)),
                            );
                        }
                    }
                }
            }
        }
        Operation::List => {
            let mut table = term::Table::default();
            let proj = project::get(&storage, &urn)?
                .ok_or_else(|| anyhow!("project {} not found on local device", urn))?;
            let mut peers = HashSet::new();

            for (_, peer) in git::remotes(&repo)? {
                if !peers.insert(peer) {
                    // Don't show duplicate peers.
                    continue;
                }

                let delegate = if proj.remotes.contains(&peer) {
                    term::format::badge_primary("delegate")
                } else {
                    String::new()
                };

                if let Some(person) = project::person(&storage, &urn, &peer)? {
                    table.push([
                        term::format::bold(person.subject().name.to_string()),
                        term::format::tertiary(peer),
                        delegate,
                    ]);
                } else {
                    table.push([String::new(), term::format::tertiary(peer), delegate]);
                }
            }
            table.render();
        }
    }

    Ok(())
}
