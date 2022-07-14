use std::convert::TryFrom;
use std::convert::TryInto;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use librad::profile::Profile;
use librad::PeerId;
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::seed::{
    Address, Protocol, DEFAULT_SEED_API_PORT, DEFAULT_SEED_GIT_PORT, DEFAULT_SEED_P2P_PORT,
};
use crate::sync::Seed;

pub const DEFAULT_SEEDS: &[(&str, &str)] = &[
    (
        "pine.radicle.garden",
        "hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa",
    ),
    (
        "willow.radicle.garden",
        "hyd7wpd8p5aqnm9htsfoatxkckmw6ingnsdudns9code5xq17h1rhw",
    ),
    (
        "maple.radicle.garden",
        "hyd1to75dyfpizchxp43rdwhisp8nbr76g5pxa5f4y7jh4pa6jjzns",
    ),
];

/// Configuration file name for the local (working copy) scope.
pub const FILE_NAME_LOCAL: &str = "Radicle.toml";
/// Configuration file name for the profile scope.
pub const FILE_NAME_PROFILE: &str = "config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedConfig {
    /// "Pet" name of the seed.
    pub name: Option<String>,
    /// P2P protocol URL, eg. `link` protocol.
    pub p2p: Url,
    /// Git (HTTPS) repository URL.
    pub git: Url,
    /// HTTP API URL.
    pub api: Url,
}

impl TryFrom<SeedConfig> for Seed<String> {
    type Error = anyhow::Error;

    fn try_from(cfg: SeedConfig) -> Result<Self, Self::Error> {
        let addr: Address = cfg.p2p.try_into()?;
        let seed: Seed<String> = addr.try_into()?;

        Ok(seed)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub seed: Vec<SeedConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SEEDS
                .iter()
                .map(|(host, peer)| {
                    let host = String::from(*host);
                    let peer = PeerId::from_str(peer).ok();

                    let mut p2p: Url =
                        Address::new(Host::Domain(host.clone()), Protocol::Link { peer }).into();
                    let mut git: Url =
                        Address::new(Host::Domain(host.clone()), Protocol::Git { local: false })
                            .into();
                    let mut api: Url =
                        Address::new(Host::Domain(host.clone()), Protocol::Api { local: false })
                            .into();

                    p2p.set_port(Some(DEFAULT_SEED_P2P_PORT)).ok();
                    git.set_port(Some(DEFAULT_SEED_GIT_PORT)).ok();
                    api.set_port(Some(DEFAULT_SEED_API_PORT)).ok();

                    SeedConfig {
                        name: Some(host),
                        p2p,
                        git,
                        api,
                    }
                })
                .collect(),
        }
    }
}

impl Config {
    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let content = fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;

        Ok(config)
    }

    pub fn load(profile: &Profile) -> Result<Self, io::Error> {
        Self::local().or_else(|_| Self::profile(profile))
    }

    pub fn local() -> Result<Self, io::Error> {
        Self::read(Path::new(FILE_NAME_LOCAL))
    }

    pub fn profile(profile: &Profile) -> Result<Self, io::Error> {
        Self::read(Self::path(profile))
    }

    pub fn init(profile: &Profile) -> Result<Self, anyhow::Error> {
        let config = Self::default();
        let path = Self::path(profile);

        config.write(path)?;

        Ok(config)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let contents = toml::to_string_pretty(self)?;
        fs::write(path, contents)?;

        Ok(())
    }

    pub fn path(profile: &Profile) -> PathBuf {
        // This is a bit of a hack, since we don't have a way of getting
        // the profile root.
        profile
            .paths()
            .seeds_file()
            .with_file_name(FILE_NAME_PROFILE)
    }

    pub fn seeds(&self) -> impl Iterator<Item = &SeedConfig> {
        self.seed.iter()
    }
}
