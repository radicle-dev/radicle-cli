use std::path::Path;

use anyhow::{anyhow, Context as _, Result};
use librad::crypto::peer::PeerId;
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

pub fn get_seed_id(host: &str) -> Result<PeerId, anyhow::Error> {
    let seed = format!("https://{}:{}/v1/peer", host, DEFAULT_SEED_API_PORT);

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

pub fn push_delegate_id(
    repo: &Path,
    seed: &Url,
    self_id: &str,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    let url = seed.join(self_id)?;

    git::git(
        repo,
        [
            "push",
            "--signed",
            url.as_str(),
            &format!(
                "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
                self_id, peer_id
            ),
        ],
    )
}

pub fn push_project_id(
    repo: &Path,
    seed: &Url,
    project_id: &str,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    let url = seed.join(project_id)?;

    git::git(
        repo,
        [
            "push",
            "--signed",
            "--atomic",
            url.as_str(),
            &format!(
                "refs/namespaces/{}/refs/rad/id:refs/remotes/{}/rad/id",
                project_id, peer_id
            ),
        ],
    )
}

pub fn push_refs(
    repo: &Path,
    seed: &Url,
    project_id: &str,
    peer_id: PeerId,
) -> Result<String, anyhow::Error> {
    let url = seed.join(project_id)?;

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

pub fn fetch_remotes(
    repo: &Path,
    seed: &Url,
    project_id: &str,
    remotes: &[PeerId],
) -> Result<String, anyhow::Error> {
    let url = seed.join(project_id)?;
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
