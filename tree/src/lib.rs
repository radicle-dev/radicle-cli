use std::collections::HashMap;
use std::ffi::OsString;

use anyhow::anyhow;

use rad_common::seed::{self, SeedOptions};
use rad_common::{git, profile, project};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "tree",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad tree [--seed <host> | --seed-url <url>]

Options

    --seed <host>        Seed to query for source trees
    --seed-url <url>     Seed URL to query for source trees
    --help               Print help
"#,
};

/// Tool options.
#[derive(Debug)]
pub struct Options {
    seed: SeedOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (seed, unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);

        if let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => {
                    return Err(anyhow!(arg.unexpected()));
                }
            }
        }

        Ok((Options { seed }, vec![]))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let (urn, repo) = project::cwd()?;
    let seed = &if let Some(seed_url) = options.seed.seed_url() {
        seed_url
    } else if let Ok(seed) = seed::get_seed(seed::Scope::Any) {
        seed
    } else {
        anyhow::bail!("a seed node must be specified with `--seed` or `--seed-url`");
    };

    let profile = profile::default()?;
    let storage = profile::read_only(&profile)?;
    let project = if let Some(p) = project::get(&storage, &urn)? {
        p
    } else {
        anyhow::bail!("project {} not found in local storage", urn);
    };

    let spinner = term::spinner(&format!(
        "Listing {} remotes on {}...",
        term::format::highlight(project.name),
        term::format::highlight(seed.host_str().unwrap_or("seed"))
    ));
    let remotes = git::list_remotes(&repo, seed, &urn)?;
    let mut commits: HashMap<_, String> = HashMap::new();

    spinner.finish();
    term::blank();

    for (peer, branches) in remotes {
        term::info!("{}", term::format::bold(peer));

        let mut table = term::Table::default();
        for (branch, oid) in branches {
            let message: String = if let Some(m) = commits.get(&oid) {
                m.to_owned()
            } else if let Ok(commit) = seed::get_commit(seed.clone(), &urn, &oid) {
                commits.insert(oid, commit.header.summary.clone());
                commit.header.summary
            } else {
                String::new()
            };

            table.push([
                term::format::tertiary(branch),
                term::format::secondary(oid.to_string()),
                term::format::italic(message),
            ]);
        }
        table.render_tree();
        term::blank();
    }
    Ok(())
}
