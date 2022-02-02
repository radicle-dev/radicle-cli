use librad::git::{identities, tracking, Urn};
use librad::profile::Profile;

use rad_common::{git, keys, profile, project, seed};
use rad_terminal::components as term;
use rad_terminal::components::{Args, Error, Help};

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
    rad sync [--seed <host>] [--fetch] [--http]

OPTIONS
    --seed <host>    Use the given seed node for syncing
    --fetch          Fetch updates (default: false)
    --http           Use HTTP instead of HTTPS for syncing (default: false)
    --help           Print help
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
    pub http: bool,
    pub verbose: bool,
    pub fetch: bool,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();
        let mut seed: Option<Addr> = None;
        let mut verbose = false;
        let mut fetch = false;
        let mut http = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("seed") => {
                    let value = parser.value()?;
                    let value = value.to_string_lossy();
                    let value = value.as_ref();
                    let addr =
                        Addr::from_str(value).context("invalid host specified for `--seed`")?;

                    seed = Some(addr);
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("http") => {
                    http = true;
                }
                Long("fetch") => {
                    fetch = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => {
                    return Err(anyhow::anyhow!(arg.unexpected()));
                }
            }
        }

        Ok(Options {
            seed,
            http,
            fetch,
            verbose,
        })
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = Profile::load()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;
    let monorepo = profile.paths().git_dir();

    let repo = project::repository()?;
    let remote = project::remote(&repo)?;
    let project_urn = &remote.url.urn;
    let project_id = Urn::encode_id(&remote.url.urn);
    let git_version = git::version()?;

    term::info!("Git version {}", git_version);

    if git_version < git::VERSION_REQUIRED {
        anyhow::bail!(
            "a minimum git version of {} is required, please update your installation",
            git::VERSION_REQUIRED
        );
    }

    if identities::project::get(&storage, project_urn)?.is_none() {
        anyhow::bail!(
            "this project was not found in your local storage, perhaps it was initialized with another profile?"
        );
    }

    let seed = &if let Some(seed) = options.seed {
        if options.http {
            Url::parse(&format!("http://{}", seed)).unwrap()
        } else {
            Url::parse(&format!("https://{}", seed)).unwrap()
        }
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
            term::format::highlight(project_urn),
            term::format::highlight(seed)
        );
        term::blank();

        let track_everyone = tracking::default_only(&storage, project_urn)
            .context("couldn't read tracking graph")?;

        let remotes = if track_everyone {
            vec![]
        } else {
            tracking::tracked_peers(&storage, Some(project_urn))?.collect::<Result<Vec<_>, _>>()?
        };

        let spinner = term::spinner(&format!(
            "Fetching {} remotes...",
            if remotes.is_empty() {
                "all".to_owned()
            } else {
                remotes.len().to_string()
            }
        ));

        match seed::fetch_remotes(monorepo, seed, &project_id, &remotes) {
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
    let user_urn = profile::user(&storage)?;
    let self_id = Urn::encode_id(&user_urn);
    let signing_key = git::git(monorepo, ["config", "--local", git::CONFIG_SIGNING_KEY])
        .context("git signing key is not properly configured")?;

    term::info!("Git signing key {}", term::format::dim(signing_key));
    term::info!(
        "Syncing 🌱 project {} to {}",
        term::format::highlight(project_urn),
        term::format::highlight(seed)
    );
    term::blank();

    let mut spinner = term::spinner(&format!("Syncing delegate identity {}...", &self_id));
    match seed::push_delegate_id(monorepo, seed, &self_id, peer_id) {
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

    spinner = term::spinner("Syncing project identity...");
    match seed::push_project_id(monorepo, seed, &project_id, peer_id) {
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

    spinner = term::spinner("Syncing project refs...");
    match seed::push_refs(monorepo, seed, &project_id, peer_id) {
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
