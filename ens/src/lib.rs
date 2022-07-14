use std::ffi::OsString;

use anyhow::anyhow;

use ethers::prelude::{Address, Chain, Http, Provider, Signer, SignerMiddleware};
use librad::git::identities::local::LocalIdentity;
use librad::git::Storage;

use radicle_common::args::{Args, Error, Help};
use radicle_common::ethereum::{
    self,
    resolver::{self, PublicResolver},
    ProviderOptions, SignerOptions,
};
use radicle_common::{keys, person, seed};
use radicle_terminal as term;

pub const HELP: Help = Help {
    name: "ens",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    rad ens               [<option>...]
    rad ens --setup       [<option>...] [--rpc-url <url>] --ledger-hdpath <hd-path>
    rad ens --setup       [<option>...] [--rpc-url <url>] --keystore <file>
    rad ens --setup       [<option>...] [--rpc-url <url>] --walletconnect
    rad ens [<operation>] [<option>...]

    If no operation is specified, `--show` is implied.

Operations

    --show                       Show ENS data for your local radicle identity
    --setup [<name>]             Associate your local identity with an ENS name
    --set-local <name>           Set an ENS name for your local radicle identity

Options

    --help                       Print help

Wallet options

    --rpc-url <url>              JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
    --ledger-hdpath <hdpath>     Account derivation path when using a Ledger hardware device
    --keystore <file>            Keystore file containing encrypted private key (default: none)
    --walletconnect              Use WalletConnect

Environment variables

    ETH_RPC_URL  Ethereum JSON-RPC URL (overwrite with '--rpc-url')
    ETH_HDPATH   Hardware wallet derivation path (overwrite with '--ledger-hdpath')
"#,
};

#[derive(Debug)]
pub enum Operation {
    Show,
    Setup(Option<String>),
    SetLocal(String),
}

#[derive(Debug)]
pub struct Options {
    pub operation: Operation,
    pub provider: ethereum::ProviderOptions,
    pub signer: ethereum::SignerOptions,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let parser = lexopt::Parser::from_args(args);
        let (provider, parser) = ProviderOptions::from(parser)?;
        let (signer, mut parser) = SignerOptions::from(parser)?;
        let mut operation = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("setup") if operation.is_none() => {
                    let val = parser.value().ok();
                    let name = if let Some(val) = val {
                        Some(
                            val.into_string()
                                .map_err(|_| anyhow!("invalid ENS name specified"))?,
                        )
                    } else {
                        None
                    };
                    operation = Some(Operation::Setup(name));
                }
                Long("set-local") if operation.is_none() => {
                    let val = parser.value().ok();
                    if let Some(name) = val {
                        operation = Some(Operation::SetLocal(
                            name.into_string()
                                .map_err(|_| anyhow!("invalid ENS name specified"))?,
                        ));
                    } else {
                        return Err(anyhow!("an ENS name must be specified"));
                    }
                }
                Long("show") if operation.is_none() => {
                    operation = Some(Operation::Show);
                }
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok((
            Options {
                operation: operation.unwrap_or(Operation::Show),
                provider,
                signer,
            },
            vec![],
        ))
    }
}

pub fn run(options: Options, ctx: impl term::Context) -> anyhow::Result<()> {
    let profile = ctx.profile()?;
    let signer = term::signer(&profile)?;
    let storage = keys::storage(&profile, signer)?;
    let rt = radicle_common::tokio::runtime::Runtime::new()?;
    let id = person::local(&storage)?;

    match options.operation {
        Operation::Show => {
            if let Some(person) = person::verify(&storage, &id.urn())? {
                term::success!("Your local identity is {}", term::format::dim(id.urn()));

                if let Some(ens) = person.payload().get_ext::<person::Ens>()? {
                    term::success!(
                        "Your local identity is associated with ENS name {}",
                        term::format::highlight(ens.name)
                    );
                } else {
                    term::warning("Your local identity is not associated with an ENS name");
                }
            }
        }
        Operation::Setup(name) => {
            term::headline(&format!(
                "Associating local 🌱 identity {} with ENS",
                term::format::highlight(&id.urn()),
            ));
            let name = term::text_input("ENS name", name)?;
            let provider = ethereum::provider(options.provider)?;
            let signer_opts = options.signer;
            let (wallet, provider) =
                rt.block_on(term::ethereum::get_wallet(signer_opts, provider))?;
            rt.block_on(setup(&name, id, provider, wallet, &storage))?;
        }
        Operation::SetLocal(name) => set_ens_payload(&name, &storage)?,
    }

    Ok(())
}

