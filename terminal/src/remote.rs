use std::collections::HashSet;

use librad::git::storage::Storage;
use librad::git::Urn;

use radicle_common::{git, project};

use crate as term;

pub fn list(storage: &Storage, repo: &git::Repository, urn: &Urn) -> anyhow::Result<()> {
    let mut table = term::Table::default();
    let proj = project::get(&storage, urn)?
        .ok_or_else(|| anyhow::anyhow!("project {} not found on local device", urn))?;
    let mut peers = HashSet::new();

    for (_, peer) in git::remotes(repo)? {
        if !peers.insert(peer) {
            // Don't show duplicate peers.
            continue;
        }

        let delegate = if proj.remotes.contains(&peer) {
            term::format::badge_primary("delegate")
        } else {
            String::new()
        };

        if let Some(person) = project::person(&storage, urn.clone(), &peer)? {
            table.push([
                term::format::bold(person.subject().name.to_string()),
                term::format::tertiary(peer),
                delegate,
            ]);
        } else {
            table.push([String::new(), term::format::tertiary(peer), delegate]);
        }
    }
    table.render();

    Ok(())
}

pub fn remove(
    name: &str,
    storage: &Storage,
    repo: &git::Repository,
    urn: &Urn,
) -> anyhow::Result<()> {
    match project::find_remote(name, storage, repo, urn)? {
        Some(name) => {
            repo.remote_delete(&name)?;
            term::success!(
                "Remote {} {} removed",
                term::format::highlight(&name),
                term::format::dim(format!("{:?}", name)),
            );
        }
        None => {
            anyhow::bail!("remote '{}' not found", name)
        }
    }

    Ok(())
}
