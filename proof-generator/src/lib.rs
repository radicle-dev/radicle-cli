use anyhow::anyhow;
use coins_bip32::path::DerivationPath;
use ethers::{
    prelude::Signer,
    providers::{Http, Middleware, Provider},
    signers::{HDPath, Ledger},
    types::{Signature, H160, H256},
};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow,
    convert::TryFrom,
    fmt::{Debug, Display},
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    str,
};

/// The options allowed to be provided to the CLI
#[derive(Debug, Clone)]
pub struct Options {
    /// GPG key to sign the proof.
    pub gpg_key: String,
    /// Output path of created proof
    pub output: PathBuf,
    /// RPC url
    pub rpc_url: String,
    /// Account derivation path when using a Ledger hardware wallet.
    pub ledger_hdpath: Option<DerivationPath>,
    /// Keystore file containing encrypted private key (default: none).
    pub keystore: Option<PathBuf>,
}

/// Proof that a GPG key belongs to the same person as an Ethereum key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    /// Message to be signed by signee
    msg: String,
    /// GPG signature of message
    gpg_sig: String,
    /// ETH signature of message
    eth_sig: Signature,
    /// GPG key fingerprint of the signee
    gpg_key: String,
    /// ETH address of the signee
    eth_key: H160,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// No wallet specified.
    #[error("no wallet specified")]
    NoWallet,
    /// Not able to retrieve block .
    #[error("not able to retrieve block")]
    NoBlock,
    /// Not able to retrieve block hash .
    #[error("not able to retrieve block hash")]
    NoBlockHash,
    /// ETH signature failed
    #[error("eth signature failed")]
    ETHSigFailed,
    /// GPG signature failed
    #[error("{0}")]
    GPGSigFailed(String),
}

/// Sign a message with a GPG private key using the GPG CLI
fn gpg_sign(key: &str, message: &str) -> anyhow::Result<String> {
    let mut gpg = Command::new("gpg")
        .arg("--clear-sign")
        .arg("-u")
        .arg(key)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    gpg.stdin.as_mut().unwrap().write_all(message.as_bytes())?;

    let output = gpg.wait_with_output()?;
    if output.status.success() {
        let raw_output = String::from_utf8_lossy(&output.stdout);
        Ok(raw_output.to_string())
    } else {
        Err(anyhow!(Error::GPGSigFailed(
            String::from_utf8_lossy(output.stderr.borrow()).to_string()
        )))
    }
}

/// Sign a message with a ETH private key using either a keystore file or a Ledger HW
pub async fn eth_sign<S: 'static + Signer>(
    signer: &S,
    proof: &str,
) -> Result<Signature, <S as Signer>::Error> {
    signer.sign_message(proof).await
}

/// Create the message to be signed
fn create_message<T: Display, K: Debug>(ownership: &T, evidence: &K, block_hash: &H256) -> String {
    format!(
        "As the owner of GPG key {}, my Ethereum address is {:?} as of {:?}",
        &ownership, &evidence, &block_hash
    )
}

/// Sign the message with GPG and ETH keypairs
async fn create_proof<T: 'static + Signer>(
    gpg_key: &str,
    signer: &T,
    block_hash: &H256,
) -> anyhow::Result<Proof> {
    let msg = create_message(&gpg_key, &signer.address(), block_hash);

    log::info!("Signing message with ETH keypair..");
    let eth_sig = eth_sign(signer, &msg)
        .await
        .map_err(|_| anyhow!(Error::ETHSigFailed))?;
    log::debug!("ETH Signature: {:?}.", eth_sig);

    log::info!("Signing message with GPG keypair..");
    let gpg_sig = gpg_sign(gpg_key, &msg)?;
    log::debug!("GPG Signature: {:?}.", gpg_sig);

    Ok(Proof {
        msg,
        gpg_sig,
        eth_sig,
        gpg_key: gpg_key.to_string(),
        eth_key: signer.address(),
    })
}

/// The main lib function that runs the functionality of the program
/// - Obtains a block hash from a block from 1 day ago.
/// - Gets either a keystore file or in its absence a Ledger HW as signer to sign a message.
/// - Creates a message that will be signed by the defined signer.
/// - Write both proofs to a JSON file.
pub async fn run(opts: Options) -> anyhow::Result<()> {
    let provider =
        Provider::<Http>::try_from(opts.rpc_url).expect("could not instantiate HTTP Provider");
    let latest_block_number = provider.get_block_number().await?;
    // 5760 blocks earlier is aprox. 1 day ago, this due to avoid referencing blocks that are affected by reorgs of the chain.
    let block_number = latest_block_number.saturating_sub(ethers::prelude::U64::from(5760));
    let block = provider
        .get_block(block_number)
        .await?
        .ok_or_else(|| anyhow!(Error::NoBlock))?;
    let block_hash = block.hash.ok_or_else(|| anyhow!(Error::NoBlockHash))?;
    if let Some(keypath) = &opts.keystore {
        use colored::*;

        log::info!("Decrypting keystore..");
        let prompt = format!("{} Password: ", "??".cyan());
        let password = rpassword::prompt_password_stdout(&prompt).unwrap();
        let signer = ethers::signers::LocalWallet::decrypt_keystore(keypath, password)
            .map_err(|_| anyhow!("keystore decryption failed"))?;
        log::debug!("Keystore decrypted: {:?}.", signer);

        let proof = create_proof(&opts.gpg_key, &signer, &block_hash).await?;
        fs::write(&opts.output, serde_json::to_string(&proof)?)?;

        Ok(())
    } else if let Some(path) = &opts.ledger_hdpath {
        log::info!("Connecting to Ledger..");

        let hdpath = path.derivation_string();
        let signer = Ledger::new(HDPath::Other(hdpath), 1).await?;
        log::info!("Successfully connected to Ledger..");

        let proof = create_proof(&opts.gpg_key, &signer, &block_hash).await?;
        fs::write(&opts.output, serde_json::to_string(&proof)?)?;

        Ok(())
    } else {
        Err(anyhow!(Error::NoWallet))
    }
}
