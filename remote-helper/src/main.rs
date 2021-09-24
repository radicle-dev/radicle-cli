use ethers::abi::{Detokenize, ParamType};
use ethers::contract::abigen;
use ethers::prelude::*;
use ethers::types::{Address, NameOrAddress};

use link_identities::git::Urn;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context as _;

use std::convert::TryFrom;
use std::env;
use std::future;
use std::process;
use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;

#[derive(Debug, Clone)]
struct Remote {
    org: NameOrAddress,
    urn: Urn,
}

/// Text record key that holds the Git server address.
const ENS_SEED_HOST: &str = "eth.radicle.seed.host";
/// URL scheme supported.
const URL_SCHEME: &str = "radicle";
/// Ethereum TLD.
const ETH_TLD: &str = ".eth";
/// Failure exit code.
const EXIT_FAILURE: i32 = 1;

// Generated contract to query ENS resolver.
abigen!(
    EnsTextResolver,
    "[function text(bytes32,string) external view returns (string)]",
);

impl FromStr for Remote {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let url = url::Url::parse(input).unwrap();

        if url.scheme() != URL_SCHEME {
            bail!("Invalid URL scheme {:?}", url.scheme());
        }
        if url.cannot_be_a_base() {
            bail!("Invalid URL {:?}", input);
        }

        let host = url
            .host_str()
            .map(|h| h.trim_end_matches(".git"))
            .ok_or_else(|| anyhow!("Invalid URL host {:?}", input))?;
        let path = url
            .path()
            .strip_prefix('/')
            .ok_or_else(|| anyhow!("Missing URL path: {:?}", input))?;

        let org = if let Ok(addr) = host.parse::<Address>() {
            NameOrAddress::Address(addr)
        } else if host.contains('.') {
            NameOrAddress::Name(host.to_owned())
        } else {
            bail!(
                "Invalid URL host {:?}: expected a domain name or Ethereum address",
                host
            );
        };
        let urn = Urn::from_str(&format!("rad:git:{}", path))
            .with_context(|| format!("Invalid project identifier {:?}", path))?;

        Ok(Self { org, urn })
    }
}

fn fatal(err: anyhow::Error) -> ! {
    eprintln!("Fatal: {}", err);
    process::exit(EXIT_FAILURE);
}

#[tokio::main]
async fn main() {
    let mut args = env::args().skip(2);

    let url = if let Some(arg) = args.next() {
        arg
    } else {
        fatal(anyhow!("Not enough arguments supplied"));
    };

    match url.parse() {
        Ok(url) => {
            if let Err(err) = run(url).await {
                fatal(err);
            }
        }
        Err(err) => {
            fatal(err);
        }
    }
}

async fn run(remote: Remote) -> anyhow::Result<()> {
    let domain = resolve(remote.org).await?;
    let http_url = format!("https://{}/{}", domain, remote.urn.encode_id());

    // TODO: Use `exec` here.
    let mut child = Command::new("git")
        .arg("remote-https")
        .arg(http_url)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .spawn()?;

    let status = child.wait()?;

    process::exit(status.code().unwrap_or(EXIT_FAILURE))
}

async fn resolve(org: NameOrAddress) -> anyhow::Result<String> {
    // Only resolve ENS names.
    if let NameOrAddress::Name(ref domain) = org {
        if !domain.ends_with(ETH_TLD) {
            return Ok(domain.clone());
        }
    }

    let rpc_url = env::var("ETH_RPC_URL")
        .ok()
        .and_then(|url| if url.is_empty() { None } else { Some(url) })
        .ok_or_else(|| anyhow::anyhow!("'ETH_RPC_URL' must be set to an Ethereum JSON-RPC URL"))?;

    let provider =
        Provider::<Http>::try_from(rpc_url.as_str()).context("JSON-RPC URL parsing failed")?;

    let (_address, name) = match org {
        NameOrAddress::Name(name) => (provider.resolve_name(name.as_str()).await?, name),
        NameOrAddress::Address(addr) => (
            future::ready(addr).await,
            provider.lookup_address(addr).await?,
        ),
    };

    let resolver = {
        let bytes = provider
            .call(&ens::get_resolver(ens::ENS_ADDRESS, &name).into(), None)
            .await?;
        let tokens = ethers::abi::decode(&[ParamType::Address], bytes.as_ref())?;

        Address::from_tokens(tokens).unwrap()
    };

    let contract = EnsTextResolver::new(resolver, provider.into());
    let host = contract
        .text(ens::namehash(&name).0, ENS_SEED_HOST.to_owned())
        .call()
        .await?;

    Ok(host)
}
