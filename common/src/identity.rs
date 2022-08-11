use std::convert::TryFrom;
use std::str::FromStr;

use anyhow::anyhow;
use url::Url;

use librad::git::Urn;
use librad::PeerId;

use crate::project::URL_SCHEME;
use crate::{seed, sync};

/// Identity origin.
///
/// Represents a location from which an identity can be fetched.
/// To construct one, use the [`TryFrom<Url>`] or [`FromStr`]
/// instances.
#[derive(Debug, Eq, PartialEq)]
pub struct Origin {
    /// URN.
    pub urn: Urn,
    /// If available, the address of a seed which has this project.
    pub seed: Option<sync::Seed<String>>,
}

impl Origin {
    /// Create an origin from a URN.
    pub fn from_urn(urn: Urn) -> Self {
        Self { urn, seed: None }
    }
}

impl FromStr for Origin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(urn) = Urn::from_str(s) {
            Ok(Self { urn, seed: None })
        } else if let Ok(url) = Url::from_str(s) {
            Self::try_from(url)
        } else {
            Err(anyhow!("invalid origin '{}'", s))
        }
    }
}

impl TryFrom<Url> for Origin {
    type Error = anyhow::Error;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        let mut segments = if let Some(segments) = url.path_segments() {
            segments
        } else {
            anyhow::bail!("invalid radicle URL '{}': missing path", url.to_string());
        };

        if url.scheme() != URL_SCHEME {
            anyhow::bail!("not a radicle URL '{}': invalid scheme", url.to_string());
        }

        let host = url.host();
        let port = url.port().unwrap_or(seed::DEFAULT_SEED_P2P_PORT);
        let peer = PeerId::from_str(url.username())?;
        let seed = host.map(|h| sync::Seed {
            peer,
            addrs: format!("{}:{}", h, port),
            label: None,
        });

        let urn = if let Some(id) = segments.next() {
            if id.is_empty() {
                anyhow::bail!("invalid radicle URL '{}': empty path", url.to_string());
            }
            Urn::try_from_id(id).map_err(|_| anyhow!("invalid urn '{}'", id))?
        } else {
            anyhow::bail!("invalid radicle URL '{}': missing path", url.to_string());
        };

        Ok(Self { urn, seed })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_origin_from_url() {
        let url = Url::parse("rad://willow.radicle.garden/hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y")
            .unwrap();
        Origin::try_from(url).unwrap_err();

        let expected_urn = Urn::try_from_id("hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap();
        let url = Url::parse("rad://hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@pine.radicle.garden:8776/hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y")
            .unwrap();
        let origin = Origin::try_from(url).unwrap();

        assert_eq!(
            origin.seed,
            Some(sync::Seed {
                peer: PeerId::from_str("hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa")
                    .unwrap(),
                addrs: String::from("pine.radicle.garden:8776"),
                label: None,
            })
        );
        assert_eq!(origin.urn, expected_urn);

        let url = Url::parse("rad://hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa@pine.radicle.garden/hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y")
            .unwrap();
        let origin = Origin::try_from(url).unwrap();

        assert_eq!(
            origin.seed,
            Some(sync::Seed {
                peer: PeerId::from_str("hyb5to4rshftx4apgmu9s6wnsp4ddmp1mz6ijh4qqey7fb8wrpawxa")
                    .unwrap(),
                addrs: String::from("pine.radicle.garden:8776"),
                label: None,
            })
        );
        assert_eq!(origin.urn, expected_urn);
    }

    #[test]
    fn test_origin_from_str() {
        let origin = Origin::from_str("rad:git:hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap();
        assert_eq!(
            origin.urn,
            Urn::try_from_id("hnrkbjg7r54q48sqsaho1n4qfxhi4nbmdh51y").unwrap()
        );
        assert!(origin.seed.is_none());
    }
}
