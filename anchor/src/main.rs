use std::path::PathBuf;
use std::process;

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
    pub rpc_url: String,
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

impl From<Options> for anchor::Options {
    fn from(opts: Options) -> Self {
        let Options {
            org,
            project,
            commit,
            rpc_url,
            keystore,
            testnet,
            dry,
        } = opts;

        Self {
            org,
            project,
            commit,
            rpc_url,
            keystore,
            testnet,
            dry,
        }
    }
}

#[tokio::main]
async fn main() {
    let opts = Options::from_env();

    logger::init(log::Level::Debug).unwrap();

    if let Err(err) = anchor::run(opts.into()).await {
        log::error!("Error: {}", err);
        process::exit(1);
    }
}