fn set_ens_payload(name: &str, storage: &Storage) -> anyhow::Result<()> {
    term::info!("Setting ENS name for local 🌱 identity");

    if term::confirm(format!(
        "Associate local identity with ENS name {}?",
        term::format::highlight(&name)
    )) {
        let doc = person::set_ens_payload(
            person::Ens {
                name: name.to_owned(),
            },
            storage,
        )?;

        term::success!("Local identity successfully updated with ENS name {}", name);
        term::blob(serde_json::to_string(&doc.payload())?);
    }
    Ok(())
}

async fn setup(
    name: &str,
    id: LocalIdentity,
    provider: Provider<Http>,
    signer: ethereum::Wallet,
    storage: &Storage,
) -> anyhow::Result<()> {
    let urn = id.urn();
    let chain_id = signer.chain_id();
    let signer = SignerMiddleware::new(provider, signer);
    let radicle_name = name.ends_with(ethereum::RADICLE_DOMAIN);
    let resolver = match PublicResolver::get(name, signer).await {
        Ok(resolver) => resolver,
        Err(err) => {
            if let resolver::Error::NameNotFound { .. } = err {
                return Err(Error::WithHint {
                    err: err.into(),
                    hint: if radicle_name {
                        "The name must be registered with ENS to continue. Go to https://app.radicle.network/register to register."
                    } else {
                        "The name must be registered with ENS to continue. Go to https://app.ens.domains to register."
                    }
                }
                .into());
            } else {
                return Err(err.into());
            }
        }
    };

    let seed_host: String = term::text_input("Seed host", None)?;
    let seed_url = url::Url::parse(&format!("https://{}", seed_host))?;

    let spinner = term::spinner("Querying seed...");
    let seed_id = match seed::get_seed_id(seed_url) {
        Ok(id) => {
            spinner.clear();
            term::text_input("Seed ID", Some(id))?
        }
        Err(err) => {
            spinner.failed();
            return Err(anyhow!("error querying seed: {}", err));
        }
    };
    let address_current = resolver.address(name).await?;
    let address: Option<Address> =
        term::text_input_optional("Address", address_current.map(ethereum::hex))?;

    let github_current = resolver.text(name, "com.github").await?;
    let github: Option<String> =
        term::text_input_optional("GitHub handle", github_current.clone())?;

    let twitter_current = resolver.text(name, "com.twitter").await?;
    let twitter: Option<String> =
        term::text_input_optional("Twitter handle", twitter_current.clone())?;

    let mut calls = vec![
        resolver
            .set_text(
                name,
                resolver::RADICLE_SEED_ID_KEY,
                &seed_id.default_encoding(),
            )?
            .calldata()
            .unwrap(), // Safe because we have call data.
        resolver
            .set_text(name, resolver::RADICLE_SEED_HOST_KEY, &seed_host)?
            .calldata()
            .unwrap(),
        resolver
            .set_text(name, resolver::RADICLE_ID_KEY, &urn.to_string())?
            .calldata()
            .unwrap(),
    ];

    if let Some(address) = address {
        if address_current.map_or(true, |a| a != address) {
            calls.push(resolver.set_address(name, address)?.calldata().unwrap());
        }
    }
    if let Some(github) = github {
        if github_current.map_or(true, |g| g != github) {
            calls.push(
                resolver
                    .set_text(name, "com.github", &github)?
                    .calldata()
                    .unwrap(),
            );
        }
    }
    if let Some(twitter) = twitter {
        if twitter_current.map_or(true, |t| t != twitter) {
            calls.push(
                resolver
                    .set_text(name, "com.twitter", &twitter)?
                    .calldata()
                    .unwrap(),
            );
        }
    }

    let call = resolver.multicall(calls)?;
    term::ethereum::transaction(call).await?;

    if chain_id == u64::from(Chain::Mainnet) {
        let spinner = term::spinner("Updating local identity...");
        match person::set_ens_payload(
            person::Ens {
                name: name.to_owned(),
            },
            storage,
        ) {
            Ok(doc) => {
                spinner.finish();
                term::blob(serde_json::to_string(&doc.payload())?);
            }
            Err(err) => {
                spinner.failed();
                return Err(err);
            }
        }

        term::info!(
            "Successfully associated local 🌱 identity with {}",
            term::format::highlight(name)
        );
    } else {
        term::warning("Skipping local ENS setup");
    }

    term::blank();
    term::tip!("To view your profile, visit:");
    term::indented(&term::format::secondary(format!(
        "https://app.radicle.network/{}",
        name
    )));

    Ok(())
}
