use std::convert::{TryFrom, TryInto};
use std::env;
use std::process;

use argh::FromArgs;

use rad_account as account;
use radicle_tools::logger;

/// Work with Ethereum accounts.
#[derive(FromArgs)]
pub struct Options {
    /// JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
    #[argh(option)]
    pub rpc_url: Option<String>,
    /// transact on the Ethereum "Rinkeby" testnet (default: false)
    #[argh(switch)]
    pub testnet: bool,
}

impl Options {
    pub fn from_env() -> Self {
        argh::from_env()
    }
}

impl TryFrom<Options> for account::Options {
    type Error = anyhow::Error;

    fn try_from(opts: Options) -> anyhow::Result<Self> {
        let Options { rpc_url, testnet } = opts;
        let rpc_url = rpc_url.or_else(|| env::var("ETH_RPC_URL").ok());

        Ok(Self { rpc_url, testnet })
    }
}

#[tokio::main]
async fn main() {
    logger::init(env!("CARGO_CRATE_NAME")).unwrap();
    logger::set_level(log::Level::Info);

    let args = Options::from_env();
    if let Err(err) = execute(args).await {
        if let Some(cause) = err.source() {
            log::error!("Error: {} ({})", err, cause);
        } else {
            log::error!("Error: {}", err);
        }
        process::exit(1);
    }
}

async fn execute(args: Options) -> anyhow::Result<()> {
    account::run(args.try_into()?).await?;

    Ok(())
}
