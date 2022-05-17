use librad::git::{refs, Storage, Urn};
use librad::PeerId;

use crate as term;
use radicle_common as common;

/// Fetch remotes and verify signatures.
pub fn fetch_remotes<'a>(
    storage: &Storage,
    seed: &common::Url,
    project: &Urn,
    remotes: impl IntoIterator<Item = &'a PeerId>,
    spinner: &mut term::Spinner,
) -> Result<String, anyhow::Error> {
    let remotes = remotes.into_iter().copied().collect::<Vec<_>>();
    let output = common::seed::fetch_remotes(storage.path(), seed, project, &remotes)?;

    verify_signed_refs(storage, project, &remotes, spinner)?;

    Ok(output)
}

/// Verify signed refs for the given remotes.
pub fn verify_signed_refs<'a>(
    storage: &Storage,
    project: &Urn,
    remotes: impl IntoIterator<Item = &'a PeerId>,
    spinner: &mut term::Spinner,
) -> Result<(), anyhow::Error> {
    spinner.message("Verifying signed refs...".to_owned());

    for remote in remotes.into_iter() {
        spinner.message(format!(
            "Verifying signed refs for {}...",
            common::fmt::peer(remote)
        ));
        if let Err(err) = refs::Refs::load(&storage, project, Some(*remote)) {
            return Err(err.into());
        }
    }

    Ok(())
}
