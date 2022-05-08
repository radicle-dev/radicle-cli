use anyhow::{anyhow, Context};

use radicle_common::args;
use radicle_common::ethereum;
use radicle_common::ethereum::ethers::abi::Detokenize;
use radicle_common::ethereum::ethers::prelude::builders::ContractCall;
use radicle_common::ethereum::ethers::prelude::*;
use radicle_common::ethereum::SignerOptions;
use radicle_common::ethereum::WalletConnect;
use radicle_common::ethereum::{Wallet, WalletError};

use crate as term;

/// Open a wallet from the given options and provider.
pub async fn open_wallet<P>(options: SignerOptions, provider: Provider<P>) -> anyhow::Result<Wallet>
where
    P: JsonRpcClient + Clone + 'static,
{
    let chain_id = provider.get_chainid().await?.as_u64();

    if let Some(keypath) = &options.keystore {
        let password = term::secret_input_with_prompt("Keystore password");
        let spinner = term::spinner("Decrypting keystore...");
        let signer = LocalWallet::decrypt_keystore(keypath, password.unsecure())
            // Nb. Can fail if the file isn't found.
            .map_err(|e| anyhow!("keystore decryption failed: {}", e))?
            .with_chain_id(chain_id);

        spinner.finish();

        Ok(Wallet::Local(signer))
    } else if let Some(path) = &options.ledger_hdpath {
        let hdpath = path.derivation_string();
        let signer = Ledger::new(HDPath::Other(hdpath), chain_id)
            .await
            .context("Could not connect to Ledger device")?;

        Ok(Wallet::Ledger(signer))
    } else if options.walletconnect {
        let signer = WalletConnect::new()
            .map_err(|_| anyhow!("Failed to create WalletConnect client"))?
            .show_qr()
            .await
            .context("Failed to connect to WalletConnect session")?;
        Ok(Wallet::WalletConnect(signer))
    } else {
        Err(WalletError::NoWallet.into())
    }
}

/// Access the wallet specified in SignerOptions
pub async fn get_wallet(
    signer_opts: SignerOptions,
    provider: Provider<Http>,
) -> anyhow::Result<(Wallet, Provider<Http>)> {
    term::tip!("Accessing your wallet...");
    let signer = match open_wallet(signer_opts, provider.clone()).await {
        Ok(signer) => signer,
        Err(err) => {
            if let Some(WalletError::NoWallet) = err.downcast_ref::<WalletError>() {
                return Err(args::Error::WithHint {
                    err,
                    hint: "Use `--ledger-hdpath` or `--keystore` to specify a wallet.",
                }
                .into());
            } else {
                return Err(err);
            }
        }
    };

    let chain = ethereum::chain_from_id(signer.chain_id());
    term::success!(
        "Using {} network",
        term::format::highlight(
            chain
                .map(|c| c.to_string())
                .unwrap_or_else(|| String::from("unknown"))
        )
    );

    Ok((signer, provider))
}

/// Submit a transaction for signing and execution.
pub async fn transaction<M, D>(call: ContractCall<M, D>) -> anyhow::Result<TransactionReceipt>
where
    D: Detokenize,
    M: Middleware + 'static,
{
    let receipt = loop {
        let spinner = term::spinner("Waiting for transaction to be signed...");
        let tx = match call.send().await {
            Ok(tx) => {
                spinner.finish();
                tx
            }
            Err(err) => {
                spinner.failed();
                return Err(err.into());
            }
        };
        term::success!(
            "Transaction {} submitted to the network.",
            term::format::highlight(ethereum::hex(*tx))
        );

        let spinner = term::spinner("Waiting for transaction to be processed...");
        if let Some(receipt) = tx.await? {
            spinner.finish();
            break receipt;
        } else {
            spinner.failed();
        }
    };

    term::blank();
    term::info!(
        "Transaction included in block #{} ({}).",
        term::format::highlight(receipt.block_number.unwrap()),
        receipt.block_hash.unwrap(),
    );

    Ok(receipt)
}
