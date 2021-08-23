use std::convert::{TryFrom, TryInto};
use std::io;
use std::path::PathBuf;

use anyhow::Result;

use multihash::derive::Multihash;
use multihash::Digest as _;
use multihash::{MultihashDigest, Sha1Digest, U20, U32};

use ethers::{
    abi::{Abi, Bytes},
    contract::Contract,
    prelude::{JsonRpcClient, Signer, SignerMiddleware},
    providers::{Http, Provider},
    signers::{HDPath, Ledger},
};

pub use ethers::types::Address;
pub use librad::git::identities::Urn;

pub struct Options {
    pub org: Address,
    pub project: Option<Urn>,
    pub commit: Option<String>,
    pub rpc_url: String,
    pub keystore: Option<PathBuf>,
    pub testnet: bool,
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

pub async fn run(opts: Options) -> Result<()> {
    let provider = Provider::<Http>::try_from(opts.rpc_url.as_str()).unwrap();
    let chain_id: u64 = if opts.testnet { 4 } else { 1 };

    log::debug!("Chain ID {}", chain_id);

    if let Some(keypath) = &opts.keystore {
        use colored::*;

        log::info!("Decrypting keystore..");

        let prompt = format!("{} Password: ", "??".cyan());
        let password = rpassword::prompt_password_stdout(&prompt).unwrap();
        let signer = ethers::signers::LocalWallet::decrypt_keystore(keypath, password)
            .unwrap()
            .with_chain_id(chain_id);

        log::debug!("Keystore decrypted.");

        anchor(opts, provider, signer).await
    } else {
        log::debug!("Connecting to Ledger..");

        let signer = Ledger::new(HDPath::LedgerLive(0), 1)
            .await
            .unwrap()
            .with_chain_id(chain_id);

        anchor(opts, provider, signer).await
    }
}

async fn anchor<P: 'static + JsonRpcClient, S: 'static + Signer>(
    opts: Options,
    provider: Provider<P>,
    signer: S,
) -> Result<()> {
    let abi: Abi = serde_json::from_str(ORG_ABI)?;
    let project = opts.project.unwrap();
    let commit = opts.commit.unwrap();

    log::info!("Anchoring..");
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
        let bytes = (0..commit.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&commit[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()?;

        if bytes.len() != 20 {
            return Err(anyhow::Error::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid SHA-1 commit specified",
            )));
        }

        let digest: Sha1Digest<multihash::U20> = Sha1Digest::wrap(&bytes)?;
        let commit = Code::multihash_from_digest(&digest);

        commit.to_bytes().to_vec()
    };

    if opts.dry {
        return Ok(());
    }
    log::info!("Sending transaction..");

    let tx = contract.method::<_, ()>("anchor", (id, tag, hash))?;
    let pending = tx.send().await?;

    log::info!("Waiting for transaction {:?} to be processed..", *pending);

    let result = pending.confirmations(1).await?.unwrap();

    log::info!(
        "Project successfully anchored in block #{} ({})",
        result.block_number.unwrap(),
        result.block_hash.unwrap(),
    );

    Ok(())
}
