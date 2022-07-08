use std::net::SocketAddr;
use std::time;

use futures_lite::StreamExt;
use thiserror::Error;

use librad::{
    git::Urn,
    net::{
        peer::{client, Client},
        protocol::request_pull,
        quic::ConnectPeer,
    },
    Signer,
};
use lnk_clib::seed::Seed;
pub use request_pull::Success;

use crate::tokio;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Client(#[from] client::error::RequestPull),
    #[error(transparent)]
    Response(#[from] request_pull::Error),
    #[error("no response")]
    NoResponse,
}

pub(super) async fn push<S, E>(
    client: &Client<S, E>,
    urn: Urn,
    seed: Seed<Vec<SocketAddr>>,
    timeout: time::Duration,
) -> Result<request_pull::Success, Error>
where
    S: Signer + Clone,
    E: ConnectPeer + Clone + Send + Sync + 'static,
{
    let mut req = client.request_pull(seed.clone(), urn.clone()).await?;

    while let Ok(Some(res)) = tokio::time::timeout(timeout, req.next()).await {
        match res {
            Ok(res) => match res {
                request_pull::Response::Success(succ) => return Ok(succ),
                request_pull::Response::Error(err) => {
                    return Err(err.into());
                }
                request_pull::Response::Progress(_) => {
                    continue;
                }
            },
            Err(err) => {
                return Err(err.into());
            }
        }
    }

    Err(Error::NoResponse)
}
