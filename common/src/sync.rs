mod push;

use std::convert::TryInto;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time;

use anyhow::anyhow;

use librad::crypto::BoxedSigner;
use librad::git::Urn;
use librad::net::protocol::SendOnly;
use librad::net::{
    self,
    peer::{client, Client},
    quic,
    quic::ConnectPeer,
    replication, Network,
};
use librad::profile::Profile;
use librad::Signer;
use link_async::Spawner;
use lnk_clib::seed::store::FileStore;

pub use lnk_clib::seed::{Seed, Seeds};
pub use lnk_sync::Mode;

use crate::config;
use crate::nonempty::NonEmpty;

/// Sync result of a seed.
#[derive(Debug)]
pub struct SyncResult {
    pub seed: Seed<Vec<SocketAddr>>,
    pub fetch: Option<Result<replication::Success, client::error::Replicate>>,
    pub push: Option<Result<push::Success, push::Error>>,
}

/// Sync the given URN with the provided list of seeds.
pub async fn sync<S, E>(
    client: &Client<S, E>,
    urn: Urn,
    seeds: Seeds,
    mode: Mode,
    timeout: time::Duration,
) -> Vec<SyncResult>
where
    S: Signer + Clone,
    E: ConnectPeer + Clone + Send + Sync + 'static,
{
    let mut syncs = Vec::with_capacity(seeds.len());
    let is_push = mode.is_push();
    let is_fetch = mode.is_fetch();
    let Seeds(seeds) = seeds;

    for seed in seeds {
        let fetch = if is_fetch {
            match tokio::time::timeout(timeout, client.replicate(seed.clone(), urn.clone(), None))
                .await
            {
                Ok(result) => Some(result),
                Err(_) => Some(Err(client::error::Replicate::NoConnection(
                    client::error::NoConnection(seed.peer),
                ))),
            }
        } else {
            None
        };

        let push = if is_push {
            Some(push::push(client, urn.clone(), seed.clone(), timeout).await)
        } else {
            None
        };

        syncs.push(SyncResult { seed, fetch, push })
    }
    syncs
}

/// Create a sync client.
pub async fn client(
    signer: BoxedSigner,
    profile: &Profile,
) -> anyhow::Result<Client<BoxedSigner, SendOnly>> {
    let spawner = Spawner::from_current().ok_or(anyhow!("cannot create spawner"))?;
    let network = Network::default();
    let config = client::Config {
        signer: signer.clone(),
        paths: profile.paths().clone(),
        replication: net::replication::Config::default(),
        user_storage: client::config::Storage::default(),
        network: network.clone(),
    };
    let endpoint = quic::SendOnly::new(signer, network).await?;
    let client = Client::new(config, Arc::new(spawner), endpoint)?;

    Ok(client)
}

/// Get the seeds configured for the profile.
/// First checks local (working copy) config, then global.
pub fn seeds(profile: &Profile) -> anyhow::Result<NonEmpty<Seed<String>>> {
    let config = config::Config::load(profile)?;
    let seeds = config
        .seeds()
        .cloned()
        .map(|s| s.try_into())
        .collect::<Result<Vec<_>, _>>()?;
    if let Ok(seeds) = seeds.try_into() {
        return Ok(seeds);
    }

    // Fallback to global seeds if no local seeds are configured.
    // Nb. These seeds are in a different format, and eventually we will consolidate the
    // seed configurations under a unified format. For now we support both.
    let seeds_file = profile.paths().seeds_file();
    let store = FileStore::<String>::new(seeds_file)?;
    let seeds: Vec<_> = store.iter()?.collect::<Result<_, _>>()?;

    seeds
        .try_into()
        .map_err(|_| anyhow!("No seeds configured for profile {}", profile.id()))
}
