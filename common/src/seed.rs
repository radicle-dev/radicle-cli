//! Seed-related functionality.
use std::convert::TryFrom;
use std::net;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context as _, Result};
use librad::crypto::peer::PeerId;
use librad::git::Urn;
use url::{Host, Url};

use crate::args::Error;
use crate::sync::Seed;
use crate::{git, project};

pub const CONFIG_SEED_KEY: &str = "rad.seed";
pub const CONFIG_PEER_KEY: &str = "rad.peer";
pub const DEFAULT_SEED_GIT_LOCAL_PORT: u16 = 8778;
pub const DEFAULT_SEED_API_PORT: u16 = 8777;
pub const DEFAULT_SEED_P2P_PORT: u16 = 8776;
pub const DEFAULT_SEED_GIT_PORT: u16 = 443;

#[derive(serde::Deserialize)]
pub struct CommitHeader {
    pub summary: String,
}

#[derive(serde::Deserialize)]
pub struct Commit {
    pub header: CommitHeader,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Protocol {
    Link { peer: Option<PeerId> },
    Git { local: bool },
    Api { local: bool },
}

impl Protocol {
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Link { .. } => DEFAULT_SEED_P2P_PORT,
            Self::Git { local: true } => DEFAULT_SEED_GIT_LOCAL_PORT,
            Self::Git { local: false } => DEFAULT_SEED_GIT_PORT,
            Self::Api { .. } => DEFAULT_SEED_API_PORT,
        }
    }

    pub fn scheme(&self) -> &'static str {
        match self {
            Self::Link { .. } => "rad",
            Self::Git { local: true } => "http",
            Self::Git { local: false } => "https",
            Self::Api { local: true } => "http",
            Self::Api { local: false } => "https",
        }
    }
}

/// Seed address with optional port and urn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub protocol: Protocol,
    pub host: Host,
    pub port: Option<u16>,
    pub urn: Option<Urn>,
}

impl Address {
    pub fn new(host: impl Into<Host>, protocol: Protocol) -> Self {
        Self {
            host: host.into(),
            protocol,
            port: None,
            urn: None,
        }
    }

    pub fn url(&self) -> Url {
        Url::from(self.clone())
    }

    pub fn port(&self) -> u16 {
        self.port.unwrap_or(self.protocol.default_port())
    }

    pub fn peer(&self) -> Option<PeerId> {
        if let Protocol::Link { peer } = self.protocol {
            return peer;
        }
        None
    }
}

impl TryFrom<Address> for Seed<String> {
    type Error = anyhow::Error;

    fn try_from(addr: Address) -> Result<Self, Self::Error> {
        if let Some(peer) = addr.peer() {
            return Ok(Seed {
                addrs: format!("{}:{}", addr.host, addr.port()),
                peer,
                label: None,
            });
        }
        Err(anyhow::anyhow!(
            "address is not a valid seed: peer-id missing"
        ))
    }
}

impl TryFrom<Url> for Address {
    type Error = anyhow::Error;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        let peer = if url.username().is_empty() {
            None
        } else {
            let peer = PeerId::from_str(url.username()).map_err(|_| {
                anyhow::anyhow!("not a valid radicle URL '{}': invalid peer-id", url)
            })?;
            Some(peer)
        };

        let protocol = match url.scheme() {
            "rad" => Protocol::Link { peer },
            "http" => Protocol::Git { local: true },
            "https" => Protocol::Git { local: false },
            scheme => {
                anyhow::bail!(
                    "not a valid URL '{}': invalid scheme '{}'",
                    url.to_string(),
                    scheme
                );
            }
        };

        let host = url
            .host()
            .ok_or_else(|| anyhow::anyhow!("not a valid radicle URL '{}': missing host", url))?
            .to_owned();
        let port = url.port();

        let urn =
            if let Some(segment) = url.path_segments().and_then(|mut segments| segments.next()) {
                if segment.is_empty() {
                    None
                } else {
                    let urn = Urn::try_from_id(segment).map_err(|_| {
                        anyhow!(
                            "not a valid radicle URL '{}': invalid path '{}': not an id",
                            url,
                            segment
                        )
                    })?;
                    Some(urn)
                }
            } else {
                None
            };

        Ok(Address {
            protocol,
            host,
            port,
            urn,
        })
    }
}

impl From<Address> for Url {
    fn from(addr: Address) -> Self {
        let s = format!("{}://{}", addr.protocol.scheme(), addr.host);
        let mut url = Url::parse(&s).unwrap();

        url.set_port(addr.port).ok();
        url.set_username(
            addr.peer()
                .map(|p| p.default_encoding())
                .unwrap_or_default()
                .as_str(),
        )
        .ok();

        if let Some(urn) = &addr.urn {
            url.set_path(&urn.encode_id());
        }
        url
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Remove need to clone for conversion.
        write!(f, "{}", Url::from(self.clone()))
    }
}

impl FromStr for Address {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if net::SocketAddr::from_str(s).is_ok() {
            anyhow::bail!(
                "invalid URL '{}': protocol scheme (eg. 'https') is missing",
                s
            );
        }
        let url = Url::from_str(s).map_err(|e| anyhow::anyhow!("invalid URL '{}': {}", s, e))?;

        Self::try_from(url)
    }
}

