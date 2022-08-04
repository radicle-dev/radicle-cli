#![allow(clippy::or_fun_call)]
use std::convert::TryInto;
use std::ffi::OsString;
use std::iter;
use std::str::FromStr;

use librad::git::Storage;
use librad::git::Urn;
use librad::profile::Profile;

use radicle_common::args;
use radicle_common::args::{Args, Error, Help};
use radicle_common::nonempty::NonEmpty;
use radicle_common::sync::Mode;
use radicle_common::{identity, keys, person, project, sync, tokio};
use radicle_terminal as term;

use anyhow::anyhow;
use url::Url;

pub const GATEWAY_HOST: &str = "app.radicle.network";
pub const HELP: Help = Help {
    name: "sync",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad sync [<urn> | <url>] [--seed <address>]... [<options>...]
    rad sync --self [--seed <address>]...

    If a <urn> is specified, seeds may be given via the `--seed` option.
    If a <url> is specified, the seed is implied.
    If neither is specified, the URN and seed of the current project is used.
    If the project has no configured seed, the active profile's default seed list is used.

Options

    --seed <address>    Sync to the given seed (may be specified multiple times)
    --self              Sync your local identity only
    --help              Print help

Seed addresses

    A seed address is of the form `<id>@<host>:<port>`.
    The `<id>` component is the "Peer ID" of the seed.
    The `<port>` component can often be omitted, in which case the default port will be used.

    Example: hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@pine.radicle.garden:8776
"#,
};

#[derive(Default, Debug)]
pub struct Options {
    pub origin: Option<identity::Origin>,
    pub seeds: Vec<sync::Seed<String>>,
    pub mode: Mode,
    pub verbose: bool,
    pub sync_self: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut verbose = false;
        let mut origin = None;
        let mut sync_self = false;
        let mut unparsed = Vec::new();
        let mut seeds = Vec::new();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("self") => {
                    sync_self = true;
                }
                Long("seed") => {
                    let value = parser.value()?;
                    let value = value.to_string_lossy();
                    let value = value.as_ref();
                    let addr = sync::Seed::from_str(value).map_err(|_| Error::WithHint {
                        err: anyhow!("invalid seed address specified: '{}'", value),
                        hint: "hint: valid seed addresses have the format <peer-id>@<host>:<port>, see `rad sync --help` for more information",
                    })?;

                    seeds.push(addr);
                }
                Value(val) if origin.is_none() => {
                    let val = val.to_string_lossy();
                    let val = identity::Origin::from_str(&val)?;

                    origin = Some(val);
                }
                arg => {
                    unparsed = iter::once(args::format(arg))
                        .chain(iter::from_fn(|| parser.value().ok()))
                        .collect();

                    break;
                }
            }
        }

        if let (
            &[_, ..],
            Some(identity::Origin {
                seed: Some(addr), ..
            }),
        ) = (seeds.as_slice(), &origin)
        {
            anyhow::bail!(
                "unexpected argument `--seed`, seed already set to '{}'",
                addr
            );
        }

        Ok((
            Options {
                origin,
                seeds,
                mode: Mode::default(),
                sync_self,
                verbose,
            },
            unparsed,
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
    let rt = tokio::runtime::Runtime::new()?;
    let urn = if let Some(origin) = &options.origin {
        origin.urn.clone()
    } else {
        project::cwd().map(|(urn, _)| urn)?
    };

    let seeds = if let Some(seed) = options.origin.as_ref().and_then(|o| o.seed.clone()) {
        NonEmpty::new(seed)
    } else if let Ok(seeds) = options.seeds.clone().try_into() {
        seeds
    } else {
        sync::seeds(&profile)?
    };

    if options.sync_self {
        sync_self(&profile, seeds, storage, options, rt)
    } else {
        sync(urn, &profile, seeds, storage, options, rt)
    }
}

pub fn sync_self(
    profile: &Profile,
    seeds: NonEmpty<sync::Seed<String>>,
    storage: Storage,
    options: Options,
    rt: tokio::runtime::Runtime,
) -> anyhow::Result<()> {
    let identity = person::local(&storage)?;
    let urn = identity.urn();

    term::headline(&format!(
        "Syncing 🌱 self to {} seed(s)",
        term::format::dim(seeds.len())
    ));

    let signer = term::signer(profile)?;
    let _result = term::sync::sync(urn, seeds, options.mode, profile, signer, &rt)?;

    if options.verbose {
        // TODO: When sync result is usable, output should go here.
    }

    Ok(())
}

pub fn sync(
    urn: Urn,
    profile: &Profile,
    seeds: NonEmpty<sync::Seed<String>>,
    storage: Storage,
    options: Options,
    rt: tokio::runtime::Runtime,
) -> anyhow::Result<()> {
    term::headline(&format!(
        "Syncing 🌱 identity {} with {} seed(s)",
        term::format::highlight(&urn),
        term::format::dim(seeds.len())
    ));

    let storage = storage.read_only();
    let signer = term::signer(profile)?;
    let _result = term::sync::sync(
        urn.clone(),
        seeds.clone(),
        options.mode,
        profile,
        signer,
        &rt,
    )?;
    term::blank();

    if options.verbose {
        // TODO: When sync result is usable, output should go here.
        // TODO: Depending on the result, we can show `~` as in partial success, `ok` as in total
        //       success and `!!` as in no success.
        // TODO: NoConnection can be due to invalid PeerId!
        // TODO: Success with no refs updated can mean the server is not tracking us.
    }

    if let Some(proj) = project::get(&storage, &urn)? {
        let peer_id = storage.peer_id();

        for seed in &seeds {
            let host = &seed.addrs;
            if let Ok(mut url) = Url::from_str(&format!("https://{}", host)) {
                url.set_port(None).ok();

                if let Some(host) = url.host() {
                    let is_routable = match host {
                        url::Host::Domain("localhost") => false,
                        url::Host::Domain(_) => true,
                        url::Host::Ipv4(ip) => {
                            !ip.is_loopback() && !ip.is_unspecified() && !ip.is_private()
                        }
                        url::Host::Ipv6(ip) => !ip.is_loopback() && !ip.is_unspecified(),
                    };

                    term::info!("🍃 Your project is available at:");
                    term::blank();

                    if is_routable {
                        if proj.remotes.contains(peer_id) {
                            term::indented(&format!(
                                "{} {}",
                                term::format::dim("(web)"),
                                term::format::highlight(format!(
                                    "https://{}/seeds/{}/{}",
                                    GATEWAY_HOST, host, urn
                                ))
                            ));
                        }
                        term::indented(&format!(
                            "{} {}",
                            term::format::dim("(web)"),
                            term::format::highlight(format!(
                                "https://{}/seeds/{}/{}/remotes/{}",
                                GATEWAY_HOST, host, urn, peer_id
                            ))
                        ));
                    } else {
                        url.set_scheme("http").ok();
                    }

                    let id = urn.encode_id();
                    let git_url = url.join(&id)?;

                    term::indented(&format!(
                        "{} {}",
                        term::format::dim("(git)"),
                        term::format::highlight(format!("{}.git", git_url)),
                    ));
                    term::blank();
                }
            }
        }
    }

    Ok(())
}
