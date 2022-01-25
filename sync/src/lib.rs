use librad::git::{tracking, Urn};
use librad::profile::Profile;

use rad_common::{git, keys, profile, project, seed};
use rad_terminal::compoments as term;
use rad_terminal::compoments::Args;

use anyhow::Context as _;
use url::Url;

pub const GATEWAY_HOST: &str = "app.radicle.network";
pub const NAME: &str = "sync";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DESCRIPTION: &str = "Synchronize radicle projects with seeds";
pub const USAGE: &str = r#"
USAGE
    rad sync [--seed URL]

OPTIONS
    --seed URL    Use the given seed node for syncing
    --help        Print help
"#;

pub struct Options {
    pub seed: Option<Url>,
    pub verbose: bool,
    pub help: bool,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_env();
        let mut seed: Option<Url> = None;
        let mut verbose = false;
        let mut help = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("seed") => {
                    seed = Some(
                        parser
                            .value()?
                            .parse()
                            .context("invalid URL specified for `--seed`")?,
                    );
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Long("help") => {
                    help = true;
                }
                _ => {
                    return Err(anyhow::anyhow!(arg.unexpected()));
                }
            }
        }

        Ok(Options {
            seed,
            verbose,
            help,
        })
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    if options.help {
        term::usage(NAME, VERSION, DESCRIPTION, USAGE);
        return Ok(());
    }

    let profile = Profile::load()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;

    term::info("Reading local git config...");

    let repo = project::repository()?;
    let remote = project::remote(&repo)?;
    let project_urn = &remote.url.urn;
    let project_id = Urn::encode_id(&remote.url.urn);
    let git_version = git::version()?;

    term::info(&format!(
        "Syncing 🌱 project {}",
        term::format::highlight(project_urn)
    ));

    let seed = &if let Some(seed) = options.seed {
        seed
    } else if let Ok(seed) = seed::get_seed() {
        seed
    } else {
        term::info("Select a seed node to sync with...");

        let selection = term::select(seed::DEFAULT_SEEDS, &seed::DEFAULT_SEEDS[0]);
        let url = Url::parse(&format!("https://{}", selection)).unwrap();

        term::success(selection);
        term::info("Saving seed configuration to git...");

        seed::set_seed(&url)?;

        url
    };

    term::info(&format!("Syncing to {}", term::format::highlight(seed)));
    term::info(&format!("Git version {}", git_version));

    if git_version < git::VERSION_REQUIRED {
        anyhow::bail!(
            "a minimum git version of {} is required, please update your installation",
            git::VERSION_REQUIRED
        );
    }

    let peer_id = profile::peer_id(&storage)?;
    let user_urn = profile::user(&storage)?;
    let monorepo = profile.paths().git_dir();
    let self_id = Urn::encode_id(&user_urn);
    let signing_key = git::git(monorepo, ["config", "--local", git::CONFIG_SIGNING_KEY])
        .context("git signing key is not properly configured")?;

    term::info(&format!("Git signing key {}", signing_key));

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

    {
        let track_everyone = tracking::default_only(&storage, project_urn)
            .context("couldn't read tracking graph")?;

        let remotes = if track_everyone {
            vec![]
        } else {
            tracking::tracked_peers(&storage, Some(project_urn))?.collect::<Result<Vec<_>, _>>()?
        };

        spinner = term::spinner(&format!(
            "Fetching remotes ({})...",
            if remotes.is_empty() {
                "*".to_owned()
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
    }

    term::success("Project synced.");
    term::blank();

    if let Some(host) = seed.host() {
        let is_routable = match host {
            url::Host::Domain("localhost") => false,
            url::Host::Domain(_) => true,
            url::Host::Ipv4(ip) => !ip.is_loopback() && !ip.is_unspecified() && !ip.is_private(),
            url::Host::Ipv6(ip) => !ip.is_loopback() && !ip.is_unspecified(),
        };
        let git_url = seed.join(&project_id)?;

        if is_routable {
            term::info(&format!(
                "Your project is available on the web at https://{}/seeds/{}/projects/{}",
                GATEWAY_HOST, host, project_urn,
            ));
        }
        term::info(&format!(
            "Your project repository is available via git at {}",
            git_url
        ));
    }

    Ok(())
}
