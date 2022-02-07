use librad::git::{identities, tracking, Urn};
use librad::profile::Profile;

use rad_common::{git, keys, profile, project, seed};
use rad_terminal::components as term;
use rad_terminal::components::{Args, Error, Help};

use anyhow::anyhow;
use anyhow::Context as _;
use url::{Host, Url};

use std::str::FromStr;

pub const GATEWAY_HOST: &str = "app.radicle.network";
pub const HELP: Help = Help {
    name: "sync",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad sync [<urn>] [--seed <host> | --seed-url <url>] [--fetch]

OPTIONS
    --seed <host>       Use the given seed node for syncing
    --seed-url <url>    Use the given seed node URL for syncing
    --fetch             Fetch updates (default: false)
    --help              Print help
"#,
};

pub struct Addr {
    pub host: Host,
    pub port: Option<u16>,
}

impl std::fmt::Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(port) = self.port {
            write!(f, "{}:{}", self.host, port)
        } else {
            write!(f, "{}", self.host)
        }
    }
}

impl FromStr for Addr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((host, port)) => {
                let host = Host::parse(host)?;
                let port = Some(port.parse()?);

                Ok(Self { host, port })
            }
            None => {
                let host = Host::parse(s)?;

                Ok(Self { host, port: None })
            }
        }
    }
}

pub struct Options {
    pub seed: Option<Addr>,
    pub seed_url: Option<Url>,
    pub urn: Option<Urn>,
    pub verbose: bool,
    pub fetch: bool,
    pub force: bool,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();
        let mut seed: Option<Addr> = None;
        let mut seed_url: Option<Url> = None;
        let mut verbose = false;
        let mut fetch = false;
        let mut urn: Option<Urn> = None;
        let mut force = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("seed") if seed_url.is_none() => {
                    let value = parser.value()?;
                    let value = value.to_string_lossy();
                    let value = value.as_ref();
                    let addr =
                        Addr::from_str(value).context("invalid host specified for `--seed`")?;

                    seed = Some(addr);
                }
                Long("seed-url") if seed.is_none() => {
                    let value = parser.value()?;
                    let value = value.to_string_lossy();
                    let value = value.as_ref();
                    let url =
                        Url::from_str(value).context("invalid URL specified for `--seed-url`")?;

                    seed_url = Some(url);
                }
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
                _ => {
                    return Err(anyhow::anyhow!(arg.unexpected()));
                }
            }
        }

        Ok(Options {
            seed,
            seed_url,
            fetch,
            force,
            urn,
            verbose,
        })
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = Profile::load()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;
    let monorepo = profile.paths().git_dir();

    let project_urn = if let Some(urn) = &options.urn {
        urn.clone()
    } else {
        let repo = project::repository()?;
        project::remote(&repo)?.url.urn
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

    let seed = &if let Some(seed) = options.seed {
        Url::parse(&format!("https://{}", seed)).unwrap()
    } else if let Some(seed) = options.seed_url {
        seed
    } else if let Ok(seed) = seed::get_seed() {
        seed
    } else {
        term::info!("Select a seed node to sync with...");

        if let Some(selection) = term::select(seed::DEFAULT_SEEDS, &seed::DEFAULT_SEEDS[0]) {
            let url = Url::parse(&format!("https://{}", selection)).unwrap();

            term::info!("Selected {}", term::format::highlight(selection));
            term::info!("Saving seed configuration to git...");

            seed::set_seed(&url)?;

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
        match seed::fetch_project(monorepo, seed, &seed_id, &project_urn) {
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

        let spinner = term::spinner("Verifying...");
        match identities::project::verify(&storage, &project_urn) {
            Ok(Some(_)) => {
                spinner.finish();
            }
            Ok(None) => {
                spinner.failed();
                term::blank();
                return Err(anyhow!("project is inaccessible"));
            }
            Err(err) => {
                spinner.failed();
                term::blank();
                return Err(err.into());
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
                    "https://{}/seeds/{}/projects/{}",
                    GATEWAY_HOST, host, project_urn
                ))
            ));
            term::indented(&format!(
                "{} {}",
                term::format::dim("(web)"),
                term::format::highlight(format!(
                    "https://{}/seeds/{}/projects/{}/remotes/{}",
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
