use std::convert::TryInto;
use std::time;

use librad::git::Urn;

use radicle_common as common;
use radicle_common::nonempty::NonEmpty;
use radicle_common::profile::Profile;
use radicle_common::signer::ToSigner;
use radicle_common::sync;
use radicle_common::sync::SyncResult;

use crate as term;

pub fn sync(
    urn: Urn,
    seeds: NonEmpty<sync::Seed<String>>,
    mode: sync::Mode,
    profile: &Profile,
    signer: impl ToSigner,
    rt: &common::tokio::runtime::Runtime,
) -> anyhow::Result<NonEmpty<SyncResult>> {
    let signer = signer.to_signer(profile)?;
    let timeout = time::Duration::from_secs(9);
    let spinner = term::spinner("Syncing...");
    let result = rt.block_on(async {
        let (seeds, _errors) = sync::Seeds::resolve(seeds.iter()).await;
        let client = sync::client(signer, profile).await?;
        let result = sync::sync(&client, urn, seeds, mode, timeout).await;

        Ok::<Vec<SyncResult>, anyhow::Error>(result)
    })?;

    let results = if let Ok(results) = result.try_into() {
        results
    } else {
        return Err(anyhow::anyhow!(
            "No seeds attempted: all seeds failed to resolve"
        ));
    };

    match mode {
        sync::Mode::Push | sync::Mode::All => spinner.finish(),
        sync::Mode::Fetch => spinner.clear(),
    }

    Ok(results)
}
