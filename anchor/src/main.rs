use argh::FromArgs;
use std::convert::{TryFrom, TryInto};
use std::process;
use std::str::FromStr;
use std::{env, path::PathBuf};

use coins_bip32::path::DerivationPath;

use rad_anchor as anchor;
use radicle_tools::logger;

use anchor::{Address, Urn};

/// Anchor a Radicle project.
#[derive(FromArgs)]
pub struct Options {
    /// radicle org under which to anchor the project
    #[argh(option)]
    pub org: Address,
    /// radicle project to anchor
    #[argh(option)]
    pub project: Urn,
    /// project commit hash to anchor
    #[argh(option)]
    pub commit: Option<String>,
    /// JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
    #[argh(option)]
    pub rpc_url: Option<String>,
    /// keystore file containing encrypted private key (default: none)
    #[argh(option)]
    pub keystore: Option<PathBuf>,
    /// account derivation path when using a Ledger hardware wallet
    #[argh(option)]
    pub ledger_hdpath: Option<DerivationPath>,
    /// execute a dry run
    #[argh(switch)]
    pub dry: bool,
    /// verbose output
    #[argh(switch, short = 'v')]
    pub verbose: bool,
}

impl Options {
    pub fn from_env() -> Self {
        argh::from_env()
    }
}

impl TryFrom<Options> for anchor::Options {
    type Error = anyhow::Error;

    fn try_from(opts: Options) -> anyhow::Result<Self> {
        let Options {
            org,
            project,
            commit,
            rpc_url,
            keystore,
            ledger_hdpath,
            dry,
            ..
        } = opts;

        let rpc_url = rpc_url
            .or_else(|| env::var("ETH_RPC_URL").ok())
            .and_then(|url| if url.is_empty() { None } else { Some(url) })
            .ok_or_else(|| {
                anyhow::anyhow!("An Ethereum JSON-RPC URL must be specified with `--rpc-url`")
            })?;

        let ledger_hdpath = ledger_hdpath.or_else(|| {
            env::var("ETH_HDPATH")
                .ok()
                .and_then(|v| DerivationPath::from_str(v.as_str()).ok())
        });

        let commit = if let Some(commit) = commit {
            commit
        } else {
            get_repository_head().map_err(|_| {
                anyhow::anyhow!(
                    "repository head could not be retrieved, \
                    please specify anchor hash with `--commit`"
                )
            })?
        };

        Ok(Self {
            org,
            project,
            commit,
            rpc_url,
            ledger_hdpath,
            keystore,
            dry,
        })
    }
}

/// Get the `HEAD` commit hash of the current repository.
fn get_repository_head() -> anyhow::Result<String> {
    use std::process::Command;

    let output = Command::new("git").arg("rev-parse").arg("HEAD").output()?;
    let string = String::from_utf8(output.stdout)?;
    let hash = string.trim_end().to_owned();

    Ok(hash)
}

#[tokio::main]
async fn main() {
    let args = Options::from_env();
    let level = if args.verbose {
        log::Level::Debug
    } else {
        log::Level::Info
    };
    logger::init(level, vec![env!("CARGO_CRATE_NAME")]).unwrap();

    match execute(args).await {
        Err(err) => {
            if let Some(&anchor::Error::NoWallet) = err.downcast_ref() {
                log::error!("Error: no wallet specified: either `--ledger-hdpath` or `--keystore` must be specified");
            } else if let Some(cause) = err.source() {
                log::error!("Error: {} ({})", err, cause);
            } else {
                log::error!("Error: {}", err);
            }
            process::exit(1);
        }
        Ok(()) => {}
    }
}

async fn execute(args: Options) -> anyhow::Result<()> {
    anchor::run(args.try_into()?).await?;

    Ok(())
}
