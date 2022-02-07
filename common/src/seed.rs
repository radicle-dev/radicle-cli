use std::path::Path;

use anyhow::{anyhow, Context as _, Result};
use librad::crypto::peer::PeerId;
use librad::git::Urn;
use url::Url;

use crate::git;

pub const CONFIG_SEED_KEY: &str = "rad.seed";
pub const DEFAULT_SEEDS: &[&str] = &[
    "pine.radicle.garden",
    "willow.radicle.garden",
    "maple.radicle.garden",
];
pub const DEFAULT_SEED_API_PORT: u16 = 8777;

pub fn get_seed() -> Result<Url, anyhow::Error> {
    let output = git::git(Path::new("."), ["config", CONFIG_SEED_KEY])
        .context("failed to lookup seed configuration")?;
    let url =
        Url::parse(&output).context(format!("`{}` is not set to a valid URL", CONFIG_SEED_KEY))?;

    Ok(url)
}

pub fn set_seed(seed: &Url) -> Result<(), anyhow::Error> {
    git::git(
        Path::new("."),
        [
            "config",
            "--global",
            CONFIG_SEED_KEY,
            seed.to_string().as_str(),
        ],
    )
    .map(|_| ())
    .context("failed to save seed configuration")
}

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

pub fn push_delegate(
    repo: &Path,
    seed: &Url,
    delegate: &Urn,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    let delegate_id = delegate.encode_id();
    let url = seed.join(&delegate_id)?;

    git::git(
        repo,
        [
            "push",
            "--signed",
            url.as_str(),
            &format!(
                "refs/namespaces/{}/refs/rad/*:refs/remotes/{}/rad/*",
                delegate_id,
                peer_id.default_encoding()
            ),
        ],
    )
}

pub fn push_project(
    repo: &Path,
    seed: &Url,
    project: &Urn,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    let project_id = project.encode_id();
    let url = seed.join(&project_id)?;

    git::git(
        repo,
        [
            "push",
            "--signed",
            "--atomic",
            url.as_str(),
            &format!(
                "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
                project_id,
                peer_id.default_encoding()
            ),
        ],
    )
}

pub fn push_refs(
    repo: &Path,
    seed: &Url,
    project: &Urn,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    let project_id = project.encode_id();
    let url = seed.join(&project_id)?;

    git::git(
        repo,
        [
            "push",
            "--signed",
            "--atomic",
            url.as_str(),
            &format!(
                "refs/namespaces/{}/refs/rad/ids/*:refs/remotes/{}/rad/ids/*",
                project_id, peer_id
            ),
            &format!(
                "refs/namespaces/{}/refs/rad/self:refs/remotes/{}/rad/self",
                project_id, peer_id
            ),
            &format!(
                "refs/namespaces/{}/refs/rad/signed_refs:refs/remotes/{}/rad/signed_refs",
                project_id, peer_id
            ),
            &format!(
                "+refs/namespaces/{}/refs/heads/*:refs/remotes/{}/heads/*",
                project_id, peer_id
            ),
        ],
    )
}

pub fn fetch_identity(repo: &Path, seed: &Url, urn: &Urn) -> Result<String, anyhow::Error> {
    let id = urn.encode_id();
    let url = seed.join(&id)?;

    git::git(
        repo,
        [
            "fetch",
            "--verbose",
            "--atomic",
            url.as_str(),
            &format!("refs/rad/*:refs/namespaces/{}/refs/rad/*", id),
        ],
    )
}

pub fn fetch_heads(repo: &Path, seed: &Url, urn: &Urn) -> Result<String, anyhow::Error> {
    let id = urn.encode_id();
    let url = seed.join(&id)?;

    git::git(
        repo,
        [
            "fetch",
            "--verbose",
            "--atomic",
            url.as_str(),
            &format!("refs/heads/*:refs/namespaces/{}/refs/heads/*", id,),
        ],
    )
}

pub fn fetch_remotes(
    repo: &Path,
    seed: &Url,
    project: &Urn,
    remotes: &[PeerId],
) -> Result<String, anyhow::Error> {
    let project_id = project.encode_id();
    let url = seed.join(&project_id)?;
    let mut args = Vec::new();

    args.extend(["fetch", "--verbose", "--force", "--atomic", url.as_str()].map(|s| s.to_string()));

    if remotes.is_empty() {
        args.push(format!(
            "refs/remotes/*:refs/namespaces/{}/refs/remotes/*",
            project_id
        ));
    } else {
        args.extend(remotes.iter().map(|remote| {
            format!(
                "refs/remotes/{}/*:refs/namespaces/{}/refs/remotes/{}/*",
                remote, project_id, remote
            )
        }));
    }

    git::git(repo, args)
}
