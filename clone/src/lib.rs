use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use librad::git::tracking;
use librad::git::Urn;
use url::Url;

use rad_common::seed::{self, SeedOptions};
use rad_common::{git, keys, profile, project};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "clone",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad clone <urn | url> [--seed <host>] [<option>...]

Options

    --seed <host>   Seed to clone from
    --help          Print help

"#,
};

#[derive(Debug)]
enum UrnUrl {
    Urn(Urn),
    Url(Url),
}

#[derive(Debug)]
pub struct Options {
    urn_url: UrnUrl,
    seed: SeedOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (seed, unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut urn_url: Option<UrnUrl> = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if urn_url.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val)
                        .map(UrnUrl::Urn)
                        .or_else(|_| Url::parse(&val).map(UrnUrl::Url))?;

                    urn_url = Some(val);
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                urn_url: urn_url.ok_or_else(|| {
                    anyhow!("to clone, a URN or URL must be provided; see `rad clone --help`")
                })?,
                seed,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    match options.urn_url {
        UrnUrl::Urn(urn) => {
            clone_project(urn, options.seed)?;
        }
        UrnUrl::Url(url) => {
            clone_repository(url)?;
        }
    }
    Ok(())
}

pub fn clone_project(urn: Urn, seed: SeedOptions) -> anyhow::Result<()> {
    rad_sync::run(rad_sync::Options {
        fetch: true,
        urn: Some(urn.clone()),
        seed: seed.clone(),
        identity: true,
        push_self: false,
        verbose: false,
        force: false,
    })?;
    let path = rad_checkout::execute(rad_checkout::Options { urn: urn.clone() })?;

    if let Some(seed_url) = seed.seed_url() {
        seed::set_seed(&seed_url, seed::Scope::Local(&path))?;
        term::success!(
            "Local repository seed for {} set to {}",
            term::format::highlight(path.display()),
            term::format::highlight(seed_url)
        );
    }

    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;
    let cfg = tracking::config::Config::default();
    let project = project::get(&storage, &urn)?
        .ok_or_else(|| anyhow!("couldn't load project {} from local state", urn))?;

    // Track all project delegates.
    for peer in project.remotes {
        tracking::track(
            &storage,
            &urn,
            Some(peer),
            cfg.clone(),
            tracking::policy::Track::Any,
        )??;
    }
    term::success!("Tracking for project delegates configured");

    term::headline(&format!(
        "🌱 Project clone successful under ./{}",
        term::format::highlight(path.file_name().unwrap_or_default().to_string_lossy())
    ));

    Ok(())
}

pub fn clone_repository(url: Url) -> anyhow::Result<()> {
    let proj = url
        .path_segments()
        .ok_or(anyhow!("couldn't get segments of URL"))?
        .last()
        .ok_or(anyhow!("couldn't get last segment of URL"))?;
    let proj = proj.strip_suffix(".git").unwrap_or(proj);
    let destination = std::env::current_dir()?.join(proj);

    let spinner = term::spinner(&format!("Cloning {} locally", term::format::bold(&proj)));
    git::clone(url.as_str(), &destination)?;
    spinner.finish();

    rad_init::run(rad_init::Options {
        path: Some(destination),
    })?;
    Ok(())
}
