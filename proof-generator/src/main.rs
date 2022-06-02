use anyhow::anyhow;
use coins_bip32::path::DerivationPath;
use proof_generator as proof;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process;

use radicle_cli::logger;
use radicle_common::tokio;

const USAGE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", "USAGE"));
const NAME: &str = env!("CARGO_CRATE_NAME");

enum Command {
    Run {
        options: proof::Options,
        verbose: bool,
    },
    Help,
}

fn parse_options() -> anyhow::Result<Command> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_env();
    let mut gpg_key: Option<String> = None;
    let mut output: Option<PathBuf> = None;
    let mut rpc_url: Option<String> = None;
    let mut ledger_hdpath: Option<DerivationPath> = None;
    let mut keystore: Option<PathBuf> = None;
    let mut verbose = false;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("gpg-key") => {
                gpg_key = Some(parser.value()?.parse()?);
            }
            Long("output") => {
                output = Some(parser.value()?.parse()?);
            }
            Long("keystore") => {
                keystore = Some(parser.value()?.parse()?);
            }
            Long("ledger-hdpath") => {
                ledger_hdpath = Some(parser.value()?.parse()?);
            }
            Long("rpc-url") => {
                rpc_url = Some(parser.value()?.parse()?);
            }
            Long("verbose") | Short('v') => {
                verbose = true;
            }
            Long("help") => {
                return Ok(Command::Help);
            }
            _ => {
                return Err(anyhow!(arg.unexpected()));
            }
        }
    }

    Ok(Command::Run {
        options: proof::Options {
            gpg_key: gpg_key
                .ok_or_else(|| anyhow!("a gpg fingerprint must be specified with '--gpg-key'"))?,
            output: output
                .ok_or_else(|| anyhow!("an output path must be specified with '--output'"))?,
            rpc_url: rpc_url
                .ok_or_else(|| anyhow!("a json rpc provider must be specified with '--rpc-url'"))?,
            ledger_hdpath,
            keystore,
        },
        verbose,
    })
}

#[tokio::main]
async fn main() {
    logger::init(NAME).unwrap();
    logger::set_level(log::Level::Error);

    if let Err(err) = execute().await {
        if let Some(&proof::Error::NoWallet) = err.downcast_ref() {
            log::error!("Error: no wallet specified: either '--ledger-hdpath' or '--keystore' must be specified");
        } else if let Some(cause) = err.source() {
            log::error!("Error: {} ({})", err, cause);
        } else {
            log::error!("Error: {}", err);
        }
        process::exit(1);
    }
}

async fn execute() -> anyhow::Result<()> {
    match parse_options()? {
        Command::Help => {
            std::io::stderr().write_all(USAGE)?;
            return Ok(());
        }
        Command::Run { options, verbose } => {
            if verbose {
                logger::set_level(log::Level::Debug);
            } else {
                logger::set_level(log::Level::Info);
            }
            proof::run(options).await?;
        }
    }
    log::info!("Proof successfully created");
    Ok(())
}
