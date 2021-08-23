use std::convert::{TryFrom, TryInto};
use std::io;
use std::process;
use std::{env, path::PathBuf};

use argh::FromArgs;

use radicle_anchor as anchor;
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
    pub project: Option<Urn>,
    /// project commit hash to anchor
    #[argh(option)]
    pub commit: Option<String>,
    /// JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
    #[argh(option)]
    pub rpc_url: Option<String>,
    /// keystore file containing encrypted private key (default: none)
    #[argh(option)]
    pub keystore: Option<PathBuf>,
    /// transact on the Ethereum "Rinkeby" testnet (default: false)
    #[argh(switch)]
    pub testnet: bool,
    /// execute a dry run
    #[argh(switch)]
    pub dry: bool,
}

impl Options {
    pub fn from_env() -> Self {
        argh::from_env()
    }
}

impl TryFrom<Options> for anchor::Options {
    type Error = anyhow::Error;

    fn try_from(opts: Options) -> Result<Self, anyhow::Error> {
        let Options {
            org,
            project,
            commit,
            rpc_url,
            keystore,
            testnet,
            dry,
        } = opts;

        let rpc_url = rpc_url
            .or_else(|| env::var("ETH_RPC_URL").ok())
            .ok_or(io::Error::new(
                io::ErrorKind::InvalidInput,
                "An Ethereum JSON-RPC URL must be specified with `--rpc-url`",
            ))?;

        Ok(Self {
            org,
            project,
            commit,
            rpc_url,
            keystore,
            testnet,
            dry,
        })
    }
}

#[tokio::main]
async fn main() {
    logger::init(log::Level::Debug).unwrap();

    let args = Options::from_env();
    if let Err(err) = execute(args).await {
        log::error!("Error: {}", err);
        process::exit(1);
    }
}

async fn execute(args: Options) -> anyhow::Result<()> {
    anchor::run(args.try_into()?).await?;

    Ok(())
}
