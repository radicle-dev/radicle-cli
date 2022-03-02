use librad::git::Storage;
use librad::git::{identities, tracking, Urn};
use librad::profile::Profile;
use librad::PeerId;

use rad_common::seed::SeedOptions;
use rad_common::{git, keys, person, profile, project, seed, seed::Scope};
use rad_terminal::args;
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

use anyhow::anyhow;
use anyhow::Context as _;
use url::Url;

use std::collections::HashSet;
use std::convert::TryInto;
use std::ffi::OsString;
use std::iter;
use std::path::Path;
use std::str::FromStr;

pub const GATEWAY_HOST: &str = "app.radicle.network";
pub const HELP: Help = Help {
    name: "sync",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad sync [--seed <host>] [--fetch] [--self]
    rad sync <urn> [--seed <host>] [--fetch] [--self]
    rad sync <url> [--fetch] [--self]

    If a <urn> is specified, a seed may be given via the `--seed` option.
    If a <url> is specified, the seed is implied.
    If neither is specified, the URN and seed of the current project is used.

Options

    --seed <host>       Use the given seed node for syncing
    --identity          Sync identity refs (default: true)
    --fetch             Fetch updates (default: false)
    --self              Sync your local identity (default: false)
    --help              Print help
"#,
};

#[derive(Default, Debug)]
pub struct Options {
    pub origin: Option<project::Origin>,
    pub seed: Option<seed::Address>,
    pub verbose: bool,
    pub fetch: bool,
    pub force: bool,
    pub identity: bool,
    pub push_self: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (SeedOptions(seed), unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut verbose = false;
        let mut fetch = false;
        let mut origin = None;
        let mut force = false;
        let mut push_self = false;
        let mut identity = true;
        let mut unparsed = Vec::new();

        while let Some(arg) = parser.next()? {
            match arg {
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("fetch") => {
                    fetch = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("self") => {
                    push_self = true;
                }
                Long("identity") => {
                    identity = true;
                }
                Long("no-identity") => {
                    identity = false;
                }
                Long("force") | Short('f') => {
                    force = true;
                }
                Value(val) if origin.is_none() => {
                    let val = val.to_string_lossy();
                    let val = project::Origin::from_str(&val)?;

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

        if fetch && push_self {
            anyhow::bail!("'--fetch' and '--self' cannot be used together");
        }

        if let (
            Some(_),
            Some(project::Origin {
                seed: Some(addr), ..
            }),
        ) = (&seed, &origin)
        {
            anyhow::bail!(
                "unexpected argument `--seed`, seed already set to '{}'",
                addr
            );
        }

        Ok((
            Options {
                origin,
                seed,
                fetch,
                force,
                push_self,
                identity,
                verbose,
            },
            unparsed,
        ))
    }

    fn from_env() -> anyhow::Result<Self> {
        let mut parser = lexopt::Parser::from_env();
        let args = iter::from_fn(|| parser.value().ok()).collect();

        match Self::from_args(args) {
            Ok((opts, unparsed)) => {
                args::finish(unparsed)?;

                Ok(opts)
            }
            Err(err) => Err(err),
        }
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;

    let project_urn = if let Some(origin) = &options.origin {
        origin.urn.clone()
    } else {
        project::urn()?
    };
    let git_version = git::version()?;

    term::info!("Git version {}", git_version);

    if git_version < git::VERSION_REQUIRED {
        anyhow::bail!(
            "a minimum git version of {} is required, please update your installation",
            git::VERSION_REQUIRED
        );
    }

    let seed: &Url = &if let Some(seed) = options.origin.as_ref().and_then(|o| o.seed_url()) {
        seed
    } else if let Some(seed) = &options.seed {
        seed.url()
    } else if let Ok(seed) = seed::get_seed(Scope::Any) {
        seed
    } else {
        term::info!("Select a seed node to sync with...");

        if let Some(selection) = term::select(
            seed::DEFAULT_SEEDS,
            &seed::DEFAULT_SEEDS[fastrand::usize(0..seed::DEFAULT_SEEDS.len())],
        ) {
            let url = Url::parse(&format!("https://{}", selection)).unwrap();

            term::info!("Selected {}", term::format::highlight(selection));

            url
        } else {
            return Ok(());
        }
    };

    if options.fetch {
        fetch(project_urn, &profile, seed, storage, options)?;
    } else if options.push_self {
        push_identity(&profile, seed, storage)?;
    } else {
        push_project(project_urn, &profile, seed, storage, options)?;
    }

    // If we're in a project repo and no seed is configured, save the seed.
    if project::cwd().is_ok() && seed::get_seed(Scope::Any).is_err() {
        seed::set_seed(seed, Scope::Local(Path::new(".")))?;

        term::success!("Saving seed configuration to local git config...");
        term::tip!("To override the seed, pass the '--seed' flag to `rad sync` or `rad push`.");
        term::tip!(
            "To change the configured seed, run `git config rad.seed <url>` with a seed URL.",
        );
    }

    Ok(())
}

pub fn push_identity(profile: &Profile, seed: &Url, storage: Storage) -> anyhow::Result<()> {
    let monorepo = profile.paths().git_dir();
    let identity = person::local(&storage)?;
    let urn = identity.urn();

    term::headline(&format!(
        "Syncing 🌱 identity {} to {}",
        term::format::highlight(&urn),
        term::format::highlight(seed)
    ));

    seed::push_identity(monorepo, seed, &urn, storage.peer_id())?;

    Ok(())
}

pub fn push_project(
    project_urn: Urn,
    profile: &Profile,
    seed: &Url,
    storage: Storage,
    options: Options,
) -> anyhow::Result<()> {
    let monorepo = profile.paths().git_dir();
    let peer_id = storage.peer_id();
    let signing_key = git::git(monorepo, ["config", "--local", git::CONFIG_SIGNING_KEY])
        .context("git signing key is not properly configured")?;
    let proj = project::get(&storage, &project_urn)?.ok_or_else(|| {
        anyhow!(
            "project {} was not found in local storage under profile {}",
            project_urn,
            profile.id()
        )
    })?;

    term::info!(
        "Radicle signing key {}",
        term::format::dim(signing_key.trim())
    );
    term::blank();
    term::info!(
        "Syncing 🌱 project {} to {}",
        term::format::highlight(&project_urn),
        term::format::highlight(seed)
    );
    term::blank();

    let mut spinner = term::spinner("Syncing...");

    // Sync project delegates to seed.
    for delegate in proj.delegates.iter() {
        spinner.message(format!("Syncing delegate {}...", &delegate.encode_id()));

        match seed::push_delegate(monorepo, seed, delegate, peer_id) {
            Ok(output) => {
                if options.verbose {
                    spinner.finish();
                    term::blob(output);
                }
            }
            Err(err) => {
                spinner.failed();
                term::blank();
                return Err(err);
            }
        }
    }

    spinner.message("Syncing project identity...".to_owned());
    match seed::push_identity(monorepo, seed, &project_urn, peer_id) {
        Ok(output) => {
            if options.verbose {
                spinner.finish();
                term::blob(output);
            }
        }
        Err(err) => {
            spinner.failed();
            term::blank();
            return Err(err);
        }
    }

    spinner.message("Syncing project refs...".to_owned());
    match seed::push_refs(monorepo, seed, &project_urn, peer_id) {
        Ok(output) => {
            if options.verbose {
                spinner.finish();
                term::blob(output);
            }
        }
        Err(err) => {
            spinner.failed();
            term::blank();
            return Err(err);
        }
    }

    if !options.verbose {
        spinner.message("Project synced.".to_owned());
    }
    spinner.finish();

    term::blank();

    if let Some(host) = seed.host() {
        let is_routable = match host {
            url::Host::Domain("localhost") => false,
            url::Host::Domain(_) => true,
            url::Host::Ipv4(ip) => !ip.is_loopback() && !ip.is_unspecified() && !ip.is_private(),
            url::Host::Ipv6(ip) => !ip.is_loopback() && !ip.is_unspecified(),
        };
        let project_id = project_urn.encode_id();
        let git_url = seed.join(&project_id)?;

        term::info!("🪴 Your project is available at:");
        term::blank();

        if is_routable {
            if proj.remotes.contains(peer_id) {
                term::indented(&format!(
                    "{} {}",
                    term::format::dim("(web)"),
                    term::format::highlight(format!(
                        "https://{}/seeds/{}/{}",
                        GATEWAY_HOST, host, project_urn
                    ))
                ));
            }
            term::indented(&format!(
                "{} {}",
                term::format::dim("(web)"),
                term::format::highlight(format!(
                    "https://{}/seeds/{}/{}/remotes/{}",
                    GATEWAY_HOST, host, project_urn, peer_id
                ))
            ));
        }
        term::indented(&format!(
            "{} {}",
            term::format::dim("(git)"),
            term::format::highlight(format!("{}.git", git_url)),
        ));
        term::blank();
    }
    Ok(())
}

pub fn fetch(
    project_urn: Urn,
    profile: &Profile,
    seed: &Url,
    storage: Storage,
    options: Options,
) -> anyhow::Result<()> {
    term::blank();
    term::info!(
        "Syncing 🌱 project {} from {}",
        term::format::highlight(&project_urn),
        term::format::highlight(seed)
    );
    term::blank();

    let track_default =
        tracking::default_only(&storage, &project_urn).context("couldn't read tracking graph")?;
    let tracked =
        tracking::tracked_peers(&storage, Some(&project_urn))?.collect::<Result<Vec<_>, _>>()?;

    if !track_default && tracked.is_empty() {
        let cfg = tracking::config::Config::default();

        tracking::track(
            &storage,
            &project_urn,
            None,
            cfg,
            tracking::policy::Track::Any,
        )??;

        term::success!(
            "Tracking relationship established for {}",
            term::format::highlight(&project_urn)
        );
    }

    let monorepo = profile.paths().git_dir();

    // Sync identity and delegates.
    if options.identity {
        let mut spinner = term::spinner("Fetching project identity...");

        match seed::fetch_identity(monorepo, seed, &project_urn) {
            Ok(output) => {
                if options.verbose {
                    spinner.finish();
                    term::blob(output);
                }
            }
            Err(err) => {
                spinner.failed();
                term::blank();

                return Err(err).with_context(|| {
                    format!(
                        "project {} was not found on {}",
                        project_urn,
                        seed.host_str().unwrap_or("seed")
                    )
                });
            }
        }

        let proj =
            project::get(&storage, &project_urn)?.ok_or(anyhow!("project could not be loaded!"))?;

        for delegate in &proj.delegates {
            spinner.message(format!(
                "Fetching project delegate {}...",
                delegate.encode_id()
            ));
            match seed::fetch_identity(monorepo, seed, delegate) {
                Ok(output) => {
                    if options.verbose {
                        spinner.finish();
                        term::blob(output);
                    }
                }
                Err(err) => {
                    spinner.failed();
                    term::blank();
                    return Err(err);
                }
            }
        }

        if !options.verbose {
            spinner.message("Fetching project identity...".to_owned());
        }
        spinner.finish();
    }

    let spinner = term::spinner("Verifying signatures...");
    let proj: project::Metadata = match identities::project::verify(&storage, &project_urn) {
        Ok(Some(proj)) => {
            spinner.finish();
            proj.into_inner().try_into()?
        }
        Ok(None) => {
            spinner.failed();
            term::blank();
            return Err(anyhow!(
                "project {} could not be found on local device",
                project_urn
            ));
        }
        Err(err) => {
            spinner.failed();
            term::blank();
            return Err(err.into());
        }
    };

    // Start with the default set of remotes that should always be tracked.
    // These are the remotes of the project delegates.
    let mut remotes: HashSet<PeerId> = proj.remotes.clone();
    // Add the explicitly tracked peers.
    remotes.extend(tracked.clone());

    let spinner = if remotes == proj.remotes {
        term::spinner("Fetching default remotes...")
    } else {
        term::spinner("Fetching tracked remotes...")
    };
    match seed::fetch_peers(monorepo, seed, &project_urn, remotes) {
        Ok(output) => {
            spinner.finish();
            if options.verbose {
                term::blob(output);
            }
        }
        Err(err) => {
            spinner.error(err);
        }
    }

    // Fetch refs from peer seeds.
    for peer in &tracked {
        if let Ok(seed) = seed::get_peer_seed(peer) {
            let spinner = term::spinner(&format!(
                "Fetching {} from {}...",
                term::format::tertiary(peer),
                term::format::tertiary(&seed)
            ));

            match seed::fetch_peers(monorepo, &seed, &project_urn, [*peer]) {
                Ok(output) => {
                    spinner.finish();
                    if options.verbose {
                        term::blob(output);
                    }
                }
                Err(err) => {
                    spinner.error(err);
                }
            }
        }
    }

    Ok(())
}
