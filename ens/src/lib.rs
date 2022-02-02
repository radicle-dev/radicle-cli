use anyhow::anyhow;

use ethers::prelude::{Address, Http, Provider, Signer, SignerMiddleware};
use librad::git::identities::local::LocalIdentity;

use rad_common::ethereum::{ProviderOptions, SignerOptions};
use rad_common::{ethereum, keys, person, profile, seed};
use rad_terminal::components as term;
use rad_terminal::components::{Args, Error, Help};

use crate::resolver::PublicResolver;

pub mod resolver;

pub const HELP: Help = Help {
    name: "ens",
    description: env!("CARGO_PKG_DESCRIPTION"),
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
USAGE
    rad ens --setup     [<option>...] [--rpc-url <url>] --ledger-hdpath <hd-path>
    rad ens --setup     [<option>...] [--rpc-url <url>] --keystore <file>
    rad ens <operation> [<option>...]

OPERATIONS
    --setup [<name>]             Associate your local radicle id with an ENS name

OPTIONS
    --help                       Print help

WALLET OPTIONS
    --rpc-url <url>              JSON-RPC URL of Ethereum node (eg. http://localhost:8545)
    --ledger-hdpath <hdpath>     Account derivation path when using a Ledger hardware device
    --keystore <file>            Keystore file containing encrypted private key (default: none)

ENVIRONMENT VARIABLES

    ETH_RPC_URL  Ethereum JSON-RPC URL (overwrite with '--rpc-url')
    ETH_HDPATH   Hardware wallet derivation path (overwrite with '--ledger-hdpath')
"#,
};

#[derive(Debug)]
pub enum Operation {
    Setup(Option<String>),
}

#[derive(Debug)]
pub struct Options {
    pub operation: Operation,
    pub provider: ethereum::ProviderOptions,
    pub signer: ethereum::SignerOptions,
}

impl Args for Options {
    fn from_env() -> anyhow::Result<Self> {
        use lexopt::prelude::*;

        let parser = lexopt::Parser::from_env();
        let (provider, parser) = ProviderOptions::from(parser)?;
        let (signer, mut parser) = SignerOptions::from(parser)?;
        let mut operation = None;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("setup") => {
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
                Long("help") => {
                    return Err(Error::Help.into());
                }
                _ => return Err(anyhow!(arg.unexpected())),
            }
        }

        Ok(Options {
            operation: operation
                .ok_or_else(|| anyhow!("an operation must be specified, see 'rad ens --help'"))?,
            provider,
            signer,
        })
    }
}

pub fn run(options: Options) -> anyhow::Result<()> {
    let profile = profile::default()?;
    let sock = keys::ssh_auth_sock();
    let (_, storage) = keys::storage(&profile, sock)?;
    let rt = tokio::runtime::Runtime::new()?;
    let id = person::local(&storage)?;
    let provider = ethereum::provider(options.provider)?;
    let signer_opts = options.signer;

    rt.block_on(transaction(options.operation, id, signer_opts, provider))?;

    Ok(())
}

async fn transaction(
    operation: Operation,
    id: LocalIdentity,
    signer_opts: SignerOptions,
    provider: Provider<Http>,
) -> anyhow::Result<()> {
    use ethereum::WalletError;

    term::tip("Accessing your wallet...");
    let signer = match ethereum::signer(signer_opts, provider.clone()).await {
        Ok(signer) => signer,
        Err(err) => {
            if let Some(WalletError::NoWallet) = err.downcast_ref::<WalletError>() {
                return Err(Error::WithHint {
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

    match operation {
        Operation::Setup(name) => {
            term::headline(&format!(
                "Associating local 🌱 identity {} with ENS",
                term::format::highlight(&id.urn()),
            ));
            let name = term::text_input("ENS name", name)?;

            setup(&name, id, provider, signer).await
        }
    }
}

async fn setup(
    name: &str,
    id: LocalIdentity,
    provider: Provider<Http>,
    signer: ethereum::Wallet,
) -> anyhow::Result<()> {
    let urn = id.urn();
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
    let seed_url = seed::get_seed().ok();
    let seed_host = seed_url.and_then(|url| url.host_str().map(|s| s.to_owned()));
    let seed_host = term::text_input("Seed host", seed_host)?;

    let spinner = term::spinner("Seed ID...");
    let seed_id = match seed::get_seed_id(&seed_host) {
        Ok(id) => {
            spinner.clear();
            term::text_input("Seed ID", Some(id))?
        }
        Err(err) => {
            spinner.failed();
            return Err(anyhow!("error fetching peer id from seed: {}", err));
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
    ethereum::transaction(call).await?;

    term::info!(
        "Successfully associated local 🌱 identity with {}",
        term::format::highlight(name)
    );

    // TODO: Link to radicle interface.

    Ok(())
}
