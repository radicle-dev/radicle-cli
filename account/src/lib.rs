use colored::*;

use ethers::signers::{HDPath, Ledger};

pub struct Options {
    pub rpc_url: Option<String>,
    pub testnet: bool,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    let chain_id: u64 = if opts.testnet { 4 } else { 1 };

    log::debug!("Chain ID {}", chain_id);
    log::info!("Reading Ledger accounts..");

    let ledger = Ledger::new(HDPath::LedgerLive(0), chain_id).await?;

    for i in 0..=8 {
        let path = HDPath::LedgerLive(i);

        println!(
            "{} {:?}",
            path.to_string().dimmed(),
            ledger.get_address_with_path(&path).await?
        );
    }

    Ok(())
}
