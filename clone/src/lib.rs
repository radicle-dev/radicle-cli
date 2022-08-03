#![allow(clippy::or_fun_call)]
use std::convert::TryFrom;
use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;
use librad::git::tracking;
use librad::git::Urn;
use url::Url;

use radicle_common::args::{Args, Error, Help};
use radicle_common::seed;
use radicle_common::Interactive;
use radicle_common::{git, identity, keys, profile, project, sync};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "clone",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad clone <urn | url> [--seed <addr>] [<option>...]

Options

    --no-confirm    Don't ask for confirmation during clone
    --seed <addr>   Seed to clone from
    --help          Print help

"#,
};

#[derive(Debug)]
enum Origin {
    Radicle(identity::Origin),
    Git(Url),
}

#[derive(Debug)]
pub struct Options {
    origin: Origin,
    interactive: Interactive,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut origin: Option<Origin> = None;
        let mut interactive = Interactive::Yes;
        let mut seed = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("seed") if seed.is_none() => {
                    seed = Some(seed::parse_value(&mut parser)?);
                }
                Long("no-confirm") => {
                    interactive = Interactive::No;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Value(val) if origin.is_none() => {
                    let val = val.to_string_lossy();
                    match Urn::from_str(&val) {
                        Ok(urn) => {
                            origin = Some(Origin::Radicle(identity::Origin::from_urn(urn)));
                        }
                        Err(_) => {
                            match Url::parse(&val) {
                                Ok(_) if seed.is_some() => {
                                    anyhow::bail!("`--seed` cannot be specified when a URL is given as origin");
                                }
                                Ok(url) if url.scheme() == project::URL_SCHEME => {
                                    let o = identity::Origin::try_from(url)?;
                                    origin = Some(Origin::Radicle(o));
                                }
                                Ok(url) => {
                                    origin = Some(Origin::Git(url));
                                }
                                Err(err) => {
                                    return Err(err.into());
                                }
                            }
                        }
                    }
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }
        let origin = origin.ok_or_else(|| {
            anyhow!("to clone, a URN or URL must be provided; see `rad clone --help`")
        })?;

        let origin = if let Origin::Radicle(identity::Origin { urn, seed: None }) = origin {
            Origin::Radicle(identity::Origin { urn, seed })
        } else {
            origin
        };

        Ok((
            Options {
                origin,
                interactive,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    match options.origin {
        Origin::Radicle(origin) => {
            clone_project(origin.urn, origin.seed, options.interactive, ctx)?;
        }
        Origin::Git(url) => {
            let profile = ctx.profile()?;
            clone_repository(url, &profile)?;
        }
    }
    Ok(())
}

pub fn clone_project(
    urn: Urn,
    seed: Option<sync::Seed<String>>,
    interactive: Interactive,
    ctx: impl term::Context,
) -> anyhow::Result<()> {
    let profile = ctx.profile()?;

    rad_sync::run(
        rad_sync::Options {
            origin: Some(identity::Origin {
                urn: urn.clone(),
                seed,
            }),
            verbose: true,
            ..rad_sync::Options::default()
        },
        profile.clone(),
    )?;
    let path = rad_checkout::execute(
        rad_checkout::Options {
            urn: urn.clone(),
            interactive,
        },
        &profile,
    )?;

    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
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

pub fn clone_repository(url: Url, profile: &profile::Profile) -> anyhow::Result<()> {
    let proj = url
        .path_segments()
        .ok_or(anyhow!("couldn't get segments of URL"))?
        .last()
        .ok_or(anyhow!("couldn't get last segment of URL"))?;
    let proj = proj.strip_suffix(".git").unwrap_or(proj);
    let destination = std::env::current_dir()?.join(proj);

    let spinner = term::spinner(&format!(
        "Cloning git repository {}...",
        term::format::highlight(&url)
    ));
    git::clone(url.as_str(), &destination)?;
    spinner.finish();

    if term::confirm(format!(
        "Initialize new 🌱 project in {}?",
        term::format::highlight(destination.display())
    )) {
        let options = rad_init::Options {
            path: Some(destination.as_path().into()),
            ..Default::default()
        };
        rad_init::init(options, profile)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use librad::PeerId;

    #[test]
    fn test_args_ok() {
        let tests = vec![
            vec!["rad://hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@pine.radicle.garden/hnrkfbrd7y9674d8ow8uioki16fniwcyoz67y"],
            vec![
                "rad:git:hnrkfbrd7y9674d8ow8uioki16fniwcyoz67y",
                "--seed",
                "hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@pine.radicle.garden:8776",
            ],
        ];

        for args in tests {
            let args = args.into_iter().map(|a| a.into()).collect();

            let (opts, leftover) = Options::from_args(args).unwrap();
            assert!(leftover.is_empty());

            if let Origin::Radicle(origin) = opts.origin {
                assert_eq!(
                    origin.urn.to_string(),
                    "rad:git:hnrkfbrd7y9674d8ow8uioki16fniwcyoz67y"
                );
                assert_eq!(
                    origin.seed.unwrap(),
                    sync::Seed {
                        peer: PeerId::from_str(
                            "hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa"
                        )
                        .unwrap(),
                        addrs: "pine.radicle.garden:8776".to_owned(),
                        label: None
                    }
                );
            } else {
                panic!("invalid origin {:?}", opts.origin);
            }
        }
    }

    #[test]
    fn test_args_error() {
        let tests = vec![
            vec![
                "rad://whyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@illow.radicle.garden/hnrkfbrd7y9674d8ow8uioki16fniwcyoz67y",
                "--seed",
                "whyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@illow.radicle.garden",
            ],
            vec![
                "https://willow.radicle.garden/hnrkfbrd7y9674d8ow8uioki16fniwcyoz67y.git",
                "--seed",
                "whyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@illow.radicle.garden",
            ],
        ];

        for args in tests {
            let args = args.into_iter().map(|a| a.into()).collect();
            Options::from_args(args).unwrap_err();
        }
    }
}
