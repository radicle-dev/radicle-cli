#![allow(clippy::or_fun_call)]
use librad::git::Storage;
use librad::git::{identities, tracking, Urn};
use librad::profile::Profile;
use librad::PeerId;

use radicle_common::args;
use radicle_common::args::{Args, Error, Help};
use radicle_common::seed::SeedOptions;
use radicle_common::{git, keys, person, profile, project, seed, seed::Scope};
use radicle_terminal as term;

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

    rad sync [--seed <host>] [--fetch] [--self] [<options>...]
    rad sync <urn> [--seed <host>] [--fetch] [--self] [<options>...]
    rad sync <url> [--fetch] [--self] [<options>...]

    If a <urn> is specified, a seed may be given via the `--seed` option.
    If a <url> is specified, the seed is implied.
    If neither is specified, the URN and seed of the current project is used.

    By default, only the project's *default* branch is synced. To sync all branches,
    use the `--all` flag. To sync a specific branch, use the `--branch` flag.

Options

    --seed <host>       Use the given seed node for syncing
    --[no-]identity     Sync identity refs (default: true)
    --fetch             Fetch updates (default: false)
    --self              Sync your local identity (default: false)
    --all               Sync all branches, not just the default branch (default: false)
    --branch <name>     Sync only the given branch
    --help              Print help
"#,
};

#[derive(Debug)]
pub enum Refs {
    /// Sync all branches.
    All,
    /// Sync only a specific branch.
    Branch(String),
    /// Sync the default branch.
    DefaultBranch,
}

impl Default for Refs {
    fn default() -> Self {
        Refs::DefaultBranch
    }
}

#[derive(Default, Debug)]
pub struct Options {
    pub origin: Option<project::Origin>,
    pub seed: Option<seed::Address>,
    pub refs: Refs,
    pub verbose: bool,
    pub fetch: bool,
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
        let mut push_self = false;
        let mut identity = true;
        let mut refs = None;
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
                Long("all") if refs.is_none() => {
                    refs = Some(Refs::All);
                }
                Long("branch") if refs.is_none() => {
                    let val = parser
                        .value()?
                        .to_str()
                        .ok_or(anyhow!("invalid head specified with `--branch`"))?
                        .to_owned();

                    refs = Some(Refs::Branch(val));
                }
                Long("default-branch") if refs.is_none() => {
                    refs = Some(Refs::DefaultBranch);
                }
                Long("identity") => {
                    identity = true;
                }
                Long("no-identity") => {
                    identity = false;
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

        if fetch {
            if push_self {
                anyhow::bail!("`--fetch` and `--self` cannot be used together");
            }
            match refs {
                Some(Refs::All) | None => {}
                Some(Refs::Branch { .. }) => {
                    anyhow::bail!("`--fetch` and `--branch` cannot be used together");
                }
                Some(Refs::DefaultBranch) => {
                    anyhow::bail!("`--fetch` and `--default-branch` cannot be used together");
                }
            }
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
                push_self,
                refs: refs.unwrap_or(Refs::DefaultBranch),
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
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;

    let project_urn = if let Some(origin) = &options.origin {
        origin.urn.clone()
    } else {
        project::cwd().map(|(urn, _)| urn)?
    };
    term::info!("Git version {}", git::check_version()?);

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
        push_self(&profile, seed, storage, options)?;
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

pub fn push_self(
    profile: &Profile,
    seed: &Url,
    storage: Storage,
    options: Options,
) -> anyhow::Result<()> {
    let monorepo = profile.paths().git_dir();
    let identity = person::local(&storage)?;
    let urn = identity.urn();

    term::headline(&format!(
        "Syncing 🌱 identity {} to {}",
        term::format::highlight(&urn),
        term::format::highlight(seed)
    ));

    let mut spinner = term::spinner("Pushing...");
    let output = seed::push_delegate(monorepo, seed, &urn, storage.peer_id())?;

    spinner.message("Local identity synced.".to_owned());
    spinner.finish();

    if options.verbose {
        term::blob(output);
    }

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
    let push_opts = seed::PushOptions {
        head: match options.refs {
            Refs::All => None,
            Refs::DefaultBranch => Some(proj.default_branch.to_string()),
            Refs::Branch(ref branch) => Some(branch.to_owned()),
        },
        all: matches!(options.refs, Refs::All),
        tags: true,
    };

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
        if let project::Delegate::Indirect { urn, .. } = &delegate {
            spinner.message(format!("Syncing delegate {}...", urn.encode_id()));

            match seed::push_delegate(monorepo, seed, urn, peer_id) {
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
    match seed::push_refs(monorepo, seed, &project_urn, peer_id, push_opts) {
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

        term::info!("🍃 Your project is available at:");
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
    let proj = if options.identity {
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

        spinner.message("Verifying project identity...".to_owned());
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

        for delegate in &proj.delegates {
            if let project::Delegate::Indirect { urn, .. } = &delegate {
                spinner.message(format!("Fetching project delegate {}...", urn.encode_id()));

                match seed::fetch_identity(monorepo, seed, urn).and_then(|out| {
                    identities::person::verify(&storage, urn)?;
                    Ok(out)
                }) {
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
        }
        spinner.finish();

        proj
    } else {
        project::get(&storage, &project_urn)?.ok_or(anyhow!("project could not be loaded!"))?
    };

    // Start with the default set of remotes that should always be tracked.
    // These are the remotes of the project delegates.
    let mut remotes: HashSet<PeerId> = proj.remotes.clone();
    // Add the explicitly tracked peers.
    remotes.extend(tracked.clone());

    let mut spinner = if remotes == proj.remotes {
        term::spinner("Fetching default remotes...")
    } else {
        term::spinner("Fetching tracked remotes...")
    };
    match term::sync::fetch_remotes(&storage, seed, &project_urn, remotes.iter(), &mut spinner) {
        Ok(output) => {
            spinner.message("Remotes fetched.".to_owned());
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
            let mut spinner = term::spinner(&format!(
                "Fetching {} from {}...",
                term::format::tertiary(peer),
                term::format::tertiary(&seed)
            ));

            match term::sync::fetch_remotes(&storage, &seed, &project_urn, [peer], &mut spinner) {
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
