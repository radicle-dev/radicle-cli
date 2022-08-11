use std::ffi::OsString;

use anyhow::Context as _;

use ethers::prelude::Chain;
use ethers::signers::{HDPath, Ledger};

use radicle_common::args::{Args, Error, Help};
use radicle_common::tokio;
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "account",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad account [--testnet]

Options

    --testnet  Use the Ethereum "Rinkeby" testnet (default: false)
"#,
};

/// Work with Ethereum accounts.
#[derive(Debug, Default)]
pub struct Options {
    /// Use the Ethereum "Rinkeby" testnet (default: false)
    pub testnet: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut testnet = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("testnet") => {
                    testnet = true;
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        Ok((Options { testnet }, vec![]))
    }
}

pub fn run(opts: Options, _ctx: impl term::Context) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        let chain_id: u64 = if opts.testnet {
            Chain::Rinkeby.into()
        } else {
            Chain::Mainnet.into()
        };

        let ledger = Ledger::new(HDPath::LedgerLive(0), chain_id)
            .await
            .context("couldn't connect to Ledger device")?;

        for i in 0..=8 {
            let path = HDPath::LedgerLive(i);

            println!(
                "{} {:?}",
                term::format::dim(path.to_string()),
                ledger.get_address_with_path(&path).await?
            );
        }

        Ok(())
    })
}
