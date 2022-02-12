use librad::git::{identities, tracking, Urn};
use librad::profile::Profile;

use rad_common::seed::SeedOptions;
use rad_common::{git, keys, profile, project, seed};
use rad_terminal::args;
use rad_terminal::args::{Args, Error, Help};
use rad_terminal::components as term;

use anyhow::anyhow;
use anyhow::Context as _;
use url::Url;

use std::ffi::OsString;
use std::iter;
use std::str::FromStr;

pub const GATEWAY_HOST: &str = "app.radicle.network";
pub const HELP: Help = Help {
    name: "sync",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad sync [<urn>] [--seed <host> | --seed-url <url>] [--fetch]

Options

    --seed <host>       Use the given seed node for syncing
    --seed-url <url>    Use the given seed node URL for syncing
    --fetch             Fetch updates (default: false)
    --help              Print help
"#,
};

#[derive(Default, Debug)]
pub struct Options {
    pub urn: Option<Urn>,
    pub verbose: bool,
    pub fetch: bool,
    pub force: bool,
    pub seed: SeedOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let (seed, unparsed) = SeedOptions::from_args(args)?;
        let mut parser = lexopt::Parser::from_args(unparsed);
        let mut verbose = false;
        let mut fetch = false;
        let mut urn: Option<Urn> = None;
        let mut force = false;
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
                Long("force") | Short('f') => {
                    force = true;
                }
                Value(val) if urn.is_none() => {
                    let val = val.to_string_lossy();
                    let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                    urn = Some(val);
                }
                arg => {
                    unparsed = iter::once(args::format(arg))
                        .chain(iter::from_fn(|| parser.value().ok()))
                        .collect();

                    break;
                }
            }
        }

        Ok((
            Options {
                seed,
                fetch,
                force,
                urn,
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
    let profile = Profile::load()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;
    let monorepo = profile.paths().git_dir();
    let mut tips = Vec::new();

    let project_urn = if let Some(urn) = &options.urn {
        urn.clone()
    } else {
        project::urn()?
    };
    let project_id = project_urn.encode_id();
    let git_version = git::version()?;

    term::info!("Git version {}", git_version);

    if git_version < git::VERSION_REQUIRED {
        anyhow::bail!(
            "a minimum git version of {} is required, please update your installation",
            git::VERSION_REQUIRED
        );
    }

    let seed = &if let Some(seed_url) = options.seed.seed_url() {
        seed_url
    } else if let Ok(seed) = seed::get_seed() {
        seed
    } else {
        term::info!("Select a seed node to sync with...");

        if let Some(selection) = term::select(
            seed::DEFAULT_SEEDS,
            &seed::DEFAULT_SEEDS[fastrand::usize(0..seed::DEFAULT_SEEDS.len())],
        ) {
            let url = Url::parse(&format!("https://{}", selection)).unwrap();

            term::info!("Selected {}", term::format::highlight(selection));
            term::info!("Saving seed configuration to git...");

            seed::set_seed(&url)?;

            tips.push("To override the seed, pass the `--seed` flag to `rad sync` or `rad push` (see `rad sync --help`).");
            tips.push("To change the configured seed, run `git config --global rad.seed <url>` with a seed URL.");

            url
        } else {
            return Ok(());
        }
    };

    if options.fetch {
        term::blank();
        term::info!(
            "Syncing 🌱 project {} from {}",
            term::format::highlight(&project_urn),
            term::format::highlight(seed)
        );
        term::blank();

        let seed_id = seed::get_seed_id(seed.clone())?;
        term::info!("Seed ID is {}", term::format::highlight(seed_id));

        let track_everyone = tracking::default_only(&storage, &project_urn)
            .context("couldn't read tracking graph")?;

        let remotes = if track_everyone {
            vec![]
        } else {
            tracking::tracked_peers(&storage, Some(&project_urn))?.collect::<Result<Vec<_>, _>>()?
        };

        let spinner = term::spinner("Fetching project identity...");
        match seed::fetch_identity(monorepo, seed, &project_urn) {
            Ok(output) => {
                spinner.finish();

                if options.verbose {
                    term::blob(output);
                }
            }
            Err(err) => {
                spinner.failed();
                term::blank();

                return Err(err)
                    .with_context(|| format!("project {} was not found on the seed", project_urn));
            }
        }

        let proj =
            project::get(&storage, &project_urn)?.ok_or(anyhow!("project could not be loaded!"))?;

        for delegate in proj.delegates {
            let spinner = term::spinner(&format!(
                "Fetching project delegate {}...",
                delegate.encode_id()
            ));
            match seed::fetch_identity(monorepo, seed, &delegate) {
                Ok(output) => {
                    spinner.finish();

                    if options.verbose {
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

        let spinner = term::spinner("Verifying...");
        match identities::project::verify(&storage, &project_urn) {
            Ok(Some(_)) => {
                spinner.finish();
            }
            Ok(None) => {
                spinner.failed();
                term::blank();
                return Err(anyhow!("project {} could not be loaded", project_urn));
            }
            Err(err) => {
                spinner.failed();
                term::blank();
                return Err(err.into());
            }
        }

        let spinner = term::spinner("Fetching project heads...");
        match seed::fetch_heads(monorepo, seed, &project_urn) {
            Ok(output) => {
                spinner.finish();

                if options.verbose {
                    term::blob(output);
                }
            }
            Err(err) => {
                spinner.failed();
                term::blank();
                return Err(err);
            }
        }

        let spinner = term::spinner(&format!(
            "Fetching {} remotes...",
            if remotes.is_empty() {
                "all".to_owned()
            } else {
                remotes.len().to_string()
            }
        ));
        match seed::fetch_remotes(monorepo, seed, &project_urn, &remotes) {
            Ok(output) => {
                spinner.finish();

                if options.verbose {
                    term::blob(output);
                }
            }
            Err(err) => {
                spinner.failed();
                term::blank();
                return Err(err);
            }
        }

        if !tips.is_empty() {
            for tip in tips {
                term::tip(tip);
            }
        }
        return Ok(());
    }

    let peer_id = profile::peer_id(&storage)?;
    let signing_key = git::git(monorepo, ["config", "--local", git::CONFIG_SIGNING_KEY])
        .context("git signing key is not properly configured")?;
    let proj = project::get(&storage, &project_urn)?.ok_or_else(|| {
        anyhow!(
            "project {} was not found in local storage under profile {}",
            project_urn,
            profile.id()
        )
    })?;

    term::info!("Git signing key {}", term::format::dim(signing_key));
    term::info!(
        "Syncing 🌱 project {} to {}",
        term::format::highlight(&project_urn),
        term::format::highlight(seed)
    );
    term::blank();

    // Sync project delegates to seed.
    for delegate in proj.delegates.iter() {
        let spinner = term::spinner(&format!("Syncing delegate {}...", &delegate.encode_id()));
        match seed::push_delegate(monorepo, seed, delegate, peer_id) {
            Ok(output) => {
                spinner.finish();

                if options.verbose {
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

    let spinner = term::spinner("Syncing project identity...");
    match seed::push_project(monorepo, seed, &project_urn, peer_id) {
        Ok(output) => {
            spinner.finish();

            if options.verbose {
                term::blob(output);
            }
        }
        Err(err) => {
            spinner.failed();
            term::blank();
            return Err(err);
        }
    }

    let spinner = term::spinner("Syncing project refs...");
    match seed::push_refs(monorepo, seed, &project_urn, peer_id) {
        Ok(output) => {
            spinner.finish();

            if options.verbose {
                term::blob(output);
            }
        }
        Err(err) => {
            spinner.failed();
            term::blank();
            return Err(err);
        }
    }

    term::success!("Project synced.");
    term::blank();

    if let Some(host) = seed.host() {
        let is_routable = match host {
            url::Host::Domain("localhost") => false,
            url::Host::Domain(_) => true,
            url::Host::Ipv4(ip) => !ip.is_loopback() && !ip.is_unspecified() && !ip.is_private(),
            url::Host::Ipv6(ip) => !ip.is_loopback() && !ip.is_unspecified(),
        };
        let git_url = seed.join(&project_id)?;

        term::info!("🌱 Your project is synced and available at:");
        term::blank();

        if is_routable {
            term::indented(&format!(
                "{} {}",
                term::format::dim("(web)"),
                term::format::highlight(format!(
                    "https://{}/seeds/{}/{}",
                    GATEWAY_HOST, host, project_urn
                ))
            ));
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

    if !tips.is_empty() {
        for tip in tips {
            term::tip(tip);
        }
    }

    Ok(())
}
