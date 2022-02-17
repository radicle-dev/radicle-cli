use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;
use librad::git::tracking;
use librad::git::Urn;

use rad_common::seed::{self, SeedOptions};
use rad_common::{keys, profile};
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

pub const HELP: Help = Help {
    name: "clone",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad clone <urn> [--no-track] [--seed <host>] [<option>...]

Options

    --no-track      Don't track the project after syncing (default: false)
    --seed <host>   Seed to clone from
    --help          Print help

"#,
};

#[derive(Debug)]
pub struct Options {
    urn: Urn,
    track: bool,
    seed: SeedOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (seed, unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut urn: Option<Urn> = None;
        let mut track = true;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("no-track") => {
                    track = false;
                }
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                urn: urn.ok_or_else(|| {
                    anyhow!("a URN to clone must be provided; see `rad clone --help`")
                })?,
                track,
                seed,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    rad_sync::run(rad_sync::Options {
        fetch: true,
        urn: Some(options.urn.clone()),
        seed: options.seed.clone(),
        verbose: false,
        force: false,
    })?;

    // Tracking influences the checkout (by creating additional remotes),
    // so we run it first.
    if options.track {
        let profile = profile::default()?;
        let sock = keys::ssh_auth_sock();
        let (_, storage) = keys::storage(&profile, sock)?;
        let cfg = tracking::config::Config::default();

        tracking::track(
            &storage,
            &options.urn,
            None,
            cfg,
            tracking::policy::Track::Any,
        )??;
    }
    let path = rad_checkout::execute(rad_checkout::Options { urn: options.urn })?;

    if let Some(seed_url) = options.seed.seed_url() {
        seed::set_seed(&seed_url, seed::Scope::Local(&path))?;
        term::success!(
            "Local repository seed for {} set to {}",
            term::format::highlight(path.display()),
            term::format::highlight(seed_url)
        );
    }

    Ok(())
}
