//! Seed-related functionality.

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
pub const DEFAULT_SEEDS: &[&str] = &[
    "hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@pine.radicle.garden:8776",
    "hyd7wpd8p5aqnm9htsfoatxkckmw6ingnsdudns9code5xq17h1rhw@willow.radicle.garden:8776",
    "hyd1to75dyfpizchxp43rdwhisp8nbr76g5pxa5f4y7jh4pa6jjzns@maple.radicle.garden:8776",
];
pub const DEFAULT_SEED_API_PORT: u16 = 8777;
pub const DEFAULT_SEED_P2P_PORT: u16 = 8776;

/// Git configuration scope.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub enum Scope<'a> {
    /// Local repository scope.
    Local(&'a Path),
    /// Global (user) scope.
    Global,
    /// Any (default) scope.
    #[default]
    Any,
}

#[derive(serde::Deserialize)]
pub struct CommitHeader {
    pub summary: String,
}

#[derive(serde::Deserialize)]
pub struct Commit {
    pub header: CommitHeader,
}

/// Seed address with optional port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub host: Host,
    pub port: Option<u16>,
}

impl Address {
    /// ```
    /// use std::str::FromStr;
    /// use radicle_common::seed as seed;
    ///
    /// let addr = seed::Address::from_str("willow.radicle.garden").unwrap();
    /// assert_eq!(addr.url().to_string(), "https://willow.radicle.garden/");
    ///
    /// let addr = seed::Address::from_str("localhost").unwrap();
    /// assert_eq!(addr.url().to_string(), "https://localhost/");
    ///
    /// let addr = seed::Address::from_str("127.0.0.1").unwrap();
    /// assert_eq!(addr.url().to_string(), "http://127.0.0.1/");
    /// ```
    pub fn url(&self) -> Url {
        match self.host {
            url::Host::Domain(_) => Url::parse(&format!("https://{}", self)).unwrap(),
            _ => Url::parse(&format!("http://{}", self)).unwrap(),
        }
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(port) = self.port {
            write!(f, "{}:{}", self.host, port)
        } else {
            write!(f, "{}", self.host)
        }
    }
}

impl FromStr for Address {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((host, port)) => {
                let host = Host::parse(host)?;
                let port = Some(port.parse()?);

                Ok(Self { host, port })
            }
            None => {
                let host = Host::parse(s)?;

                Ok(Self { host, port: None })
            }
        }
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

/// Get the configured seed within a scope.
pub fn get_seeds(scope: Scope) -> Result<Vec<Seed<String>>, anyhow::Error> {
    let seed_regexp = "^rad.seed.*.address$";
    let (path, args) = match scope {
        Scope::Any => (Path::new("."), vec!["config", "--get-regexp", seed_regexp]),
        Scope::Local(path) => (path, vec!["config", "--local", "--get-regexp", seed_regexp]),
        Scope::Global => (
            Path::new("."),
            vec!["config", "--global", "--get-regexp", seed_regexp],
        ),
    };
    let output = git::git(path, args).context("failed to lookup seed configuration")?;

    // Output looks like:
    //
    //      rad.seed.<peer-id>.address <address>
    //      rad.seed.<peer-id>.address <address>
    //      rad.seed.<peer-id>.address <address>
    //
    let mut seeds = Vec::new();
    for line in output.lines() {
        if let Some((_, val)) = line.split_once(' ') {
            let seed =
                Seed::from_str(val).context(format!("`{}` is not a valid seed address", val))?;

            seeds.push(seed);
        } else {
            return Err(anyhow!(
                "failed to parse seed configuration; malformed output: `{}`",
                line
            ));
        }
    }

    Ok(seeds)
}

/// Get the configured seed within a scope.
pub fn get_seed(scope: Scope) -> Result<Url, anyhow::Error> {
    let (path, args) = match scope {
        Scope::Any => (Path::new("."), vec!["config", CONFIG_SEED_KEY]),
        Scope::Local(path) => (path, vec!["config", "--local", CONFIG_SEED_KEY]),
        Scope::Global => (Path::new("."), vec!["config", "--global", CONFIG_SEED_KEY]),
    };
    let output = git::git(path, args).context("failed to lookup seed configuration")?;
    let url =
        Url::parse(&output).context(format!("`{}` is not set to a valid URL", CONFIG_SEED_KEY))?;

    Ok(url)
}

/// Set the configured seed within a scope.
pub fn set_seed(seed: &Seed<String>, scope: Scope) -> Result<(), anyhow::Error> {
    let seed = seed.to_string();
    let (path, args) = match scope {
        Scope::Any => (
            Path::new("."),
            vec!["config", CONFIG_SEED_KEY, seed.as_str()],
        ),
        Scope::Local(path) => (
            path,
            vec!["config", "--local", CONFIG_SEED_KEY, seed.as_str()],
        ),
        Scope::Global => (
            Path::new("."),
            vec!["config", "--global", CONFIG_SEED_KEY, seed.as_str()],
        ),
    };

    git::git(path, args)
        .map(|_| ())
        .context("failed to save seed configuration")
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
    commit: &git2::Oid,
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
