use std::convert::{Infallible, TryFrom, TryInto};
use std::path::PathBuf;

use anyhow::Context;
use anyhow::{anyhow, bail};

use multihash::derive::Multihash;
use multihash::Digest as _;
use multihash::{MultihashDigest, Sha1Digest, U20, U32};

use coins_bip32::path::DerivationPath;

use ethers::{
    abi::{Abi, Detokenize},
    contract::Contract,
    prelude::{builders::ContractCall, Bytes, JsonRpcClient, Signer, SignerMiddleware, U256},
    providers::{Http, Provider},
    signers::{HDPath, Ledger, LocalWallet},
};

use ethers::prelude::Middleware;

pub use ethers::types::Address;
pub use link_identities::git::Urn;

use safe_transaction_client as safe;

/// Anchor options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Radicle org under which to anchor the project.
    pub org: Address,
    /// Radicle project to anchor.
    pub project: Urn,
    /// Project commit hash to anchor.
    pub commit: String,
    /// JSON-RPC URL of Ethereum node (eg. http://localhost:8545).
    pub rpc_url: String,
    /// Account derivation path when using a Ledger hardware wallet.
    pub ledger_hdpath: Option<DerivationPath>,
    /// Keystore file containing encrypted private key (default: none).
    pub keystore: Option<PathBuf>,
    /// Execute a dry run.
    pub dry_run: bool,
}

const PROJECT_COMMIT_ANCHOR: u32 = 0x0;
const ORG_ABI: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/abis/OrgV1.json"));

/// Ethereum network.
#[derive(Debug)]
enum Network {
    Homestead,
    Rinkeby,
}

impl Network {
    const fn safe_transaction_url(&self) -> &'static str {
        match self {
            Self::Homestead => "https://safe-transaction.gnosis.io",
            Self::Rinkeby => "https://safe-transaction.rinkeby.gnosis.io",
        }
    }
}

impl TryFrom<u64> for Network {
    type Error = ();

    fn try_from(other: u64) -> Result<Self, ()> {
        match other {
            1 => Ok(Self::Homestead),
            4 => Ok(Self::Rinkeby),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Multihash, PartialEq)]
#[mh(alloc_size = U32)]
pub enum Code {
    #[mh(code = 0x11, hasher = multihash::Sha1, digest = Sha1Digest<U20>)]
    Sha1,
}

/// Anchor error.
#[derive(thiserror::Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Error<S: std::error::Error> {
    /// No wallet specified.
    #[error("no wallet specified")]
    NoWallet,
    /// Gnosis Safe error.
    #[error("safe transaction error: {0}")]
    Safe(#[from] safe::Error),
    /// Signature error.
    #[error("signer error: {0}")]
    Signer(S),
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
        let signer = LocalWallet::decrypt_keystore(keypath, password)
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
        Err(anyhow!(Error::<Infallible>::NoWallet))
    }
}

async fn anchor<P: 'static + JsonRpcClient + Clone, S: 'static + Signer>(
    opts: Options,
    provider: Provider<P>,
    signer: S,
) -> anyhow::Result<()> {
    let abi: Abi = serde_json::from_str(ORG_ABI)?;
    let project = opts.project;
    let commit = opts.commit;
    let chain_id = signer.chain_id();
    let network =
        Network::try_from(chain_id).map_err(|_| anyhow!("unsupported chain id '{}'", chain_id))?;

    log::info!("Anchoring..");
    log::info!("Chain ID {} ({:?})", chain_id, network);
    log::info!("Radicle ID {}", project);
    log::info!("Org {:?}", opts.org);
    log::info!("Anchor hash {}", commit);
    log::info!("Anchor type 'git commit' ({:#x})", PROJECT_COMMIT_ANCHOR);

    let contract = Contract::new(opts.org, abi.clone(), provider.clone());

    let org_owner: Address = contract.method("owner", ())?.call().await?;
    log::info!("Org owner {:#?}", org_owner);

    let safe_client = safe::Client::new(network.safe_transaction_url());
    let safe = match safe_client.get_safe(org_owner) {
        Ok(safe) => Some(safe),
        Err(err) if err.is_not_found() => None,
        Err(err) => {
            bail!("request to safe transaction API failed: {:?}", err);
        }
    };

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

        commit.to_bytes().into()
    };

    if opts.dry_run {
        return Ok(());
    }

    if let Some(safe) = safe {
        log::info!("Found Gnosis Safe at {}", org_owner);

        let call = contract.method::<_, ()>("anchor", (id, tag, hash))?;
        let data = call.calldata().unwrap();

        anchor_safe(opts.org, data, &safe, &signer).await
    } else {
        let signer = SignerMiddleware::new(provider, signer);
        let contract = Contract::new(opts.org, abi, signer);
        let call = contract.method::<_, ()>("anchor", (id, tag, hash))?;

        anchor_eoa(call).await
    }
}

async fn anchor_safe<S: Signer + 'static>(
    to: Address,
    data: Bytes,
    safe: &safe::Safe<'_>,
    signer: &S,
) -> anyhow::Result<()> {
    let safe_tx = safe.create_transaction(to, U256::zero(), data, safe::Operation::Call);
    let signed_tx = safe_tx
        .sign(signer)
        .await
        .map_err(Error::<S::Error>::Signer)?;

    safe.propose(signed_tx)?;

    Ok(())
}

async fn anchor_eoa<M: Middleware + 'static, D: Detokenize>(
    call: ContractCall<M, D>,
) -> anyhow::Result<()> {
    log::info!("Sending transaction..");

    let result = loop {
        let pending = call.send().await?;
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
