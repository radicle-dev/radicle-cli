use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::git::storage::Storage;
use librad::git::tracking::git::tracking;
use librad::git::Urn;
use librad::PeerId;

use radicle_common::args::{Args, Error, Help};
use radicle_common::profile::Profile;
use radicle_common::{fmt, git, keys, project};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "untrack",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad untrack <peer-id>

    Must be run within a project working copy.

Options

    --help              Print help
"#,
};

/// Tool options.
#[derive(Debug)]
pub struct Options {
    pub peer: String,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut peer: Option<String> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Value(val) if peer.is_none() => {
                    let val = val.to_string_lossy();
                    peer = Some(val.to_string());
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        let peer = peer.ok_or(Error::Usage)?;
        Ok((Options { peer }, vec![]))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let (urn, repo) =
        project::cwd().context("this command must be run in the context of a project")?;
    let profile = ctx.profile()?;

    execute(&urn, Some(&repo), options, &profile)
}

fn get_peer_id(
    project: &project::Metadata,
    storage: &Storage,
    name: &String,
) -> anyhow::Result<Option<PeerId>> {
    for (id, peer) in project::tracked(project, storage)? {
        if peer.name() == *name {
            return Ok(Some(id));
        }
    }

    Ok(None)
}

pub fn execute(
    urn: &Urn,
    repo: Option<&git::Repository>,
    options: Options,
    profile: &Profile,
) -> anyhow::Result<()> {
    let signer = term::signer(profile)?;
    let storage = keys::storage(profile, signer)?;

    let peer = if let Ok(val) = PeerId::from_str(&options.peer) {
        val
    } else {
        let project = project::get(&storage, urn)?
            .ok_or_else(|| anyhow!("project {} not found in local storage", &urn))?;

        if let Some(v) = get_peer_id(&project, &storage, &options.peer)? {
            v
        } else {
            anyhow::bail!("invalid <peer-id> '{}'", options.peer)
        }
    };

    tracking::untrack(
        &storage,
        urn,
        peer,
        tracking::UntrackArgs {
            policy: tracking::policy::Untrack::MustExist,
            prune: true,
        },
    )??;

    if let Some(repo) = repo {
        term::remote::remove(&peer.to_string(), &storage, repo, urn)?;
    };

    term::success!(
        "Tracking relationship {} removed for {}",
        term::format::dim(fmt::peer(&peer)),
        term::format::highlight(urn)
    );

    Ok(())
}
