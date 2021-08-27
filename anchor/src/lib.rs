use std::convert::{TryFrom, TryInto};
use std::path::PathBuf;

use anyhow::Context;
use anyhow::{anyhow, bail};

use multihash::derive::Multihash;
use multihash::Digest as _;
use multihash::{MultihashDigest, Sha1Digest, U20, U32};

use coins_bip32::path::DerivationPath;

use ethers::{
    abi::{Abi, Bytes},
    contract::Contract,
    prelude::{JsonRpcClient, Signer, SignerMiddleware},
    providers::{Http, Provider},
    signers::{HDPath, Ledger},
};

use ethers::prelude::Middleware;

pub use ethers::types::Address;
pub use link_identities::git::Urn;

pub struct Options {
    pub org: Address,
    pub project: Urn,
    pub commit: String,
    pub rpc_url: String,
    pub ledger_hdpath: Option<DerivationPath>,
    pub keystore: Option<PathBuf>,
    pub dry: bool,
}

const PROJECT_COMMIT_ANCHOR: u32 = 0x0;
const ORG_ABI: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/abis/OrgV1.json"));

#[derive(Clone, Copy, Debug, Eq, Multihash, PartialEq)]
#[mh(alloc_size = U32)]
pub enum Code {
    #[mh(code = 0x11, hasher = multihash::Sha1, digest = Sha1Digest<U20>)]
    Sha1,
}

/// Anchor error.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// No wallet specified.
    #[error("no wallet specified")]
    NoWallet,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    let provider =
        Provider::<Http>::try_from(opts.rpc_url.as_str()).context("JSON-RPC URL parsing failed")?;
    let chain_id = provider.get_chainid().await?.as_u64();

    if let Some(keypath) = &opts.keystore {
        use colored::*;

        log::info!("Decrypting keystore..");

        let prompt = format!("{} Password: ", "??".cyan());
        let password = rpassword::prompt_password_stdout(&prompt).unwrap();
        let signer = ethers::signers::LocalWallet::decrypt_keystore(keypath, password)
            .map_err(|_| anyhow!("keystore decryption failed"))?
            .with_chain_id(chain_id);

        log::debug!("Keystore decrypted: {:?}.", signer);

        anchor(opts, provider, signer).await
    } else if let Some(path) = &opts.ledger_hdpath {
        log::debug!("Connecting to Ledger..");

        let hdpath = path.derivation_string();
        let signer = Ledger::new(HDPath::Other(hdpath), chain_id).await?;

        anchor(opts, provider, signer).await
    } else {
        Err(anyhow!(Error::NoWallet))
    }
}

async fn anchor<P: 'static + JsonRpcClient, S: 'static + Signer>(
    opts: Options,
    provider: Provider<P>,
    signer: S,
) -> anyhow::Result<()> {
    let abi: Abi = serde_json::from_str(ORG_ABI)?;
    let project = opts.project;
    let commit = opts.commit;

    log::info!("Anchoring..");
    log::info!("Chain ID {}", signer.chain_id());
    log::info!("Radicle ID {}", project);
    log::info!("Org {:?}", opts.org);
    log::info!("Anchor hash {}", commit);
    log::info!("Anchor type 'git commit' ({:#x})", PROJECT_COMMIT_ANCHOR);

    let client = SignerMiddleware::new(provider, signer);
    let contract = Contract::new(opts.org, abi, client);

    // The project id, as a `bytes32`.
    let id: [u8; 32] = {
        let bytes = project.id.as_bytes();
        let mut padded = vec![0; 12];

        padded.extend(bytes);
        padded.try_into().unwrap()
    };
    // The anchor tag as a `uint32`.
    let tag: u32 = PROJECT_COMMIT_ANCHOR;
    // The anchor hash as a `bytes` in multihash format.
    let hash: Bytes = {
        if commit.len() != 40 {
            bail!("Invalid SHA-1 commit specified");
        }
        let bytes = (0..commit.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&commit[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()?;

        let digest: Sha1Digest<multihash::U20> = Sha1Digest::wrap(&bytes)?;
        let commit = Code::multihash_from_digest(&digest);

        commit.to_bytes().to_vec()
    };

    if opts.dry {
        return Ok(());
    }
    log::info!("Sending transaction..");

    let tx = contract.method::<_, ()>("anchor", (id, tag, hash))?;
    let result = loop {
        let pending = tx.send().await?;
        let tx_hash = *pending;

        log::info!("Waiting for transaction {:?} to be included..", tx_hash);

        if let Some(result) = pending.await? {
            break result;
        } else {
            log::info!("Transaction {} dropped, retrying..", tx_hash);
        }
    };

    log::info!(
        "Project successfully anchored in block #{} ({})",
        result.block_number.unwrap(),
        result.block_hash.unwrap(),
    );

    Ok(())
}
