use std::env;
use std::process;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context as _;

use librad::crypto::keystore::crypto::{KdfParams, Pwhash};
use librad::crypto::keystore::pinentry::Prompt;
use librad::crypto::keystore::Keystore as _;
use librad::crypto::BoxedSigner;
use librad::git::storage::Storage;
use librad::git::tracking::git::tracking;
use librad::git::Urn;
use librad::profile::Profile;
use librad::PeerId;

use radicle_tools::logger;

const USAGE: &str = "rad untrack <urn> [--peer <peer-id>]";

#[derive(Debug)]
struct Options {
    urn: Urn,
    peer: Option<PeerId>,
}

fn main() {
    logger::init(env!("CARGO_CRATE_NAME")).unwrap();
    logger::set_level(log::Level::Info);

    match run() {
        Err(err) => {
            log::error!("Error: {}", err);
            log::info!("Usage: {}", USAGE);

            process::exit(1);
        }
        Ok(()) => {}
    }
}

/// Create a [`Prompt`] for unlocking the key storage.
pub fn prompt() -> Pwhash<Prompt<'static>> {
    let prompt = Prompt::new("please enter your passphrase: ");
    Pwhash::new(prompt, KdfParams::recommended())
}

fn run() -> anyhow::Result<()> {
    let options = parse_options()?;

    let profile = Profile::load()?;
    let paths = profile.paths();

    let keyring = rad_clib::keys::file_storage(&profile, prompt());
    let key = keyring.get_key()?.secret_key;
    let signer: BoxedSigner = key.into();
    let storage = Storage::open(paths, signer)?;

    if let Some(peer) = options.peer {
        tracking::untrack(
            &storage,
            &options.urn,
            peer,
            tracking::policy::Untrack::MustExist,
        )??;

        log::info!(
            "Tracking relationship with {} removed for {}",
            peer,
            options.urn,
        );
    } else {
        tracking::untrack_all(&storage, &options.urn, tracking::policy::UntrackAll::Any)?
            .for_each(drop);

        log::info!("Tracking relationships for {} removed", options.urn);
    }

    Ok(())
}

fn parse_options() -> anyhow::Result<Options> {
    use lexopt::prelude::*;

    let mut parser = lexopt::Parser::from_env();
    let mut urn: Option<Urn> = None;
    let mut peer: Option<PeerId> = None;

    while let Some(arg) = parser.next()? {
        match arg {
            Long("peer") => {
                peer = Some(
                    parser
                        .value()?
                        .parse()
                        .context("invalid value specified for '--peer'")?,
                );
            }
            Long("help") => {
                log::info!("Usage: {}", USAGE);
                process::exit(0);
            }
            Value(val) if urn.is_none() => {
                let val = val.to_string_lossy();
                let val = Urn::from_str(&val).context(format!("invalid URN '{}'", val))?;

                urn = Some(val);
            }
            _ => {
                return Err(anyhow!(arg.unexpected()));
            }
        }
    }

    Ok(Options {
        urn: urn.ok_or_else(|| anyhow!("a URN to untrack must be specified"))?,
        peer,
    })
}