/// Parse a seed value from an options parser.
pub fn parse_value(parser: &mut lexopt::Parser) -> anyhow::Result<Seed<String>> {
    let value = parser.value()?;
    let value = value.to_string_lossy();
    let value = value.as_ref();
    let seed = Seed::from_str(value).map_err(|_| Error::WithHint {
        err: anyhow!("invalid seed address specified: '{}'", value),
        hint: "hint: valid seed addresses have the format <peer-id>@<addr>, eg. hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@pine.radicle.garden:8776",
    })?;

    Ok(seed)
}

/// Set the configured "peer" seed within the local repository.
pub fn set_peer_seed(seed: &Seed<String>, peer_id: &PeerId) -> Result<(), anyhow::Error> {
    let seed = seed.to_string();
    let path = Path::new(".");
    let key = format!("{}.{}.seed", CONFIG_PEER_KEY, peer_id.default_encoding());
    let args = ["config", "--local", &key, seed.as_str()];

    git::git(path, args)
        .map(|_| ())
        .context("failed to save seed configuration")
}

/// Get the configured "peer" seed within the local repository.
pub fn get_peer_seed(peer_id: &PeerId) -> Result<Url, anyhow::Error> {
    let path = Path::new(".");
    let key = format!("{}.{}.seed", CONFIG_PEER_KEY, peer_id.default_encoding());
    let args = ["config", &key];

    let output = git::git(path, args).context("failed to lookup seed configuration")?;
    let url = Url::parse(&output).context(format!("`{}` is not set to a valid URL", key))?;

    Ok(url)
}

/// Query a seed node for its [`PeerId`].
pub fn get_seed_id(mut seed: Url) -> Result<PeerId, anyhow::Error> {
    seed.set_port(Some(DEFAULT_SEED_API_PORT)).unwrap();
    seed = seed.join("/v1/peer")?;

    let agent = ureq::Agent::new();
    let obj: serde_json::Value = agent.get(seed.as_str()).call()?.into_json()?;

    let id = obj
        .get("id")
        .ok_or(anyhow!("missing 'id' in seed API response"))?
        .as_str()
        .ok_or(anyhow!("'id' is not a string"))?;
    let id = PeerId::from_default_encoding(id)?;

    Ok(id)
}

/// Query a seed node for a project commit.
pub fn get_commit(
    mut seed: Url,
    project: &Urn,
    commit: &git::Oid,
) -> Result<Commit, anyhow::Error> {
    seed.set_port(Some(DEFAULT_SEED_API_PORT)).unwrap();
    seed = seed.join(&format!("/v1/projects/{}/commits/{}", project, commit))?;

    let agent = ureq::Agent::new();
    let val: serde_json::Value = agent.get(seed.as_str()).call()?.into_json()?;
    let commit = serde_json::from_value(val)?;

    Ok(commit)
}

/// Query a seed node for a project's remotes.
pub fn get_remotes(mut seed: Url, project: &Urn) -> Result<Vec<project::PeerInfo>, anyhow::Error> {
    seed.set_port(Some(DEFAULT_SEED_API_PORT)).unwrap();
    seed = seed.join(&format!("/v1/projects/{}/remotes", project))?;

    let agent = ureq::Agent::new();
    let val: serde_json::Value = agent.get(seed.as_str()).call()?.into_json()?;
    let response = serde_json::from_value(val)?;

    Ok(response)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_address_url_roundtrip() {
        let addr = Address::from_str("http://willow.radicle.garden").unwrap();
        assert_eq!(Address::from_str(addr.url().as_str()).unwrap(), addr);

        let addr = Address::from_str("https://willow.radicle.garden:443").unwrap();
        assert_eq!(Address::from_str(addr.url().as_str()).unwrap(), addr);

        let addr = Address::from_str("rad://hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@willow.radicle.garden:8776").unwrap();
        assert_eq!(Address::from_str(addr.url().as_str()).unwrap(), addr);

        let addr = Address::from_str("rad://hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@willow.radicle.garden:8776/hnrkmg77m8tfzj4gi4pa4mbhgysfgzwntjpao").unwrap();
        assert_eq!(Address::from_str(addr.url().as_str()).unwrap(), addr);
    }

    #[test]
    fn test_address_parse() {
        let peer =
            PeerId::from_str("hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa").unwrap();
        let addr = Address::from_str(&format!(
            "rad://{}@willow.radicle.garden:9999/hnrkmg77m8tfzj4gi4pa4mbhgysfgzwntjpao",
            peer
        ))
        .unwrap();

        assert_eq!(addr.port, Some(9999));
        assert_eq!(addr.protocol, Protocol::Link { peer: Some(peer) });
        assert_eq!(addr.host.to_string(), String::from("willow.radicle.garden"));
        assert_eq!(
            addr.urn,
            Some(Urn::from_str("rad:git:hnrkmg77m8tfzj4gi4pa4mbhgysfgzwntjpao").unwrap())
        );

        let addr = Address::from_str("rad://willow.radicle.garden").unwrap();
        assert_eq!(addr.port, None);
        assert_eq!(addr.protocol, Protocol::Link { peer: None });
        assert_eq!(addr.host.to_string(), String::from("willow.radicle.garden"));
        assert_eq!(addr.urn, None);
        assert_eq!(addr.port(), DEFAULT_SEED_P2P_PORT);
    }
}
