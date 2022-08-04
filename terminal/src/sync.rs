use std::time;

use librad::git::Urn;

use radicle_common as common;
use radicle_common::nonempty::NonEmpty;
use radicle_common::profile::Profile;
use radicle_common::signer::ToSigner;
use radicle_common::sync;
use radicle_common::sync::SyncResult;

use crate as term;

// TODO: Don't return a result.
pub fn sync(
    urn: Urn,
    seeds: NonEmpty<sync::Seed<String>>,
    mode: sync::Mode,
    profile: &Profile,
    signer: impl ToSigner,
    rt: &common::tokio::runtime::Runtime,
) -> anyhow::Result<Vec<SyncResult>> {
    let signer = signer.to_signer(profile)?;
    let timeout = time::Duration::from_secs(9);
    let spinner = term::spinner("Syncing...");
    let result = rt.block_on(async {
        let (seeds, _errors) = sync::Seeds::resolve(seeds.iter()).await;
        let client = sync::client(signer, profile).await?;
        let result = sync::sync(&client, urn, seeds, mode, timeout).await;

        Ok::<Vec<SyncResult>, anyhow::Error>(result)
    })?;
    spinner.finish();

    Ok(result)
}
