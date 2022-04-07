use std::ffi::OsString;

use anyhow::anyhow;

use librad::git::Storage;
use librad::PeerId;

use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

use rad_common::{keys, patch, profile, project};

pub const HELP: Help = Help {
    name: "patch",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad patch [<option>...]

Options
    --list    Prints all patches (default: false)
    --help    Print help
"#,
};

#[derive(Default, Debug)]
pub struct Options {
    pub list: bool,
    pub verbose: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut list = false;
        let mut verbose = false;

        if let Some(arg) = parser.next()? {
            match arg {
                Long("list") | Short('l') => {
                    list = true;
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options { list, verbose }, vec![]))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, repo) = project::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_signer, storage) = keys::storage(&profile, sock)?;
    let project = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("couldn't load project {} from local state", urn))?;

    if options.list {
        list(project, &storage, &repo)?;
    } else {
        create()?;
    }

    Ok(())
}

fn list(
    project: project::Metadata,
    storage: &Storage,
    repo: &git2::Repository,
) -> anyhow::Result<()> {
    let peer = None;
    term::headline(&format!(
        "🌱 Listing patches for {}.",
        term::format::highlight(project.name.clone())
    ));

    term::info!("[{}]", term::format::secondary("Open"));
    term::blank();

    list_filtered(storage, peer, &project, repo, patch::State::Open)?;
    term::blank();
    term::blank();

    term::info!("[{}]", term::format::positive("Merged"));
    term::blank();

    list_filtered(storage, peer, &project, repo, patch::State::Merged)?;
    term::blank();

    Ok(())
}

fn create() -> anyhow::Result<()> {
    term::warning("Not implemented yet!");

    Ok(())
}

fn list_filtered(
    storage: &Storage,
    peer: Option<PeerId>,
    project: &project::Metadata,
    repo: &git2::Repository,
    state: patch::State,
) -> anyhow::Result<()> {
    let mut table = term::Table::default();
    let patches: Vec<patch::Metadata> = patch::list(&storage, peer, project, repo)?;
    let filtered: Vec<&patch::Metadata> = patches
        .iter()
        .filter(|patch| patch.state() == state)
        .collect();

    for patch in filtered {
        patch::print(storage, peer, project, patch, &mut table)?;
    }
    table.render();
    Ok(())
}
