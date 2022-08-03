#![allow(clippy::extra_unused_lifetimes)]
use librad::crypto::keystore::pinentry::SecUtf8;
#[cfg(not(feature = "ethereum"))]
use librad::git::local::url::LocalUrl;
use librad::profile::{LnkHome, LNK_HOME};
use link_identities::git::Urn;
use radicle_git_helpers::remote_helper;

use radicle_common::{keys, profile, signer::ToSigner as _};

use anyhow::anyhow;
#[cfg(feature = "ethereum")]
use futures_lite::future;

use std::env;
use std::process;
use std::str::FromStr;

#[derive(Debug, Clone)]
enum Remote {
    #[cfg(feature = "ethereum")]
    Org {
        org: ethers::types::NameOrAddress,
        urn: Urn,
    },
    Project {
        urn: Urn,
    },
}

/// Failure exit code.
const EXIT_FAILURE: i32 = 1;

impl FromStr for Remote {
    type Err = anyhow::Error;

    #[cfg(not(feature = "ethereum"))]
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let url: LocalUrl = LocalUrl::from_str(input)?;

        Ok(Self::Project { urn: url.urn })
    }

    #[cfg(feature = "ethereum")]
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        use anyhow::bail;
        use anyhow::Context as _;
        use ethers::types::{Address, NameOrAddress};

        if let Ok(url) = url::Url::parse(input) {
            if url.scheme() != ethereum::URL_SCHEME {
                bail!("Invalid URL scheme {:?}", url.scheme());
            }
            if url.cannot_be_a_base() {
                bail!("Invalid URL {:?}", input);
            }

            let base = url
                .host_str()
                .map(|h| h.trim_end_matches(".git"))
                .ok_or_else(|| anyhow!("Invalid URL base {:?}", input))?;

            if let Ok(urn) = Urn::try_from_id(base) {
                return Ok(Self::Project { urn });
            }

            let org = if let Ok(addr) = base.parse::<Address>() {
                NameOrAddress::Address(addr)
            } else if base.contains('.') {
                NameOrAddress::Name(base.to_owned())
            } else {
                bail!(
                    "Invalid URL base {:?}: expected a project id, domain name or ethereum address",
                    base
                );
            };

            let path = url
                .path()
                .strip_prefix('/')
                .ok_or_else(|| anyhow!("Missing URL path: {:?}", input))?;
            let urn = Urn::try_from_id(path)
                .with_context(|| format!("Invalid project identifier {:?}", path))?;

            Ok(Self::Org { org, urn })
        } else {
            let urn = Urn::from_str(&format!("rad:git:{}", input))?;

            Ok(Self::Project { urn })
        }
    }
}

fn fatal(err: anyhow::Error) -> ! {
    eprintln!("Fatal: {}", err);
    process::exit(EXIT_FAILURE);
}

fn main() {
    let mut args = env::args().skip(2);

    let url = if let Some(arg) = args.next() {
        arg
    } else {
        fatal(anyhow!("Not enough arguments supplied"));
    };

    match Remote::from_str(&url) {
        Ok(url) => {
            if let Err(err) = run(url) {
                fatal(err);
            }
        }
        Err(err) => {
            fatal(err);
        }
    }
}

fn run(remote: Remote) -> anyhow::Result<()> {
    match remote {
        #[cfg(feature = "ethereum")]
        Remote::Org { org, urn } => {
            use std::process::Command;
            use std::process::Stdio;

            let domain = future::block_on(ethereum::resolve(org))?;
            let http_url = format!("https://{}/{}", domain, urn.encode_id());

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
        Remote::Project { urn: _urn } => {
            let profile = profile::default()?;
            let signer = if let Ok(sock) = keys::ssh_auth_sock() {
                sock.to_signer(&profile)?
            } else if let Ok(pass) = env::var(keys::RAD_PASSPHRASE) {
                keys::load_secret_key(&profile, SecUtf8::from(pass))?.to_signer(&profile)?
            } else {
                fatal(anyhow!("no signers found: ssh-agent is not running"));
            };
            let config = remote_helper::Config {
                signer: Some(signer),
            };

            // This is a workaround because the remote helper library
            // doesn't take a profile as config parameter, so we have
            // to configure it this way.
            if let LnkHome::Root(root) = profile::home() {
                env::set_var(LNK_HOME, root);
            }
            remote_helper::run(config)
        }
    }
}

#[cfg(feature = "ethereum")]
pub mod ethereum {
    use std::convert::TryFrom;
    use std::env;
    use std::future;

    use anyhow::Context as _;
    use ethers::abi::{Detokenize, ParamType};
    use ethers::contract::abigen;
    use ethers::prelude::*;
    use ethers::types::{Address, NameOrAddress};

    /// Text record key that holds the Git server address.
    pub const ENS_SEED_HOST: &str = "eth.radicle.seed.host";
    /// URL scheme supported.
    pub const URL_SCHEME: &str = "rad";
    /// Ethereum TLD.
    pub const ETH_TLD: &str = ".eth";

    // Generated contract to query ENS resolver.
    abigen!(
        EnsTextResolver,
        "[function text(bytes32,string) external view returns (string)]",
    );

    pub async fn resolve(org: NameOrAddress) -> anyhow::Result<String> {
        // Only resolve ENS names.
        if let NameOrAddress::Name(ref domain) = org {
            if !domain.ends_with(ETH_TLD) {
                return Ok(domain.clone());
            }
        }

        let rpc_url = env::var("ETH_RPC_URL")
            .ok()
            .and_then(|url| if url.is_empty() { None } else { Some(url) })
            .ok_or_else(|| {
                anyhow::anyhow!("'ETH_RPC_URL' must be set to an Ethereum JSON-RPC URL")
            })?;

        let provider =
            Provider::<Http>::try_from(rpc_url.as_str()).context("JSON-RPC URL parsing failed")?;

        let (_address, name) = match org {
            NameOrAddress::Name(name) => (provider.resolve_name(name.as_str()).await?, name),
            NameOrAddress::Address(addr) => (
                future::ready(addr).await,
                provider.lookup_address(addr).await?,
            ),
        };
        eprintln!("Resolving ENS record {} for {}", ENS_SEED_HOST, name);

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

        eprintln!("Resolved {} to {}", ENS_SEED_HOST, host);

        Ok(host)
    }
}
